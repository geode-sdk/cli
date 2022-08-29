#![allow(unused_variables)]
#![allow(unused_mut)]

use crate::config::Config;
use crate::util::{spritesheet, font};
use crate::{mod_file, cache};

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::fs;
use clap::Subcommand;
use serde_json::Value;

use crate::{fatal, warn, done};

#[derive(Subcommand, Debug)]
#[clap(rename_all = "kebab-case")]
pub enum Package {
	/// Install a .geode package to the current profile
    Install {
        /// Location of the .geode package to install
        path: PathBuf
    },

    /// Create a .geode package
    New {
    	/// Location of the mod's folder
    	path: PathBuf,

    	/// Location where to put the output file
    	#[clap(long)]
    	out: PathBuf,

    	/// Whether to install the generated package after creation
    	#[clap(short, long)]
    	install: bool
    }
}

pub fn install(config: &mut Config, pkg_path: &Path) {
    let mod_path = config.get_profile(&config.current_profile)
    	.unwrap_or_else(|| fatal!("No current profile to install to!"))
    	.borrow().gd_path
    	.join("geode")
    	.join("mods");

    if !mod_path.exists() {
        fs::create_dir_all(&mod_path)
        	.unwrap_or_else(|e| fatal!("Could not create mod installation directory: {}", e));
    }

	// this could probably be simplified, but i'm not rusty enough to know how to
	if pkg_path.extension().unwrap_or(OsStr::new("")) != OsStr::new("geode") {
		warn!(
			"File {} does not appear to be a .geode package, installing anyway",
			pkg_path.to_str().unwrap()
		);
	}

	fs::copy(pkg_path, mod_path.join(pkg_path.file_name().unwrap()))
		.unwrap_or_else(|e| fatal!("Could not install mod: {}", e));

    done!("Installed {}", pkg_path.file_name().unwrap().to_str().unwrap());
}

fn create_package(config: &mut Config, path: &Path, out_path: &Path, do_install: bool) {
	// Test if possible to create file
	if !out_path.exists() || out_path.is_dir() {
		fs::write(out_path, "")
			.unwrap_or_else(|e| fatal!("Could not create package: {}", e));

		fs::remove_file(out_path).unwrap();
	}

	// Parse mod.json
	let mod_json: Value = serde_json::from_str(
		&fs::read_to_string(out_path.join("mod.json"))
			.unwrap_or_else(|e| fatal!("Could not read mod.json: {}", e))
	).unwrap_or_else(|e| fatal!("Could not parse mod.json: {}", e));

	let mod_file_info = mod_file::get_mod_file_info(&mod_json, &path);
	let mut cache_bundle = cache::get_cache_bundle(out_path);
	let working_dir = dirs::cache_dir().unwrap().join(format!("geode_pkg_{}", mod_file_info.id));

	// Reset working directory
	fs::remove_dir_all(&working_dir).unwrap_or(());
	fs::create_dir(&working_dir).unwrap_or(());

	// Create spritesheets
	for sheet in mod_file_info.resources.spritesheets.values() {
		let out = spritesheet::get_spritesheet(sheet, &working_dir, &mut cache_bundle);
		todo!();
	}

	// Create fonts
	for font in mod_file_info.resources.fonts.values() {
		let out = font::get_font(font, &working_dir, &mut cache_bundle);
		todo!();
	}
}

pub fn subcommand(config: &mut Config, cmd: Package) {
	match cmd {
		Package::Install { path } => install(config, &path),

		Package::New { path, out, install } => create_package(config, &path, &out, install)
	}
}
