use crate::link::{string2c, opt2c};
use std::path::{PathBuf};
use colored::*;
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};

pub mod template_ui;
pub mod config;
pub mod link;

#[cfg(windows)]
use crate::windows_ansi::enable_ansi_support;
use crate::config::Configuration;

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
        path: Option<PathBuf>,

        #[clap(long)]
        dev: Option<String>,
    },
    /// Create a new Geode project
    New {
        /// Mod name
        name: Option<String>,
        /// Where to create the project, defaults
        /// to the current folder
        location: Option<PathBuf>,
    },
    /// Package a mod.json and a platform binary file 
    /// into a .geode file
    Pkg {
        /// Path to the mod's mod.json file
        resource_dir: String,
        /// Path to the directory containing the mod's 
        /// platform binary.
        exec_dir: String,

        /// Path to put the generated .geode file
        out_file: String,
        
        /// Automatically copy the generated .geode 
        /// file to the Geode mods directory
        #[clap(short, long)]
        install: bool,

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
        Commands::New { location, name } => template_ui::cli_create_template(name, location),

        Commands::About {} => {
            println!(
                " == {} == \nGeode Version: {}\nCLI Version: {}\nGeode Installation: {}",
                GEODE_CLI_NAME.to_string().green(),
                unsafe {link::geode_version()}.to_string().red(),
                GEODE_CLI_VERSION.to_string().yellow(),
                Configuration::install_path().to_str().unwrap().purple()
            );
        },

        Commands::Pkg { resource_dir, exec_dir, out_file, install, cached } => {
                call_extern!(link::geode_package(
                    string2c(resource_dir),
                    string2c(exec_dir),
                    string2c(&out_file),
                    true,
                    cached
                ));

                if install {
                    call_extern!(link::geode_install_package(
                        string2c(Configuration::install_path().to_str().unwrap()),
                        string2c(out_file)
                    ));
                }
            },

        Commands::Config { path, dev } => {
            let mut some_set = false;
            if path.is_some() {
                Configuration::set_install_path(path.unwrap());
                some_set = true;
            }
            if dev.is_some() {
                Configuration::set_dev_name(dev.unwrap());
                some_set = true;
            }
            if !some_set {
                print_error!("Please provide some setting to set the value of");
            }
        },

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

            let mut res = link::CPackInfo {
                suffix_removals: 0,
                created_files: std::ptr::null_mut()
            };

            call_extern!(link::geode_sprite_sheet(
                string2c(src.to_str().unwrap()),
                string2c(dest.to_str().unwrap()),
                variants,
                opt2c(name),
                opt2c(prefix),
                (&mut res) as *mut link::CPackInfo
            ));

            bar.finish_with_message(format!("{}", "Spritesheet created!".bright_green()));

            for file in res.get_files() {
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

            call_extern!(link::geode_sprite_variants(
                string2c(src.to_str().unwrap()),
                string2c(dest.to_str().unwrap())
            ));
            bar.finish_with_message(format!("{}", "Variants created!".bright_green()));
        },

        Commands::Update { version, check } => {
            if check {
                let mut has = false;

                call_extern!(link::geode_update_check(
                    string2c(Configuration::install_path().to_str().unwrap()),
                    opt2c(version),
                    (&mut has) as *mut bool
                ));
            } else {
                call_extern!(link::geode_update(
                    string2c(Configuration::install_path().to_str().unwrap()),
                    opt2c(version)
                ));
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
            let mut target_path = Configuration::install_path().join("geode").join("mods");
            target_path = target_path.join(path.file_name().unwrap());

            std::fs::rename(path, target_path).unwrap();
        }
    }
}
