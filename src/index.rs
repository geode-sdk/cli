use crate::config::Config;
use crate::server::{ApiResponse, PaginatedData};
use crate::util::logging::ask_value;
use crate::{done, fatal, index_admin, index_auth, index_dev, info, NiceUnwrap};
use clap::Subcommand;
use reqwest::header::USER_AGENT;
use semver::VersionReq;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha3::{Digest, Sha3_256};
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use zip::read::ZipFile;

#[derive(Deserialize)]
pub struct ServerModVersion {
	#[allow(unused)]
	pub name: String,
	pub version: String,
	pub download_link: String,
	#[allow(unused)]
	pub hash: String,
}

#[derive(Subcommand, Debug)]
#[clap(rename_all = "kebab-case")]
pub enum Index {
	/// Login with your GitHub account
	Login {
		/// Existing access token to use
		#[clap(long)]
		token: Option<String>,

		#[clap(long, conflicts_with = "token")]
		github_token: Option<String>
	},

	/// Invalidate all existing access tokens (logout)
	Invalidate,

	/// Edit your developer profile
	Profile,

	/// Interact with your own mods
	Mods {
		#[clap(subcommand)]
		action: MyModAction,
	},

	/// Install a mod from the index to the current profile
	Install {
		/// Mod ID to install
		id: String,

		/// Mod version to install, defaults to latest
		version: Option<VersionReq>,
	},

	/// Set the URL for the index (pass default to reset)
	Url {
		/// URL to set
		#[clap(long, short)]
		url: Option<String>,
	},

	/// Secrets...
	Admin {
		#[clap(subcommand)]
		commands: AdminAction,
	},
}

#[derive(Deserialize, Debug, Clone, Subcommand, PartialEq)]
pub enum MyModAction {
	/// Create a new mod
	Create {
		/// Direct download link to the .geode file
		download_link: Option<String>,
	},
	/// Update an existing mod
	Update {
		/// Direct download link to the .geode file
		download_link: Option<String>,
	},
	/// List your published mods
	Published,
	/// List your pending mods
	Pending,
	/// Edit data about a mod
	Edit,
}

#[derive(Deserialize, Debug, Clone, Subcommand, PartialEq)]
pub enum AdminAction {
	/// List mods that are pending verification
	ListPending,
	/// Alter a developer's verified status
	DevStatus,
}

pub fn install_mod(
	config: &Config,
	id: &String,
	version: &VersionReq,
	ignore_platform: bool,
) -> PathBuf {
	let compare = {
		let string = version.to_string();
		if string == "*" {
			None
		} else {
			Some(string)
		}
	};
	let found = get_mod_versions(id, 1, 1, config, !ignore_platform, compare)
		.nice_unwrap("Couldn't fetch versions from index");

	if found.data.is_empty() {
		fatal!("Couldn't find dependency on index");
	}

	let found_version = found.data.first().unwrap();

	info!(
		"Installing mod '{}' version '{}'",
		id, found_version.version
	);

	let bytes = reqwest::blocking::get(get_index_url(
		format!("v1/mods/{}/versions/{}/download", id, found_version.version),
		config,
	))
	.nice_unwrap("Unable to download mod")
	.bytes()
	.nice_unwrap("Unable to download mod");

	let dest = config
		.get_current_profile()
		.mods_dir()
		.join(format!("{id}.geode"));

	let mut hasher = Sha3_256::new();
	hasher.update(&bytes);
	let hash = hex::encode(hasher.finalize());

	if hash != found_version.version {
		fatal!(
			"Downloaded file doesn't match nice_unwraped hash\n\
			    {hash}\n\
			 vs {}\n\
			Try again, and if the issue persists, report this on GitHub: \
			https://github.com/geode-sdk/cli/issues/new",
			found_version.version
		);
	}

	fs::write(&dest, bytes).nice_unwrap("Unable to install .geode file");

	dest
}

fn submit(action: MyModAction, config: &mut Config) {
	let mut is_update = false;
	let download_link = match action {
		MyModAction::Create { download_link } => download_link,
		MyModAction::Update { download_link } => {
			is_update = true;
			download_link
		}
		_ => fatal!("Invalid action"),
	};

	if config.index_token.is_none() {
		fatal!("You are not logged in");
	}

	let download_link =
		download_link.unwrap_or_else(|| ask_value("Download URL for the .geode file", None, true));
	let mut id: Option<String> = None;
	#[derive(Deserialize)]
	struct SimpleModJson {
		id: String,
	}

	if is_update {
		info!("Fetching mod id from .geode file");
		let mut zip_data: Cursor<Vec<u8>> = Cursor::new(vec![]);

		let mut response =
			reqwest::blocking::get(&download_link).nice_unwrap("Unable to download mod");
		response
			.copy_to(&mut zip_data)
			.nice_unwrap("Unable to write to index");

		let mut zip_archive =
			zip::ZipArchive::new(zip_data).nice_unwrap("Unable to decode .geode file");

		let json_file = zip_archive
			.by_name("mod.json")
			.nice_unwrap("Unable to read mod.json");

		let json = serde_json::from_reader::<ZipFile, SimpleModJson>(json_file)
			.nice_unwrap("Unable to parse mod.json");

		id = Some(json.id);
	}

	if let Some(id) = id {
		update_mod(&id, &download_link, config);
	} else {
		create_mod(&download_link, config);
	}
}

fn create_mod(download_link: &str, config: &mut Config) {
	if config.index_token.is_none() {
		fatal!("You are not logged in");
	}

	let client = reqwest::blocking::Client::new();

	let url = get_index_url("/v1/mods", config);

	info!("Creating mod");

	let response = client
		.post(url)
		.header(USER_AGENT, "GeodeCLI")
		.bearer_auth(config.index_token.clone().unwrap())
		.json(&json!({ "download_link": download_link }))
		.send()
		.nice_unwrap("Unable to connect to Geode Index");

	if response.status() == 401 {
		config.index_token = None;
		config.save();
		fatal!("Invalid token. Please login again.");
	}

	if response.status() != 204 {
		let body: ApiResponse<String> = response
			.json()
			.nice_unwrap("Unable to parse response from Geode Index");
		fatal!("Unable to create mod: {}", body.error);
	}

	info!("Mod created successfully");
}

fn update_mod(id: &str, download_link: &str, config: &mut Config) {
	if config.index_token.is_none() {
		fatal!("You are not logged in");
	}

	let client = reqwest::blocking::Client::new();

	let url = get_index_url(format!("/v1/mods/{}/versions", id), config);

	info!("Updating mod");

	let response = client
		.post(url)
		.header(USER_AGENT, "GeodeCLI")
		.bearer_auth(config.index_token.clone().unwrap())
		.json(&json!({ "download_link": download_link }))
		.send()
		.nice_unwrap("Unable to connect to Geode Index");

	if response.status() == 401 {
		config.index_token = None;
		config.save();
		fatal!("Invalid token. Please login again.");
	}

	if response.status() != 204 {
		let body: ApiResponse<String> = response
			.json()
			.nice_unwrap("Unable to parse response from Geode Index");
		fatal!("Unable to create version for mod: {}", body.error);
	}

	info!("Mod updated successfully");
}

fn set_index_url(url: String, config: &mut Config) {
	if url == "default" {
		config.index_url = "https://api.geode-sdk.org".to_string();
	} else {
		config.index_url = url;
	}
	config.index_token = None;
	config.save();
	info!("Index URL set to: {}", config.index_url);
}

pub fn get_index_url(path: impl AsRef<str>, config: &Config) -> String {
	format!(
		"{}/{}",
		config.index_url.trim_end_matches('/'),
		path.as_ref().trim_start_matches('/')
	)
}

pub fn get_mod_versions(
	id: &str,
	page: u32,
	per_page: u32,
	config: &Config,
	check_platform: bool,
	compare: Option<String>,
) -> Result<PaginatedData<ServerModVersion>, String> {
	let url = get_index_url(format!("v1/mods/{}/versions", id), config);

	let client = reqwest::blocking::Client::new();
	let page = page.to_string();
	let per_page = per_page.to_string();
	let platform = config.get_current_profile().platform_str().to_string();

	let mut query: Vec<(&str, &str)> = vec![("page", &page), ("per_page", &per_page)];
	if let Some(c) = &compare {
		query.push(("compare", c.as_str()));
	}

	if check_platform {
		query.push(("platforms", &platform));
	}

	let response = client
		.get(url)
		.query(&query)
		.header(USER_AGENT, "GeodeCLI")
		.send()
		.nice_unwrap("Couldn't connect to the index");

	if response.status() != 200 {
		return Err("Failed to fetch mod versions".to_string());
	}

	let body = match response.json::<ApiResponse<PaginatedData<ServerModVersion>>>() {
		Err(e) => {
			return Err(format!("Failed to parse index response: {}", e));
		}
		Ok(b) => b,
	};

	Ok(body.payload)
}

pub fn subcommand(cmd: Index) {
	let mut _config = Config::new();
	let config = &mut _config;
	match cmd {
		Index::Install { id, version } => {
			let config = Config::new().assert_is_setup();
			install_mod(
				&config,
				&id,
				&version.unwrap_or(VersionReq::STAR),
				false,
			);
			done!("Mod installed");
		}
		Index::Login { token, github_token } => index_auth::login(config, token, github_token),
		Index::Invalidate => index_auth::invalidate(config),
		Index::Url { url } => {
			if let Some(u) = url {
				set_index_url(u, config);
			} else {
				info!("Your current index URL is: {}", config.index_url);
			}
		}
		Index::Mods { action } => match action {
			MyModAction::Create { .. } => submit(action, config),
			MyModAction::Update { .. } => submit(action, config),
			MyModAction::Published => index_dev::print_own_mods(true, config),
			MyModAction::Pending => index_dev::print_own_mods(false, config),
			MyModAction::Edit => index_dev::edit_own_mods(config),
		},
		Index::Profile => index_dev::edit_profile(config),
		Index::Admin { commands } => index_admin::subcommand(commands, config),
	}
	config.save();
}
