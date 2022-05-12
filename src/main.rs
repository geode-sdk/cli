use crate::link::{string2c, opt2c};
use std::path::{PathBuf};
use colored::*;
use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};

pub mod template_ui;
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

    /// Update Geode
    Update {
        // if you want to switch to a certain version
        version: Option<String>,

        #[clap(long)]
        check: bool
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
        Commands::New { location, name } => template_ui::cli_create_template(name, location),

        Commands::Pkg { resource_dir, exec_dir, out_file, install, cached } => {
            call_extern!(link::geode_package(
                string2c(resource_dir),
                string2c(exec_dir),
                string2c(&out_file),
                true,
                cached,
            ));

            if install {
                call_extern!(link::geode_install_package(
                    string2c(Config::work_inst().path.to_str().unwrap()),
                    string2c(out_file)
                ));
            }
        },

        Commands::Amend { geode_file, file_to_add, dir_in_zip } => {
            call_extern!(link::geode_amend_package(
                string2c(geode_file.to_str().unwrap()),
                string2c(file_to_add.to_str().unwrap()),
                string2c(dir_in_zip.to_str().unwrap()),
            ));
            println!("{}", "Amended package :)".green());
        },

        Commands::Config { cwi, dev } => {
            let mut some_set = false;
            if cwi.is_some() {
                if cwi.unwrap() >= Config::get().installations.as_ref().unwrap().len() {
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
                    Path: {}\n\
                    Default developer: {}\n\
                    Data directory: {}\n\
                    Selected Installation: {}\n\
                    -> Path: {}\n\
                    -> Loader Version: {}",
                    GEODE_CLI_NAME.to_string().green(),
                    GEODE_CLI_VERSION.to_string().yellow(),
                    std::env::current_exe().unwrap().to_str().unwrap().cyan(),
                    match Config::get().default_developer.as_ref() {
                        Some(s) => s,
                        None => "<none>"
                    }.purple(),
                    Config::data_dir().to_str().unwrap().cyan(),
                    Config::get().working_installation.unwrap().to_string().red(),
                    Config::work_inst().path.to_str().unwrap().cyan(),
                    unsafe {link::geode_version()}.to_string().red(),
                );
            }
        },

        Commands::Sheet { src, dest, variants, name, prefix } => {
            let bar = progress_bar("Creating spritesheet(s)...");

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

        Commands::Sprite { src, dest, prefix } => {
            let bar = progress_bar("Creating variants...");
            call_extern!(link::geode_sprite_variants(
                string2c(src.to_str().unwrap()),
                string2c(dest.to_str().unwrap()),
                opt2c(prefix)
            ));
            bar.finish_with_message(format!("{}", "Variants created!".bright_green()));
        },

        Commands::Font { ttf_path, dest, fontsize, name, variants, prefix, charset, outline } => {
            let bar = progress_bar("Creating font...");
            call_extern!(link::geode_create_bitmap_font_from_ttf(
                string2c(ttf_path.to_str().unwrap()),
                string2c(dest.unwrap_or(std::env::current_dir().unwrap()).to_str().unwrap()),
                opt2c(name),
                fontsize,
                opt2c(prefix),
                variants,
                opt2c(charset),
                outline,
            ));
            bar.finish_with_message(format!("{}", "Bitmap font created!".bright_green()));
        },

        Commands::Update { version, check } => {
            if check {
                let mut has = false;

                call_extern!(link::geode_update_check(
                    string2c(Config::work_inst().path.to_str().unwrap()),
                    opt2c(version),
                    (&mut has) as *mut bool
                ));
            } else {
                call_extern!(link::geode_update(
                    string2c(Config::work_inst().path.to_str().unwrap()),
                    opt2c(version)
                ));
            }
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
