use crate::config::Config;
use crate::file::{copy_dir_recursive, read_dir_recursive};
use crate::util::logging::ask_value;
use crate::util::mod_file::{parse_mod_info, try_parse_mod_info};
use crate::{done, fatal, info, warn, NiceUnwrap};
use clap::Subcommand;
use colored::Colorize;
use reqwest::header::{AUTHORIZATION, USER_AGENT};
use semver::VersionReq;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha3::{Digest, Sha3_256};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

#[derive(Subcommand, Debug)]
#[clap(rename_all = "kebab-case")]
pub enum Index {
	/// Create a new entry to be used in the index
	New {
		/// Output folder of entry
		output: PathBuf,
	},

	/// Install a mod from the index to the current profile
	Install {
		/// Mod ID to install
		id: String,

		/// Mod version to install, defaults to latest
		version: Option<VersionReq>,
	},

	/// Updates the index cache
	Update,
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

	let mut zip_data = io::Cursor::new(Vec::new());

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

	for path in {
		read_dir_recursive(&dir)
			.nice_unwrap("Unable to read index")
	} {
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

pub fn install_mod(config: &Config, id: &String, version: &VersionReq) -> PathBuf {
	let entry = get_entry(config, id, version)
		.nice_unwrap(format!("Unable to find '{id}' version '{version}'"));

	let plat = if cfg!(windows) {
		"windows"
	} else if cfg!(target_os = "macos") {
		"macos"
	} else {
		fatal!("This platform doesn't support installing mods");
	};

	if !entry.platforms.contains(plat) {
		fatal!("Mod '{}' is not available on '{}'", id, plat);
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

pub fn subcommand(config: &mut Config, cmd: Index) {
	match cmd {
		Index::New { output } => create_entry(&output),
		Index::Update => update_index(config),
		Index::Install { id, version } => {
			update_index(config);
			install_mod(config, &id, &version.unwrap_or(VersionReq::STAR));
			done!("Mod installed");
		}
	}
}
