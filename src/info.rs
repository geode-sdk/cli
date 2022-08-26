/**
 * geode info 
 */
use std::path::{PathBuf};
use crate::config::Config;
use crate::{fail, fatal, done};
use colored::Colorize;
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum Info {
	/// Set value
	Set {
		/// Field to get
		field: String,

		/// New value
		value: String
	},
	/// Get value
	Get {
		/// Field to get
		field: String,

		/// Output raw value
		#[clap(long)]
		raw: bool
	},
	/// List possible values
	List
}

const CONFIGURABLES: [&str; 3] = [
	"default-developer",
	"sdk-path",
	"sdk-nightly"
];

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
			} else if field == "sdk-path" {
				config.sdk_path = Some(PathBuf::from(value));
			} else if field == "sdk-nightly" {
				config.sdk_nightly = get_bool(&value)
					.unwrap_or_else(|| fatal!("'{}' cannot be parsed as a bool", value));
			} else {
				fail!("Unknown field {}", field);
				return;
			}

			done!("{}", done_str);
		},

		Info::Get { field, raw } => {
			let out = if field == "default-developer" {
				config.default_developer.as_deref().unwrap_or("")
			} else if field == "sdk-path" {
				config.sdk_path.as_ref().and_then(|x| Some(x.to_str().unwrap())).unwrap_or("")
			} else if field == "sdk-nightly" {
				if config.sdk_nightly {
					"true"
				} else {
					"false"
				}
			} else {
				if raw {
					std::process::exit(1);
				} else {
					fail!("Unknown field {}", field);
					return;
				}
			};

			if raw {
				print!("{}", out);
			} else {
				println!("{} = {}", field.bright_cyan(), out.bright_green());
			}
		},

		Info::List => {
			for i in CONFIGURABLES {
				println!("{}", i);
			}
		}
	}
}