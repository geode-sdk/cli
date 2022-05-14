
use std::path::{PathBuf};
use colored::*;
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};

pub mod util;
pub mod spritesheet;
pub mod font;
pub mod dither;
pub mod template;
pub mod package;
pub mod config;
pub mod link;

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
    }
}

fn progress_bar(text: &str) -> ProgressBar {
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

    Config::init();

    let args = Cli::parse();

    match args.command {
        Commands::New { location, name } => template::build_template(name, location),

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
                    &Config::work_inst().path,
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

        Commands::Config { cwi, dev } => {
            let mut some_set = false;
            if let Some(ver) = cwi {
                if ver >= Config::get().installations.as_ref().unwrap().len() {
                    print_error!(
                        "Provided index is higher than your \
                        amount of installations!"
                    );
                }
                Config::get().working_installation = cwi;
                some_set = true;
                println!("Updated working installation");
            }
            if dev.is_some() {
                Config::get().default_developer = dev;
                some_set = true;
                println!("Updated default developer");
            }
            if !some_set {
                println!(
                    " == {} == \n\
                    Version: {}\n\
                    Target Geode Version: {}\n\
                    Path: {}\n\
                    Default developer: {}\n\
                    Data directory: {}\n\
                    Selected Installation: {}\n\
                    -> Path: {}",
                    GEODE_CLI_NAME.to_string().green(),
                    GEODE_CLI_VERSION.to_string().yellow(),
                    unsafe {link::geode_version()}.to_string().red(),
                    std::env::current_exe().unwrap().to_str().unwrap().cyan(),
                    match Config::get().default_developer.as_ref() {
                        Some(s) => s,
                        None => "<none>"
                    }.purple(),
                    Config::data_dir().to_str().unwrap().cyan(),
                    Config::get().working_installation.unwrap().to_string().red(),
                    Config::work_inst().path.to_str().unwrap().cyan(),
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
            std::fs::rename(
                &path,
                Config::work_inst().path
                    .join("geode")
                    .join("mods")
                    .join(path.file_name().unwrap())
            ).unwrap();
        }
    }

    Config::save();
}
