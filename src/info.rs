use crate::config::Config;
use crate::util::config::Profile;
use crate::{done, fail, info, NiceUnwrap};
use clap::Subcommand;
use colored::Colorize;
use std::cell::RefCell;
use std::io::BufRead;

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

pub fn subcommand(config: &mut Config, cmd: Info) {
	match cmd {
		Info::Set { field, value } => {
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
		}

		Info::Get { field, raw } => {
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
			if config.profiles.is_empty() {
				info!("Please enter the platform you are using (win, mac, android32, android64), empty for host platform:");

				let platform = loop {
					let mut buf = String::new();
					match std::io::stdin().lock().read_line(&mut buf) {
						Ok(_) => {}
						Err(e) => {
							fail!("Unable to read input: {}", e);
							continue;
						}
					};

					let platform = buf.trim().to_lowercase();
					if platform.is_empty() {
						break std::env::consts::OS;
					} else if platform == "win" {
						break "win";
					} else if platform == "mac" {
						break "mac";
					} else if platform == "android32" {
						break "android32";
					} else if platform == "android64" {
						break "android64";
					} else {
						fail!("Invalid platform");
					}
				};

				info!("Please enter the path to the Geometry Dash app:");

				let path = loop {
					let mut buf = String::new();
					match std::io::stdin().lock().read_line(&mut buf) {
						Ok(_) => {}
						Err(e) => {
							fail!("Unable to read input: {}", e);
							continue;
						}
					};

					// Verify path is valid
					let path = PathBuf::from(buf.trim());

					#[allow(clippy::collapsible_else_if)]
					if platform == "win" {
						if path.is_dir() {
							fail!(
								"The path must point to the Geometry Dash exe,\
								not the folder it's in"
							);
							continue;
						} else if path.extension().and_then(|p| p.to_str()).unwrap_or("") != "exe" {
							fail!("The path must point to a .exe file");
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
					} else {
						break path;
					}

					break path;
					// todo: maybe do some checksum verification
					// to make sure GD 2.113 is in the folder
				};

				info!("Please enter a name for the profile:");
				let name = loop {
					let mut buf = String::new();
					match std::io::stdin().lock().read_line(&mut buf) {
						Ok(_) => break buf,
						Err(e) => fail!("Unable to read input: {}", e),
					};
				};

				config.profiles.push(RefCell::new(Profile::new(
					name.trim().into(),
					path,
					platform.to_string(),
				)));
				config.current_profile = Some(name.trim().into());
				done!("Profile added");
			}

			config.sdk_nightly =
				Config::try_sdk_path().map_or(false, |path| path.join("bin/nightly").exists());

			done!("Config setup finished");
		}
	}
}
