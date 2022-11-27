use clap::Subcommand;
use zip::ZipArchive;
use crate::config::{geode_root, Config};
use crate::input::ask_value;
use crate::mod_file;
use crate::{fatal, done, info, warn, geode_assert, NiceUnwrap};
use crypto::digest::Digest;
use crypto::sha3::Sha3;
use serde::Serialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::io;
use colored::Colorize;

#[derive(Subcommand, Debug)]
#[clap(rename_all = "kebab-case")]
pub enum Index {
	/// Create a new entry to be used in the index
	New {
		/// Output folder of entry
		output: PathBuf
	},

	/// Updates the index cache
	Update
}

fn update_index() {
	let index_dir = geode_root().join("geode/index");
	let current_sha = fs::read_to_string(index_dir.join("current")).unwrap_or(String::new());

	let client = reqwest::blocking::Client::new();

	let response = client.get("https://api.github.com/repos/geode-sdk/mods/commits/main")
		.header("Accept", "application/vnd.github.sha")
		.header("If-None-Match", format!("\"{}\"", current_sha))
		.header("User-Agent", "GeodeCli")
		.send()
		.nice_unwrap("Unable to fetch index version");

	if response.status() == 304 {
		done!("Index is already latest version");
		return;
	}
	geode_assert!(response.status() == 200, "Version check received status code {}", response.status());
	let latest_sha = response.text().nice_unwrap("Unable to decode index version");

	let mut zip_data = io::Cursor::new(Vec::new());

	client.get("https://github.com/geode-sdk/mods/zipball/main")
		.send().nice_unwrap("Unable to download index")
		.copy_to(&mut zip_data).nice_unwrap("Unable to write to index");

	let mut zip_archive = ZipArchive::new(zip_data).nice_unwrap("Unable to decode index zip");


	let before_items = if index_dir.join("index").exists() {
		let mut items = fs::read_dir(index_dir.join("index"))
			.unwrap()
			.into_iter()
			.map(|x| x.unwrap().path())
			.collect::<Vec<_>>();
		items.sort();

		fs::remove_dir_all(index_dir.join("index")).nice_unwrap("Unable to remove old index version");
		Some(items)
	} else {
		None
	};

	let extract_dir = std::env::temp_dir().join("zip");
	if extract_dir.exists() {
		fs::remove_dir_all(&extract_dir).nice_unwrap("Unable to prepare new index");
	}
	fs::create_dir(&extract_dir).unwrap();
	zip_archive.extract(&extract_dir).nice_unwrap("Unable to extract new index");

	let new_root_dir = fs::read_dir(extract_dir).unwrap().next().unwrap().unwrap().path();
	for item in fs::read_dir(new_root_dir).unwrap() {
		let item_path = item.unwrap().path();
		let dest_path = index_dir.join(item_path.file_name().unwrap());

		if dest_path.exists() {
			if dest_path.is_dir() {
				fs::remove_dir_all(&dest_path).unwrap();
			} else {
				fs::remove_file(&dest_path).unwrap();
			}
		}

		fs::rename(item_path, dest_path).nice_unwrap("Unable to copy new index");
	}
	
	
	let mut after_items = fs::read_dir(index_dir.join("index"))
		.unwrap()
		.into_iter()
		.map(|x| x.unwrap().path())
		.collect::<Vec<_>>();
	after_items.sort();

	if let Some(before_items) = before_items {
		if before_items != after_items {
			info!("Changelog:");

			for i in &before_items {
				if !after_items.contains(i) {
					println!("            {} {}", "-".red(), i.file_name().unwrap().to_str().unwrap());
				}
			}

			for i in &after_items {
				if !before_items.contains(i) {
					println!("            {} {}", "+".green(), i.file_name().unwrap().to_str().unwrap());
				}
			}
		}
	}

	fs::write(index_dir.join("current"), latest_sha).nice_unwrap("Unable to save version");
	done!("Successfully updated index")
}

fn create_index_json(path: &Path) {
	let url = ask_value("URL", None, true);

	let response = reqwest::blocking::get(&url).nice_unwrap("Unable to access .geode file at URL");

	let file_name = reqwest::Url::parse(&url).unwrap()
		.path_segments()
		.and_then(|segments| segments.last())
		.and_then(|name| if name.is_empty() { None } else { Some(name.to_string()) })
		.unwrap_or_else(|| ask_value("Filename", None, true));

	let file_contents = response
		.bytes()
		.nice_unwrap("Unable to access .geode file at URL");

	let mut hasher = Sha3::sha3_256();
	hasher.input(&file_contents);
	let hash = hasher.result_str();

	let platform_str = ask_value("Supported platforms (comma separated)", None, true);
	let platforms = platform_str.split(",").collect::<Vec<_>>();

	let category_str = ask_value("Categories (comma separated)", None, true);
	let categories = category_str.split(",").collect::<Vec<_>>();

	let index_json = json!({
		"download": {
			"url": url,
			"name": file_name,
			"hash": hash,
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
		&path.join("index.json"),
		String::from_utf8(ser.into_inner()).unwrap(),
	).nice_unwrap("Unable to write to project");
}

fn create_entry(out_path: &Path) {
	geode_assert!(out_path.exists(), "Path does not exist");
	geode_assert!(out_path.is_dir(), "Path is not a directory");

	let root_path = PathBuf::from(ask_value("Project root directory", Some("."), true));

	let mod_json_path = root_path.join("mod.json");
	let about_path = root_path.join("about.md");
	let logo_path = root_path.join("logo.png");

	geode_assert!(mod_json_path.exists(), "Unable to find project mod.json");

	// Get mod id
	let mod_json: Value = serde_json::from_str(
		&fs::read_to_string(&mod_json_path).nice_unwrap("Could not read mod.json"),
	).nice_unwrap("Could not parse mod.json");
	let mod_info = mod_file::get_mod_file_info(&mod_json, &mod_json_path);

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

pub fn subcommand(_config: &mut Config, cmd: Index) {
	match cmd {
		Index::New { output } => create_entry(&output),

		Index::Update => update_index()
	}
}
