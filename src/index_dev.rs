use std::fmt::Display;

use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
	config::Config,
	done, fatal, index, info,
	logging::{self, ask_value},
	server::ApiResponse,
	warn, NiceUnwrap,
};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SimpleDevMod {
	pub id: String,
	pub featured: bool,
	pub download_count: i32,
	pub versions: Vec<SimpleDevModVersion>,
	pub developers: Vec<ModDeveloper>,
}

impl Display for SimpleDevMod {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		writeln!(f, "{}", self.id)?;
		writeln!(f, "- Featured: {}", self.featured)?;
		writeln!(f, "- Download count: {}", self.download_count)?;
		writeln!(f, "- Developers:")?;
		for (i, developer) in self.developers.iter().enumerate() {
			writeln!(f, "  {}. {}", i + 1, developer)?;
		}
		writeln!(f, "- Versions:")?;
		for (i, version) in self.versions.iter().enumerate() {
			writeln!(f, "  {}. {}", i + 1, version)?;
		}

		Ok(())
	}
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SimpleDevModVersion {
	pub name: String,
	pub version: String,
	pub download_count: i32,
	pub validated: bool,
}

impl Display for SimpleDevModVersion {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		writeln!(f, "{}", self.version)?;
		writeln!(f, "   - Name: {}", self.name)?;
		writeln!(f, "   - Download count: {}", self.download_count)?;
		writeln!(f, "   - Validated: {}", self.validated)?;

		Ok(())
	}
}

#[derive(Deserialize, Clone)]
pub struct DeveloperProfile {
	pub id: i32,
	pub username: String,
	pub display_name: String,
	pub verified: bool,
	pub admin: bool,
}

impl Display for DeveloperProfile {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		writeln!(f, "Username: {}", self.username)?;
		writeln!(f, "Display name: {}", self.display_name)?;
		writeln!(f, "Verified: {}", self.verified)?;
		writeln!(f, "Admin: {}", self.admin)?;

		Ok(())
	}
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct ModDeveloper {
	pub id: i32,
	pub username: String,
	pub display_name: String,
	pub is_owner: bool,
}

impl Display for ModDeveloper {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		writeln!(f, "{}", self.username)?;
		writeln!(f, "   - Display name: {}", self.display_name)?;
		writeln!(f, "   - Owner: {}", self.is_owner)?;

		Ok(())
	}
}

pub fn print_own_mods(validated: bool, config: &mut Config) {
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
		print!("{}. {}", i + 1, entry);
	}
}

fn get_own_mods(validated: bool, config: &mut Config) -> Vec<SimpleDevMod> {
	if config.index_token.is_none() {
		fatal!("You are not logged in");
	}

	let client = reqwest::blocking::Client::new();

	let validated_str = match validated {
		true => "accepted",
		false => "pending",
	};

	let url = index::get_index_url(format!("/v1/me/mods?status={}", validated_str), config);

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
		fatal!("Unable to fetch mods: {}", body.error);
	}

	if response.status() == 401 {
		config.index_token = None;
		config.save();
		fatal!("Invalid token. Please login again.");
	}

	let mods = response
		.json::<ApiResponse<Vec<SimpleDevMod>>>()
		.nice_unwrap("Unable to parse response from Geode Index");

	mods.payload
}

pub fn edit_own_mods(config: &mut Config) {
	loop {
		let mods = get_own_mods(true, config);
		if mods.is_empty() {
			fatal!("You have no published mods");
		}

		logging::clear_terminal();

		println!("Select a mod to edit:");
		println!("----------------");
		for (i, entry) in mods.iter().enumerate() {
			println!("{}. {}", i + 1, &entry.id);
		}
		println!("----------------");

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
	logging::clear_terminal();
	info!("Editing mod '{}'", mod_to_edit.id);
	println!("{}", mod_to_edit);

	loop {
		println!("----------------");
		println!("Commands: ");
		println!("  - 1: Add a developer");
		println!("  - 2: Remove a developer");
		println!("  - 3: Transfer ownership");
		let response = ask_value("Action number (enter q to go back)", None, true);
		if response == "q" {
			return true;
		}
		if let Ok(index) = response.parse::<usize>() {
			match index {
				1 => {
					add_developer(mod_to_edit, config);
					return false;
				}
				2 => {
					remove_developer(mod_to_edit, config);
					return false;
				}
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
	let url = index::get_index_url(format!("/v1/mods/{}/developers", mod_to_edit.id), config);

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
		warn!("Unable to add developer: {}", body.error);
	} else {
		info!("Developer added successfully");
	}
}

fn remove_developer(mod_to_edit: &SimpleDevMod, config: &mut Config) {
	let username = ask_value("Username", None, true);

	let client = reqwest::blocking::Client::new();
	let url = index::get_index_url(
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
		warn!("Unable to remove developer: {}", body.error);
	} else {
		info!("Developer removed successfully");
	}
}

pub fn get_user_profile(config: &mut Config) -> DeveloperProfile {
	if config.index_token.is_none() {
		fatal!("You are not logged in");
	}

	let client = reqwest::blocking::Client::new();

	let url = index::get_index_url("/v1/me", config);

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
		fatal!("Unable to fetch profile: {}", body.error);
	}

	let profile = response
		.json::<ApiResponse<DeveloperProfile>>()
		.nice_unwrap("Unable to parse response from Geode Index");

	profile.payload
}

pub fn edit_profile(config: &mut Config) {
	let mut profile = get_user_profile(config);

	let client = reqwest::blocking::Client::new();
	let mut status_message: Option<String> = None;

	loop {
		logging::clear_terminal();

		if status_message.is_some() {
			info!("{}\n", status_message.clone().unwrap());
			status_message = None;
		}

		println!("Your profile:");
		println!("----------------");
		println!("Username: {}", profile.username);
		println!("Display name: {}", profile.display_name);
		println!("Verified: {}", profile.verified);
		println!("Admin: {}", profile.admin);
		println!("----------------");
		println!("Commands:");
		println!("  - 1: Change display name");
		let response = ask_value("Action number (enter q to exit)", None, true);
		if response == "q" {
			break;
		}
		if let Ok(index) = response.parse::<usize>() {
			match index {
				1 => {
					let new_display_name = ask_value("New display name", None, true);
					let url = index::get_index_url("/v1/me", config);
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
						fatal!("Unable to update profile: {}", body.error);
					}

					profile.display_name = new_display_name;
					status_message = Some("Display name updated successfully".to_string());
				}
				_ => warn!("Invalid number"),
			}
		} else {
			warn!("Invalid number");
		}
	}
}
