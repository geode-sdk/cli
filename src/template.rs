use crate::config::Config;
use crate::sdk::get_version;
use crate::util::logging::{ask_confirm, ask_value};
use crate::{done, info, warn, NiceUnwrap};
use git2::build::RepoBuilder;
use path_absolutize::Absolutize;
use regex::Regex;

use serde::Serialize;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

struct CreateTemplate {
	pub template: String,
	pub project_location: PathBuf,
	pub name: String,
	pub version: String,
	pub id: String,
	pub developer: String,
	pub description: String,
	pub strip: bool,
	pub action: bool,
}

fn create_template(template: CreateTemplate) {
	if template.project_location.exists() {
		warn!("The provided location already exists.");
		if !ask_confirm("Are you sure you want to proceed?", false) {
			info!("Aborting");
			return;
		}
	} else {
		fs::create_dir_all(&template.project_location)
			.nice_unwrap("Unable to create project directory");
	}

	let (used_template, branch) = if template.template.contains('@') {
		template.template.split_once('@').unwrap()
	} else if template.template.contains('/') {
		(template.template.as_str(), "main")
	} else if template.template.is_empty() {
		("geode-sdk/example-mod", "main")
	} else {
		(
			"geode-sdk/example-mod",
			match template.template.to_ascii_lowercase().as_str() {
				"default" => "main",
				"minimal" => "minimal",
				"custom layer" => "custom-layer",
				_ => {
					warn!("Invalid template name, using default template");
					"main"
				}
			},
		)
	};

	// Clone repository
	RepoBuilder::new()
		.branch(branch)
		.clone(
			format!("https://github.com/{}", used_template).as_str(),
			&template.project_location,
		)
		.nice_unwrap("Unable to clone repository");

	if fs::remove_dir_all(template.project_location.join(".git")).is_err() {
		warn!("Unable to remove .git directory");
	}

	// Replace "Template" with project name (no spaces)
	let filtered_name: String = template
		.name
		.chars()
		.filter(|c| !c.is_whitespace())
		.collect();

	for file in &["README.md", "CMakeLists.txt"] {
		let file = template.project_location.join(file);

		let Ok(contents) = fs::read_to_string(&file) else {
			continue;
		};
		let contents = contents.replace("Template", &filtered_name);
		fs::write(file, contents).unwrap();
	}

	// Strip comments from template
	if template.strip {
		let cmake_path = template.project_location.join("CMakeLists.txt");
		let cpp_path = template.project_location.join("src/main.cpp");

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
	if template.action {
		let action_path = template
			.project_location
			.join(".github/workflows/multi-platform.yml");
		fs::create_dir_all(action_path.parent().unwrap())
			.nice_unwrap("Unable to create .github/workflows directory");
		let action = reqwest::blocking::get("https://raw.githubusercontent.com/geode-sdk/build-geode-mod/main/examples/multi-platform.yml").nice_unwrap("Unable to download action");
		fs::write(
			action_path,
			action.text().nice_unwrap("Unable to write action"),
		)
		.nice_unwrap("Unable to write action");
	}

	let mod_json_path = template.project_location.join("mod.json");

	let mod_json_content: String = {
		if mod_json_path.exists() {
			let mod_json =
				fs::read_to_string(&mod_json_path).nice_unwrap("Unable to read mod.json file");

			mod_json
				.replace("$GEODE_VERSION", &get_version().to_string())
				.replace("$MOD_VERSION", &template.version)
				.replace("$MOD_ID", &template.id)
				.replace("$MOD_NAME", &template.name)
				.replace("$MOD_DEVELOPER", &template.developer)
				.replace("$MOD_DESCRIPTION", &template.description)
		} else {
			// Default mod.json
			let mod_json = json!({
				"geode":        get_version().to_string(),
				"version":      template.version,
				"id":           template.id,
				"name":         template.name,
				"developer":    template.developer,
				"description":  template.description,
			});

			// Format neatly
			let buf = Vec::new();
			let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
			let mut ser = serde_json::Serializer::with_formatter(buf, formatter);
			mod_json.serialize(&mut ser).unwrap();

			// Write formatted json
			String::from_utf8(ser.into_inner()).unwrap()
		}
	};

	fs::write(mod_json_path, mod_json_content)
		.nice_unwrap("Unable to write mod.json, are permissions correct?");
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

pub fn build_template(location: Option<PathBuf>) {
	let mut config = Config::new().assert_is_setup();

	info!("This utility will walk you through setting up a new mod.");
	info!("You can change any of the properties you set here later on by editing the generated mod.json file.");

	info!("Choose a template for the mod to be created:");

	let template_options = [
		(
			"Default - Simple mod that adds a button to the main menu.",
			"",
		),
		(
			"Minimal - Minimal mod with only the bare minimum to compile.",
			"minimal",
		),
		("Other..", ""),
	];

	let template_index = dialoguer::Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
		.items(
			template_options
				.iter()
				.map(|(name, _)| name)
				.collect::<Vec<_>>()
				.as_slice(),
		)
		.default(0)
		.interact_opt()
		.nice_unwrap("Unable to get template")
		.unwrap_or(0);

	let template = if template_index == template_options.len() - 1 {
		println!();
		info!("Here you can use any github repository");
		info!("Use this syntax: 'user/repo' or 'user/repo@branch'");
		ask_value("Template", Some(""), false)
	} else {
		template_options[template_index].1.to_string()
	};

	let final_name = ask_value("Name", possible_name(&location).as_deref(), true);

	let location = location.unwrap_or_else(|| std::env::current_dir().unwrap().join(&final_name));
	let location = location.absolutize().unwrap();

	let final_version = ask_value("Version", Some("v1.0.0"), true);

	let final_developer = ask_value("Developer", config.default_developer.as_deref(), true);

	if config.default_developer.is_none() {
		info!(
			"Using '{}' as the default developer for all future projects. \
			If this is undesirable, you can set a default developer using \
			`geode config set default-developer <name>`",
			&final_developer
		);
		config.default_developer = Some(final_developer.clone());
		config.save();
	}

	let final_description = ask_value("Description", None, false);
	let final_location = PathBuf::from(ask_value(
		"Location",
		Some(&location.to_string_lossy()),
		true,
	));

	let mod_id = format!(
		"{}.{}",
		final_developer.to_lowercase().replace(' ', "_").replace("\"", ""),
		final_name.to_lowercase().replace(' ', "_").replace("\"", "")
	);

	let action = ask_confirm("Do you want to add the cross-platform Github action?", true);

	let strip = ask_confirm(
		"Do you want to remove comments from the default template?",
		false,
	);

	info!("Creating project {}", mod_id);
	create_template(CreateTemplate {
		template,
		project_location: final_location,
		name: final_name.replace("\"", "\\\""),
		version: final_version,
		id: mod_id,
		developer: final_developer.replace("\"", "\\\""),
		description: final_description.replace("\"", "\\\""),
		strip,
		action,
	});
}
