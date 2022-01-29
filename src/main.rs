extern crate colored;

use std::path::{PathBuf, Path};
use colored::*;
use clap::{Parser, Subcommand};

use std::fs::File;
use std::io::{self, *};



pub mod util;
pub mod package;
pub mod install;
pub mod template;
pub mod config;
pub mod windowsAnsi;

use crate::windowsAnsi::enable_ansi_support;
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
        build_path: String,
        /// Path to the directory containing the mod's 
        /// platform binary.
        build_dir: PathBuf,
        /// Whether to copy the created .geode file in 
        /// <geode_install_dir>/geode/mods
        #[clap(short, long)]
        install: bool,
    },

    Update {}
}


fn _read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where P: AsRef<Path>, {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

fn main() {
    if cfg!(windows)
    {
        enable_ansi_support();
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

        Commands::Pkg { build_path, build_dir: _, install: _ } => package::create_geode(build_path),

        Commands::Config { path } => Configuration::set_install_path(path),

        Commands::Update {} => install::update_geode()
    }
}
