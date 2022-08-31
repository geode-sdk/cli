/**
 * geode profile list: List profiles of geode
 * geode profile switch: Switch main geode profile
 * geode profile add: Add geode profile to the index
 * geode profile remove: Remove geode profile from the index
 * geode profile rename: Rename geode profile
 */

use std::path::Path;
use std::cell::RefCell;
use clap::Subcommand;
use colored::Colorize;
use std::path::PathBuf;
use crate::config::{Config, Profile as CfgProfile};
use crate::{done, fail};

#[derive(Subcommand, Debug)]
pub enum Profile {
	/// List profiles
	List,

	/// Switch main profile
	Switch {
		/// New main profile
		profile: String
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
		name: String
	},

	/// Rename profile
	Rename {
		/// Profile to rename
		old: String,

		/// New name
		new: String
	}
}

fn is_valid_geode_dir(_dir: &Path) -> bool {
	//TODO: this
	return true;
}

pub fn subcommand(config: &mut Config, cmd: Profile) {
	match cmd {
		Profile::List => {
			for profile in &config.profiles {
				let name = &profile.borrow().name;
				let path = &profile.borrow().gd_path;

				println!("{} [ path = {} ]", name.bright_cyan(), path.to_string_lossy().bright_green());
			}
		},

		Profile::Switch { profile } => {
			if config.get_profile(&profile).is_none() {
				fail!("Profile '{}' does not exist", profile);
			} else if config.current_profile == profile {
				fail!("'{}' is already the current profile", profile);
			} else {
				done!("'{}' is now the current profile", &profile);
				config.current_profile = profile;
			}
		},

		Profile::Add { name, location } => {
			if config.get_profile(&name).is_some() {
				fail!("A profile named '{}' already exists", name);
			} else if !is_valid_geode_dir(&location) {
				fail!("The specified path does not point to a valid Geode installation");
			} else {
				done!("A new profile named '{}' has been created", &name);
				config.profiles.push(RefCell::new(CfgProfile::new(name, location)));
			}

		},

		Profile::Remove { name } => {
			if config.get_profile(&name).is_none() {
				fail!("Profile '{}' does not exist", name);
			} else {
				config.profiles.retain(|x| x.borrow().name != name);
				done!("'{}' has been removed", name);
			}
		},

		Profile::Rename { old, new } => {
			config.rename_profile(&old, new);
		}
	}
}