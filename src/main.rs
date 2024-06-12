use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod file;
mod index;
mod index_admin;
mod index_auth;
mod index_dev;
mod info;
mod package;
mod profile;
mod project;
mod project_build;
mod sdk;
mod server;
mod template;
mod util;

use crate::profile::RunBackground;
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
		#[clap(long, conflicts_with = "stay")]
		background: bool,

		/// Do not exit CLI after Geometry Dash exits if running in foreground
		#[clap(long, conflicts_with = "background")]
		stay: bool,

		/// Launch arguments for Geometry Dash
		#[clap(last = true, allow_hyphen_values = true)]
		launch_args: Vec<String>,
	},

	/// Builds the project at the current directory
	Build {
		/// Which platform to cross-compile to, if possible
		#[clap(long, short)]
		platform: Option<String>,

		/// Whether to only configure cmake
		#[clap(long, short, default_value_t = false)]
		configure_only: bool,

		/// Whether to only build project
		#[clap(long, short, default_value_t = false)]
		build_only: bool,

		/// Android NDK path, uses ANDROID_NDK_ROOT env var otherwise
		#[clap(long)]
		ndk: Option<String>,

		/// Sets the cmake build type, defaults to Debug or RelWithDebInfo depending on platform
		#[clap(long)]
		config: Option<String>,

		/// Extra cmake arguments when configuring
		#[clap(last = true, allow_hyphen_values = true)]
		extra_conf_args: Vec<String>,
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
		GeodeCommands::Run {
			background,
			stay,
			launch_args,
		} => profile::run_profile(
			&config,
			None,
			match (background, stay) {
				(false, false) => RunBackground::Foreground,
				(false, true) => RunBackground::ForegroundStay,
				(true, false) => RunBackground::Background,
				(true, true) => panic!("Impossible argument combination (background and stay)"),
			},
			launch_args,
		),
		GeodeCommands::Build {
			platform,
			configure_only,
			build_only,
			ndk,
			config,
			extra_conf_args,
		} => project_build::build_project(
			platform,
			configure_only,
			build_only,
			ndk,
			config,
			extra_conf_args,
		),
	}

	config.save();
}
