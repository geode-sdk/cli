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

mod util;
mod template;
mod package;
mod profile;
mod info;
mod sdk;

use util::*;

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

    /// Install a .geode package to current profile, alias of `geode package install`
    Install {
        /// Location of the .geode package to install
        path: PathBuf
    },

    /// Subcommand for managing profiles
    Profile {
        #[clap(subcommand)]
        commands: crate::profile::Profile
    },

    /// Subcommand for managing configurable data
    Config {
        #[clap(subcommand)]
        commands: crate::info::Info
    },

    /// Subcommand for managing the Geode SDK
    Sdk {
        #[clap(subcommand)]
        commands: crate::sdk::Sdk
    },

    /// Subcommand for managing Geode packages
    Package {
        #[clap(subcommand)]
        commands: crate::package::Package
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

        GeodeCommands::Sdk { commands } => sdk::subcommand(&mut config, commands),

        GeodeCommands::Package { commands } => package::subcommand(&mut config, commands)
    }

    config.save();
}
