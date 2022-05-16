#[cfg(windows)]
use directories::BaseDirs;

use std::path::{PathBuf};
use std::vec::Vec;
use std::process::exit;
use std::fs;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use colored::Colorize;
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Installation {
	pub path: PathBuf,
	pub executable: String,
    #[serde(flatten)]
    other: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
	pub default_installation: usize,
	pub working_installation: Option<usize>,
	pub installations: Option<Vec<Installation>>,
	pub default_developer: Option<String>,
    #[serde(flatten)]
    other: HashMap<String, Value>,
}

impl Config {
	pub fn data_dir() -> PathBuf {
		// get data dir per-platform
		let data_dir: PathBuf;
		#[cfg(windows)] {
			data_dir = BaseDirs::new().unwrap().data_local_dir().to_path_buf().join("Geode");
		};
		#[cfg(target_os = "macos")] {
			data_dir = PathBuf::from("/Users/Shared/Geode");
		};
		#[cfg(not(any(windows, target_os = "macos")))] {
			use std::compile_error;
			compile_error!("implement config.json directory");
		};
		data_dir
	}

	pub fn init(&mut self) {
		let config_json = Config::data_dir().join("config.json");
		if !config_json.exists() {
			println!(
				"{}{}{}{}",
				"WARNING: It seems you don't have Geode installed! \
				Please install Geode first using the official installer \
				(".yellow(),
				"https://github.com/geode-sdk/installer/releases/latest".cyan(),
				")".yellow(),
				"\nYou may still use the CLI, but be warned that certain \
				operations will cause crashes.\n".purple()
			);
			fs::create_dir_all(Config::data_dir()).unwrap();
			return;
		}
		*self = match serde_json::from_str(
		&fs::read_to_string(&config_json).unwrap()
		) {
			Ok(p) => p,
			Err(e) => {
				println!("Unable to parse config.json: {}", e);
				exit(1);
			}
		};
		if self.installations.is_none() {
			println!(
				"{}{}{}{}",
				"WARNING: It seems you don't have any installations of Geode! \
				Please install Geode first using the official installer \
				(".yellow(),
				"https://github.com/geode-sdk/installer/releases/latest".cyan(),
				")".yellow(),
				"\nYou may still use the CLI, but be warned that certain \
				operations will cause crashes.\n".purple()
			);
			return;
		}
		if self.working_installation.is_none() {
			self.working_installation = Some(
				self.default_installation
			);
		}
	}

	pub fn new() -> Config {
		let mut config = Config {
			default_installation: 0,
			working_installation: None,
			installations: None,
			default_developer: None,
			other: HashMap::new(),
		};
		config.init();
		config
	}

	pub fn save(&mut self) {
		fs::write(
			Config::data_dir().join("config.json"),
			serde_json::to_string(self).unwrap()
		).unwrap();
	}

	pub fn work_inst(&mut self) -> &Installation {
		&self.installations.as_ref().unwrap()[
			self.working_installation.unwrap()
		]
	}
}
