use serde_json::Value;
use std::path::PathBuf;
use serde::Serialize;
use std::path::Path;
use crate::config::Config;
use clap::Subcommand;
use crate::input::ask_value;
use serde_json::json;
use crate::{fatal, warn, NiceUnwrap};
use crypto::digest::Digest;
use crate::mod_file;
use crypto::sha3::Sha3;
use std::fs;

#[derive(Subcommand, Debug)]
#[clap(rename_all = "kebab-case")]
pub enum Index {
	/// Create a new entry to be used in the index
	New {
		/// Output folder of entry
		output: PathBuf
	}
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
	if !out_path.exists() {
		fatal!("Path does not exist");
	}
	if out_path.is_file() {
		fatal!("Path is a file");
	}

	let root_path = PathBuf::from(ask_value("Project root directory", Some("."), true));

	let mod_json_path = root_path.join("mod.json");
	let about_path = root_path.join("about.md");
	let logo_path = root_path.join("logo.png");

	if !mod_json_path.exists() {
		fatal!("Unable to find project mod.json");
	}

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

pub fn subcommand(config: &mut Config, cmd: Index) {
	match cmd {
		Index::New { output } => create_entry(&output)
	}
}
