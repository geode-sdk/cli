use std::path::PathBuf;
use colored::*;
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};
use resources::{create_resources, parse_resources};
use std::fs;

#[cfg(windows)]
use std::process::Command;

pub mod util;
pub mod spritesheet;
pub mod font;
pub mod dither;
pub mod template;
pub mod package;
pub mod config;
pub mod link;
pub mod resources;

#[cfg(windows)]
pub mod windows_ansi;

#[cfg(windows)]
use crate::windows_ansi::enable_ansi_support;
use crate::config::Config;

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
    /// Display / modify Geode configuration
    Config {
        /// Current working installation index
        #[clap(long)]
        cwi: Option<usize>,
        
        /// Default developer name
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
        
        /// Use cached resources
        #[clap(long)]
        cached: bool,
    },

    /// Add a file to a .geode package
    Amend {
        /// Path to the .geode file to amend
		geode_file: PathBuf,

        /// Path to the file to add
		file_to_add: PathBuf,

        /// Directory in the .geode package where to 
        /// add the file
		dir_in_zip: PathBuf
    },

    /// Temporarily open a .geode package for editing
    Edit {
        geode_file: PathBuf,
        tmp_folder: Option<PathBuf>
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

    /// Create a bitmap font out of a TTF file
    Font {
        /// Path to TTF font
        ttf_path: PathBuf,

        /// Font size
        fontsize: u32,

        /// Font name, if not specified defaults to same name as TTF
        name: Option<String>,

        /// Path to directory where to put the resulting bitmap font files
        #[clap(short, long)]
        dest: Option<PathBuf>,

        /// Create variants (High, Medium, Low)
        #[clap(short, long)]
        variants: bool,

        /// Prefix
        #[clap(long)]
        prefix: Option<String>,

        /// Character set; for example 0-0,8-9,13,29,32-126,160-255. 
        /// Defaults to ASCII
        #[clap(long)]
        charset: Option<String>,

        /// Font outline size, defaults to 0. If passed a number greater 
        /// than 0, a black outline will be added to the font
        #[clap(long, default_value_t = 0)]
        outline: u32,
    },

    /// Create variants (High, Medium, Low) of a sprite
    Sprite {
        /// Path to the sprite
        src: PathBuf,

        /// Path to directory where to put the resulting sprites
        dest: PathBuf,

        /// Prefix
        #[clap(long)]
        prefix: Option<String>
    },

    /// Install a .geode file to the current
    /// selected installation
    Install {
        /// Path to .geode file to install
        path: PathBuf
    },

    /// Process a folder of resources based on a json file
    Resources {
        /// Folder with resources. Use a resource.json file 
        /// to list your resources in the same format as 
        /// [mod.json].resources
        src: PathBuf,

        /// Folder to put the resulting files
        dest: PathBuf,

        /// Prefix
        #[clap(long)]
        prefix: Option<String>,

        /// Use cached resources
        #[clap(long)]
        cached: bool,
    },

    /// Launch the selected Geometry Dash installation
    Launch {},
}

pub fn progress_bar(text: &str) -> ProgressBar {
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
    bar.set_message(format!("{}", text.bright_cyan()));
    bar
}

fn main() {
    #[cfg(windows)] 
    match enable_ansi_support() {
        Ok(_) => {},
        Err(e) => println!("Unable to enable ANSI support: {}", e)
    }
    
	let mut config = Config::new();

    let args = Cli::parse();

    match args.command {
        Commands::New { location, name } => template::build_template(&mut config, name, location),

        Commands::Pkg { resource_dir, exec_dir, out_file, install, cached } => {
            if let Err(e) = package::create_geode(
                &resource_dir,
                &exec_dir,
                &out_file,
                true,
                cached,
            ) {
                print_error!("Error packaging geode file: {}", e);
            }

            if install {
                if let Err(e) = package::install_geode_file(
                    &config.work_inst().path,
                    &out_file
                ) {
                    print_error!("Error installing package: {}", e);
                }
            }
        },

        Commands::Amend { geode_file, file_to_add, dir_in_zip } => {
            if let Err(e) = package::amend_geode(
                &geode_file,
                &file_to_add,
                &dir_in_zip,
            ) {
                print_error!("Error amending package: {}", e);
            }
            println!("{}", "Amended package :)".green());
        },

        Commands::Edit { geode_file, tmp_folder } => {
            if let Err(e) = package::edit_geode_interactive(
                &geode_file,
                tmp_folder.clone(),
            ) {
                print_error!("Error editing package: {}", e);
            }
            println!("{}", "Edited package :)".green());
        },

        Commands::Config { cwi, dev } => {
            let mut some_set = false;
            if let Some(ver) = cwi {
                if ver >= config.installations.as_ref().unwrap().len() {
                    print_error!(
                        "Provided index is higher than your \
                        amount of installations!"
                    );
                }
                config.working_installation = cwi;
                some_set = true;
                println!("Updated working installation");
            }
            if dev.is_some() {
                config.default_developer = dev;
                some_set = true;
                println!("Updated default developer");
            }
            if !some_set {
                println!(
                    " == {} == \n\
                    Version: {}{}\n\
                    Target Geode Version: {}\n\
                    Path: {}\n\
                    Default developer: {}\n\
                    Data directory: {}\n\
                    Installations: {} (Selected: {})\n\
                    {}",
                    GEODE_CLI_NAME.to_string().green(),
                    "v".yellow(),
                    GEODE_CLI_VERSION.to_string().yellow(),
                    unsafe {link::geode_target_version()}.to_string().yellow(),
                    std::env::current_exe().unwrap().to_str().unwrap().cyan(),
                    match config.default_developer.as_ref() {
                        Some(s) => s,
                        None => "<none>"
                    }.purple(),
                    Config::data_dir().to_str().unwrap().cyan(),
                    config.installations.as_ref().unwrap().len().to_string().red(),
                    config.working_installation.unwrap().to_string().red(),
                    config.installations_as_string(),
                );
            }
        },

        Commands::Sheet { src, dest, variants, name, prefix } => {
            let bar = progress_bar("Creating spritesheet(s)...");

            let res = match spritesheet::pack_sprites_in_dir(
                &src,
                &dest,
                variants,
                name.as_deref(),
                prefix.as_deref()
            ) {
                Ok(a) => a,
                Err(e) => print_error!("Error creating spritesheet: {}", e)
            };

            bar.finish_with_message(format!("{}", "Spritesheet created!".bright_green()));

            for file in res.created_files {
                println!("{} -> {}", "[ info ]".bright_yellow(), file);
            }

            println!("{} You might want to delete the tmp dirs",
                "[ info ]".bright_yellow()
            );
        },

        Commands::Sprite { src, dest, prefix } => {
            let bar = progress_bar("Creating variants...");
            match spritesheet::create_variants_of_sprite(
                &src,
                &dest,
                prefix.as_deref()
            ) {
                Ok(_) => (),
                Err(e) => print_error!("Error creating variants: {}", e)
            }
            bar.finish_with_message(format!("{}", "Variants created!".bright_green()));
        },

        Commands::Font { ttf_path, dest, fontsize, name, variants, prefix, charset, outline } => {
            let bar = progress_bar("Creating font...");
            match font::create_bitmap_font_from_ttf(
                &ttf_path,
                &dest.unwrap_or_else(|| std::env::current_dir().unwrap()),
                name.as_deref(),
                fontsize,
                prefix.as_deref(),
                variants,
                charset.as_deref(),
                outline,
            ) {
                Ok(_) => (),
                Err(e) => print_error!("Error creating font: {}", e)
            }
            bar.finish_with_message(format!("{}", "Bitmap font created!".bright_green()));
        },

        Commands::Install { path } => {
            package::install_geode_file(
                &config.work_inst().path,
                &path
            ).unwrap();
        },

        Commands::Launch {} => {
            println!("{}", "Launching Geometry Dash...".bright_cyan());
            #[cfg(windows)] 
            Command::new(&config.work_inst().path.join(&config.work_inst().executable))
                .current_dir(&config.work_inst().path)
                .spawn()
                .expect("Unable to launch Geometry Dash");
        },

        Commands::Resources { src, dest, prefix, cached } => {
            if !src.join("resources.json").exists() {
                print_error!(
                    "Missing {}! Create it and list your resources 
                    in the same format as [mod.json].resources",
                    src.join("resources.json").to_str().unwrap()
                );
            }
            let r = parse_resources(
                serde_json::from_str::<serde_json::Value>(
                    &fs::read_to_string(src.join("resources.json")).unwrap()
                ).unwrap().as_object().unwrap(),
                &src
            ).unwrap();
            create_resources(
                &r,
                cached,
                &prefix.unwrap_or(String::new()),
                &dest,
                true
            ).unwrap();
            println!("{}", "Resources created!".bright_green());
        },
    }

    config.save();
}
