use colored::*;
use crate::GEODE_VERSION;
use crate::print_error;
use git2::Repository;
use path_absolutize::Absolutize;
use rustyline::Editor;
use serde_json::{json, to_string_pretty};
use std::path::PathBuf;
use std::{fs, path::Path};

use fs_extra::dir as fs_dir;

pub fn create_template(project_name: String, location: Option<PathBuf>) {
	let is_location_default = location.is_none();
	let loc = match location {
	    Some(s) => s,
	    None => std::env::current_dir().unwrap()
	};

	let mut version = String::from("v1.0.0");
	let mut developer = String::from("");
	let mut description = String::from("");
	let mut buffer = if is_location_default {
		loc.absolutize().unwrap().join(&project_name).to_str().unwrap().to_string()
	} else {
		loc.absolutize().unwrap().to_str().unwrap().to_string()
	};

	let mut rl = Editor::<()>::new();

	let mut prompts = [
	    ("Developer", &mut developer, true),
	    ("Version", &mut version, true),
	    ("Description", &mut description, true),
	    ("Location", &mut buffer, true),
	];
	
	for (prompt, ref mut var, required) in prompts.iter_mut() {
	    let text = format!("{}: ", prompt);

		loop {
			let readline = rl.readline_with_initial(text.as_str(), (var.as_str(), ""));
			match readline {
				Ok(line) => {
					rl.add_history_entry(line.as_str());
					if line.is_empty() && *required {
						println!("{}", "Please enter a value".red());
					} else {
						**var = line;
						break;
					}
				},
				Err(err) => {
					print_error!("Error: {}", err);
				}
			}
		}
	}
	
	buffer = buffer.trim().to_string();
	version = version.trim().to_string();
	developer = developer.trim().to_string();
	description = description.trim().to_string();

	let project_location = Path::new(&buffer);

	let id = format!("com.{}.{}", developer.to_lowercase(), project_name.to_lowercase());

	let mut binary_name = project_name.to_lowercase();
	binary_name.retain(|c| !c.is_whitespace());
	
	println!(
	    "Creating mod with ID {} named {} by {} version {} in {}",
	    id.green(),
	    project_name.green(),
	    developer.green(),
	    version.green(),
	    project_location.to_str().unwrap().green()
	);

	if project_location.exists() {
	    print_error!("Unable to create project in existing directory");
	} else {
		if fs::create_dir_all(&project_location).is_err() {
			print_error!("Unable to create directory for project");
		}
	}

	match Repository::clone("https://github.com/geode-sdk/example-mod", &project_location) {
	    Ok(_) => (),
	    Err(e) => print_error!("Failed to clone template: {}", e),
	};

	fs::remove_dir_all(&project_location.join(".git")).unwrap();

	for thing in fs::read_dir(&project_location).unwrap() {
	    if !thing.as_ref().unwrap().metadata().unwrap().is_dir() {
	        let file = thing.unwrap().path();
	        let contents = fs::read_to_string(&file).unwrap().replace("$Template", &project_name);

	        fs::write(file, contents).unwrap();
	    }
	}

	let tmp_sdk = std::env::temp_dir().join("sdk");

	if tmp_sdk.exists() {
	    fs_dir::remove(&tmp_sdk).unwrap();
	}

	match Repository::clone_recurse("https://github.com/geode-sdk/sdk", &tmp_sdk) {
	    Ok(_) => (),
	    Err(e) => print_error!("Failed to clone sdk: {}", e),
	};

	copy_dir::copy_dir(&tmp_sdk, project_location.join("sdk")).unwrap();
	fs_dir::remove(tmp_sdk).unwrap();

	
	let mod_json = json!({
	    "geode":        GEODE_VERSION,
	    "version":      version,
	    "id":           id,
	    "name":         project_name,
	    "developer":    developer,
	    "description":  description,
	    "details":      null,
	    "credits":      null,
	    "binary": {
	        "*": binary_name
	    },
	    "dependencies": [
	        {
	            "id": "com.geode.api",
	            "required": true
	        }
	    ]
	});

	fs::write(
	    &project_location.join("mod.json"),
	    to_string_pretty(&mod_json).unwrap()
	).expect("Unable to write to specified project");
}