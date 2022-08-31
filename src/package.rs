#![allow(unused_variables)]
#![allow(unused_mut)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::fs;

use clap::Subcommand;
use serde_json::{Value};
use zip::ZipWriter;
use zip::write::FileOptions;

use crate::config::Config;
use crate::util::spritesheet;
use crate::util::bmfont;
use crate::{mod_file, cache};
use crate::{fail, warn, info, done};
use crate::NiceUnwrap;

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
    	/// Location of mo'sd folder
    	root_path: PathBuf,

    	/// Add binary file
    	#[clap(short, long)]
    	binary: Vec<PathBuf>,

    	/// Location of output file
    	#[clap(short, long)]
    	output: PathBuf,

    	/// Whether to install the generated package after creation
    	#[clap(short, long)]
    	install: bool
    }
}

pub fn install(config: &mut Config, pkg_path: &Path) {
    let mod_path = config.get_profile(&config.current_profile)
    	.nice_unwrap("No current profile to install to!")
    	.borrow().gd_path
    	.join("geode")
    	.join("mods");

    if !mod_path.exists() {
        fs::create_dir_all(&mod_path).nice_unwrap("Could not setup mod installation");
    }
	fs::copy(pkg_path, mod_path.join(pkg_path.file_name().unwrap())).nice_unwrap("Could not install mod");

    done!("Installed {}", pkg_path.file_name().unwrap().to_str().unwrap());
}

fn zip_folder(path: &Path, output: &Path) {
	info!("Zipping");

	// Setup zip
	let mut zip_file = ZipWriter::new(fs::File::create(output).unwrap());
	let zip_options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

	// Iterate files in target path
	for item in walkdir::WalkDir::new(path) {
		let item = item.unwrap();

		// Only look at files
		if item.metadata().unwrap().is_file() {
			// Relativize
			let mut relative_path = item.path().strip_prefix(path).unwrap().to_str().unwrap().to_string();
			
			// Windows is weird and needs this change
			if cfg!(windows) {
			    relative_path = relative_path.replace('/', "\\");
			}

			zip_file.start_file(relative_path, zip_options).unwrap();
			zip_file.write_all(&fs::read(item.path()).unwrap()).unwrap();
		}
	}

	zip_file.finish().nice_unwrap("Unable to zip");

	done!("Successfully packaged {}", output.file_name().unwrap().to_str().unwrap().bright_yellow());
}

fn create_package(config: &mut Config, root_path: &Path, binaries: Vec<PathBuf>, mut output: PathBuf, do_install: bool) {
	// If it's a directory, add file path to it
	if output.is_dir() {
		output.push(root_path.file_name().unwrap());
		output.set_extension("geode");
		warn!("Specified output is a directory. Creating package at {}", output.display());
	}

	// Ensure at least one binary
	if binaries.is_empty() {
		fail!("No binaries added");
		info!("Help: Add a binary with `--binary <bin_path>`");
		return;
	}

	// Test if possible to create file
	if !output.exists() || output.is_dir() {
		fs::write(&output, "").nice_unwrap("Could not create package");
		fs::remove_file(&output).unwrap();
	}

	// Parse mod.json
	let mod_json: Value = serde_json::from_str(
		&fs::read_to_string(root_path.join("mod.json")).nice_unwrap("Could not read mod.json")
	).nice_unwrap("Could not parse mod.json");

	let mod_file_info = mod_file::get_mod_file_info(&mod_json, &root_path);

	// Setup working directory
	let working_dir = dirs::cache_dir().unwrap().join(format!("geode_pkg_{}", mod_file_info.id));
	fs::remove_dir_all(&working_dir).unwrap_or(());
	fs::create_dir(&working_dir).unwrap_or(());

	// Move mod.json
	fs::copy(root_path.join("mod.json"), working_dir.join("mod.json")).unwrap();

	// Resource directory
	let resource_dir = working_dir.join("resources");
	fs::create_dir(&resource_dir).unwrap();

	// Setup cache
	let mut cache_bundle = cache::get_cache_bundle(&output);
	let mut new_cache = cache::ResourceCache::new();

	// Create spritesheets
	for sheet in mod_file_info.resources.spritesheets.values() {
		let sheet_file = spritesheet::get_spritesheet_bundles(sheet, &resource_dir, &mut cache_bundle);
		new_cache.add_sheet(sheet, sheet_file.cache_name(&working_dir));
	}

	// Create fonts
	for font in mod_file_info.resources.fonts.values() {
		let font_file = bmfont::get_font(font, &resource_dir, &mut cache_bundle);
		new_cache.add_font(font, font_file);
	}

	// Move other resources
	for file in &mod_file_info.resources.files {
		std::fs::copy(file, resource_dir.join(file.file_name().unwrap()))
			.nice_unwrap(format!("Could not copy file at '{}'", file.display()));
	}

	for binary in &binaries {
		std::fs::copy(binary, working_dir.join(binary.file_name().unwrap()))
			.nice_unwrap(format!("Could not copy binary at '{}'", binary.display()));
	}

	new_cache.save(&working_dir);

	zip_folder(&working_dir, &output);

	if do_install {
		install(config, &output);
	}
}

pub fn subcommand(config: &mut Config, cmd: Package) {
	match cmd {
		Package::Install { path } => install(config, &path),

		Package::New { root_path, binary: binaries, output, install } => create_package(config, &root_path, binaries, output, install)
	}
}
