use crate::input::ask_value;
use crate::config::Config;
use crate::sdk::get_version;
use crate::{done, info, warn};
use git2::Repository;
use path_absolutize::Absolutize;
use regex::Regex;

use serde::Serialize;
use serde_json::json;
use std::fs;
use std::io::{stdin, stdout, Write};
use std::path::PathBuf;

fn create_template(
	project_location: PathBuf,
	name: String,
	version: String,
	id: String,
	developer: String,
	description: String,
	strip: bool
) {
	if project_location.exists() {
		warn!("The provided location already exists.");
		print!("         Are you sure you want to proceed? (y/N) ");

		stdout().flush().unwrap();

		let mut ans = String::new();
		stdin().read_line(&mut ans).unwrap();
		ans = ans.trim().to_string();
		if !(ans == "y" || ans == "Y") {
			info!("Aborting");
			return;
		}
	} else {
		fs::create_dir_all(&project_location).expect("Unable to create project directory");
	}

	// Clone repository
	Repository::clone(
		"https://github.com/geode-sdk/example-mod",
		&project_location,
	).expect("Unable to clone repository");

	fs::remove_dir_all(project_location.join(".git")).unwrap();

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
		let cpp_regex = Regex::new(r".*/\*\*\n(?:\s*\* .*\n)*\s*\*/\n?").unwrap();

		let cmake_text = fs::read_to_string(&cmake_path).expect("Unable to read template file CMakeLists.txt");
		let cpp_text = fs::read_to_string(&cpp_path).expect("Unable to read template file main.cpp");

		fs::write(cmake_path, &*cmake_regex.replace_all(&cmake_text, "")).expect("Unable to access template file CMakeLists.txt");
		fs::write(cpp_path, &*cpp_regex.replace_all(&cpp_text, "")).expect("Unable to access template file main.cpp");
	}

	// Default mod.json
	let mod_json = json!({
		"geode":        get_version().to_string(),
		"version":      version,
		"id":           id,
		"name":         name,
		"developer":    developer,
		"description":  description
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
	).expect("Unable to write to project");

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
				.expect("Unable to initialize project with CMake");
		} else {
			warn!("CMake not found. CMake is required to build Geode projects.");
		}
	}

	done!("Succesfully initialized project! Happy modding :)");
}

pub fn build_template(config: &mut Config, name: Option<String>, location: Option<PathBuf>, strip: bool) {
	let final_name = ask_value("Name", name.as_deref(), true);

	let location = location.unwrap_or_else(|| std::env::current_dir().unwrap().join(&final_name));
	let location = location.absolutize().unwrap();

	let final_version = ask_value("Version", Some("v1.0.0"), true);

	let final_developer = ask_value("Developer", config.default_developer.as_deref(), true);

	if config.default_developer.is_none() {
		info!(
			"Using '{}' as the default developer for all future projects.",
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

	info!("Creating project {}", mod_id);

	create_template(
		final_location,
		final_name,
		final_version,
		mod_id,
		final_developer,
		final_description,
		strip
	);
}
