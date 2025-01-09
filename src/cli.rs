use crate::mod_file::PlatformName;

/// Command-line interface for Geode
#[derive(clap::Parser, Debug)]
#[clap(version)]
pub struct Args {
	#[clap(subcommand)]
	pub command: GeodeCommands,
}

#[derive(clap::Subcommand, Debug)]
pub enum GeodeCommands {
	/// Initialize a new Geode project
	New {
		/// The target directory to create the project in
		path: Option<std::path::PathBuf>,

		/// Whether to make an API project for use by other mods
        	#[arg(short = 'a', long, num_args = 0, action = clap::ArgAction::SetFalse)]
        	api: Option<bool>,
	},

	/// Generate shell completions and print it to stdout
	Completions { shell: clap_complete::Shell },

	/// Generate manpage and print it to stdout
	GenerateManpage {},

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
		platform: Option<PlatformName>,

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
