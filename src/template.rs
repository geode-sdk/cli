use colored::*;
use crate::GEODE_VERSION;
use crate::print_error;
use git2::Repository;
use path_absolutize::Absolutize;
use rustyline::Editor;
use serde_json::{json, to_string_pretty};
use std::path::PathBuf;
use std::{fs, path::Path};
use std::io::{stdin,stdout,Write};
use indicatif::{ProgressBar, ProgressStyle};

use crate::{Configuration};

fn ask_value(prompt: &str, default: &str, required: bool) -> String {
	let text = format!("{}{}: ", prompt, if required { "" } else { " (optional)" });

	let mut rl = Editor::<()>::new();

	loop {
		let readline = rl.readline_with_initial(text.as_str(), (default, ""));
		match readline {
			Ok(line) => {
				rl.add_history_entry(line.as_str());
				if line.is_empty() {
					if required {
						println!("{}", "Please enter a value".red());
					} else {
						return default.to_string();
					}
				} else {
					return line.trim().to_string();
				}
			},
			Err(err) => {
				print_error!("Error: {}", err);
			}
		}
	}
}

pub fn create_template(project_name: Option<String>, location: Option<PathBuf>) {
	let is_location_default = location.is_none();
	let loc = match location {
	    Some(s) => s,
	    None => std::env::current_dir().unwrap()
	};

	let name = match project_name {
		Some(s) => ask_value("Name", s.as_str(), true),
		None => ask_value("Name", "", true)
	};
	let version = ask_value("Version", "v1.0.0", true);
	let developer = ask_value(
		"Developer", Configuration::dev_name().as_str(), true
	);

	if Configuration::get().default_developer.is_none() {
		println!("{}{}{}\n{}{}",
			"Using ".bright_cyan(),
			developer,
			" as default developer name for future projects.".bright_cyan(),
			"If this is undesirable, use ".bright_cyan(),
			"`geode config --dev <NAME>`".bright_yellow()
		);
		Configuration::set_dev_name(developer.clone());
	}

	let description = ask_value("Description", "", false);
	let buffer = if is_location_default {
		loc.absolutize().unwrap().join(&name).to_str().unwrap().to_string()
	} else {
		loc.absolutize().unwrap().to_str().unwrap().to_string()
	};
	let locstr = ask_value("Location", buffer.as_str(), true);
	let project_location = Path::new(&locstr);

	let id = format!("com.{}.{}", developer.to_lowercase(), name.to_lowercase());

	let mut binary_name = name.to_lowercase();
	binary_name.retain(|c| !c.is_whitespace());
	
	println!(
	    "Creating mod with ID {} named {} by {} version {} in {}",
	    id.green(),
	    name.green(),
	    developer.green(),
	    version.green(),
	    project_location.to_str().unwrap().green()
	);

	if project_location.exists() {
	    println!("{}", "It appears that the provided location already exists.".bright_yellow());
	    print!("{}", "Are you sure you want to proceed? (y/N) ".bright_yellow());
		stdout().flush().unwrap();
		let mut ans = String::new();
		stdin().read_line(&mut ans).unwrap();
		ans = ans.trim().to_string();
		if !(ans == "y" || ans == "Y") {
			println!("{}", "Aborting".bright_red());
			return;
		}
	} else {
		if fs::create_dir_all(&project_location).is_err() {
			print_error!("Unable to create directory for project");
		}
	}

	let bar = ProgressBar::new_spinner();
	bar.enable_steady_tick(120);
	bar.set_style(
		ProgressStyle::default_spinner()
			.tick_strings(&[
				"[##    ]",
				"[###   ]",
				"[####  ]",
				"[ #### ]",
				"[   ###]",
				"[    ##]",
				"[#    #]",
				"[ done ]",
			])
			.template("{spinner:.cyan} {msg}"),
	);
	bar.set_message(format!("{}", "Creating...".bright_cyan()));

	match Repository::clone("https://github.com/geode-sdk/example-mod", &project_location) {
	    Ok(_) => (),
	    Err(e) => print_error!("Failed to clone template: {}", e),
	};

	fs::remove_dir_all(&project_location.join(".git")).unwrap();

	for thing in fs::read_dir(&project_location).unwrap() {
	    if !thing.as_ref().unwrap().metadata().unwrap().is_dir() {
	        let file = thing.unwrap().path();
	        let contents = fs::read_to_string(&file).unwrap().replace("$Template", &name);

	        fs::write(file, contents).unwrap();
	    }
	}

	match Repository::clone_recurse("https://github.com/geode-sdk/sdk", &project_location.join("sdk")) {
	    Ok(_) => (),
	    Err(e) => print_error!("Failed to clone sdk: {}", e),
	};
	
	let mod_json = json!({
	    "geode":        GEODE_VERSION,
	    "version":      version,
	    "id":           id,
	    "name":         name,
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

	bar.finish_with_message(format!("{}", "Succesfully initialized project! Happy modding :)".bright_cyan()));
}
