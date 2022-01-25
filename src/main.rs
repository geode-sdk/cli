use std::path::PathBuf;
use colored::*;
use clap::Parser;
use clap::Subcommand;
use path_absolutize::*;
use rustyline::Editor;
use serde_json::{json, to_string_pretty};
use std::fs;

#[derive(Parser)]
#[clap(version, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new Geode project
    New {
        name: Option<String>,
        location: Option<PathBuf>,
    },
}

fn remove_whitespace(s: &mut String) {
    s.retain(|c| !c.is_whitespace());
}

fn main() {
    let args = Cli::parse();
    match args.command {
        Commands::New { location, name } => {
            let loc = match location {
                Some(s) => s,
                None => std::env::current_dir().unwrap()
            };
            let mut absolute_location = loc.absolutize().unwrap();
            let mut project_name = match name {
                Some(s) => s,
                None => absolute_location.file_name().unwrap().to_str().unwrap().to_string()
            };
            let mut version = String::from("v1.0.0");
            let mut developer = String::from("");
            let mut description = String::from("");
            let mut buffer = absolute_location.to_str().unwrap().to_string();

            let mut rl = Editor::<()>::new();

            let mut prompts = [
                ("Mod name", &mut project_name, Color::Green),
                ("Developer", &mut developer, Color::Red),
                ("Version", &mut version, Color::Blue),
                ("Description", &mut description, Color::Yellow),
                ("Where to", &mut buffer, Color::Magenta),
            ];
            
            let mut ix = 0;
            loop {
                if ix > prompts.len() - 1 {
                    break;
                }
                let (prompt, ref mut var, _) = prompts[ix];
                let text = format!("{}: ", prompt);
                let readline = rl.readline_with_initial(text.as_str(), (var.as_str(), ""));
                match readline {
                    Ok(line) => {
                        rl.add_history_entry(line.as_str());
                        if line.is_empty() {
                            println!("{}", "Please enter a value".red());
                            continue;
                        }
                        **var = line;
                        ix += 1;
                    },
                    Err(err) => {
                        panic!("Error: {}", err);
                    }
                }
            }
            
            buffer = buffer.trim().to_string();
            version = version.trim().to_string();
            developer = developer.trim().to_string();
            project_name = project_name.trim().to_string();
            description = description.trim().to_string();

            absolute_location = std::borrow::Cow::from(std::path::Path::new(&buffer));

            let id = format!("com.{}.{}", developer.to_lowercase(), project_name.to_lowercase());

            let mut binary_name = project_name.to_lowercase();
            remove_whitespace(&mut binary_name);
            
            println!(
                "Creating mod with ID {} named {} by {} version {} in {}",
                id.cyan(),
                project_name.green(),
                developer.red(),
                version.yellow(),
                absolute_location.to_str().unwrap().purple()
            );

            let mod_json = json!({
                "geode":        1,
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

            let mod_json_path = absolute_location.join("mod.json");

            fs::write(mod_json_path, to_string_pretty(&mod_json).unwrap()).unwrap();
        }
    }
}
