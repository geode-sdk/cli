use colored::Colorize;
use crate::config::Config;
use crate::link::geode_version;
use git2::Repository;
use path_absolutize::Absolutize;
use rustyline::Editor;
use std::fs;
use std::io::{stdin, stdout, Write};
use std::path::{Path, PathBuf};
use serde_json::{json};
use serde::Serialize;
use crate::progress_bar;

#[macro_export]
macro_rules! print_error {
    ($x:expr $(, $more:expr)*) => {{
        println!("{}", format!($x, $($more),*).red());
        ::std::process::exit(1);
    }}
}

pub fn create_template(
    project_location: &Path,
    name: &str,
    version: &str,
    id: &str,
    developer: &str,
    description: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let project_name: String = name.chars().filter(|c| !c.is_whitespace()).collect();

    Repository::clone("https://github.com/geode-sdk/example-mod", project_location)?;
    fs::remove_dir_all(project_location.join(".git")).unwrap();

    for entry in fs::read_dir(&project_location).unwrap() {
        if !entry.as_ref().unwrap().metadata().unwrap().is_dir() {
            let file = entry.unwrap().path();
            
            let contents = fs::read_to_string(&file).unwrap().replace("Template", &project_name);
            fs::write(file, contents).unwrap();
        }
    }

    let mod_json = json!({
        "geode":        unsafe {geode_version().to_string()},
        "version":      version,
        "id":           id,
        "name":         name,
        "developer":    developer,
        "description":  description,
        "binary": project_name,
        "dependencies": [
            {
                "id": "com.geode.api",
                "required": true
            }
        ]
    });

    let buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
    let mut ser = serde_json::Serializer::with_formatter(buf, formatter);
    mod_json.serialize(&mut ser).unwrap();
    fs::write(
        &project_location.join("mod.json"),
        String::from_utf8(ser.into_inner()).unwrap()
    ).expect("Unable to write to specified project");

    Ok(())
}

fn ask_value(prompt: &str, default: &str, required: bool) -> String {
    let text = format!("{}{}: ", prompt, if required { "" } else { " (optional)" });

    let mut line_reader = Editor::<()>::new();

    loop {
        match line_reader.readline_with_initial(&text, (default, "")) {
            Ok(line) => {
                line_reader.add_history_entry(&line);

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
            Err(err) => print_error!("Error: {}", err)
        }
    }
}

pub fn build_template(name: Option<String>, location: Option<PathBuf>) {
    let name = match name {
        Some(s) => ask_value("Name", s.as_str(), true),
        None => ask_value("Name", "", true)
    };

    let default_location = match location {
        Some(s) => s.join(&name),
        None => std::env::current_dir().unwrap()
    };
    let default_location = default_location.absolutize().unwrap();

    let version = ask_value("Version", "v1.0.0", true);
    let developer = ask_value(
        "Developer",
        Config::get().default_developer.as_ref().unwrap_or(&String::new()),
        true
    );

    if Config::get().default_developer.is_none() {
        println!("{}{}{}\n{}{}",
            "Using ".bright_cyan(),
            developer,
            " as default developer name for future projects.".bright_cyan(),
            "If this is undesirable, use ".bright_cyan(),
            "`geode config --dev <NAME>`".bright_yellow()
        );
        Config::get().default_developer = Some(developer.clone());
    }

    let description = ask_value("Description", "", false);
    let project_location = PathBuf::from(ask_value("Location", &default_location.to_string_lossy(), true));

    let id = format!(
        "com.{}.{}",
        developer.to_lowercase().replace(' ', "_"),
        name.to_lowercase().replace(' ', "_")
    );
    
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
    } else if fs::create_dir_all(&project_location).is_err() {
        print_error!("Unable to create directory for project");
    }

    let bar = progress_bar("Creating...");

    if let Err(e) = create_template(
        &project_location,
        &name,
        &version,
        &id,
        &developer,
        &description,
    ) {
        print_error!("Error creating template: {}", e);
    }

    bar.finish_with_message(format!("{}", "Succesfully initialized project! Happy modding :)".bright_cyan()));
}
