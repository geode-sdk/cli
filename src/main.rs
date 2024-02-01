use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod file;
mod index;
mod indexer;
mod info;
mod package;
mod profile;
mod project;
mod sdk;
mod template;
mod util;

use util::*;

/// Command-line interface for Geode
#[derive(Parser, Debug)]
#[clap(version)]
struct Args {
	#[clap(subcommand)]
	command: GeodeCommands,
}

#[derive(Subcommand, Debug)]
enum GeodeCommands {
	/// Initialize a new Geode project
	New {
		/// The target directory to create the project in
		path: Option<PathBuf>,
	},

	/// Options for managing profiles (installations of Geode)
	Profile {
		#[clap(subcommand)]
		commands: crate::profile::Profile,
	},

	/// Options for configuring Geode CLI
	Config {
		#[clap(subcommand)]
		commands: crate::info::Info,
	},

	/// Options for installing & managing the Geode SDK
	Sdk {
		#[clap(subcommand)]
		commands: crate::sdk::Sdk,
	},

	/// Tools for working with the current mod project
	Project {
		#[clap(subcommand)]
		commands: crate::project::Project,
	},

	/// Options for working with .geode packages
	Package {
		#[clap(subcommand)]
		commands: crate::package::Package,
	},

	/// Tools for interacting with the Geode mod index
	Index {
		#[clap(subcommand)]
		commands: crate::index::Index,
	},

	/// Run default instance of Geometry Dash
	Run {
		/// Run Geometry Dash in the background instead of the foreground
		#[clap(long)]
		background: bool,
	},
}

fn main() {
	#[cfg(windows)]
	match ansi_term::enable_ansi_support() {
		Ok(_) => {}
		Err(_) => println!("Unable to enable color support, output may look weird!"),
	};

	let args = Args::parse();

	let mut config = config::Config::new();

	match args.command {
		GeodeCommands::New { path } => template::build_template(&mut config, path),
		GeodeCommands::Profile { commands } => profile::subcommand(&mut config, commands),
		GeodeCommands::Config { commands } => info::subcommand(&mut config, commands),
		GeodeCommands::Sdk { commands } => sdk::subcommand(&mut config, commands),
		GeodeCommands::Package { commands } => package::subcommand(&mut config, commands),
		GeodeCommands::Project { commands } => project::subcommand(&mut config, commands),
		GeodeCommands::Index { commands } => index::subcommand(&mut config, commands),
		GeodeCommands::Run { background } => profile::run_profile(&config, None, background),
	}

	config.save();
}
