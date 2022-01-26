use std::process::exit;
use std::path::{PathBuf, Path};
use colored::*;
use clap::{Parser, Subcommand};
use path_absolutize::*;
use rustyline::Editor;
use serde::{Serialize, Deserialize};
use serde_json::{json, to_string_pretty, Value};
use std::fs::{self, *};
use std::io::{self, *};
use git2::Repository;
use fs_extra::dir as fs_dir;

use sysinfo::{System, SystemExt};

const GEODE_VERSION: i32 = 1;
const GEODE_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");
const GEODE_CLI_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Parser)]
#[clap(version, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Serialize, Deserialize)]
struct Configuration {
    geode_install_path: PathBuf,
}

#[derive(Subcommand)]
enum Commands {
    /// List information about Geode
    About {},
    /// Create a new Geode project
    New {
        /// Mod name
        name: String,
        /// Where to create the project, defaults
        /// to the current folder
        location: Option<PathBuf>,
    },
    /// Package a mod.json and a platform binary file 
    /// into a .geode file
    Pkg {
        /// Path to the mod's mod.json file
        mod_json_path: String,
        /// Path to the directory containing the mod's 
        /// platform binary. If omitted, will recursively 
        /// look for a platform binary file starting from 
        /// the current folder
        build_dir: Option<PathBuf>,
        /// Whether to copy the created .geode file in 
        /// <geode_install_dir>/geode/mods
        #[clap(short, long)]
        install: bool,
    },
}

fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where P: AsRef<Path>, {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

fn figure_out_gd_path() -> Result<PathBuf> {
    let mut sys = System::new();
    sys.refresh_processes();

    let gd_procs = sys.get_process_by_name("Geometry Dash");

    if gd_procs.is_empty() {
        return Err(Error::new(ErrorKind::Other, "Please re-run with Geometry Dash open"));
    }

    if gd_procs.len() > 1 {
        return Err(Error::new(ErrorKind::Other, "It seems there are two instances of Geometry Dash open. Please re-run with only one instance."));
    }
    let mut p = PathBuf::from(gd_procs[0].exe.clone()).parent().unwrap().to_path_buf();

    if cfg!(target_os = "macos") {
        p = p.parent().unwrap().to_path_buf();
    }
    Ok(p)
}



fn remove_whitespace(s: &mut String) {
    s.retain(|c| !c.is_whitespace());
}

fn add_platform_extension(s: &mut String) {
    if cfg!(windows) {
        s.push_str(".dll");
    } else if cfg!(mac) || cfg!(ios) {
        s.push_str(".dylib");
    } else if cfg!(android) {
        s.push_str(".so");
    }
}

fn main() {
    let mut config = Configuration { geode_install_path: PathBuf::new() };
    let exe_path = std::env::current_exe().unwrap();
    let save_dir = exe_path.parent().unwrap();
    let save_file = save_dir.join("config.json");

    if save_file.exists() {
        let raw = fs::read_to_string(&save_file).unwrap();
        config = match serde_json::from_str(&raw) {
            Ok(p) => p,
            Err(_) => config
        }
    }

    if config.geode_install_path.as_os_str().is_empty() {
        match figure_out_gd_path() {
            Ok(install_path) => {
                config.geode_install_path = install_path;
                println!("Loaded default GD path automatically: {:?}", config.geode_install_path);
            },
            Err(err) => {
                println!("Unable to figure out GD path: {}", err);
                exit(1);
            },
        }
    }

    let raw = serde_json::to_string(&config).unwrap();
    fs::write(save_file, raw).unwrap();

    let args = Cli::parse();
    match args.command {
        Commands::New { location, name } => {
            let loc = match location {
                Some(s) => s,
                None => std::env::current_dir().unwrap()
            };
            let mut project_name = name;

            let mut version = String::from("v1.0.0");
            let mut developer = String::from("");
            let mut description = String::from("");
            let mut buffer = loc.absolutize().unwrap().to_str().unwrap().to_string();

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

            let project_location = Path::new(&buffer).join(&project_name);

            let id = format!("com.{}.{}", developer.to_lowercase(), project_name.to_lowercase());

            let mut binary_name = project_name.to_lowercase();
            remove_whitespace(&mut binary_name);
            
            println!(
                "Creating mod with ID {} named {} by {} version {} in {}",
                id.cyan(),
                project_name.green(),
                developer.red(),
                version.yellow(),
                project_location.parent().unwrap().to_str().unwrap().purple()
            );

            if project_location.exists() {
                println!("{}", "Unable to create project in existing directory".red());
                exit(1);
            }

            match Repository::clone("https://github.com/geode-sdk/example-mod", &project_location) {
                Ok(_) => (),
                Err(e) => panic!("failed to clone template: {}", e),
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
                Err(e) => panic!("failed to clone sdk: {}", e),
            };

            let options = fs_dir::CopyOptions::new();
            fs_dir::copy(&tmp_sdk.join("SDK"), &project_location, &options).unwrap();
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


        },

        Commands::About {} => {
            println!(
                " == {} == \nGeode Version: {}\nCLI Version: {}\nGeode Installation: {}",
                GEODE_CLI_NAME.green(),
                GEODE_VERSION.to_string().red(),
                GEODE_CLI_VERSION.yellow(),
                config.geode_install_path.to_str().unwrap().purple()
            );
        },

        Commands::Pkg { mod_json_path, build_dir: _, install: _ } => {
            let raw = fs::read_to_string(mod_json_path).unwrap();
            let mod_json: Value = match serde_json::from_str(&raw) {
                Ok(p) => p,
                Err(_) => panic!("mod.json is not a valid JSON file!")
            };
            // how do i check if a key exists in a json?!?!?!?
            let mut binary: String;
            if mod_json["binary"].is_string() {
                binary = mod_json["binary"].to_string();
                add_platform_extension(&mut binary);
            } else if mod_json["binary"].is_object() {
                let bin = &mod_json["binary"];
                if cfg!(windows) || cfg!(linux) {
                    binary = bin["windows"].to_string();
                } else if cfg!(mac) {
                    binary = bin["macos"].to_string();
                } else if cfg!(ios) {
                    binary = bin["ios"].to_string();
                } else if cfg!(android) {
                    binary = bin["android"].to_string();
                } else {
                    panic!("You are not on a supported platform :(");
                }
                if binary.is_empty() {
                    binary = bin["*"].to_string();
                }
                match bin["auto"].as_bool() {
                    Some(v) => if v { add_platform_extension(&mut binary); },
                    None => add_platform_extension(&mut binary),
                }
            } else {
                panic!("[mod.json].binary is not a string nor an object!");
            }
            println!("binary name: {}", binary);
        },
    }
}
