/**
 * geode new: Create new geode project from template
 * geode info: Subcommand for listing information about the current state
 * geode package: Subcommand for managing .geode files
 * geode sdk: Subcommand for managing geode sdk
 * geode profile: Subcommand for managing geode installations
 * geode install: alias of `geode package install`
 */
use std::path::PathBuf;
use clap::{Parser, Subcommand};

mod logging;
mod template;
mod config;
mod package;
mod profile;
mod info;
mod sdk;


/// Command-line interface for Geode
#[derive(Parser, Debug)]
#[clap(version)]
struct Args {
    #[clap(subcommand)]
    command: GeodeCommands
}

#[derive(Subcommand, Debug)]
enum GeodeCommands {
    /// Create template mod project
    New {
        /// Mod project directory
        #[clap(short, long)]
        path: Option<PathBuf>,

        /// Mod name
        #[clap(short, long)]
        name: Option<String>
    },

    /// Install geode file to current profile
    Install {
        /// Location of .geode file to install
        path: PathBuf
    },

    Profile {
        #[clap(subcommand)]
        commands: crate::profile::Profile
    },

    Config {
        #[clap(subcommand)]
        commands: crate::info::Info
    },

    Sdk {
        #[clap(subcommand)]
        commands: crate::sdk::Sdk
    }
}


fn main() {
    let args = Args::parse();

    let mut config = config::Config::new();

    match args.command {
        GeodeCommands::New { name, path} => template::build_template(&mut config, name, path),
        GeodeCommands::Install { path } => package::install(&mut config, &path),

        GeodeCommands::Profile { commands } => profile::subcommand(&mut config, commands),

        GeodeCommands::Config { commands } => info::subcommand(&mut config, commands),

        GeodeCommands::Sdk { commands } => sdk::subcommand(&mut config, commands)
    }

    config.save();
}
