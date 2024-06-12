use crate::util::mod_file::DependencyImportance;
use crate::{done, fail, fatal, index, info, warn, NiceUnwrap};
use crate::{
	file::read_dir_recursive,
	package::get_working_dir,
	template,
	util::{
		config::Config,
		mod_file::{parse_mod_info, try_parse_mod_info, Dependency, ModFileInfo},
	},
};
use clap::Subcommand;
use edit_distance::edit_distance;
use semver::Version;
use std::env;
use std::{
	collections::HashMap,
	fs,
	path::{Path, PathBuf},
};

#[derive(Subcommand, Debug)]
#[clap(rename_all = "kebab-case")]
pub enum Project {
	/// Initialize a new Geode project (same as `geode new`)
	New {
		/// The target directory to create the project in
		path: Option<PathBuf>,
	},

	/// Clear this project's cached resource files
	ClearCache,

	/// Check & install the dependencies for this project
	Check {
		/// Where to install the dependencies; usually the project's build
		/// directory. A directory called geode-deps will be created inside
		/// the specified installation directory. If not specified, "build"
		/// is assumed
		install_dir: Option<PathBuf>,

		/// Any external dependencies as a list in the form of `mod.id:version`.
		/// An external dependency is one that the CLI will not verify exists in
		/// any way; it will just assume you have it installed through some
		/// other means (usually through building it as part of the same project)
		#[clap(long, num_args(0..))]
		externals: Vec<String>,
	},
}

fn find_build_directory(root: &Path) -> Option<PathBuf> {
	// this works for 99% of users.
	// if you want to parse the CMakeLists.txt file to find the true build
	// directory 100% of the time, go ahead, but i'm not doing it
	if root.join("build").exists() {
		Some(root.join("build"))
	} else {
		None
	}
}

fn clear_cache(dir: &Path) {
	// Parse mod.json
	let mod_info = parse_mod_info(dir);

	// Remove cache directory
	let workdir = get_working_dir(&mod_info.id);
	fs::remove_dir_all(workdir).nice_unwrap("Unable to remove cache directory");

	// Remove cached .geode package
	let dir = find_build_directory(dir);
	if let Some(dir) = dir {
		for file in fs::read_dir(dir).nice_unwrap("Unable to read build directory") {
			let path = file.unwrap().path();
			let Some(ext) = path.extension() else {
				continue;
			};
			if ext == "geode" {
				fs::remove_file(path).nice_unwrap("Unable to delete cached .geode package");
			}
		}
	} else {
		warn!(
			"Unable to find cached .geode package, can't clear it. It might be \
            that this is not supported on the current platform, or that your \
            build directory has a different name"
		);
	}

	done!("Cache for {} cleared", mod_info.id);
}

#[derive(PartialEq)]
#[allow(clippy::large_enum_variant)]
enum Found {
	/// No matching dependency found
	None,
	/// No matching dependency found, but one with a similar ID was found
	Maybe(String),
	/// Dependency found, but it was the wrong version
	Wrong(Version),
	/// Dependency found
	Some(PathBuf, ModFileInfo),
}

impl Found {
	fn promote_value(&self) -> usize {
		match self {
			Found::None => 0,
			Found::Maybe(_) => 1,
			Found::Wrong(_) => 2,
			Found::Some(_, _) => 3,
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

fn find_index_dependency(dep: &Dependency, config: &Config) -> Result<Found, String> {
	info!("Fetching dependency from index");
	let found =
		index::get_mod_versions(&dep.id, 1, 10, config, true, Some(dep.version.to_string()))?;

	if found.data.is_empty() {
		return Ok(Found::None);
	}

	let first = found.data.first().unwrap();
	info!("Dependency found: {}, version {}", dep.id, first.version);
	info!("Downloading dependency");

	let client = reqwest::blocking::Client::new();

	let result = client
		.get(&first.download_link)
		.send()
		.map_err(|x| format!("Failed to download dependency: {}", x))?;

	if result.status() != 200 {
		return Err(format!(
			"Failed to download dependency. Bad status code: {}",
			result.status()
		));
	}

	let bytes = result
		.bytes()
		.map_err(|x| format!("Failed to parse dependency binary: {}", x))?;

	info!("Success");
	info!("Writing dependency to temp file");

	let mut path = env::temp_dir();
	path.push(format!("{}.geode", dep.id));

	if let Err(e) = std::fs::write(&path, bytes) {
		return Err(format!("Failed to write dependency to temp file: {}", e));
	}

	let mod_info =
		try_parse_mod_info(&path).map_err(|x| format!("Couldn't parse mod.json: {}", x))?;

	Ok(Found::Some(path, mod_info))
}

fn find_dependency(
	dep: &Dependency,
	dir: &Path,
	search_recursive: bool,
	mods_v2: bool,
) -> Result<Found, std::io::Error> {
	// for checking if the id was possibly misspelled, it must be at most 3
	// steps away from the searched one
	let mut closest_score = 4usize;
	let mut found = Found::None;
	let mut dir = dir.to_path_buf();

	// this doesnt work with the fuzzy search misspelling check or whatever
	// someone else can fix it i dont care kthx
	if mods_v2 {
		dir = dir.join(&dep.id);
		if !dir.exists() {
			return Ok(Found::None);
		}
	}
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
			if dep.version.matches(&info.version) {
				found.promote(Found::Some(dir, info));
				break;
			} else {
				found.promote(Found::Wrong(info.version));
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

pub fn check_dependencies(
	config: &Config,
	input: PathBuf,
	output: PathBuf,
	externals: Vec<String>,
) {
	let mod_info = parse_mod_info(&input);

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
			if ext.contains(':') {
				let mut split = ext.split(':');
				let name = split.next().unwrap().to_string();
				let ver = split.next().unwrap();
				(name, Some(Version::parse(ver.strip_prefix('v').unwrap_or(ver))
					.nice_unwrap("Invalid version in external {name}")
				))
			}
			else {
				(ext, None)
			}
		)
		.collect::<HashMap<_, _>>();

	let mut errors = false;

	let dep_dir = output.join("geode-deps");
	fs::create_dir_all(&dep_dir).nice_unwrap("Unable to create dependency directory");

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
						dep.id,
						dep.version
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
		let found_in_index = match find_index_dependency(&dep, config) {
			Ok(f) => f,
			Err(e) => {
				warn!("Failed to fetch dependency {} from index: {}", &dep.id, e);
				Found::None
			}
		};
		// check installed mods
		let found_in_installed =
			find_dependency(&dep, &config.get_current_profile().mods_dir(), true, false)
				.nice_unwrap("Unable to read installed mods");

		// if not found in either        hjfod  code
		if !matches!(found_in_index, Found::Some(_, _))
			&& !matches!(found_in_installed, Found::Some(_, _))
		{
			if dep.importance == DependencyImportance::Required
				|| dep.required.is_some() && dep.required.unwrap()
			{
				fail!(
					"Dependency '{0}' not found in installed mods nor index! \
					If this is a mod that hasn't been published yet, install it \
					locally first, or if it's a closed-source mod that won't be \
					on the index, mark it as external in your CMake using \
					setup_geode_mod(... EXTERNALS {0}:{1})",
					dep.id,
					dep.version
				);
				errors = true;
			} else {
				info!(
					"Dependency '{}' not found in installed mods nor index",
					dep.id
				)
			}
			// bad version
			match (&found_in_index, &found_in_installed) {
				(in_index @ Found::Wrong(ver), _) | (in_index, Found::Wrong(ver)) => {
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
				}
				_ => {}
			}
			// misspelled message
			match (&found_in_index, &found_in_installed) {
				(in_index @ Found::Maybe(m), _) | (in_index, Found::Maybe(m)) => {
					info!(
						"Another mod with a similar ID was found in {}: {m} \
						- maybe you misspelled?",
						if matches!(in_index, Found::Maybe(_)) {
							"index"
						} else {
							"installed mods"
						}
					);
				}
				_ => {}
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

			(Found::Wrong(version), Found::Some(path, indx_info)) => {
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
				let geode_path = config
					.get_current_profile()
					.mods_dir()
					.join(format!("{}.geode", indx_info.id));
				std::fs::copy(path, &geode_path).nice_unwrap("Failed to install .geode");
				path_to_dep_geode = geode_path;
				_geode_info = indx_info;
			}

			(_, Found::Some(path, indx_info)) => {
				info!(
					"Dependency '{}' found on the index, installing (version '{}')",
					dep.id, indx_info.version
				);
				let geode_path = config
					.get_current_profile()
					.mods_dir()
					.join(format!("{}.geode", indx_info.id));
				std::fs::copy(path, &geode_path).nice_unwrap("Failed to install .geode");
				path_to_dep_geode = geode_path;
				_geode_info = indx_info;
			}

			_ => unreachable!(),
		}

		// check already installed dependencies
		// let found_in_deps = find_dependency(
		// 	&dep, &dep_dir, false
		// ).nice_unwrap("Unable to read dependencies");

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
			.nice_unwrap("Unable to unzip")
			.extract(dep_dir.join(&dep.id))
			.nice_unwrap("Unable to extract geode package");

		// add a note saying if the dependencey is required or not (for cmake to
		// know if to link or not)
		fs::write(
			dep_dir.join(dep.id).join("geode-dep-options.json"),
			format!(
				r#"{{ "required": {} }}"#,
				if dep.importance == DependencyImportance::Required
					|| dep.required.is_some() && dep.required.unwrap()
				{
					"true"
				} else {
					"false"
				}
			),
		)
		.nice_unwrap("Unable to save dep options");
	}

	if errors {
		fatal!("Some dependencies were unresolved");
	} else {
		done!("All dependencies resolved");
	}
}

pub fn subcommand(config: &mut Config, cmd: Project) {
	match cmd {
		Project::New { path } => template::build_template(config, path),
		Project::ClearCache => clear_cache(&std::env::current_dir().unwrap()),
		Project::Check {
			install_dir,
			externals,
		} => check_dependencies(
			config,
			std::env::current_dir().unwrap(),
			install_dir.unwrap_or("build".into()),
			externals,
		),
	}
}
