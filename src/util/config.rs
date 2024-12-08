use std::cell::{Ref, RefCell};
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

use crate::{done, fail, fatal, warn, NiceUnwrap};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Profile {
	pub name: String,
	pub gd_path: PathBuf,

	#[serde(default = "profile_platform_default")]
	pub platform: String,

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
	pub sdk_version: Option<String>,
	pub index_token: Option<String>,
	#[serde(default = "default_index_url")]
	pub index_url: String,
	#[serde(flatten)]
	other: HashMap<String, Value>,
}

fn default_index_url() -> String {
	"https://api.geode-sdk.org".to_string()
}

// old config.json structures for migration
// TODO: remove this in 3.0
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct OldConfigInstallation {
	pub path: PathBuf,
	pub executable: String,
}

// TODO: remove this in 3.0
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct OldConfig {
	pub default_installation: usize,
	pub working_installation: Option<usize>,
	pub installations: Option<Vec<OldConfigInstallation>>,
	pub default_developer: Option<String>,
}

pub fn profile_platform_default() -> String {
	if cfg!(target_os = "windows") {
		"win".to_owned()
	} else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
		"mac-intel".to_owned()
	} else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
		"mac-arm".to_owned()
	} else {
		"win".to_owned()
	}
}

pub fn geode_root() -> PathBuf {
	// get data dir per-platform
	let data_dir: PathBuf;
	#[cfg(any(windows, target_os = "linux", target_os = "android"))]
	{
		data_dir = dirs::data_local_dir().unwrap().join("Geode");
	};
	#[cfg(target_os = "macos")]
	{
		data_dir = PathBuf::from("/Users/Shared/Geode");
	};
	#[cfg(not(any(
		windows,
		target_os = "macos",
		target_os = "linux",
		target_os = "android"
	)))]
	{
		use std::compile_error;
		compile_error!("implement root directory");
	};
	data_dir
}

fn migrate_location(name: &str, mut path: PathBuf, platform: &str) -> PathBuf {
	// Migrate folder to executable
	if (platform == "win") && path.is_dir() {
		path.push("GeometryDash.exe");

		if !path.exists() {
			warn!(
				"Unable to find GeometryDash.exe in profile \
				  '{}', please update the GD path for it",
				name
			);
		}
	} else if path.file_name().unwrap() == "Contents" {
		path = path.parent().unwrap().to_path_buf();
	}

	path
}

impl Profile {
	pub fn new(name: String, location: PathBuf, platform: String) -> Profile {
		Profile {
			gd_path: migrate_location(&name, location, &platform),
			name,
			platform,
			other: HashMap::<String, Value>::new(),
		}
	}

	pub fn gd_dir(&self) -> PathBuf {
		if self.platform == "win" {
			self.gd_path.parent().unwrap().to_path_buf()
		} else {
			self.gd_path.clone()
		}
	}

	pub fn geode_dir(&self) -> PathBuf {
		if self.platform == "win" {
			self.gd_path.parent().unwrap().join("geode")
		} else if self.platform == "android32" || self.platform == "android64" {
			self.gd_path.join("game/geode")
		} else {
			self.gd_path.join("Contents/geode")
		}
	}

	pub fn mods_dir(&self) -> PathBuf {
		self.geode_dir().join("mods")
	}

	pub fn platform_str(&self) -> &str {
		self.platform.as_str()
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

	pub fn get_current_profile(&self) -> Ref<Profile> {
		self.get_profile(&self.current_profile)
			.nice_unwrap("No current profile found!")
			.borrow()
	}

	pub fn try_sdk_path() -> Result<PathBuf, String> {
		let sdk_var = std::env::var("GEODE_SDK").map_err(|_| {
			"Unable to find Geode SDK (GEODE_SDK isn't set). Please install \
				it using `geode sdk install` or use `geode sdk set-path` to set \
				it to an existing clone. If you just installed the SDK using \
				`geode sdk install`, please restart your terminal / computer to \
				apply changes."
		})?;

		let path = PathBuf::from(sdk_var);
		if !path.is_dir() {
			return Err(format!(
				"Internal Error: GEODE_SDK doesn't point to a directory ({}). This \
				might be caused by having run `geode sdk set-path` - try restarting \
				your terminal / computer, or reinstall using `geode sdk install --reinstall`",
				path.display()
			));
		}
		if !path.join("VERSION").exists() {
			return Err(
				"Internal Error: GEODE_SDK/VERSION not found. Please reinstall \
				the Geode SDK using `geode sdk install --reinstall`"
					.into(),
			);
		}

		Ok(path)
	}

	pub fn sdk_path() -> PathBuf {
		Self::try_sdk_path().nice_unwrap("Unable to get SDK path")
	}

	/// Path to cross-compilation tools
	pub fn cross_tools_path() -> PathBuf {
		geode_root().join("cross-tools")
	}

	pub fn assert_is_setup(self) -> Config {
		if self.profiles.is_empty() {
			fatal!("No Geode profiles found! Setup one by using `geode config setup`");
		}
		self
	}

	fn default_fallback() -> Config {
		Config {
			current_profile: None,
			profiles: Vec::new(),
			default_developer: None,
			sdk_nightly: false,
			sdk_version: None,
			other: HashMap::<String, Value>::new(),
			index_token: None,
			index_url: "https://api.geode-sdk.org".to_string(),
		}
	}

	pub fn new() -> Config {
		if !geode_root().exists() {
			return Config::default_fallback();
		}

		let config_json = geode_root().join("config.json");

		let mut output: Config = if !config_json.exists() {
			// Create new config
			return Config::default_fallback();
		} else {
			// Parse config
			let config_json_str =
				&std::fs::read_to_string(&config_json).nice_unwrap("Unable to read config.json");
			match serde_json::from_str(config_json_str) {
				Ok(json) => json,
				Err(_) => Config::default_fallback(),
			}
		};

		// migrate old profiles from mac to mac-arm or mac-intel
		output.profiles.iter_mut().for_each(|profile| {
			let p = profile.get_mut();
			if p.platform == "mac" {
				p.platform = profile_platform_default();
			}
		});

		output.save();

		if !output.profiles.is_empty() && output.get_profile(&output.current_profile).is_none() {
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
