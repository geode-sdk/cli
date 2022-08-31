use std::io::{stdout, stdin, Write};
use std::path::{PathBuf, Path};
use crate::config::Config;
use clap::Subcommand;
use git2::build::RepoBuilder;
use git2::{FetchOptions, RemoteCallbacks, Repository};
use colored::Colorize;

use crate::{fail, warn, info, done};
use crate::NiceUnwrap;

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
	}
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
		Sdk::Update { nightly, stable } => update(config, nightly, stable)
	}
}