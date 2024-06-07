use crate::config::{Config, Profile as CfgProfile};
use crate::{done, fail, info, warn, NiceUnwrap};
use clap::{Subcommand, ValueEnum};
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

	/// Get the GD path for a profile
	Path {
		/// The profile to get a path for, or none for default
		profile: Option<String>,

		/// Whether to get the parent directory of the path
		/// (by default on Windows, the path leads to the .exe itself)
		#[clap(short, long)]
		dir: bool,
	},

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

		/// Platform of the target
		platform: Option<String>,
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
		#[clap(long, conflicts_with = "stay")]
		background: bool,

		/// Do not exit CLI after Geometry Dash exits if running in foreground
		#[clap(long, conflicts_with = "background")]
		stay: bool,

		/// Launch arguments for Geometry Dash
		#[clap(last = true, allow_hyphen_values = true)]
		launch_args: Vec<String>,
	},
}

#[derive(ValueEnum, PartialEq, Clone, Debug)]
pub enum RunBackground {
	Foreground,
	Background,
	ForegroundStay,
}

fn is_valid_geode_dir(_dir: &Path) -> bool {
	//TODO: this
	true
}

pub fn run_profile(
	config: &Config,
	profile: Option<String>,
	background: RunBackground,
	launch_args: Vec<String>,
) {
	let profile = &profile
		.clone()
		.map(|p| config.get_profile(&Some(p)).map(|p| p.borrow()))
		.unwrap_or(Some(config.get_current_profile()))
		.nice_unwrap(format!(
			"Profile '{}' does not exist",
			profile.unwrap_or_default()
		));
	let path = &profile.gd_path;

	let mut cmd = if profile.platform_str() == "win" {
		let mut out = Command::new(path);
		out.args(launch_args);
		out.current_dir(path.parent().unwrap());
		out
	} else {
		let mut out = Command::new(path.join("Contents/MacOS/Geometry Dash"));
		out.args(launch_args);

		if path.join("Contents/MacOS/steam_appid.txt").exists() {
			warn!("Steam version detected. Output may not be available.");

			out.env(
				"DYLD_INSERT_LIBRARIES",
				path.parent()
					.unwrap()
					.parent()
					.unwrap()
					.parent()
					.unwrap()
					.parent()
					.unwrap()
					.join("Steam.AppBundle")
					.join("Steam")
					.join("Contents")
					.join("MacOS")
					.join("steamloader.dylib"),
			);
		}

		out
	};

	info!("Starting Geometry Dash");

	let mut child = cmd.spawn().nice_unwrap("Unable to start Geometry Dash");
	if background != RunBackground::Background {
		child.wait().unwrap();
	}

	if background == RunBackground::ForegroundStay {
		info!("Press any key to exit");
		crossterm_input::input().read_char().unwrap_or('\0');
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

		Profile::Path { profile, dir } => {
			let profile = profile
				.clone()
				.map(|p| config.get_profile(&Some(p)).map(|p| p.borrow()))
				.unwrap_or(Some(config.get_current_profile()))
				.nice_unwrap(format!(
					"Profile '{}' does not exist",
					profile.unwrap_or_default()
				));
			println!(
				"{}",
				if dir {
					profile.gd_dir()
				} else {
					profile.gd_path.clone()
				}
				.display()
			);
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

		Profile::Add {
			name,
			location,
			platform,
		} => {
			if config.get_profile(&Some(name.to_owned())).is_some() {
				fail!("A profile named '{}' already exists", name);
			} else if !is_valid_geode_dir(&location) {
				fail!("The specified path does not point to a valid Geode installation");
			} else {
				done!("A new profile named '{}' has been created", &name);
				let profile = match platform {
					Some(platform) => match platform.as_str() {
						"win" | "windows" => "win",
						"mac" | "macos" => "mac",
						"android32" => "android32",
						"android64" => "android64",
						_ => "",
					},
					None => {
						if cfg!(target_os = "windows") {
							"win"
						} else if cfg!(target_os = "macos") {
							"mac"
						} else {
							""
						}
					}
				};
				if profile.is_empty() {
					fail!("Platform must be specified for this system");
				}
				config.profiles.push(RefCell::new(CfgProfile::new(
					name,
					location,
					profile.to_string(),
				)));
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

		Profile::Run {
			profile,
			background,
			stay,
			launch_args,
		} => run_profile(
			config,
			profile,
			match (background, stay) {
				(false, false) => RunBackground::Foreground,
				(false, true) => RunBackground::ForegroundStay,
				(true, false) => RunBackground::Background,
				(true, true) => panic!("Impossible argument combination (background and stay)"),
			},
			launch_args,
		),
	}
}
