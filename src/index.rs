use crate::config::Config;
use crate::file::{copy_dir_recursive, read_dir_recursive};
use crate::server::ApiResponse;
use crate::util::logging::ask_value;
use crate::util::mod_file::{parse_mod_info, try_parse_mod_info};
use crate::{done, fatal, info, warn, NiceUnwrap};
use clap::Subcommand;
use cli_clipboard::ClipboardProvider;
use colored::Colorize;
use reqwest::header::{AUTHORIZATION, USER_AGENT};
use semver::VersionReq;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha3::{Digest, Sha3_256};
use std::collections::HashSet;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use zip::read::ZipFile;
use zip::ZipArchive;

#[derive(Subcommand, Debug)]
#[clap(rename_all = "kebab-case")]
pub enum Index {
	/// Create a new entry to be used in the index
	New {
		/// Output folder of entry
		output: PathBuf,
	},

	/// Login with your GitHub account
	Login,

	/// Invalidate all existing access tokens (logout)
	Invalidate,

	/// Edit your developer profile
	Profile,

	/// Submit a mod (or a mod update) to the index
	Submit { action: CreateModAction },

	/// Interact with your own mods
	Mods { action: MyModAction },

	/// Install a mod from the index to the current profile
	Install {
		/// Mod ID to install
		id: String,

		/// Mod version to install, defaults to latest
		version: Option<VersionReq>,
	},

	/// Updates the index cache
	Update,

	/// Set the URL for the index (pass default to reset)
	Url {
		/// URL to set
		url: String,
	},

	/// Secrets...
	Admin { action: AdminAction },
}

#[derive(Deserialize, Debug, Clone, clap::ValueEnum, PartialEq)]
pub enum CreateModAction {
	/// Create a new mod
	Create,
	/// Update an existing mod
	Update,
}

#[derive(Deserialize, Debug, Clone, clap::ValueEnum, PartialEq)]
pub enum MyModAction {
	/// List your published mods
	Published,
	/// List your pending mods
	Pending,
	/// Edit data about a mod
	Edit,
}

#[derive(Deserialize, Debug, Clone, clap::ValueEnum, PartialEq)]
pub enum AdminAction {
	/// List mods that are pending verification
	ListPending,
	/// Validate a mod that is pending verification
	Validate,
	/// Reject a mod that is pending verification
	Reject,
	/// Alter a developer's verified status
	DevStatus,
}

#[allow(unused)]
#[derive(Deserialize)]
pub struct EntryMod {
	download: String,
	hash: String,
}

#[allow(unused)]
#[derive(Deserialize)]
pub struct Entry {
	r#mod: EntryMod,
	platforms: HashSet<String>,
	#[serde(default)]
	tags: Vec<String>,
	#[serde(default)]
	featured: bool,
}

#[derive(Debug, Deserialize)]
struct LoginAttempt {
	uuid: String,
	interval: i32,
	uri: String,
	code: String,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SimpleDevMod {
	pub id: String,
	pub featured: bool,
	pub download_count: i32,
	pub versions: Vec<SimpleDevModVersion>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SimpleDevModVersion {
	pub name: String,
	pub version: String,
	pub download_count: i32,
	pub validated: bool,
}

#[derive(Deserialize, Clone)]
pub struct DeveloperProfile {
	pub id: i32,
	pub username: String,
	pub display_name: String,
	pub verified: bool,
	pub admin: bool,
}

pub fn update_index(config: &Config) {
	let index_dir = config.get_current_profile().index_dir();

	let target_index_dir = index_dir.join("geode-sdk_mods");
	// note to loader devs: never change the format pretty please
	let checksum = index_dir.join("geode-sdk_mods.checksum");
	let current_sha = fs::read_to_string(&checksum).unwrap_or_default();

	let client = reqwest::blocking::Client::new();

	let response = client
		.get("https://api.github.com/repos/geode-sdk/mods/commits/main")
		.header("Accept", "application/vnd.github.sha")
		.header("If-None-Match", format!("\"{}\"", current_sha))
		.header(USER_AGENT, "GeodeCli")
		.header(
			AUTHORIZATION,
			std::env::var("GITHUB_TOKEN").map_or("".into(), |token| format!("Bearer {token}")),
		)
		.send()
		.nice_unwrap("Unable to fetch index version");

	if response.status() == 304 {
		done!("Index is up-to-date");
		return;
	}
	assert!(
		response.status() == 200,
		"Version check received status code {}",
		response.status()
	);
	let latest_sha = response
		.text()
		.nice_unwrap("Unable to decode index version");

	let mut zip_data = Cursor::new(Vec::new());

	client
		.get("https://github.com/geode-sdk/mods/zipball/main")
		.send()
		.nice_unwrap("Unable to download index")
		.copy_to(&mut zip_data)
		.nice_unwrap("Unable to write to index");

	let mut zip_archive = ZipArchive::new(zip_data).nice_unwrap("Unable to decode index zip");

	let before_items = if target_index_dir.join("mods-v2").exists() {
		let mut items = fs::read_dir(target_index_dir.join("mods-v2"))
			.unwrap()
			.map(|x| x.unwrap().path())
			.collect::<Vec<_>>();
		items.sort();

		fs::remove_dir_all(&target_index_dir).nice_unwrap("Unable to remove old index version");
		Some(items)
	} else {
		None
	};

	let extract_dir = std::env::temp_dir().join("geode-nuevo-index-zip");
	if extract_dir.exists() {
		fs::remove_dir_all(&extract_dir).nice_unwrap("Unable to prepare new index");
	}
	fs::create_dir(&extract_dir).unwrap();
	zip_archive
		.extract(&extract_dir)
		.nice_unwrap("Unable to extract new index");

	let new_root_dir = fs::read_dir(&extract_dir)
		.unwrap()
		.next()
		.unwrap()
		.unwrap()
		.path();
	copy_dir_recursive(&new_root_dir, &target_index_dir).nice_unwrap("Unable to copy new index");

	// we don't care if temp dir removal fails
	drop(fs::remove_dir_all(extract_dir));

	let mut after_items = fs::read_dir(target_index_dir.join("mods-v2"))
		.unwrap()
		.map(|x| x.unwrap().path())
		.collect::<Vec<_>>();
	after_items.sort();

	if let Some(before_items) = before_items {
		if before_items != after_items {
			info!("Changelog:");

			for i in &before_items {
				if !after_items.contains(i) {
					println!(
						"            {} {}",
						"-".red(),
						i.file_name().unwrap().to_str().unwrap()
					);
				}
			}

			for i in &after_items {
				if !before_items.contains(i) {
					println!(
						"            {} {}",
						"+".green(),
						i.file_name().unwrap().to_str().unwrap()
					);
				}
			}
		}
	}

	fs::write(checksum, latest_sha).nice_unwrap("Unable to save version");
	done!("Successfully updated index")
}

pub fn index_mods_dir(config: &Config) -> PathBuf {
	config
		.get_current_profile()
		.index_dir()
		.join("geode-sdk_mods")
		.join("mods-v2")
}

pub fn get_entry(config: &Config, id: &String, version: &VersionReq) -> Option<Entry> {
	let dir = index_mods_dir(config).join(id);

	for path in read_dir_recursive(&dir).nice_unwrap("Unable to read index") {
		let Ok(mod_info) = try_parse_mod_info(&path) else {
			continue;
		};
		if &mod_info.id == id && version.matches(&mod_info.version) {
			return Some(
				serde_json::from_str(
					&fs::read_to_string(path.join("entry.json"))
						.nice_unwrap("Unable to read index entry"),
				)
				.nice_unwrap("Unable to parse index entry"),
			);
		}
	}

	None
}

pub fn install_mod(
	config: &Config,
	id: &String,
	version: &VersionReq,
	ignore_platform: bool,
) -> PathBuf {
	let entry = get_entry(config, id, version)
		.nice_unwrap(format!("Unable to find '{id}' version '{version}'"));

	let plat = if cfg!(windows) || cfg!(target_os = "linux") {
		"windows"
	} else if cfg!(target_os = "macos") {
		"macos"
	} else {
		fatal!("This platform doesn't support installing mods");
	};

	if !entry.platforms.contains(plat) {
		if ignore_platform {
			warn!("Mod '{}' is not available on '{}'", id, plat);
		} else {
			fatal!("Mod '{}' is not available on '{}'", id, plat);
		}
	}

	info!("Installing mod '{}' version '{}'", id, version);

	let bytes = reqwest::blocking::get(entry.r#mod.download)
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

	if hash != entry.r#mod.hash {
		fatal!(
			"Downloaded file doesn't match nice_unwraped hash\n\
			    {hash}\n\
			 vs {}\n\
			Try again, and if the issue persists, report this on GitHub: \
			https://github.com/geode-sdk/cli/issues/new",
			entry.r#mod.hash
		);
	}

	fs::write(&dest, bytes).nice_unwrap("Unable to install .geode file");

	dest
}

fn create_index_json(path: &Path) {
	let url = ask_value("URL", None, true);

	let response = reqwest::blocking::get(&url).nice_unwrap("Unable to access .geode file at URL");

	let file_name = reqwest::Url::parse(&url)
		.unwrap()
		.path_segments()
		.and_then(|segments| segments.last())
		.and_then(|name| {
			if name.is_empty() {
				None
			} else {
				Some(name.to_string())
			}
		})
		.unwrap_or_else(|| ask_value("Filename", None, true));

	let file_contents = response
		.bytes()
		.nice_unwrap("Unable to access .geode file at URL");

	let mut hasher = Sha3_256::new();
	hasher.update(&file_contents);
	let hash = hasher.finalize();

	let platform_str = ask_value("Supported platforms (comma separated)", None, true);
	let platforms = platform_str.split(',').collect::<Vec<_>>();

	let category_str = ask_value("Categories (comma separated)", None, true);
	let categories = category_str.split(',').collect::<Vec<_>>();

	let index_json = json!({
		"download": {
			"url": url,
			"name": file_name,
			"hash": hex::encode(hash),
			"platforms": platforms
		},
		"categories": categories
	});

	// Format neatly
	let buf = Vec::new();
	let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
	let mut ser = serde_json::Serializer::with_formatter(buf, formatter);
	index_json.serialize(&mut ser).unwrap();

	// Write formatted json
	std::fs::write(
		path.join("index.json"),
		String::from_utf8(ser.into_inner()).unwrap(),
	)
	.nice_unwrap("Unable to write to project");
}

fn create_entry(out_path: &Path) {
	assert!(out_path.exists(), "Path does not exist");
	assert!(out_path.is_dir(), "Path is not a directory");

	let root_path = PathBuf::from(ask_value("Project root directory", Some("."), true));

	let mod_json_path = root_path.join("mod.json");
	let about_path = root_path.join("about.md");
	let logo_path = root_path.join("logo.png");

	assert!(mod_json_path.exists(), "Unable to find project mod.json");

	// Get mod id
	let mod_info = parse_mod_info(&mod_json_path);

	let entry_path = out_path.join(mod_info.id);
	if entry_path.exists() {
		warn!("Directory not empty");
	} else {
		fs::create_dir(&entry_path).nice_unwrap("Unable to create folder");
	}

	create_index_json(&entry_path);
	fs::copy(&mod_json_path, entry_path.join("mod.json")).nice_unwrap("Unable to copy mod.json");

	if about_path.exists() {
		fs::copy(&about_path, entry_path.join("about.md")).nice_unwrap("Unable to copy about.md");
	} else {
		warn!("No about.md found, skipping");
	}

	if logo_path.exists() {
		fs::copy(&logo_path, entry_path.join("logo.png")).nice_unwrap("Unable to copy logo.png");
	} else {
		warn!("No logo.png found, skipping");
	}
}

fn login(config: &mut Config) {
	if config.index_token.is_some() {
		warn!("You are already logged in");
		let token = config.index_token.clone().unwrap();
		info!("{}", token);
		return;
	}

	let client = reqwest::blocking::Client::new();

	let response: reqwest::blocking::Response = client
		.post(get_index_url("/v1/login/github".to_string(), config))
		.header(USER_AGENT, "GeodeCli")
		.json(&{})
		.send()
		.nice_unwrap("Unable to connect to Geode Index");

	if response.status() != 200 {
		fatal!("Unable to connect to Geode Index");
	}

	let parsed = response
		.json::<ApiResponse<LoginAttempt>>()
		.nice_unwrap("Unable to parse login response");

	let login_data = parsed
		.payload
		.nice_unwrap("Invalid response received from Geode Index");

	info!("You will need to complete the login process in your web browser");
	info!("Your login code is: {}", &login_data.code);
	if let Ok(mut ctx) = cli_clipboard::ClipboardContext::new() {
		if ctx.set_contents(login_data.code.to_string()).is_ok() {
			info!("The code has been copied to your clipboard");
		}
	}
	open::that(&login_data.uri).nice_unwrap("Unable to open browser");

	loop {
		info!("Checking login status");
		if let Some(token) = poll_login(&client, &login_data.uuid, config) {
			config.index_token = Some(token);
			config.save();
			done!("Login successful");
			break;
		}

		std::thread::sleep(std::time::Duration::from_secs(login_data.interval as u64));
	}
}

fn poll_login(
	client: &reqwest::blocking::Client,
	uuid: &str,
	config: &mut Config,
) -> Option<String> {
	#[derive(Serialize)]
	struct LoginPoll {
		uuid: String,
	}

	let body: LoginPoll = LoginPoll {
		uuid: uuid.to_string(),
	};

	let response = client
		.post(get_index_url("/v1/login/github/poll".to_string(), config))
		.json(&body)
		.header(USER_AGENT, "GeodeCLI")
		.send()
		.nice_unwrap("Unable to connect to Geode Index");

	if response.status() != 200 {
		return None;
	}

	let parsed = response
		.json::<ApiResponse<String>>()
		.nice_unwrap("Unable to parse login response");

	parsed.payload
}

fn invalidate(config: &mut Config) {
	if config.index_token.is_none() {
		warn!("You are not logged in");
		return;
	}
	loop {
		let response = ask_value(
			"Do you want to log out of all devices (y/n)",
			Some("n"),
			true,
		);

		match response.to_lowercase().as_str() {
			"y" => {
				invalidate_index_tokens(config);
				config.index_token = None;
				config.save();
				done!("All tokens for the current account have been invalidated successfully");
				break;
			}
			"n" => {
				done!("Operation cancelled");
				break;
			}
			_ => {
				warn!("Invalid response");
			}
		}
	}
}

pub fn invalidate_index_tokens(config: &mut Config) {
	if config.index_token.is_none() {
		fatal!("You are not logged in");
	}

	let token = config.index_token.clone().unwrap();

	let client = reqwest::blocking::Client::new();

	let response = client
		.delete(get_index_url("/v1/me/tokens".to_string(), config))
		.header(USER_AGENT, "GeodeCLI")
		.bearer_auth(token)
		.send()
		.nice_unwrap("Unable to connect to Geode Index");

	if response.status() == 401 {
		fatal!("Invalid token. Please login again.");
	}
	if response.status() != 204 {
		fatal!("Unable to invalidate token");
	}
}

fn submit(action: CreateModAction, config: &mut Config) {
	if config.index_token.is_none() {
		fatal!("You are not logged in");
	}

	let download_link = ask_value("Download URL for the .geode file", None, true);
	let mut id: Option<String> = None;
	#[derive(Deserialize)]
	struct SimpleModJson {
		id: String,
	}

	if action == CreateModAction::Update {
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

	#[derive(Serialize)]
	struct Payload {
		download_link: String,
	}

	let payload = Payload {
		download_link: download_link.to_string(),
	};

	let url = get_index_url("/v1/mods".to_string(), config);

	info!("Creating mod");

	let response = client
		.post(url)
		.header(USER_AGENT, "GeodeCLI")
		.bearer_auth(config.index_token.clone().unwrap())
		.json(&payload)
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
		fatal!("Unable to create mod: {}", body.error.unwrap());
	}

	info!("Mod created successfully");
}

fn update_mod(id: &str, download_link: &str, config: &mut Config) {
	if config.index_token.is_none() {
		fatal!("You are not logged in");
	}

	let client = reqwest::blocking::Client::new();

	#[derive(Serialize)]
	struct Payload {
		download_link: String,
	}

	let payload = Payload {
		download_link: download_link.to_string(),
	};

	let url = get_index_url(format!("/v1/mods/{}/versions", id), config);

	info!("Updating mod");

	let response = client
		.post(url)
		.header(USER_AGENT, "GeodeCLI")
		.bearer_auth(config.index_token.clone().unwrap())
		.json(&payload)
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
		fatal!("Unable to create version for mod: {}", body.error.unwrap());
	}

	info!("Mod updated successfully");
}

fn print_own_mods(validated: bool, config: &mut Config) {
	if config.index_token.is_none() {
		fatal!("You are not logged in");
	}

	let payload = get_own_mods(validated, config);

	if payload.is_empty() {
		if validated {
			done!("You have no published mods");
		} else {
			done!("You have no pending mods");
		}
		return;
	}

	if validated {
		info!("Your published mods:");
	} else {
		info!("Your pending mods:");
	}

	for (i, entry) in payload.iter().enumerate() {
		info!("{}. ID: {}", i + 1, &entry.id);
		info!("  Featured: {}", entry.featured);
		info!("  Download count: {}", entry.download_count);
		info!("  Versions:");
		for (i, version) in entry.versions.iter().enumerate() {
			info!("    {}. Name: {}", i + 1, version.name);
			info!("      Version: {}", version.version);
			info!("      Download count: {}", version.download_count);
			info!("      Validated: {}", version.validated);
		}
		if i != payload.len() - 1 {
			info!("");
		}
	}
}

fn get_own_mods(validated: bool, config: &mut Config) -> Vec<SimpleDevMod> {
	if config.index_token.is_none() {
		fatal!("You are not logged in");
	}

	let client = reqwest::blocking::Client::new();

	let validated_str = match validated {
		true => "true",
		false => "false",
	};

	let url = get_index_url(format!("/v1/me/mods?validated={}", validated_str), config);

	let response = client
		.get(url)
		.header(USER_AGENT, "GeodeCLI")
		.bearer_auth(config.index_token.clone().unwrap())
		.send()
		.nice_unwrap("Unable to connect to Geode Index");

	if response.status() != 200 {
		let body: ApiResponse<String> = response
			.json()
			.nice_unwrap("Unable to parse response from Geode Index");
		fatal!("Unable to fetch mods: {}", body.error.unwrap());
	}

	if response.status() == 401 {
		config.index_token = None;
		config.save();
		fatal!("Invalid token. Please login again.");
	}

	let mods = response
		.json::<ApiResponse<Vec<SimpleDevMod>>>()
		.nice_unwrap("Unable to parse response from Geode Index");

	mods.payload.unwrap_or_else(Vec::new)
}

fn edit_own_mods(config: &mut Config) {
	let mods = get_own_mods(true, config);
	if mods.is_empty() {
		fatal!("You have no published mods");
	}

	info!("Select a mod to edit:");
	for (i, entry) in mods.iter().enumerate() {
		info!("{}. ID: {}", i + 1, &entry.id);
	}

	loop {
		let response = ask_value("Mod number (enter q to go back)", None, true);
		if response == "q" {
			break;
		}
		if let Ok(index) = response.parse::<usize>() {
			if index > 0 && index <= mods.len() {
				let should_break = edit_mod(&mods[index - 1], config);
				if should_break {
					break;
				}
			} else {
				warn!("Invalid number");
			}
		} else {
			warn!("Invalid number");
		}
	}
}

fn edit_mod(mod_to_edit: &SimpleDevMod, config: &mut Config) -> bool {
	info!("Editing mod '{}'", mod_to_edit.id);

	loop {
		info!("----------------");
		info!("Possible actions:");
		info!("1. Add a developer");
		info!("2. Remove a developer");
		info!("3. Transfer ownership");
		let response = ask_value("Action number (enter q to go back)", None, true);
		if response == "q" {
			return true;
		}
		if let Ok(index) = response.parse::<usize>() {
			match index {
				1 => add_developer(mod_to_edit, config),
				2 => remove_developer(mod_to_edit, config),
				// coming soon
				3 => unimplemented!(),
				_ => warn!("Invalid number"),
			}
		} else {
			warn!("Invalid number");
		}
	}
}

fn add_developer(mod_to_edit: &SimpleDevMod, config: &mut Config) {
	let username = ask_value("Username", None, true);

	let client = reqwest::blocking::Client::new();
	let url = get_index_url(format!("/v1/mods/{}/developers", mod_to_edit.id), config);

	let response = client
		.post(url)
		.header(USER_AGENT, "GeodeCLI")
		.bearer_auth(config.index_token.clone().unwrap())
		.json(&json!({ "username": username }))
		.send()
		.nice_unwrap("Unable to connect to Geode Index");

	if response.status() != 204 {
		let body: ApiResponse<String> = response
			.json()
			.nice_unwrap("Unable to parse response from Geode Index");
		fatal!("Unable to add developer: {}", body.error.unwrap());
	}

	info!("Developer added successfully");
}

fn remove_developer(mod_to_edit: &SimpleDevMod, config: &mut Config) {
	let username = ask_value("Username", None, true);

	let client = reqwest::blocking::Client::new();
	let url = get_index_url(
		format!("/v1/mods/{}/developers/{}", mod_to_edit.id, username),
		config,
	);

	let response = client
		.delete(url)
		.header(USER_AGENT, "GeodeCLI")
		.bearer_auth(config.index_token.clone().unwrap())
		.send()
		.nice_unwrap("Unable to connect to Geode Index");

	if response.status() != 204 {
		let body: ApiResponse<String> = response
			.json()
			.nice_unwrap("Unable to parse response from Geode Index");
		fatal!("Unable to remove developer: {}", body.error.unwrap());
	}

	info!("Developer removed successfully");
}

fn edit_profile(config: &mut Config) {
	if config.index_token.is_none() {
		fatal!("You are not logged in");
	}

	let client = reqwest::blocking::Client::new();

	let url = get_index_url("/v1/me".to_string(), config);

	let response = client
		.get(url)
		.header(USER_AGENT, "GeodeCLI")
		.bearer_auth(config.index_token.clone().unwrap())
		.send()
		.nice_unwrap("Unable to connect to Geode Index");

	if response.status() != 200 {
		let body: ApiResponse<String> = response
			.json()
			.nice_unwrap("Unable to parse response from Geode Index");
		fatal!("Unable to fetch profile: {}", body.error.unwrap());
	}

	let profile = response
		.json::<ApiResponse<DeveloperProfile>>()
		.nice_unwrap("Unable to parse response from Geode Index");

	let mut profile = profile
		.payload
		.nice_unwrap("Invalid response received from Geode Index");

	info!("Your profile:");
	info!("Username: {}", profile.username);
	info!("Display name: {}", profile.display_name);
	info!("Verified: {}", profile.verified);
	info!("Admin: {}", profile.admin);

	loop {
		info!("----------------");
		info!("Possible actions:");
		info!("1. Change display name");
		let response = ask_value("Action number (enter q to go back)", None, true);
		if response == "q" {
			break;
		}
		if let Ok(index) = response.parse::<usize>() {
			match index {
				1 => {
					let new_display_name = ask_value("New display name", None, true);
					let url = get_index_url("/v1/me".to_string(), config);
					let response = client
						.put(url)
						.header(USER_AGENT, "GeodeCLI")
						.bearer_auth(config.index_token.clone().unwrap())
						.json(&json![
							{
								"display_name": new_display_name
							}
						])
						.send()
						.nice_unwrap("Unable to connect to Geode Index");

					if response.status() != 204 {
						let body: ApiResponse<String> = response
							.json()
							.nice_unwrap("Unable to parse response from Geode Index");
						fatal!("Unable to update profile: {}", body.error.unwrap());
					}

					profile.display_name = new_display_name;
					info!("Display name updated successfully");
				}
				_ => warn!("Invalid number"),
			}
		} else {
			warn!("Invalid number");
		}
	}
}

fn set_index_url(url: String, config: &mut Config) {
	if url == "default" {
		config.index_url = "https://api.geode-sdk.org".to_string();
	} else {
		config.index_url = url;
	}
	config.save();
	info!("Index URL set to: {}", config.index_url);
}

fn get_index_url(path: String, config: &Config) -> String {
	format!(
		"{}/{}",
		config.index_url.trim_end_matches('/'),
		path.trim_start_matches('/')
	)
}

pub fn subcommand(config: &mut Config, cmd: Index) {
	match cmd {
		Index::New { output } => create_entry(&output),
		Index::Update => update_index(config),
		Index::Install { id, version } => {
			update_index(config);
			install_mod(config, &id, &version.unwrap_or(VersionReq::STAR), false);
			done!("Mod installed");
		}
		Index::Login => login(config),
		Index::Invalidate => invalidate(config),
		Index::Url { url } => set_index_url(url, config),
		Index::Submit { action } => submit(action, config),
		Index::Mods { action } => match action {
			MyModAction::Published => print_own_mods(true, config),
			MyModAction::Pending => print_own_mods(false, config),
			MyModAction::Edit => edit_own_mods(config),
		},
		Index::Profile => edit_profile(config),
		Index::Admin { action } => (),
	}
}
