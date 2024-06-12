use std::fs::{self, read_dir};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use clap::Subcommand;
use zip::write::FileOptions;
use zip::ZipWriter;

use crate::config::Config;
use crate::util::bmfont;
use crate::util::cache::CacheBundle;
use crate::util::mod_file::{parse_mod_info, ModFileInfo};
use crate::util::spritesheet;
use crate::{cache, project};
use crate::{done, fatal, info, warn, NiceUnwrap};

#[derive(Subcommand, Debug)]
#[clap(rename_all = "kebab-case")]
pub enum Package {
	/// Install a .geode package to the current profile
	Install {
		/// Location of the .geode package to install
		path: PathBuf,
	},

	/// Create a .geode package
	New {
		/// Location of mod's folder
		root_path: PathBuf,

		/// Add an external binary file. By default, all known binary files
		/// (.dll, .lib, .dylib, .so) named after the mod ID in the root path
		/// are included
		#[clap(short, long, num_args(1..))]
		binary: Vec<PathBuf>,

		/// Location of output file. If not provided, the resulting file is named
		/// {mod.id}.geode and placed at the root path
		#[clap(short, long)]
		output: Option<PathBuf>,

		/// Whether to install the generated package after creation
		#[clap(short, long)]
		install: bool,
	},

	/// Merge multiple packages
	Merge {
		/// Packages to merge
		packages: Vec<PathBuf>,
	},

	/// Check the dependencies of a project.
	/// Currently just an alias for `geode project check`, will be removed in
	/// CLI v3.0.0!
	#[deprecated(since = "2.0.0", note = "Will be removed in v3.0.0")]
	Setup {
		/// Location of package
		input: PathBuf,

		/// Package build directory
		output: PathBuf,

		/// Any external dependencies as a list in the form of `mod.id:version`.
		/// An external dependency is one that the CLI will not verify exists in
		/// any way; it will just assume you have it installed through some
		/// other means (usually through building it as part of the same project)
		#[clap(long, num_args(0..))]
		externals: Vec<String>,
	},

	/// Process the resources specified by a package
	Resources {
		/// Location of mod's folder
		root_path: PathBuf,

		/// Folder to place the created resources in
		output: PathBuf,

		/// Less verbose output
		#[clap(long)]
		shut_up: bool,
	},
}

pub fn install(config: &mut Config, pkg_path: &Path) {
	let mod_path = config.get_current_profile().mods_dir();

	if !mod_path.exists() {
		fs::create_dir_all(&mod_path).nice_unwrap("Could not setup mod installation");
	}
	fs::copy(pkg_path, mod_path.join(pkg_path.file_name().unwrap()))
		.nice_unwrap("Could not install mod");

	done!(
		"Installed {}",
		pkg_path.file_name().unwrap().to_str().unwrap()
	);
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
			let mut relative_path = item
				.path()
				.strip_prefix(path)
				.unwrap()
				.to_str()
				.unwrap()
				.to_string();

			relative_path = relative_path.replace('\\', "/");

			zip_file.start_file(relative_path, zip_options).unwrap();
			zip_file.write_all(&fs::read(item.path()).unwrap()).unwrap();
		}
	}

	zip_file.finish().nice_unwrap("Unable to zip");

	done!(
		"Successfully packaged {}",
		output
			.file_name()
			.unwrap()
			.to_str()
			.unwrap()
			.bright_yellow()
	);
}

pub fn get_working_dir(id: &String) -> PathBuf {
	let working_dir = dirs::cache_dir().unwrap().join(format!("geode_pkg_{}", id));
	fs::remove_dir_all(&working_dir).unwrap_or(());
	fs::create_dir(&working_dir).unwrap_or(());
	working_dir
}

fn create_resources(
	#[allow(unused)] config: &mut Config,
	mod_info: &ModFileInfo,
	#[allow(unused_mut)] mut cache_bundle: &mut Option<CacheBundle>,
	cache: &mut cache::ResourceCache,
	working_dir: &Path,
	output_dir: &PathBuf,
	shut_up: bool,
) {
	// Make sure output directory exists
	fs::create_dir_all(output_dir).nice_unwrap("Could not create resource directory");

	// Create spritesheets
	for sheet in mod_info.resources.spritesheets.values() {
		let sheet_file = spritesheet::get_spritesheet_bundles(
			sheet,
			output_dir,
			cache_bundle,
			mod_info,
			shut_up,
		);
		cache.add_sheet(sheet, sheet_file.cache_name(working_dir));
	}

	// Create fonts
	for font in mod_info.resources.fonts.values() {
		let font_file = bmfont::get_font_bundles(font, output_dir, cache_bundle, mod_info, shut_up);
		cache.add_font(font, font_file.cache_name(working_dir));
	}

	if !&mod_info.resources.sprites.is_empty() {
		info!("Copying sprites");
	}
	// Resize sprites
	for sprite_path in &mod_info.resources.sprites {
		let mut sprite = spritesheet::read_to_image(sprite_path);

		// Sprite base name
		let base = sprite_path.file_stem().and_then(|x| x.to_str()).unwrap();

		// Collect all errors
		(|| {
			spritesheet::downscale(&mut sprite, 1);
			sprite.save(output_dir.join(base.to_string() + "-uhd.png"))?;

			spritesheet::downscale(&mut sprite, 2);
			sprite.save(output_dir.join(base.to_string() + "-hd.png"))?;

			spritesheet::downscale(&mut sprite, 2);
			sprite.save(output_dir.join(base.to_string() + ".png"))
		})()
		.nice_unwrap(&format!(
			"Unable to copy sprite at {}",
			sprite_path.display()
		));
	}

	if !&mod_info.resources.files.is_empty() {
		info!("Copying files");
	}
	// Move other resources
	for file in &mod_info.resources.files {
		std::fs::copy(file, output_dir.join(file.file_name().unwrap()))
			.nice_unwrap(&format!("Unable to copy file at '{}'", file.display()));
	}

	if !&mod_info.resources.libraries.is_empty() {
		info!("Copying libraries");
	}
	// Move other resources
	for file in &mod_info.resources.libraries {
		std::fs::copy(file, working_dir.join(file.file_name().unwrap()))
			.nice_unwrap(&format!("Unable to copy file at '{}'", file.display()));
	}
}

fn create_package_resources_only(
	config: &mut Config,
	root_path: &Path,
	output_dir: &PathBuf,
	shut_up: bool,
) {
	// Parse mod.json
	let mod_info = parse_mod_info(root_path);

	// Setup cache
	let mut cache_bundle = cache::get_cache_bundle_from_dir(output_dir);
	let mut new_cache = cache::ResourceCache::new();

	create_resources(
		config,
		&mod_info,
		&mut cache_bundle,
		&mut new_cache,
		output_dir,
		output_dir,
		shut_up,
	);

	new_cache.save(output_dir);

	done!("Resources created at {}", output_dir.to_str().unwrap());
}

fn create_package(
	config: &mut Config,
	root_path: &Path,
	binaries: Vec<PathBuf>,
	raw_output: Option<PathBuf>,
	do_install: bool,
) {
	// Parse mod.json
	let mod_file_info = parse_mod_info(root_path);

	let mut output = raw_output.unwrap_or(root_path.join(format!("{}.geode", mod_file_info.id)));

	// If it's a directory, add file path to it
	if output.is_dir() {
		output.push(&mod_file_info.id);
		output.set_extension("geode");
		warn!(
			"Specified output is a directory. Creating package at {}",
			output.display()
		);
	}

	// Test if possible to create file
	if !output.exists() || output.is_dir() {
		fs::write(&output, "").nice_unwrap("Could not create package");
		fs::remove_file(&output).unwrap();
	}

	// Setup working directory
	let working_dir = get_working_dir(&mod_file_info.id);

	// Move mod.json
	fs::copy(root_path.join("mod.json"), working_dir.join("mod.json")).unwrap();

	// Setup cache
	let mut cache_bundle = cache::get_cache_bundle(&output);
	let mut new_cache = cache::ResourceCache::new();

	// Create resources
	create_resources(
		config,
		&mod_file_info,
		&mut cache_bundle,
		&mut new_cache,
		&working_dir,
		&working_dir.join("resources").join(&mod_file_info.id),
		false,
	);

	// Custom hardcoded resources
	for file in &["logo.png", "about.md", "changelog.md", "support.md"] {
		let path = root_path.join(file);
		if path.exists() {
			std::fs::copy(path, working_dir.join(file))
				.nice_unwrap(&format!("Could not copy {file}"));
		}
	}

	// Copy headers
	if let Some(ref api) = mod_file_info.api {
		for header in &api.include {
			let out = working_dir.join(header);
			out.parent().map(fs::create_dir_all);
			fs::copy(root_path.join(header), &out).nice_unwrap(&format!(
				"Unable to copy header {} to {}",
				header.display(),
				out.display()
			));
		}
	}

	let mut binaries_added = false;
	for file in read_dir(root_path).nice_unwrap("Unable to read root directory") {
		let Ok(file) = file else {
			continue;
		};
		let path = file.path();
		let Some(name) = path.file_stem() else {
			continue;
		};
		let Some(ext) = path.extension() else {
			continue;
		};
		if name.to_string_lossy() == mod_file_info.id
			&& matches!(
				ext.to_string_lossy().as_ref(),
				"ios.dylib" | "dylib" | "dll" | "lib" | "so" | "android32.so" | "android64.so"
			) {
			let binary = name.to_string_lossy().to_string() + "." + ext.to_string_lossy().as_ref();
			std::fs::copy(path, working_dir.join(&binary))
				.nice_unwrap(&format!("Unable to copy binary '{}'", binary));
			binaries_added = true;
		}
	}

	// Copy other binaries
	for binary in &binaries {
		let mut binary_name = binary.file_name().unwrap().to_str().unwrap().to_string();
		if let Some(ext) = [
			".ios.dylib",
			".dylib",
			".dll",
			".lib",
			".android32.so",
			".android64.so",
			".so",
		]
		.iter()
		.find(|x| binary_name.ends_with(**x))
		{
			binary_name = mod_file_info.id.to_string() + ext;
		}

		std::fs::copy(binary, working_dir.join(binary_name))
			.nice_unwrap(&format!("Unable to copy binary at '{}'", binary.display()));
		binaries_added = true;
	}

	// Ensure at least one binary
	if !binaries_added {
		warn!("No binaries added to the resulting package");
		info!("Help: Add a binary with `--binary <bin_path>`");
	}

	new_cache.save(&working_dir);

	zip_folder(&working_dir, &output);

	if do_install {
		install(config, &output);
	}
}

pub fn mod_json_from_archive<R: Seek + Read>(input: &mut zip::ZipArchive<R>) -> serde_json::Value {
	let mut text = String::new();

	input
		.by_name("mod.json")
		.nice_unwrap("Unable to find mod.json in package")
		.read_to_string(&mut text)
		.nice_unwrap("Unable to read mod.json");

	serde_json::from_str::<serde_json::Value>(&text).nice_unwrap("Unable to parse mod.json")
}

fn merge_packages(inputs: Vec<PathBuf>) {
	let mut archives: Vec<_> = inputs
		.iter()
		.map(|x| {
			zip::ZipArchive::new(fs::File::options().read(true).write(true).open(x).unwrap())
				.nice_unwrap("Unable to unzip")
		})
		.collect();

	// Sanity check
	let mut mod_ids: Vec<_> = archives
		.iter_mut()
		.map(|x| {
			mod_json_from_archive(x)
				.get("id")
				.nice_unwrap("[mod.json]: Missing key 'id'")
				.as_str()
				.nice_unwrap("[mod.json].id: Expected string")
				.to_string()
		})
		.collect();

	let mod_id = mod_ids.remove(0);

	// They have to be the same mod
	mod_ids.iter().for_each(|x| {
		if *x != mod_id {
			fatal!(
				"Cannot merge packages with different mod id: {} and {}",
				x,
				mod_id
			);
		}
	});

	let mut out_archive = ZipWriter::new_append(archives.remove(0).into_inner())
		.nice_unwrap("Unable to create zip writer");

	for archive in &mut archives {
		let potential_names = [".dylib", ".so", ".dll", ".lib"];

		// Rust borrow checker lol xd
		let files: Vec<_> = archive.file_names().map(|x| x.to_string()).collect();

		for file in files {
			if potential_names.iter().any(|x| file.ends_with(*x)) {
				println!("{}", file);

				out_archive
					.raw_copy_file(archive.by_name(&file).nice_unwrap("Unable to fetch file"))
					.nice_unwrap("Unable to transfer binary");
			}
		}
	}

	out_archive.finish().nice_unwrap("Unable to write to zip");
	done!(
		"Successfully merged binaries into {}",
		inputs[0].to_str().unwrap()
	);
}

pub fn subcommand(config: &mut Config, cmd: Package) {
	match cmd {
		Package::Install { path } => install(config, &path),

		Package::New {
			root_path,
			binary: binaries,
			output,
			install,
		} => create_package(config, &root_path, binaries, output, install),

		Package::Merge { packages } => {
			if packages.len() < 2 {
				fatal!("Merging requires at least two packages");
			}
			merge_packages(packages)
		}

		#[allow(deprecated)]
		Package::Setup {
			input,
			output,
			externals,
		} => project::check_dependencies(config, input, output, externals),

		Package::Resources {
			root_path,
			output,
			shut_up,
		} => create_package_resources_only(config, &root_path, &output, shut_up),
	}
}
