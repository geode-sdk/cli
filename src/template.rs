use git2::Repository;
use std::io::{stdin, stdout, Write};
use std::path::PathBuf;
use std::fs;
use rustyline::Editor;
use path_absolutize::Absolutize;
use serde_json::json;
use serde::Serialize;
use crate::{fail, fatal, warn, info, done};
use crate::config::Config;

fn create_template(
    project_location: PathBuf,
    name: String,
    version: String,
    id: String,
    developer: String,
    description: String
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
        fs::create_dir_all(&project_location)
            .unwrap_or_else(|e| fatal!("Unable to create project directory: {}", e));
    }

	// Clone repository
	Repository::clone("https://github.com/geode-sdk/example-mod", &project_location)
        .unwrap_or_else(|e| fatal!("Unable to clone repository: {}", e));

	fs::remove_dir_all(project_location.join(".git")).unwrap();

	// Replace "Template" with project name (no spaces)
	let filtered_name: String = name.chars().filter(|c| !c.is_whitespace()).collect();

	for entry in fs::read_dir(&project_location).unwrap() {
	    if !entry.as_ref().unwrap().metadata().unwrap().is_dir() {
	        let file = entry.unwrap().path();
	        
	        let contents = fs::read_to_string(&file).unwrap().replace("Template", &filtered_name);
	        fs::write(file, contents).unwrap();
	    }
	}

	// Default mod.json
	let mod_json = json!({
	    "geode":        "3", // TODO: fix
	    "version":      version,
	    "id":           id,
	    "name":         name,
	    "developer":    developer,
	    "description":  description,
	    "binary": filtered_name
	});

	// Format neatly
	let buf = Vec::new();
	let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
	let mut ser = serde_json::Serializer::with_formatter(buf, formatter);
	mod_json.serialize(&mut ser).unwrap();

	// Write formatted json
	fs::write(
	    &project_location.join("mod.json"),
	    String::from_utf8(ser.into_inner()).unwrap()
	).unwrap_or_else(|e| fatal!("Unable to write to project: {}", e));

	done!("Succesfully initialized project! Happy modding :)");
}


// fix this
fn ask_value(prompt: &str, default: Option<&str>, required: bool) -> String {
    let text = format!("{}{}: ", prompt, if required { "" } else { " (optional)" });
    let mut line_reader = Editor::<()>::new();
    loop {
        let line = line_reader.readline_with_initial(&text, (default.unwrap_or(""), "")).unwrap();
        line_reader.add_history_entry(&line);

        if line.is_empty() {
            if required {
                fail!("Please enter a value");
            } else {
                return default.unwrap_or("").to_string();
            }
        } else {
            return line.trim().to_string();
        }
    }
}

pub fn build_template(config: &mut Config, name: Option<String>, location: Option<PathBuf>) {
	let final_name = ask_value("Name", name.as_deref(), true);

	let location = location.unwrap_or(std::env::current_dir().unwrap().join(&final_name));
	let location = location.absolutize().unwrap();

    let final_version = ask_value("Version", Some("v1.0.0"), true);

    let final_developer = ask_value(
        "Developer",
        config.default_developer.as_ref().map(|x| &**x),
        true
    );

    if config.default_developer.is_none() {
    	info!("Using '{}' as the default developer for all future projects.", &final_developer);
    	config.default_developer = Some(final_developer.clone());
    }

    let final_description = ask_value("Description", None, false);
    let final_location = PathBuf::from(ask_value("Location", Some(&location.to_string_lossy()), true));

    let mod_id = format!(
        "com.{}.{}",
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
    	final_description
    );
}
