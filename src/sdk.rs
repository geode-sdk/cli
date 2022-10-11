use std::fmt::Display;
use std::io::{stdout, stdin, Write};
use std::path::{PathBuf, Path};
use crate::config::Config;
use clap::Subcommand;
use git2::build::RepoBuilder;
use git2::{FetchOptions, RemoteCallbacks, Repository};
use colored::Colorize;
use std::fs;

use crate::{fail, warn, info, done};
use crate::NiceUnwrap;

#[derive(Debug, Clone, PartialEq)]
pub struct Version {
	pub major: u32,
	pub minor: u32,
	pub patch: u32,
}

impl Version {
	pub fn to_string(&self) -> String {
		self.into()
	}
}

impl From<String> for Version {
	fn from(str: String) -> Self {
		let mut iter = str.split(".");
		let (major, minor, patch) = (
			iter.next().and_then(|n| n.parse::<u32>().ok()).nice_unwrap("Invalid major part in version"),
			iter.next().and_then(|n| n.parse::<u32>().ok()).nice_unwrap("Invalid minor part in version"),
			iter.next().and_then(|n| n.parse::<u32>().ok()).nice_unwrap("Invalid patch part in version")
		);
		Version { major, minor, patch }
	}
}

impl From<&Version> for String {
	fn from(ver: &Version) -> Self {
		format!("v{}.{}.{}", ver.major, ver.minor, ver.patch)
	}
}

impl Display for Version {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_fmt(format_args!("v{}.{}.{}", self.major, self.minor, self.patch))
	}
}

#[derive(Subcommand, Debug)]
pub enum Sdk {
	/// Install SDK
	Install {
		/// Uninstall existing SDK and reinstall
		#[clap(long)]
		reinstall: bool,

		/// Path to install
		path: Option<PathBuf>
	},

	/// Uninstall SDK
	Uninstall,

	/// Update SDK
	Update {
		/// Set update branch to nightly
		nightly: bool,

		/// Set update branch to stable
		#[clap(conflicts_with("nightly"))]
		stable: bool
	},

	/// Get SDK version
	Version,
}

fn uninstall(config: &mut Config) -> bool {
	let sdk_path: &Path = if let Some(p) = &config.sdk_path {
		if !p.exists() {
			fail!("SDK path \"{}\" does not exist", p.display());
			return false;
		}

		p
	} else {
		fail!("Unable to uninstall SDK as it is not installed");
		return false;
	};

	warn!("Are you sure you want to uninstall SDK?");
	print!("         (type 'Yes' to proceed) ");

	stdout().flush().unwrap();

	let mut ans = String::new();
	stdin().read_line(&mut ans).unwrap();
	ans = ans.trim().to_string();
	if ans != "Yes" {
		fail!("Aborting");
	    return false;
	}

	if let Err(e) = std::fs::remove_dir_all(sdk_path) {
		fail!("Unable to uninstall SDK: {}", e);
		return false;
	}

	config.sdk_path = None;
	done!("Uninstalled Geode SDK");
	return true;
}

fn install(config: &mut Config, path: PathBuf) {

	let parent = path.parent().unwrap();

	if config.sdk_path.is_some() {
		fail!("SDK is already installed");
		info!("Use --reinstall if you want to remove the existing installation");
	} else if !parent.exists() {
		fail!("Parent folder {} does not exist", parent.display());
	} else if path.exists() {
		fail!("Target path already exists");
	} else {
		info!("Downloading SDK");

		let mut callbacks = RemoteCallbacks::new();
		callbacks.sideband_progress(|x| {
			print!("{} {}", "| Info |".bright_cyan(), std::str::from_utf8(x).unwrap());
			true
		});

		let mut fetch = FetchOptions::new();
		fetch.remote_callbacks(callbacks);

		let mut repo = RepoBuilder::new();
		repo.fetch_options(fetch);


		repo.clone("https://github.com/geode-sdk/geode", &path).nice_unwrap("Could not download SDK");
		
		config.sdk_path = Some(path);
		done!("Successfully installed SDK");
	}
}

fn update(config: &mut Config, nightly: bool, stable: bool) {
	// Switch branch if necessary
	if nightly && stable {
		unreachable!("Contact geode developers");
	} else if nightly {
		config.sdk_nightly = true;
	} else if stable {
		config.sdk_nightly = false;
	}
	let branch = config.sdk_nightly.then_some("nightly").unwrap_or("main");

	// Initialize repository
	let repo = Repository::open(
		config.sdk_path.as_ref().nice_unwrap("Unable to update SDK as it is not installed")
	).nice_unwrap("Could not initialize local SDK repository");

	// Fetch
	let mut remote = repo.find_remote(
		repo.remotes().unwrap().iter().next().unwrap().unwrap()
	).unwrap();

	let mut callbacks = RemoteCallbacks::new();
	callbacks.sideband_progress(|x| {
		print!("{} {}", "| Info |".bright_cyan(), std::str::from_utf8(x).unwrap());
		true
	});

	remote.fetch(
		&[branch],  
		Some(FetchOptions::new().remote_callbacks(callbacks)),
		None
	).nice_unwrap("Could not fetch latest update");

	// Check if can fast-forward
	let fetch_head = repo.find_reference("FETCH_HEAD").unwrap();
	let fetch_commit = repo.reference_to_annotated_commit(&fetch_head).unwrap();

	let merge_analysis = repo.merge_analysis(&[&fetch_commit]).unwrap().0;

	if merge_analysis.is_up_to_date() {
		done!("SDK is up to date");
	} else if !merge_analysis.is_fast_forward() {
		fail!("Cannot update SDK, it has local changes");
		info!("Help: go into the repository and manually do `git pull`");
	} else {
		// Change head and checkout
		let refname = format!("refs/heads/{}", branch);
		let mut reference = repo.find_reference(&refname).unwrap();
		reference.set_target(fetch_commit.id(), "Fast-Forward").unwrap();
		repo.set_head(&refname).unwrap();
		repo.checkout_head(None).unwrap();

		done!("Successfully updated SDK.");
	}
}

pub fn get_version(config: &mut Config) -> Version {
	Version::from(
		fs::read_to_string(
			config.sdk_path.as_ref().nice_unwrap("SDK not installed!").join("VERSION")
		).nice_unwrap("Unable to read SDK version, make sure you are using SDK v0.4.2 or later")
	)
}

pub fn subcommand(config: &mut Config, cmd: Sdk) {
	match cmd {
		Sdk::Install { reinstall, path } => {
			if reinstall {
				if !uninstall(config) {
					return;
				}
			}

			let default_path = if cfg!(target_os = "macos") {
				PathBuf::from("/Users/Shared/Geode/sdk")
			} else {
				todo!();
			};

			install(config, path.unwrap_or(default_path));
		},
		Sdk::Uninstall => { uninstall(config); },
		Sdk::Update { nightly, stable } => update(config, nightly, stable),
		Sdk::Version => info!("Geode SDK version: {}", get_version(config))
	}
}
