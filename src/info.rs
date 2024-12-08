use crate::config::{self, Config};
use crate::logging::ask_value;
use crate::util::config::Profile;
use crate::{done, fail, warn, NiceUnwrap};
use clap::Subcommand;
use colored::Colorize;
use std::cell::RefCell;

/**
 * geode info
 */
use std::path::PathBuf;

#[derive(Subcommand, Debug)]
pub enum Info {
	/// Set value
	Set {
		/// Field to get
		field: String,

		/// New value
		value: String,
	},

	/// Get value
	Get {
		/// Field to get
		field: String,

		/// Output raw value
		#[clap(long)]
		raw: bool,
	},

	/// List possible values
	List,

	/// Setup config (if you have manually installed Geode)
	Setup {},
}

const CONFIGURABLES: [&str; 3] = ["default-developer", "sdk-path", "sdk-nightly"];

fn get_bool(value: &str) -> Option<bool> {
	let lower = value.to_ascii_lowercase();

	if lower == "true" || lower == "yes" || lower == "y" {
		Some(true)
	} else if lower == "false" || lower == "no" || lower == "n" {
		Some(false)
	} else {
		None
	}
}

pub fn subcommand(cmd: Info) {
	match cmd {
		Info::Set { field, value } => {
			let mut config = Config::new().assert_is_setup();

			let done_str = format!("Set {} to {}", field, &value);

			if field == "default-developer" {
				config.default_developer = Some(value);
			} else if field == "sdk-nightly" {
				config.sdk_nightly =
					get_bool(&value).nice_unwrap(format!("'{}' cannot be parsed as a bool", value));
			} else if field == "sdk-path" {
				fail!("Set the SDK Path using `geode sdk set-path <PATH>`");
				return;
			} else {
				fail!("Unknown field {}", field);
				return;
			}

			done!("{}", done_str);
			config.save();
		}

		Info::Get { field, raw } => {
			let config = Config::new().assert_is_setup();

			let sdk_path;

			let out = if field == "default-developer" {
				config.default_developer.as_deref().unwrap_or("")
			} else if field == "sdk-path" {
				sdk_path = Config::sdk_path();
				sdk_path.to_str().unwrap_or("")
			} else if field == "sdk-nightly" {
				if config.sdk_nightly {
					"true"
				} else {
					"false"
				}
			} else if raw {
				std::process::exit(1);
			} else {
				fail!("Unknown field {}", field);
				return;
			};

			if raw {
				print!("{}", out);
			} else {
				println!("{} = {}", field.bright_cyan(), out.bright_green());
			}
		}

		Info::List => {
			for i in CONFIGURABLES {
				println!("{}", i);
			}
		}

		Info::Setup {} => {
			let mut config = Config::new();

			if config.profiles.is_empty() {
				let default = config::profile_platform_default();
				let platform = ask_value(
					"What platform you are using? (win, mac, android32, android64)",
					Some(default.as_str()),
					true,
				);
				let mut platform = platform.trim().to_lowercase();
				if platform == "mac" {
					platform = default;
				}
				if !["win", "mac-intel", "mac-arm", "android32", "android64"]
					.contains(&platform.as_str())
				{
					fail!("Invalid platform");
				}

				let path = loop {
					let buf = ask_value("Path to the Geometry Dash app/executable", None, true);
					let buf = buf
						.trim_matches(|c| c == '"' || c == ' ')
						.replace("\\ ", " ");

					// Verify path is valid

					let path = PathBuf::from(buf.trim());
					if !path.exists() {
						fail!("The path must exist");
						continue;
					}

					#[allow(clippy::collapsible_else_if)]
					if platform == "win" {
						if path.is_dir() {
							fail!(
								"The path must point to the Geometry Dash exe, not the folder it's in"
							);
							continue;
						} else if path.extension().and_then(|p| p.to_str()).unwrap_or("") != "exe" {
							fail!("The path must point to the Geometry Dash .exe file");
							continue;
						}
					} else if platform == "mac" {
						if !path.is_dir()
							|| path.extension().and_then(|p| p.to_str()).unwrap_or("") != "app"
						{
							fail!("The path must point to the Geometry Dash app");
							continue;
						} else if !path.join("Contents/MacOS/Geometry Dash").exists() {
							fail!("The path must point to the Geometry Dash app, not a Steam shortcut");
							continue;
						}
					}

					break path;
					// todo: maybe do some checksum verification
					// to make sure GD 2.113 is in the folder
				};

				let name = ask_value("Profile name", None, true);

				config.profiles.push(RefCell::new(Profile::new(
					name.trim().into(),
					path,
					platform,
				)));
				config.current_profile = Some(name.trim().into());
				done!("Profile added");
			} else {
				warn!("Profiles already exist, skipping profile setup");
			}

			config.sdk_nightly =
				Config::try_sdk_path().map_or(false, |path| path.join("bin/nightly").exists());

			done!("Config setup finished");
			config.save();
		}
	}
}
