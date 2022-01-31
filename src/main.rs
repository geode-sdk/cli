use std::path::{PathBuf};
use colored::*;
use clap::{Parser, Subcommand};


pub mod util;
pub mod package;
pub mod install;
pub mod template;
pub mod config;
pub mod windows_ansi;
pub mod spritesheet;

#[cfg(windows)]
use crate::windows_ansi::enable_ansi_support;
use crate::config::Configuration;

pub const GEODE_VERSION: i32 = 1;
pub const GEODE_CLI_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GEODE_CLI_NAME: &str = env!("CARGO_PKG_NAME");


#[derive(Parser)]
#[clap(version, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List information about Geode
    About {},
    /// Modify Geode configuration
    Config {
        #[clap(long)]
        path: PathBuf,
    },
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
        resource_dir: PathBuf,
        /// Path to the directory containing the mod's 
        /// platform binary.
        exec_dir: PathBuf,

        /// Path to put the generated .geode file
        out_file: PathBuf
    },

    Info {
        #[clap(long)]
        modpath: bool
    },

    /// Update Geode
    Update {
        // if you want to switch to a certain version
        version: Option<String>,

        #[clap(long)]
        check: bool
    }
}

fn main() {
    #[cfg(windows)]
    match enable_ansi_support() {
        Ok(_) => {},
        Err(e) => println!("Unable to enable ANSI support: {}", e)
    }

    Configuration::get();

    let args = Cli::parse();

    match args.command {
        Commands::New { location, name } => template::create_template(name, location),

        Commands::About {} => {
            println!(
                " == {} == \nGeode Version: {}\nCLI Version: {}\nGeode Installation: {}",
                GEODE_CLI_NAME.to_string().green(),
                GEODE_VERSION.to_string().red(),
                GEODE_CLI_VERSION.to_string().yellow(),
                Configuration::install_path().to_str().unwrap().purple()
            );
        },

        Commands::Pkg { resource_dir, exec_dir, out_file } => package::create_geode(&resource_dir, &exec_dir, &out_file),

        Commands::Config { path } => Configuration::set_install_path(path),

        Commands::Info { modpath } => {
            if modpath {
                println!("{}", Configuration::install_path().join("geode").join("mods").display());
            } else {
                print_error!("Please specify thing you want information from");
            }
        },

        Commands::Update { version, check } => {
            if check {
                install::check_update(version);
            } else {
                install::update_geode(version)
            }
        }
    }
}
