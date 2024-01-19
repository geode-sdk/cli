
use crate::config::Config;
use crate::gd::{get_gd_versions, get_latest_gd_version};
use crate::sdk::get_version;
use crate::util::logging::{ask_confirm, ask_value};
use crate::{done, info, warn, NiceUnwrap};
use git2::Repository;
use path_absolutize::Absolutize;
use regex::Regex;

use serde::Serialize;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

fn create_template(
	project_location: PathBuf,
	name: String,
	version: String,
	id: String,
	developer: String,
	description: String,
	gd: String,
	strip: bool
) {
	if project_location.exists() {
		warn!("The provided location already exists.");
		if !ask_confirm("Are you sure you want to proceed?", false) {
			info!("Aborting");
			return;
		}
	} else {
		fs::create_dir_all(&project_location).nice_unwrap("Unable to create project directory");
	}

	// Clone repository
	Repository::clone(
		"https://github.com/geode-sdk/example-mod",
		&project_location,
	).nice_unwrap("Unable to clone repository");

	if let Err(_) = fs::remove_dir_all(project_location.join(".git")) {
		warn!("Unable to remove .git directory");
	}

	// Replace "Template" with project name (no spaces)
	let filtered_name: String = name.chars().filter(|c| !c.is_whitespace()).collect();

	for file in &["README.md", "CMakeLists.txt"] {
		let file = project_location.join(file);

		let contents = fs::read_to_string(&file)
			.unwrap()
			.replace("Template", &filtered_name);
		fs::write(file, contents).unwrap();
	}

	// Strip comments from template
	if strip {
		let cmake_path = project_location.join("CMakeLists.txt");
		let cpp_path = project_location.join("src/main.cpp");

		let cmake_regex = Regex::new(r"\n#.*").unwrap();
		let cpp_regex = Regex::new(r".*/\*\*\r?\n(?:\s*\* .*\r?\n)*\s*\*/\r?\n?").unwrap();

		let cmake_text = fs::read_to_string(&cmake_path).nice_unwrap("Unable to read template file CMakeLists.txt");
		let cpp_text = fs::read_to_string(&cpp_path).nice_unwrap("Unable to read template file main.cpp");

		fs::write(cmake_path, &*cmake_regex.replace_all(&cmake_text, "")).nice_unwrap("Unable to access template file CMakeLists.txt");
		fs::write(cpp_path, &*cpp_regex.replace_all(&cpp_text, "")).nice_unwrap("Unable to access template file main.cpp");
	}

	// Default mod.json
	let mod_json = json!({
		"geode":        get_version().to_string(),
		"version":      version,
		"id":           id,
		"name":         name,
		"developer":    developer,
		"description":  description,
		"gd": gd
	});

	// Format neatly
	let buf = Vec::new();
	let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
	let mut ser = serde_json::Serializer::with_formatter(buf, formatter);
	mod_json.serialize(&mut ser).unwrap();

	// Write formatted json
	fs::write(
		&project_location.join("mod.json"),
		String::from_utf8(ser.into_inner()).unwrap(),
	).nice_unwrap("Unable to write to project");

	// FIXME: should this be here? at least have an option,
	// right now you can't even tell its running cmake
	// made macOS only as windows requires -A win32
	#[cfg(target_os = "macos")] {
		// Generate build folder and compiler_commands.json
		if let Ok(path) = which::which("cmake") {
			std::process::Command::new(path)
				.current_dir(&project_location)
				.arg("-B")
				.arg("build")
				.arg("-DCMAKE_EXPORT_COMPILE_COMMANDS=1")
				.output()
				.nice_unwrap("Unable to initialize project with CMake");
		} else {
			warn!("CMake not found. CMake is required to build Geode projects.");
		}
	}

	done!("Succesfully initialized project! Happy modding :)");
}

fn possible_name(path: &Option<PathBuf>) -> Option<String> {
	let dir_name;
	let Some(path) = path else { return None; };
	if path.is_absolute() {
		dir_name = path.file_name()?.to_string_lossy().to_string();
	}
	else {
		dir_name = std::env::current_dir().ok()?.join(path).file_name()?.to_string_lossy().to_string();
	}
	Some(dir_name)
}

pub fn build_template(config: &mut Config, location: Option<PathBuf>) {
	info!("This utility will walk you through setting up a new mod.");
	info!("You can change any of the properties you set here later on by editing the generated mod.json file.");

	let final_name = ask_value("Name", possible_name(&location).as_deref(), true);

	let location = location.unwrap_or_else(|| std::env::current_dir().unwrap().join(&final_name));
	let location = location.absolutize().unwrap();

	let final_version = ask_value("Version", Some("v1.0.0"), true);
	let mut gd = String::from("");
	loop {
		gd = ask_value("Geometry Dash Version", Some(&get_latest_gd_version()), true);

		let accepted_versions = get_gd_versions();
		let accepted_versions_str = accepted_versions.join(", ");
		let found = accepted_versions.into_iter().find(|x| { x == &gd.as_str() });
		if found.is_some() {
			break;
		}
		info!("Geometry Dash version isn't valid, please choose a valid version ({})", accepted_versions_str);
	}

	let final_developer = ask_value("Developer", config.default_developer.as_deref(), true);

	if config.default_developer.is_none() {
		info!(
			"Using '{}' as the default developer for all future projects. \
			If this is undesirable, you can set a default developer using \
			`geode config set default-developer <name>`",
			&final_developer
		);
		config.default_developer = Some(final_developer.clone());
	}

	let final_description = ask_value("Description", None, false);
	let final_location = PathBuf::from(ask_value(
		"Location",
		Some(&location.to_string_lossy()),
		true,
	));

	let mod_id = format!(
		"{}.{}",
		final_developer.to_lowercase().replace(' ', "_"),
		final_name.to_lowercase().replace(' ', "_")
	);

	let strip = ask_confirm(
		"Do you want to remove comments from the default template?", false
	);

	info!("Creating project {}", mod_id);

	create_template(
		final_location,
		final_name,
		final_version,
		mod_id,
		final_developer,
		final_description,
		gd,
		strip
	);
}
