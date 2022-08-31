use std::cell::RefCell;
use std::collections::HashMap;
#[cfg(windows)]
use directories::BaseDirs;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

use crate::{fail, warn, done, info};
use crate::NiceUnwrap;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Profile {
	pub name: String,
	pub gd_path: PathBuf,

    #[serde(flatten)]
    other: HashMap<String, Value>
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
	pub current_profile: String,
	pub profiles: Vec<RefCell<Profile>>,
	pub default_developer: Option<String>,
	pub sdk_path: Option<PathBuf>,
	pub sdk_nightly: bool,
    #[serde(flatten)]
    other: HashMap<String, Value>,
}

pub fn geode_root() -> PathBuf {
	// get data dir per-platform
	let data_dir: PathBuf;
	#[cfg(windows)] {
		data_dir = dirs::data_local_dir().unwrap().join("Geode");
	};
	#[cfg(target_os = "macos")] {
		data_dir = PathBuf::from("/Users/Shared/Geode");
	};
	#[cfg(not(any(windows, target_os = "macos")))] {
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
			other: HashMap::<String, Value>::new()
		}
	}
}

impl Config {
	pub fn get_profile(&self, name: &str) -> Option<&RefCell<Profile>> {
		self.profiles.iter().filter(|x| x.borrow().name == name).next()
	}

	pub fn new() -> Config {
		if !geode_root().exists() {
			warn!("It seems you don't have Geode installed. Some operations will not work");
			info!("You can install geode using the official installer");
			info!("At {}", "https://github.com/geode-sdk/installer/releases/latest".bright_cyan());

			return Config {
				current_profile: String::new(),
				profiles: Vec::new(),
				default_developer: None,
				sdk_path: None,
				sdk_nightly: false,
				other: HashMap::<String, Value>::new()
			};
		}

		let config_json = geode_root().join("config.json");

		let mut output: Config = if !config_json.exists() {
			// Create new config
			Config {
				current_profile: String::new(),
				profiles: Vec::new(),
				default_developer: None,
				sdk_path: None,
				sdk_nightly: false,
				other: HashMap::<String, Value>::new()
			}
		} else {
			serde_json::from_str(&std::fs::read_to_string(&config_json).unwrap())
				.nice_unwrap("Unable to parse config.json")
		};

		output.save();

		if output.profiles.is_empty() {
			warn!("No Geode profiles found! Some operations will be unavailable.");
			info!("Install Geode using the official installer (https://github.com/geode-sdk/installer/releases/latest)");

		} else if output.get_profile(&output.current_profile.clone()).is_none() {
			output.current_profile = output.profiles[0].borrow().name.clone();
		}

		output
	}

	pub fn save(&self) {
		std::fs::write(
			geode_root().join("config.json"),
			serde_json::to_string(self).unwrap()
		).nice_unwrap("Unable to save config");
	}

	pub fn rename_profile(&mut self, old: &str, new: String) {
		let profile = self.get_profile(old)
			.nice_unwrap(format!("Profile named '{}' does not exist", old));

		if self.get_profile(&new).is_some() {
			fail!("The name '{}' is already taken!", new);
		} else {
			done!("Successfully renamed '{}' to '{}'", old, &new);
			profile.borrow_mut().name = new;
		}
	}
}
