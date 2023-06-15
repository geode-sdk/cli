use crate::config::{Config, Profile as CfgProfile};
use crate::{done, fail, info, warn, fatal};
use clap::Subcommand;
use colored::Colorize;
use std::cell::RefCell;
use std::process::Command;

/**
 * geode profile list: List profiles of geode
 * geode profile switch: Switch main geode profile
 * geode profile add: Add geode profile to the index
 * geode profile remove: Remove geode profile from the index
 * geode profile rename: Rename geode profile
 */
use std::path::Path;
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum Profile {
	/// List profiles
	List,

	/// Switch main profile
	Switch {
		/// New main profile
		profile: String,
	},

	/// Add profile
	Add {
		/// New profile location
		location: PathBuf,

		/// New profile name
		#[clap(short, long)]
		name: String,
	},

	/// Remove profile
	Remove {
		/// Profile to remove
		name: String,
	},

	/// Rename profile
	Rename {
		/// Profile to rename
		old: String,

		/// New name
		new: String,
	},

	/// Open Geometry Dash based on profile
	Run {
		/// Profile to run, uses default if none is provided
		profile: Option<String>,

		/// Run Geometry Dash in the background instead of the foreground
		#[clap(long)]
		background: bool
	}
}

fn is_valid_geode_dir(_dir: &Path) -> bool {
	//TODO: this
	true
}

pub fn run_profile(config: &Config, profile: Option<String>, background: bool) {
	let prof_instance = profile.clone()
		.map(|p| config.get_profile(&Some(p)).map(|p| p.borrow()))
		.unwrap_or(Some(config.get_current_profile()));

	if prof_instance.is_none() {
		fatal!("Profile '{}' does not exist", profile.unwrap_or(String::new()));
	}

	let path = &prof_instance.unwrap().gd_path;

	let mut cmd = if cfg!(windows) {
		let mut out = Command::new(path.join("GeometryDash.exe"));
		out.current_dir(path);
		out
	} else {
		let mut out = Command::new(path.join("MacOS").join("Geometry Dash"));

		if path.join("MacOS").join("steam_appid.txt").exists() {
			warn!("Steam version detected. Output may not be available.");

			out.env("DYLD_INSERT_LIBRARIES", path
				.parent().unwrap()
				.parent().unwrap()
				.parent().unwrap()
				.parent().unwrap()
				.parent().unwrap()
				.join("Steam.AppBundle")
				.join("Steam")
				.join("Contents")
				.join("MacOS")
				.join("steamloader.dylib")
			);
		}

		out
	};

	info!("Starting Geometry Dash");

	if let Ok(mut child) = cmd.spawn() {
		if !background {
			child.wait().unwrap();
		}
	} else {
		fail!("Unable to start Geometry Dash");
	}
}

pub fn subcommand(config: &mut Config, cmd: Profile) {
	match cmd {
		Profile::List => {
			for profile in &config.profiles {
				let name = &profile.borrow().name;
				let path = &profile.borrow().gd_path;

				let indicator = if config.current_profile.as_ref() == Some(name) {
					"* "
				} else {
					""
				};

				println!(
					"{}{} [ path = {} ]",
					indicator.bright_cyan(),
					name.bright_cyan(),
					path.to_string_lossy().bright_green()
				);
			}
		}

		Profile::Switch { profile } => {
			if config.get_profile(&Some(profile.to_owned())).is_none() {
				fail!("Profile '{}' does not exist", profile);
			} else if config.current_profile == Some(profile.to_owned()) {
				fail!("'{}' is already the current profile", profile);
			} else {
				done!("'{}' is now the current profile", &profile);
				config.current_profile = Some(profile);
			}
		}

		Profile::Add { name, location } => {
			if config.get_profile(&Some(name.to_owned())).is_some() {
				fail!("A profile named '{}' already exists", name);
			} else if !is_valid_geode_dir(&location) {
				fail!("The specified path does not point to a valid Geode installation");
			} else {
				done!("A new profile named '{}' has been created", &name);
				config
					.profiles
					.push(RefCell::new(CfgProfile::new(name, location)));
			}
		}

		Profile::Remove { name } => {
			if config.get_profile(&Some(name.to_owned())).is_none() {
				fail!("Profile '{}' does not exist", name);
			} else {
				config.profiles.retain(|x| x.borrow().name != name);
				done!("'{}' has been removed", name);
			}
		}

		Profile::Rename { old, new } => {
			config.rename_profile(&old, new);
		}

		Profile::Run { profile, background } => run_profile(config, profile, background)
	}
}
