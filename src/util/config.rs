use std::cell::RefCell;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

use crate::NiceUnwrap;
use crate::{done, fail, fatal, info, warn};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Profile {
	pub name: String,
	pub gd_path: PathBuf,

	#[serde(flatten)]
	other: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
	pub current_profile: Option<String>,
	pub profiles: Vec<RefCell<Profile>>,
	pub default_developer: Option<String>,
	pub sdk_nightly: bool,
	#[serde(flatten)]
	other: HashMap<String, Value>,
}

// old config.json structures for migration

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct OldConfigInstallation {
	pub path: PathBuf,
	pub executable: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct OldConfig {
	pub default_installation: usize,
	pub working_installation: Option<usize>,
	pub installations: Option<Vec<OldConfigInstallation>>,
	pub default_developer: Option<String>,
}

impl OldConfig {
	pub fn migrate(&self) -> Config {
		let profiles = self
			.installations
			.as_ref()
			.map(|insts| {
				insts
					.iter()
					.map(|inst| {
						RefCell::from(Profile {
							name: inst
								.executable
								.strip_suffix(".exe")
								.unwrap_or(&inst.executable)
								.into(),
							gd_path: inst.path.clone(),
							other: HashMap::new(),
						})
					})
					.collect::<Vec<_>>()
			})
			.unwrap_or_default();
		Config {
			current_profile: profiles
				.get(
					self.working_installation
						.unwrap_or(self.default_installation),
				)
				.map(|i| i.borrow().name.clone()),
			profiles,
			default_developer: self.default_developer.to_owned(),
			sdk_nightly: false,
			other: HashMap::new(),
		}
	}
}

pub fn geode_root() -> PathBuf {
	// get data dir per-platform
	let data_dir: PathBuf;
	#[cfg(any(windows, target_os = "linux"))]
	{
		data_dir = dirs::data_local_dir().unwrap().join("Geode");
	};
	#[cfg(target_os = "macos")]
	{
		data_dir = PathBuf::from("/Users/Shared/Geode");
	};
	#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
	{
		use std::compile_error;
		compile_error!("implement root directory");
	};
	data_dir
}

impl Profile {
	pub fn new(name: String, location: PathBuf) -> Profile {
		Profile {
			name,
			gd_path: location,
			other: HashMap::<String, Value>::new(),
		}
	}
}

impl Config {
	pub fn get_profile(&self, name: &Option<String>) -> Option<&RefCell<Profile>> {
		if let Some(name) = name {
			self.profiles.iter().find(|x| &x.borrow().name == name)
		} else {
			None
		}
	}

	pub fn sdk_path() -> PathBuf {
		let sdk_var = std::env::var("GEODE_SDK")
			.nice_unwrap("Unable to find Geode SDK. Please install the Geode SDK or use `geode sdk set-path` to make it visible");

		let path = PathBuf::from(sdk_var);
		if !path.is_dir() {
			fail!("Internal Error: GEODE_SDK doesn't point to a directory");
			std::process::exit(1)
		}
		if !path.join("VERSION").exists() {
			fail!(
				"Internal Error: GEODE_SDK/VERSION not found. Please reinstall the Geode SDK."
			);
			std::process::exit(1)
		}

		path
	}

	pub fn new() -> Config {
		if !geode_root().exists() {
			warn!("It seems you don't have Geode installed. Some operations will not work");
			warn!("You can setup Geode using `geode config setup`");

			return Config {
				current_profile: None,
				profiles: Vec::new(),
				default_developer: None,
				sdk_nightly: false,
				other: HashMap::<String, Value>::new(),
			};
		}

		let config_json = geode_root().join("config.json");

		let mut output: Config = if !config_json.exists() {
			info!("Setup Geode using `geode config setup`");
			// Create new config
			Config {
				current_profile: None,
				profiles: Vec::new(),
				default_developer: None,
				sdk_nightly: false,
				other: HashMap::<String, Value>::new(),
			}
		} else {
			// Parse config
			let config_json_str =
				&std::fs::read_to_string(&config_json).nice_unwrap("Unable to read config.json");
			match serde_json::from_str(config_json_str) {
				Ok(json) => json,
				Err(e) => {
					// Try migrating old config
					if let Ok(json) = serde_json::from_str::<OldConfig>(config_json_str) {
						info!("Migrating old config.json");
						json.migrate()
					} else {
						fatal!("Unable to parse config.json: {}", e);
					}
				}
			}
		};

		output.save();

		if output.profiles.is_empty() {
			warn!("No Geode profiles found! Some operations will be unavailable.");
			warn!("Setup Geode using `geode config setup`");
		} else if output.get_profile(&output.current_profile).is_none() {
			output.current_profile = Some(output.profiles[0].borrow().name.clone());
		}

		output
	}

	pub fn save(&self) {
		std::fs::create_dir_all(geode_root()).nice_unwrap("Unable to create Geode directory");
		std::fs::write(
			geode_root().join("config.json"),
			serde_json::to_string(self).unwrap(),
		)
		.nice_unwrap("Unable to save config");
	}

	pub fn rename_profile(&mut self, old: &str, new: String) {
		let profile = self
			.get_profile(&Some(String::from(old)))
			.nice_unwrap(format!("Profile named '{}' does not exist", old));

		if self.get_profile(&Some(new.to_owned())).is_some() {
			fail!("The name '{}' is already taken!", new);
		} else {
			done!("Successfully renamed '{}' to '{}'", old, &new);
			profile.borrow_mut().name = new;
		}
	}
}
