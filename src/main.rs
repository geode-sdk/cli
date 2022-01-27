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
use winreg::enums::*;
use winreg::RegKey;
use path_slash::PathBufExt;

use sysinfo::{System, SystemExt, ProcessExt};

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
    if cfg!(windows) {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let steam_key = hklm.open_subkey("SOFTWARE\\WOW6432Node\\Valve\\Steam")?;
        let install_path: String = steam_key.get_value("InstallPath")?;

        let test_path = PathBuf::from(&install_path).join("steamapps/common/Geometry Dash/GeometryDash.exe");

        if test_path.exists() && test_path.is_file() {
            let p = PathBuf::from(&test_path.to_slash().unwrap());
            return Ok(p);
        }

        let config_path = PathBuf::from(&install_path).join("config/libraryfolders.vdf");

        for line_res in read_lines(&config_path)? {
            let mut line = line_res?;
            if line.to_string().contains("\"path\"") {
                line = line.replace("\"path\"", "");
                let end = line.rfind("\"").unwrap();
                let start = line[0..end].rfind("\"").unwrap();
                let result = &line[start+1..end];

                let path = PathBuf::from(&result).join("steamapps/common/Geometry Dash/GeometryDash.exe");

                if path.exists() && path.is_file() {
                    let p = PathBuf::from(&path.to_slash().unwrap());
                    return Ok(p);
                }
            }
        }

        Err(Error::new(ErrorKind::Other, "Unable to find GD path"))
    } else {
        let mut sys = System::new();
        sys.refresh_processes();

        let mut gd_procs = sys.processes_by_exact_name("Geometry Dash");

        let gd_proc = match gd_procs.next() {
            Some(e) => e,
            None => return Err(Error::new(ErrorKind::Other, "Please re-run with Geometry Dash open")),
        };

        match gd_procs.next() {
            Some(_) => return Err(Error::new(ErrorKind::Other, "It seems there are two instances of Geometry Dash open. Please re-run with only one instance.")),
            None => (),
        }

        let mut p = gd_proc.exe().parent().unwrap().to_path_buf();

        if cfg!(target_os = "macos") {
            p = p.parent().unwrap().to_path_buf();
        }
        Ok(p)
    }
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
            let mut init_git = String::from("");

            let mut rl = Editor::<()>::new();

            let mut prompts = [
                ("Mod name", &mut project_name, Color::Green, true),
                ("Developer", &mut developer, Color::Green, true),
                ("Version", &mut version, Color::Green, true),
                ("Description", &mut description, Color::Green, true),
                ("Location", &mut buffer, Color::Green, true),
                ("Initialize git repository? (Y,n)", &mut init_git, Color::Green, false),
            ];
            
            let mut ix = 0;
            loop {
                if ix > prompts.len() - 1 {
                    break;
                }
                let (prompt, ref mut var, _, required) = prompts[ix];
                let text = format!("{}: ", prompt);
                let readline = rl.readline_with_initial(text.as_str(), (var.as_str(), ""));
                match readline {
                    Ok(line) => {
                        rl.add_history_entry(line.as_str());
                        if line.is_empty() && required {
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
            init_git = init_git.trim().to_string();

            let project_location = Path::new(&buffer).join(&project_name);

            let id = format!("com.{}.{}", developer.to_lowercase(), project_name.to_lowercase());

            let mut binary_name = project_name.to_lowercase();
            remove_whitespace(&mut binary_name);
            
            println!(
                "Creating mod with ID {} named {} by {} version {} in {}",
                id.green(),
                project_name.green(),
                developer.green(),
                version.green(),
                project_location.parent().unwrap().to_str().unwrap().green()
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

            

            if init_git.is_empty() || init_git.to_lowercase() == "y" {
                let repo = match Repository::init(&project_location) {
                    Ok(r) => r,
                    Err(e) => panic!("failed to init git repo: {}", e),
                };

                let mut sm = match repo.submodule("https://github.com/geode-sdk/sdk", Path::new("sdk"), true) {
                    Ok(r) => r,
                    Err(e) => panic!("failed to add sdk as a submodule: {}", e),
                };

                match sm.clone(None) {
                    Ok(_) => (),
                    Err(e) => panic!("failed to clone sdk: {}", e)
                };
                

                match sm.add_finalize() {
                    Ok(_) => (),
                    Err(e) => panic!("failed to finalize submodule creation: {}", e)
                };
            } else {
                let tmp_sdk = std::env::temp_dir().join("sdk");

                if tmp_sdk.exists() {
                    fs_dir::remove(&tmp_sdk).unwrap();
                }

                match Repository::clone_recurse("https://github.com/geode-sdk/sdk", &tmp_sdk) {
                    Ok(_) => (),
                    Err(e) => panic!("failed to clone sdk: {}", e),
                };

                let options = fs_dir::CopyOptions::new();
                fs_dir::copy(&tmp_sdk, &project_location, &options).unwrap();
                fs_dir::remove(tmp_sdk).unwrap();
            }

            

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
