use crate::config::Config;
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
	win_gd: String,
	andr_gd: String,
	mac_gd: String,
	ios_gd: String,
	strip: bool,
	action: bool,
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
	)
	.nice_unwrap("Unable to clone repository");

	if fs::remove_dir_all(project_location.join(".git")).is_err() {
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
		let cpp_regex = Regex::new(r"(?m)^.*/\*[\s\S]*?\*/\r?\n?|^.*//.*\r?\n?").unwrap();

		let cmake_text = fs::read_to_string(&cmake_path)
			.nice_unwrap("Unable to read template file CMakeLists.txt");
		let cpp_text =
			fs::read_to_string(&cpp_path).nice_unwrap("Unable to read template file main.cpp");

		fs::write(cmake_path, &*cmake_regex.replace_all(&cmake_text, ""))
			.nice_unwrap("Unable to access template file CMakeLists.txt");
		fs::write(cpp_path, &*cpp_regex.replace_all(&cpp_text, ""))
			.nice_unwrap("Unable to access template file main.cpp");
	}

	// Add cross-platform action
	// Download the action from https://raw.githubusercontent.com/geode-sdk/build-geode-mod/main/examples/multi-platform.yml
	if action {
		let action_path = project_location.join(".github/workflows/multi-platform.yml");
		fs::create_dir_all(action_path.parent().unwrap())
			.nice_unwrap("Unable to create .github/workflows directory");
		let action = reqwest::blocking::get("https://raw.githubusercontent.com/geode-sdk/build-geode-mod/main/examples/multi-platform.yml").nice_unwrap("Unable to download action");
		fs::write(
			action_path,
			action.text().nice_unwrap("Unable to write action"),
		)
		.nice_unwrap("Unable to write action");
	}

	// Default mod.json
	let mut mod_json = json!({
		"geode":        get_version().to_string(),
		"gd":           {
			"win": win_gd,
			"android": andr_gd,
			"mac": mac_gd,
			"ios": ios_gd
		},
		"version":      version,
		"id":           id,
		"name":         name,
		"developer":    developer,
		"description":  description,
	});
	if win_gd == "." {
		mod_json["gd"].as_object_mut().unwrap().remove("win");
	}
	if andr_gd == "." {
		mod_json["gd"].as_object_mut().unwrap().remove("android");
	}
	if mac_gd == "." {
		mod_json["mac"].as_object_mut().unwrap().remove("mac");
	}

	// Format neatly
	let buf = Vec::new();
	let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
	let mut ser = serde_json::Serializer::with_formatter(buf, formatter);
	mod_json.serialize(&mut ser).unwrap();

	// Write formatted json
	fs::write(
		project_location.join("mod.json"),
		String::from_utf8(ser.into_inner()).unwrap(),
	)
	.nice_unwrap("Unable to write to project");

	done!("Succesfully initialized project! Happy modding :)");
}

fn possible_name(path: &Option<PathBuf>) -> Option<String> {
	let path = path.as_ref()?;
	Some(if path.is_absolute() {
		path.file_name()?.to_string_lossy().to_string()
	} else {
		std::env::current_dir()
			.ok()?
			.join(path)
			.file_name()?
			.to_string_lossy()
			.to_string()
	})
}

pub fn build_template(config: &mut Config, location: Option<PathBuf>) {
	info!("This utility will walk you through setting up a new mod.");
	info!("You can change any of the properties you set here later on by editing the generated mod.json file.");

	let final_name = ask_value("Name", possible_name(&location).as_deref(), true);

	let location = location.unwrap_or_else(|| std::env::current_dir().unwrap().join(&final_name));
	let location = location.absolutize().unwrap();

	let final_version = ask_value("Version", Some("v1.0.0"), true);

	info!("This is what Geometry Dash version your mod targets.");
	info!("See https://docs.geode-sdk.org/mods/configuring for more details.");
	info!("If you don't want to specify a version for a specific platform, just place a dot.");
	let mut win_gd;
	loop {
		win_gd = ask_value("GD Windows Version", Some("2.206"), true);
		if win_gd.starts_with("2.") || win_gd == "*" || win_gd == "." {
			break;
		}

		info!("GD version isn't valid, please choose a valid version (2.xxx or *)");
	}
	let mut andr_gd;
	loop {
		andr_gd = ask_value("GD Android Version", Some("2.206"), true);
		if andr_gd.starts_with("2.") || andr_gd == "*" || andr_gd == "." {
			break;
		}

		info!("GD version isn't valid, please choose a valid version (2.xxx or *)");
	}
	let mut mac_gd;
	loop {
		mac_gd = ask_value("GD Mac Version", Some("2.206"), true);
		if mac_gd.starts_with("2.") || mac_gd == "*" || mac_gd == "." {
			break;
		}
		info!("GD version isn't valid, please choose a valid version (2.xxx or *)");
	}
	let mut ios_gd;
	loop {
		ios_gd = ask_value("GD iOS Version", Some("2.206"), true);
		if ios_gd.starts_with("2.") || mac_gd == "*" || mac_gd == "." {
			break;
		}
		info!("GD version isn't valid, please choose a valid version (2.xxx or *)");
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

	let action = ask_confirm("Do you want to add the cross-platform Github action?", true);

	let strip = ask_confirm(
		"Do you want to remove comments from the default template?",
		false,
	);

	info!("Creating project {}", mod_id);

	create_template(
		final_location,
		final_name,
		final_version,
		mod_id,
		final_developer,
		final_description,
		win_gd,
		andr_gd,
		mac_gd,
		ios_gd,
		strip,
		action,
	);
}
