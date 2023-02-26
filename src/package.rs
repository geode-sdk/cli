

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write, Seek};
use std::path::{Path, PathBuf};

use clap::Subcommand;
use edit_distance::edit_distance;
use semver::{Version, VersionReq};
use zip::write::FileOptions;
use zip::ZipWriter;

use crate::config::Config;
use crate::file::read_dir_recursive;
use crate::index::{update_index, index_mods_dir, install_mod};
use crate::util::bmfont;
use crate::util::cache::CacheBundle;
use crate::util::mod_file::{ModFileInfo, parse_mod_info, try_parse_mod_info, Dependency};
use crate::util::spritesheet;
use crate::cache;
use crate::{done, fail, info, warn, fatal};

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

		/// Add binary file
		#[clap(short, long, num_args(1..))]
		binary: Vec<PathBuf>,

		/// Location of output file
		#[clap(short, long)]
		output: PathBuf,

		/// Whether to install the generated package after creation
		#[clap(short, long)]
		install: bool,
	},

	/// Merge multiple packages
	Merge {
		/// Packages to merge
		packages: Vec<PathBuf>
	},

	/// Check the dependencies & other information from a package; 
	/// output is returned as JSON
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
		fs::create_dir_all(&mod_path).expect("Could not setup mod installation");
	}
	fs::copy(pkg_path, mod_path.join(pkg_path.file_name().unwrap()))
		.expect("Could not install mod");

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

			// Windows is weird and needs this change
			if cfg!(windows) {
				relative_path = relative_path.replace('/', "\\");
			}

			zip_file.start_file(relative_path, zip_options).unwrap();
			zip_file.write_all(&fs::read(item.path()).unwrap()).unwrap();
		}
	}

	zip_file.finish().expect("Unable to zip");

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

fn get_working_dir(id: &String) -> PathBuf {
	let working_dir = dirs::cache_dir().unwrap().join(format!("geode_pkg_{}", id));
	fs::remove_dir_all(&working_dir).unwrap_or(());
	fs::create_dir(&working_dir).unwrap_or(());
	working_dir
}

fn create_resources(
	#[allow(unused)]
	config: &mut Config,
	mod_info: &ModFileInfo,
	#[allow(unused_mut)]
	mut cache_bundle: &mut Option<CacheBundle>,
	cache: &mut cache::ResourceCache,
	working_dir: &Path,
	output_dir: &PathBuf,
	sprite_output_dir: &PathBuf,
	shut_up: bool,
) {
	// Make sure output directory exists
	fs::create_dir_all(output_dir).expect("Could not create resource directory");
	fs::create_dir_all(sprite_output_dir).expect("Could not create sprite resource directory");

	// Create spritesheets
	for sheet in mod_info.resources.spritesheets.values() {
		let sheet_file = spritesheet::get_spritesheet_bundles(
			sheet,
			sprite_output_dir,
			cache_bundle,
			mod_info,
			shut_up,
		);
		cache.add_sheet(sheet, sheet_file.cache_name(working_dir));
	}

	// Create fonts
	for font in mod_info.resources.fonts.values() {
		let font_file = bmfont::get_font_bundles(font, sprite_output_dir, cache_bundle, mod_info, shut_up);
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
			sprite.save(sprite_output_dir.join(base.to_string() + "-uhd.png"))?;

			spritesheet::downscale(&mut sprite, 2);
			sprite.save(sprite_output_dir.join(base.to_string() + "-hd.png"))?;

			spritesheet::downscale(&mut sprite, 2);
			sprite.save(sprite_output_dir.join(base.to_string() + ".png"))
		})()
		.expect(&format!(
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
			.expect(&format!("Unable to copy file at '{}'", file.display()));
	}

	if !&mod_info.resources.libraries.is_empty() {
		info!("Copying libraries");
	}
	// Move other resources
	for file in &mod_info.resources.libraries {
		std::fs::copy(file, working_dir.join(file.file_name().unwrap()))
			.expect(&format!("Unable to copy file at '{}'", file.display()));
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
	mut output: PathBuf,
	do_install: bool,
) {
	// If it's a directory, add file path to it
	if output.is_dir() {
		output.push(root_path.file_name().unwrap());
		output.set_extension("geode");
		warn!(
			"Specified output is a directory. Creating package at {}",
			output.display()
		);
	}

	// Ensure at least one binary
	if binaries.is_empty() {
		fail!("No binaries added");
		info!("Help: Add a binary with `--binary <bin_path>`");
		return;
	}

	// Test if possible to create file
	if !output.exists() || output.is_dir() {
		fs::write(&output, "").expect("Could not create package");
		fs::remove_file(&output).unwrap();
	}

	// Parse mod.json
	let mod_file_info = parse_mod_info(root_path);

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
		&working_dir.join("resources"),
		&working_dir.join("resources").join(&mod_file_info.id),
		false,
	);

	// Custom hardcoded resources
	for file in &[
		"logo.png",
		"about.md",
		"changelog.md",
		"support.md"
	] {
		let path = root_path.join(file);
		if path.exists() {
			std::fs::copy(path, working_dir.join(file))
				.expect(&format!("Could not copy {file}"));
		}
	}

	// Copy headers
	if let Some(ref api) = mod_file_info.api {
		for header in &api.include {
			let out = working_dir.join(header.strip_prefix(&root_path).unwrap_or(header));
			out.parent().map(fs::create_dir_all);
			fs::copy(root_path.join(&header), &out)
				.expect(&format!("Unable to copy header {} to {}", header.to_string_lossy(), out.display()));
		}
	}

	// Copy binaries
	for binary in &binaries {
		let mut binary_name = binary.file_name().unwrap().to_str().unwrap().to_string();
		if let Some(ext) = [".ios.dylib", ".dylib", ".dll", ".lib", ".so"].iter().find(|x| binary_name.contains(**x)) {
			binary_name = mod_file_info.id.to_string() + ext;
		}

		std::fs::copy(binary, working_dir.join(binary_name))
			.expect(&format!("Unable to copy binary at '{}'", binary.display()));
	}

	new_cache.save(&working_dir);

	zip_folder(&working_dir, &output);

	if do_install {
		install(config, &output);
	}
}

pub fn mod_json_from_archive<R: Seek + Read>(input: &mut zip::ZipArchive<R>) -> serde_json::Value {
	let mut text = String::new();

	input.by_name("mod.json")
		 .expect("Unable to find mod.json in package")
		 .read_to_string(&mut text)
		 .expect("Unable to read mod.json");

	serde_json::from_str::<serde_json::Value>(&text).expect("Unable to parse mod.json")
}

fn merge_packages(inputs: Vec<PathBuf>) {
	let mut archives: Vec<_> = inputs.iter().map(|x| {
		zip::ZipArchive::new(fs::File::options().read(true).write(true).open(x).unwrap()).expect("Unable to unzip")
	}).collect();

	// Sanity check
	let mut mod_ids: Vec<_> = archives.iter_mut().map(|x|
		mod_json_from_archive(x)
			.get("id")
			.expect("[mod.json]: Missing key 'id'")
			.as_str()
			.expect("[mod.json].id: Expected string")
			.to_string()
	).collect();

	let mod_id = mod_ids.remove(0);

	// They have to be the same mod
	mod_ids.iter().for_each(|x| {
		if *x != mod_id {
			fatal!("Cannot merge packages with different mod id: {} and {}", x, mod_id);
		}
	});

	let mut out_archive = ZipWriter::new_append(archives.remove(0).into_inner()).expect("Unable to create zip writer");

	for archive in &mut archives {
		let potential_names = [".dylib", ".so", ".dll", ".lib"];
		
		// Rust borrow checker lol xd
		let files: Vec<_> = archive.file_names().map(|x| x.to_string()).collect();

		for file in files {
			if potential_names.iter().filter(|x| file.ends_with(*x)).next().is_some() {
				println!("{}", file);

				out_archive.raw_copy_file(
					archive.by_name(&file).expect("Unable to fetch file")
				).expect("Unable to transfer binary");
			}
		}
	}

	out_archive.finish().expect("Unable to write to zip");
	done!("Successfully merged binaries into {}", inputs[0].to_str().unwrap());
}

#[derive(PartialEq)]
enum Found {
	/// No matching dependency found
	None,
	/// No matching dependency found, but one with a similar ID was found
	Maybe(String),
	/// Dependency found, but it was not an API
	NotAnApi,
	/// Dependency found, but it was the wrong version
	Wrong(Version),
	/// Dependency found
	Some(PathBuf, ModFileInfo),
}

impl Found {
	fn promote_value(&self) -> usize {
		match self {
			Found::None         => 0,
			Found::Maybe(_)     => 1,
			Found::NotAnApi     => 2,
			Found::Wrong(_)     => 3,
			Found::Some(_, _)   => 4,
		}
	}

	/// Set the value of Found if the value is more important than the 
	/// existing value
	pub fn promote(&mut self, value: Found) {
		if self.promote_value() < value.promote_value() {
			*self = value;
		}
	}

	pub fn promote_eq(&mut self, value: Found) {
		if self.promote_value() <= value.promote_value() {
			*self = value;
		}
	}
}

fn find_dependency(
	dep: &Dependency,
	dir: &PathBuf,
	search_recursive: bool
) -> Result<Found, std::io::Error> {
	// for checking if the id was possibly misspelled, it must be at most 3 
	// steps away from the searched one
	let mut closest_score = 4usize;
	let mut found = Found::None;
	for dir in if search_recursive {
		read_dir_recursive(&dir)?
	} else {
		dir.read_dir()?.map(|d| d.unwrap().path()).collect()
	} {
		let Ok(info) = try_parse_mod_info(&dir) else {
			continue;
		};
		// check if the id matches
		if dep.id == info.id {
			if info.api.is_some() {
				if dep.version.matches(&info.version) {
					found.promote(Found::Some(dir, info));
					break;
				}
				else {
					found.promote(Found::Wrong(info.version));
				}
			}
			else {
				found.promote(Found::NotAnApi);
			}
		}
		// otherwise check if maybe the id was misspelled
		else {
			let dist = edit_distance(&dep.id, &info.id);
			if dist < closest_score {
				found.promote_eq(Found::Maybe(info.id.clone()));
				closest_score = dist;
			}
		}
	}
	Ok(found)
}

fn setup(config: &Config, input: PathBuf, output: PathBuf, externals: Vec<String>) {
	let mod_info = parse_mod_info(&input);

	// if let Some(ref api) = mod_info.api {
	// 	// copy headers elsewhere because they are still used by the build tool 
	// 	// when package new
	// 	let api_dir = output.join(format!("{}.geode_build", mod_info.id));
	// 	if api_dir.exists() {
	// 		fs::remove_dir_all(&api_dir).expect("Unable to clear directory for mod headers");
	// 	}
	// 	fs::create_dir_all(&api_dir).expect("Unable to create directory for mod headers");

	// 	for header in &api.include {
	// 		let out = api_dir.join(header);
	// 		out.parent().map(fs::create_dir_all);
	// 		fs::copy(input.join(&header), out)
	// 			.expect(&format!("Unable to copy header {}", header.to_string_lossy()));
	// 	}
	// }

	// If no dependencies, skippy wippy
	if mod_info.dependencies.is_empty() {
		return;
	}

	// Parse externals
	let externals = externals
		.into_iter()
		.map(|ext|
			// If the external is provided as name:version get those, otherwise 
			// assume it's just the name
			if ext.contains(":") {
				let mut split = ext.split(":");
				let name = split.next().unwrap().to_string();
				let ver = split.next().unwrap();
				(name, Some(Version::parse(ver.strip_prefix("v").unwrap_or(ver))
					.expect("Invalid version in external {name}")
				))
			}
			else {
				(ext, None)
			}
		)
		.collect::<HashMap<_, _>>();
	
	let mut errors = false;

	// update mods index if all of the mods aren't external
	if !mod_info.dependencies.iter().all(|d| externals.contains_key(&d.id)) {
		info!("Updating Geode mods index");
		update_index(config);
	}

	let dep_dir = output.join("geode-deps");
	fs::create_dir_all(&dep_dir).expect("Unable to create dependency directory");

	// check all dependencies
	for dep in mod_info.dependencies {
		// is this an external dependency?
		if let Some(ext) = externals.get(&dep.id) {
			// did we get a version?
			if let Some(version) = ext {
				// is it valid?
				if dep.version.matches(version) {
					info!("Dependency '{}' found as external", dep.id);
				}
				// external dependency version must match regardless of whether 
				// it's optional or not as most external dependencies are other 
				// projects being built at the same time and if those have a 
				// version mismatch you've screwed something up and should fix 
				// that
				else {
					fail!(
						"External dependency '{}' version '{version}' does not \
						match required version '{}' (note: optionality is \
						ignored when verifying external dependencies)",
						dep.id, dep.version
					);
					errors = true;
				}
			}
			// otherwise warn that a version prolly should be provided, but let 
			// it slide this time
			else {
				warn!(
					"Dependency '{}' marked as external with no version specified",
					dep.id
				);
			}
			continue;
		}

		// otherwise try to find it on installed mods and then on index

		// check index
		let found_in_index = find_dependency(
			&dep, &index_mods_dir(config), false
		).expect("Unable to read index");

		// check installed mods
		let found_in_installed = find_dependency(
			&dep, &config.get_current_profile().mods_dir(), true
		).expect("Unable to read installed mods");

		// if not found in either        hjfod  code
		if !matches!(found_in_index,     Found::Some(_, _)) &&
		   !matches!(found_in_installed, Found::Some(_, _))
		{
			if dep.required {
				fail!(
					"Dependency '{0}' not found in installed mods nor index! \
					If this is a mod that hasn't been published yet, install it \
					locally first, or if it's a closed-source mod that won't be \
					on the index, mark it as external in your CMake using \
					setup_geode_mod(... EXTERNALS {0}:{1})",
					dep.id, dep.version
				);
				errors = true;
			}
			else {
				info!(
					"Dependency '{}' not found in installed mods nor index",
					dep.id
				)
			}
			// bad version
			match (&found_in_index, &found_in_installed) {
				(in_index @ Found::Wrong(ver), _) | (in_index @ _, Found::Wrong(ver)) => {
					info!(
						"Version '{ver}' of the mod was found in {}, but it was \
						rejected because version '{}' is required by the dependency",
						if matches!(in_index, Found::Wrong(_)) {
							"index"
						} else {
							"installed mods"
						},
						dep.version
					);
				},
				_ => {},
			}
			// misspelled message
			match (&found_in_index, &found_in_installed) {
				(in_index @ Found::Maybe(m), _) | (in_index @ _, Found::Maybe(m)) => {
					info!(
						"Another mod with a similar ID was found in {}: {m} \
						- maybe you misspelled?",
						if matches!(in_index, Found::Maybe(_)) {
							"index"
						} else {
							"installed mods"
						}
					);
				},
				_ => {},
			}
			// not-an-api message
			match (&found_in_index, &found_in_installed) {
				(in_index @ Found::NotAnApi, _) | (in_index @ _, Found::NotAnApi) => {
					info!(
						"A mod with the ID '{}' was found in {}, but it was not marked \
						as an API - this may be a mistake; if you are the developer \
						of the dependency, add the \"api\" key to its mod.json",
						dep.id,
						if matches!(in_index, Found::NotAnApi) {
							"index"
						} else {
							"installed mods"
						}
					);
				},
				_ => {},
			}
			// skip rest
			continue;
		}

		let path_to_dep_geode;
		let _geode_info;
		match (found_in_installed, found_in_index) {
			(Found::Some(inst_path, inst_info), Found::Some(_, _)) => {
				info!("Dependency '{}' found", dep.id);
				path_to_dep_geode = inst_path;
				_geode_info = inst_info;
			}

			(Found::Some(inst_path, inst_info), _) => {
				warn!(
					"Dependency '{}' found in installed mods, but not on the \
					mods index - make sure that the mod is published on the \
					index when you publish yours, as otherwise users won't be \
					able to install your mod through the index!",
					dep.id
				);
				info!(
					"If '{0}' is a closed-source mod that won't be released on \
					the index, mark it as external in your CMake with \
					setup_geode_mod(... EXTERNALS {0}:{1})",
					dep.id, dep.version
				);
				path_to_dep_geode = inst_path;
				_geode_info = inst_info;
			}

			(Found::Wrong(version), Found::Some(_, indx_info)) => {
				if version > indx_info.version {
					warn!(
						"Dependency '{0}' found in installed mods, but as \
						version '{1}' whereas required is '{2}'. Index has valid \
						version '{3}', but not using it as it appears you have \
						a newer version installed. Either manually downgrade \
						the installed '{0}' to '{3}', or update your mod.json's \
						dependency requirements",
						dep.id, version, dep.version, indx_info.version
					);
					continue;
				}
				info!(
					"Dependency '{}' found on the index, installing \
					(update '{}' => '{}')",
					dep.id, version, indx_info.version
				);
				path_to_dep_geode = install_mod(
					config, &indx_info.id,
					&VersionReq::parse(&format!("=={}", indx_info.version.to_string())).unwrap()
				);
				_geode_info = indx_info;
			}

			(_, Found::Some(_, indx_info)) => {
				info!(
					"Dependency '{}' found on the index, installing (version '{}')",
					dep.id, indx_info.version
				);
				path_to_dep_geode = install_mod(
					config, &indx_info.id,
					&VersionReq::parse(&format!("={}", indx_info.version.to_string())).unwrap()
				);
				_geode_info = indx_info;
			}

			_ => unreachable!()
		}

		// check already installed dependencies
		// let found_in_deps = find_dependency(
		// 	&dep, &dep_dir, false
		// ).expect("Unable to read dependencies");

		// !this check may be added back at some point, but for now there's not 
		// too much performance benefit from doing this, and doing it might 
		// cause issues if the dependency has changes
		// check if dependency already installed
		// if let Found::Some(_, info) = found_in_deps {
		// 	if info.version == geode_info.version {
		// 		continue;
		// 	}
		// }

		// unzip the whole .geode package because there's only like a few 
		// extra files there aside from the lib, headers, and resources
		zip::ZipArchive::new(fs::File::open(path_to_dep_geode).unwrap())
			.expect("Unable to unzip")
			.extract(dep_dir.join(&dep.id))
			.expect("Unable to extract geode package");
		
		// add a note saying if the dependencey is required or not (for cmake to 
		// know if to link or not)
		fs::write(
			dep_dir.join(dep.id).join("geode-dep-options.json"),
			format!(r#"{{ "required": {} }}"#, if dep.required { "true" } else { "false" })
		).expect("Unable to save dep options");
	}

	if errors {
		fatal!("Some dependencies were unresolved");
	}
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
		},

		Package::Setup { input, output, externals } => setup(config, input, output, externals),

		Package::Resources {
			root_path,
			output,
			shut_up,
		} => create_package_resources_only(config, &root_path, &output, shut_up),
	}
}
