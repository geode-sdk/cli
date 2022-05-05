use crate::GEODE_VERSION;
use crate::throw_error;
use git2::Repository;

use serde_json::{json, to_string_pretty};

use std::{fs, path::Path};

pub fn create_template(
	project_location: &Path,
	name: &str,
	version: &str,
	id: &str,
	developer: &str,
	description: &str
) -> Result<(), Box<dyn std::error::Error>> {
	let project_name = name.chars().filter(|c| !c.is_whitespace()).collect::<String>();

	match Repository::clone("https://github.com/geode-sdk/example-mod", project_location) {
	    Ok(_) => (),
	    Err(e) => throw_error!("Failed to clone template: {}", e),
	};

	fs::remove_dir_all(project_location.join(".git")).unwrap();

	for thing in fs::read_dir(&project_location).unwrap() {
	    if !thing.as_ref().unwrap().metadata().unwrap().is_dir() {
	        let file = thing.unwrap().path();
	        let contents = fs::read_to_string(&file).unwrap().replace("$Template", &project_name);

	        fs::write(file, contents).unwrap();
	    }
	}

	match Repository::clone_recurse("https://github.com/geode-sdk/sdk", project_location.join("sdk")) {
	    Ok(_) => (),
	    Err(e) => throw_error!("Failed to clone sdk: {}", e),
	};
	
	let mod_json = json!({
	    "geode":        GEODE_VERSION,
	    "version":      version,
	    "id":           id,
	    "name":         name,
	    "developer":    developer,
	    "description":  description,
	    "binary": {
	        "*": project_name
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

	Ok(())
}
