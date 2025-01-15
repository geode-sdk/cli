use std::fmt::Display;

use crate::{
	config::Config,
	fatal,
	index::{self, AdminAction},
	index_dev::{self, DeveloperProfile},
	info,
	logging::{self, ask_value},
	server::{ApiResponse, PaginatedData},
	warn, NiceUnwrap,
};

use rand::Rng;
use reqwest::header::USER_AGENT;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
struct PendingMod {
	id: String,
	repository: Option<String>,
	versions: Vec<PendingModVersion>,
	tags: Vec<String>,
	about: Option<String>,
}

impl Display for PendingMod {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		writeln!(f, "{}", self.id)?;
		writeln!(
			f,
			"- Repository: {}",
			self.repository.as_deref().unwrap_or("None")
		)?;
		writeln!(f, "- Tags: {}", self.tags.join(", "))?;
		writeln!(f, "- About")?;
		writeln!(f, "----------------------------")?;
		writeln!(f, "{}", self.about.as_deref().unwrap_or("None"))?;
		writeln!(f, "----------------------------")?;
		// To be honest I have no idea if we should show this, it can become quite large
		// writeln!(f, "- Changelog");
		// writeln!(f, "----------------------------");
		// writeln!(f, "{}", self.changelog.as_deref().unwrap_or("None"));
		// writeln!(f, "----------------------------");
		writeln!(f, "- Versions:")?;
		for version in self.versions.iter() {
			writeln!(f, "{}", version)?;
		}

		Ok(())
	}
}

#[derive(Debug, Deserialize, Clone)]
struct PendingModVersion {
	name: String,
	version: String,
	description: Option<String>,
	geode: String,
	early_load: bool,
	api: bool,
	mod_id: String,
	gd: PendingModGD,
	dependencies: Option<Vec<PendingModDepencency>>,
	incompatibilities: Option<Vec<PendingModDepencency>>,
}

impl Display for PendingModVersion {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		writeln!(f, "{}", self.version)?;
		writeln!(f, "  - Name: {}", self.name)?;
		writeln!(f, "  - ID: {}", self.mod_id)?;
		writeln!(
			f,
			"  - Description: {}",
			self.description.as_deref().unwrap_or("None")
		)?;
		writeln!(f, "  - Geode: {}", self.geode)?;
		writeln!(f, "  - Early Load: {}", self.early_load)?;
		writeln!(f, "  - API: {}", self.api)?;
		writeln!(f, "  - GD:")?;
		writeln!(f, "    - Win: {}", self.gd.win.as_deref().unwrap_or("None"))?;
		writeln!(
			f,
			"    - Mac Intel: {}",
			self.gd.mac_intel.as_deref().unwrap_or("None")
		)?;
		writeln!(
			f,
			"    - Mac ARM: {}",
			self.gd.mac_arm.as_deref().unwrap_or("None")
		)?;
		writeln!(
			f,
			"    - Android 32: {}",
			self.gd.android32.as_deref().unwrap_or("None")
		)?;
		writeln!(
			f,
			"    - Android 64: {}",
			self.gd.android64.as_deref().unwrap_or("None")
		)?;
		writeln!(f, "    - iOS: {}", self.gd.ios.as_deref().unwrap_or("None"))?;
		if let Some(deps) = &self.dependencies {
			writeln!(f, "  - Dependencies:")?;
			for dep in deps {
				writeln!(f, "{}", dep)?;
			}
		}
		if let Some(incomps) = &self.incompatibilities {
			writeln!(f, "  - Incompatibilities:")?;
			for incomp in incomps {
				writeln!(f, "{}", incomp)?;
			}
		}
		Ok(())
	}
}

#[derive(Debug, Deserialize, Clone)]
struct PendingModGD {
	win: Option<String>,
	#[serde(rename = "mac-intel")]
	mac_intel: Option<String>,
	#[serde(rename = "mac-arm")]
	mac_arm: Option<String>,
	android32: Option<String>,
	android64: Option<String>,
	ios: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct PendingModDepencency {
	mod_id: String,
	version: String,
	importance: String,
}

impl Display for PendingModDepencency {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		writeln!(f, "    {}", self.mod_id)?;
		writeln!(f, "      - Version: {}", self.version)?;
		writeln!(f, "      - Importance: {}", self.importance)
	}
}

pub fn subcommand(action: AdminAction, config: &mut Config) {
	if config.index_token.is_none() {
		fatal!("You are not logged in!");
	}
	let profile = index_dev::get_user_profile(config);
	if !profile.admin {
		let message = get_random_message();
		fatal!("{}", message);
	}

	match action {
		AdminAction::ListPending => {
			list_pending_mods(config);
		}
		AdminAction::DevStatus => {
			update_dev_status(config);
		}
	}
}

fn get_pending_mods(page: i32, config: &Config) -> PaginatedData<PendingMod> {
	if config.index_token.is_none() {
		fatal!("You are not logged in!");
	}

	let client = reqwest::blocking::Client::new();
	let path = format!("v1/mods?status=pending&page={}&per_page=1", page);
	let url = index::get_index_url(path, config);

	let response = client
		.get(url)
		.bearer_auth(config.index_token.clone().unwrap())
		.send()
		.nice_unwrap("Failed to connect to the Geode Index");

	if response.status() != 200 {
		if let Ok(body) = response.json::<ApiResponse<String>>() {
			warn!("{}", body.error);
		}
		fatal!("Bad response from Geode Index");
	}

	let data: ApiResponse<PaginatedData<PendingMod>> = response
		.json()
		.nice_unwrap("Failed to parse response from the Geode Index");

	data.payload
}

fn list_pending_mods(config: &Config) {
	let mut page = 1;

	loop {
		let mods = get_pending_mods(page, config);

		if mods.count == 0 {
			info!("No pending mods on the index");
			break;
		}

		logging::clear_terminal();

		for entry in mods.data.iter() {
			println!("{}", entry);
		}

		println!("---------------------");
		println!("Submission {}/{}", page, mods.count);
		println!("---------------------");
		println!("Commands:");
		println!("  - n: Next submission");
		println!("  - p: Previous submission");
		println!("  - <INDEX>: Go to submission");
		println!("  - v: Validate mod");
		println!("  - r: Reject mod");
		println!("  - i: Install mod");
		println!("  - q: Quit");
		println!("---------------------");

		let choice = ask_value("Action", None, true);

		match choice.trim() {
			"n" => {
				if page < mods.count {
					page += 1;
				}
			}
			"p" => {
				if page > 1 {
					page -= 1;
				}
			}
			"v" => {
				let version_vec: &Vec<PendingModVersion> = mods.data[0].versions.as_ref();

				if version_vec.len() == 1 {
					validate_mod(&version_vec[0], &mods.data[0].id, config);
				} else {
					let version = ask_value("Version", None, true);
					if let Some(version) = version_vec.iter().find(|x| x.version == version) {
						validate_mod(version, &mods.data[0].id, config);
					} else {
						warn!("Invalid version");
					}
				}
			}
			"r" => {
				let version_vec: &Vec<PendingModVersion> = mods.data[0].versions.as_ref();

				if version_vec.len() == 1 {
					reject_mod(&version_vec[0], &mods.data[0].id, config);
				} else {
					let version = ask_value("Version", None, true);
					if let Some(version) = version_vec.iter().find(|x| x.version == version) {
						reject_mod(version, &mods.data[0].id, config);
					} else {
						warn!("Invalid version");
					}
				}
			}
			"i" => {
				let version_vec: &Vec<PendingModVersion> = mods.data[0].versions.as_ref();

				if version_vec.len() == 1 {
					download_mod(&version_vec[0], &mods.data[0].id, config);
				} else {
					let version = ask_value("Version", None, true);
					if let Some(version) = version_vec.iter().find(|x| x.version == version) {
						download_mod(version, &mods.data[0].id, config);
					} else {
						warn!("Invalid version");
					}
				}
			}
			"q" => {
				break;
			}
			_ => {
				if let Ok(new_page) = choice.parse::<i32>() {
					if new_page < 1 || new_page > mods.count {
						warn!("Invalid page number");
					} else {
						page = new_page;
					}
				} else {
					warn!("Invalid input");
				}
			}
		}
	}
}

fn get_developer_profile(username: &str, config: &Config) -> Option<DeveloperProfile> {
	let client = reqwest::blocking::Client::new();

	let url = index::get_index_url("/v1/developers", config);

	let response = client
		.get(url)
		.query(&[("query", username)])
		.header(USER_AGENT, "GeodeCLI")
		.bearer_auth(config.index_token.clone().unwrap())
		.send()
		.nice_unwrap("Unable to connect to Geode Index");

	if response.status() != 200 {
		warn!("Unable to fetch profile: {}", response.status());
		return None;
	}

	let profile: Option<DeveloperProfile> =
		match response.json::<ApiResponse<PaginatedData<DeveloperProfile>>>() {
			Err(_) => None,
			Ok(p) => {
				if p.payload.data.is_empty() {
					None
				} else {
					p.payload.data.first().cloned()
				}
			}
		};

	profile
}

fn update_dev_status(config: &Config) {
	let username = ask_value("Username", None, true);
	let developer = get_developer_profile(&username, config);
	if developer.is_none() {
		warn!("Couldn't fetch developer");
		return;
	}
	let developer = developer.unwrap();

	info!("Developer found: ");
	println!("{}", developer);

	let mut verified = developer.verified;
	loop {
		let status = ask_value("New status (verified/unverified)", None, true);
		if status == "verified" {
			if verified {
				warn!("Developer is already verified!");
			} else {
				verified = true;
				break;
			}
		} else if status == "unverified" {
			if !verified {
				warn!("Developer is already unverified!");
			} else {
				verified = false;
				break;
			}
		} else {
			warn!("Invalid option");
		}
	}

	let client = reqwest::blocking::Client::new();

	let url = index::get_index_url(
		format!("/v1/developers/{}", developer.id).to_string(),
		config,
	);
	let response = client
		.put(url)
		.bearer_auth(config.index_token.as_ref().unwrap())
		.json(&json!({ "verified": verified }))
		.send()
		.nice_unwrap("Failed to update developer");

	if response.status() != 200 {
		let json = response.json::<serde_json::Value>();
		if let Ok(j) = json {
			if j.is_object() && j.as_object().unwrap().contains_key("error") {
				let err = j.as_object().unwrap().get("error").unwrap().to_string();
				fatal!("Failed to update developer: {}", err);
			}
		} else {
			fatal!("Failed to update developer. No error received from index.");
		}
	}

	info!("Developer updated successfully");
}

fn validate_mod(version: &PendingModVersion, id: &str, config: &Config) {
	if config.index_token.is_none() {
		fatal!("You are not logged in!");
	}
	let client = reqwest::blocking::Client::new();
	let path = format!("v1/mods/{}/versions/{}", id, version.version);
	let url = index::get_index_url(path, config);

	let response = client
		.put(url)
		.bearer_auth(config.index_token.clone().unwrap())
		.json(&json!({
			"status": "accepted"
		}))
		.send()
		.nice_unwrap("Failed to connect to the Geode Index");

	if response.status() != 204 {
		if let Ok(body) = response.json::<ApiResponse<String>>() {
			warn!("{}", body.error);
		}
		fatal!("Bad response from Geode Index");
	}

	info!("Mod validated");
}

fn reject_mod(version: &PendingModVersion, id: &str, config: &Config) {
	let reason = ask_value("Reason", None, true);

	let client = reqwest::blocking::Client::new();
	let path = format!("v1/mods/{}/versions/{}", id, version.version);
	let url = index::get_index_url(path, config);

	let response = client
		.put(url)
		.bearer_auth(config.index_token.clone().unwrap())
		.json(&json!({
			"status": "rejected",
			"info": reason
		}))
		.send()
		.nice_unwrap("Failed to connect to the Geode Index");

	if response.status() != 204 {
		if let Ok(body) = response.json::<ApiResponse<String>>() {
			warn!("{}", body.error);
		}
		fatal!("Bad response from Geode Index");
	}

	info!("Mod rejected");
}

fn download_mod(version: &PendingModVersion, id: &str, config: &Config) {
	let client = reqwest::blocking::Client::new();
	let path = format!("v1/mods/{}/versions/{}/download", id, version.version);
	let url = index::get_index_url(path, config);

	let response = client
		.get(url)
		.bearer_auth(config.index_token.clone().unwrap())
		.send()
		.nice_unwrap("Failed to connect to the Geode Index");

	if response.status() != 200 {
		if let Ok(body) = response.json::<ApiResponse<String>>() {
			warn!("{}", body.error);
		}
		fatal!("Bad response from Geode Index");
	}

	let data = response.bytes().nice_unwrap("Failed to download mod");

	let mods_dir = config.get_current_profile().mods_dir();
	let mod_path = mods_dir.join(format!("{}.geode", version.mod_id));

	std::fs::write(mod_path, data).nice_unwrap("Failed to save mod");

	info!("Mod downloaded");
}

pub fn get_random_message() -> String {
	let messages = [
		"[BUZZER]",
		"Your princess is in another castle",
		"Absolutely not",
		"Get lost",
		"Sucks to be you",
		"No admin, laugh at this user",
		"Admin dashboard",
		"Why are we here? Just to suffer?",
		"You hacked the mainframe! Congrats.",
		"You're an admin, Harry",
	];

	let mut rng = rand::thread_rng();
	let index = rng.gen_range(0..messages.len());
	messages[index].to_string()
}
