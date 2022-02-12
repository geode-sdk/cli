use std::path::{PathBuf};
use colored::*;
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};

pub mod util;
pub mod package;
pub mod update;
pub mod template;
pub mod config;
pub mod windows_ansi;
pub mod spritesheet;
pub mod dither;
pub mod install;

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
        out_file: PathBuf,
        
        /// Automatically copy the generated .geode 
        /// file to the Geode mods directory
        #[clap(short, long)]
        install: bool,

        /// Copy the generated .geode file in the 
        /// API directory instead of mods
        #[clap(long)]
        api: bool,

        #[clap(long)]
        cached: bool,
    },

    /// Create a sprite sheet out of a bunch of sprites
    Sheet {
        /// Path to directory containing the sprites
        src: PathBuf,
        /// Path to directory where to put the resulting sheet
        dest: PathBuf,
        /// Create variants (High, Medium, Low). Note that 
        /// the source textures are assumed to be UHD
        #[clap(short, long)]
        variants: bool,
        /// Spritesheet name
        #[clap(short, long)]
        name: Option<String>,
        /// Prefix
        #[clap(long)]
        prefix: Option<String>,
    },

    /// Create variants (High, Medium, Low) of a sprite
    Sprite {
        /// Path to the sprite
        src: PathBuf,
        /// Path to directory where to put the resulting sprites
        dest: PathBuf,
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
    },

    Setup {},

    Install {
        /// Path to .geode file to install
        path: PathBuf
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

        Commands::Pkg { resource_dir, exec_dir, out_file, install, api, cached } => 
            package::create_geode(
                &resource_dir,
                &exec_dir,
                &out_file,
                install,
                api,
                true,
                cached
            ),

        Commands::Config { path } => Configuration::set_install_path(path),

        Commands::Info { modpath } => {
            if modpath {
                println!("{}", Configuration::install_path().join("geode").join("mods").display());
            } else {
                print_error!("Please specify thing you want information from");
            }
        },

        Commands::Sheet { src, dest, variants, name, prefix } => {
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
            bar.set_message(format!("{}", "Creating spritesheet(s)...".bright_cyan()));
            let res = spritesheet::pack_sprites_in_dir(
                &src, &dest, variants, name, prefix, 
                Some(|s: &str| println!("{}", s.yellow().bold()))
            ).unwrap();
            bar.finish_with_message(format!("{}", "Spritesheet created!".bright_green()));
            for file in res.created_files {
                println!("{} -> {}", "[ info ]".bright_yellow(), file);
            }
            println!("{} You might want to delete the tmp dirs",
                "[ info ]".bright_yellow()
            );
        },

        Commands::Sprite { src, dest } => {
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
            bar.set_message(format!("{}", "Creating variants...".bright_cyan()));
            spritesheet::create_variants_of_sprite(&src, &dest).unwrap();
            bar.finish_with_message(format!("{}", "Variants created!".bright_green()));
        },

        Commands::Update { version, check } => {
            if check {
                update::check_update(version);
            } else {
                update::update_geode(version)
            }
        },

        Commands::Setup {} => {
            match Configuration::install_file_associations() {
                Ok(_) => (
                    println!("File association for .geode files created!")
                ),
                Err(e) => {
                    print_error!("File association failed: {}", e)
                }
            }
        },

        Commands::Install {path} => {
            install::install(&path)
        }
    }
}
