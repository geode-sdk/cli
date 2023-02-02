use clap::Subcommand;
use colored::Colorize;
use crate::config::Config;
use git2::build::RepoBuilder;
use git2::{FetchOptions, RemoteCallbacks, Repository, SubmoduleUpdateOptions};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use semver::{Version, Prerelease};
use serde::Deserialize;
use std::fs;
use std::io::{stdin, stdout, Write};
use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
use crate::launchctl;

#[cfg(windows)]
use winreg::RegKey;

use crate::confirm;
use crate::{done, fail, fatal, info, warn};

#[derive(Deserialize)]
struct GithubReleaseAsset {
	name: String,
	browser_download_url: String,
}

#[derive(Deserialize)]
struct GithubReleaseResponse {
	assets: Vec<GithubReleaseAsset>,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum Branch {
	Nightly,
	Stable,
}

fn download_url(
	url: String,
	file_name: &PathBuf,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	let res = reqwest::blocking::get(url)?;
	let mut file = fs::File::create(file_name)?;
	let mut content = std::io::Cursor::new(res.bytes()?);
	std::io::copy(&mut content, &mut file)?;
	Ok(())
}

#[derive(Subcommand, Debug)]
pub enum Sdk {
	/// Install SDK
	Install {
		/// Uninstall existing SDK and reinstall
		#[clap(long)]
		reinstall: bool,

		/// Force install, even if another location exists
		#[clap(long)]
		force: bool,

		/// Path to install
		path: Option<PathBuf>,
	},

	/// Install prebuilt binaries for SDK
	InstallBinaries,

	/// Uninstall SDK
	Uninstall,

	/// Update SDK
	Update {
		/// Set update branch
		#[clap(value_enum)]
		branch: Option<Branch>,
	},

	/// Change SDK path.
	SetPath {
		/// Move old SDK to new directory
		#[clap(long)]
		r#move: bool,

		/// New SDK path
		path: PathBuf
	},

	/// Get SDK version
	Version,
}

fn uninstall() -> bool {
	let sdk_path = Config::sdk_path();

	warn!("Are you sure you want to uninstall SDK? (GEODE_SDK={sdk_path:?})");
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

	done!("Uninstalled Geode SDK");
	true
}

fn update_submodules_recurse(repo: &Repository) -> Result<(), git2::Error> {
	for mut subm in repo.submodules()? {
		let name = subm
			.name()
			.as_ref()
			.map(|s| String::from(*s))
			.unwrap_or_else(|| "<Unknown>".into());

		let mut callbacks = RemoteCallbacks::new();
		callbacks.sideband_progress(|x| {
			print!(
				"{} Cloning submodule {}: {}",
				"| Info |".bright_cyan(),
				name,
				std::str::from_utf8(x).unwrap()
			);
			true
		});

		let mut opts = FetchOptions::new();
		opts.remote_callbacks(callbacks);

		let mut sopts = SubmoduleUpdateOptions::new();
		sopts.fetch(opts);

		subm.update(true, Some(&mut sopts))?;
		update_submodules_recurse(&subm.open()?)?;
	}
	Ok(())
}

fn set_sdk_env(path: &Path) -> bool {
	let env_success: bool;

	#[cfg(windows)] {
		let hklm = RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
		if hklm
			.create_subkey("Environment")
			.map(|(env, _)| env.set_value("GEODE_SDK", &path.to_str().unwrap().to_string()))
			.is_err()
		{
			warn!(
				"Unable to set the GEODE_SDK enviroment variable to {}",
				path.to_str().unwrap()
			);
			env_success = false;
		} else {
			env_success = true;
		}
	}

	#[cfg(target_os = "linux")] {
		warn!("set_sdk_env is not implemented on linux");
		env_success = false;
	}

	#[cfg(target_os = "macos")] {
		env_success = launchctl::set_sdk_env(path.to_str().unwrap());
	}

	env_success
}

fn install(config: &mut Config, path: PathBuf, force: bool) {
	let parent = path.parent().unwrap();

	if !force && std::env::var("GEODE_SDK").is_ok() {
		if Config::try_sdk_path().is_ok() {
			fail!("SDK is already installed");
			info!("Use --reinstall if you want to remove the existing installation");
			return;
		} else {
			let env_sdk_path = std::env::var("GEODE_SDK").unwrap();
			info!("GEODE_SDK ({env_sdk_path}) is already set, but seems to point to an invalid sdk installation.");
			if !crate::logging::ask_confirm(&"Do you wish to proceed?".into(), true) {
				fatal!("Aborting");
			}
		}
	} else if !parent.exists() {
		fail!("Parent folder {} does not exist", parent.display());
		return;
	} else if path.exists() {
		fail!("Target path already exists");
		return;
	}

	info!("Downloading SDK");

	let mut callbacks = RemoteCallbacks::new();
	callbacks.sideband_progress(|x| {
		print!(
			"{} {}",
			"| Info |".bright_cyan(),
			std::str::from_utf8(x).unwrap()
		);
		true
	});

	let mut fetch = FetchOptions::new();
	fetch.remote_callbacks(callbacks);

	let mut builder = RepoBuilder::new();
	builder.fetch_options(fetch);

	let repo = builder
		.clone("https://github.com/geode-sdk/geode", &path)
		.expect("Could not download SDK");

	// update submodules, because for some reason
	// Repository::update_submodules is private
	update_submodules_recurse(&repo).expect("Unable to update submodules!");

	// set GEODE_SDK environment variable;
	if set_sdk_env(&path) {
		info!("Set GEODE_SDK environment variable automatically");
	} else {
		warn!("Unable to set GEODE_SDK environment variable automatically");
		info!(
			"Please set the GEODE_SDK enviroment variable to {}",
			path.to_str().unwrap()
		);
	}

	switch_to_tag(config, &repo);

	done!("Successfully installed SDK");
	info!("Use `geode sdk install-binaries` to install pre-built binaries");
}

fn update(config: &mut Config, branch: Option<Branch>) {
	// Switch branch if necessary
	match branch {
		Some(Branch::Nightly) => {
			info!("Switching to nightly");
			config.sdk_nightly = true;
		}
		Some(Branch::Stable) => {
			info!("Switching to stable");
			config.sdk_nightly = false;
		}
		None => {}
	};

	info!("Updating SDK");

	// Initialize repository
	let repo = Repository::open(Config::sdk_path())
		.expect("Could not initialize local SDK repository");

	// Fetch
	let mut remote = repo
		.find_remote(repo.remotes().unwrap().iter().next().unwrap().unwrap())
		.unwrap();

	let mut callbacks = RemoteCallbacks::new();
	callbacks.sideband_progress(|x| {
		print!(
			"{} {}",
			"| Info |".bright_cyan(),
			std::str::from_utf8(x).unwrap()
		);
		true
	});

	remote
		.fetch(
			&["main"],
			Some(FetchOptions::new().remote_callbacks(callbacks)),
			None,
		)
		.expect("Could not fetch latest update");

	// Check if can fast-forward
	let fetch_head = repo.find_reference("FETCH_HEAD").unwrap();
	let fetch_commit = repo.reference_to_annotated_commit(&fetch_head).unwrap();

	let merge_analysis = repo.merge_analysis(&[&fetch_commit]).unwrap().0;

	if merge_analysis.is_up_to_date() {
		switch_to_tag(config, &repo);

		done!("SDK is up to date");
	} else if !merge_analysis.is_fast_forward() {
		fail!("Cannot update SDK, it has local changes");
		info!(
			"Go into the repository at {} and manually run `git pull`",
			Config::sdk_path().to_str().unwrap()
		);
	} else {
		// Change head and checkout

		switch_to_tag(config, &repo);

		done!("Successfully updated SDK.");
	}
}

fn switch_to_tag(config: &mut Config, repo: &Repository) {
	info!("Updating head");

	if config.sdk_nightly {
		let fetch_head = repo.find_reference("FETCH_HEAD").unwrap();
		let fetch_commit = repo.reference_to_annotated_commit(&fetch_head).unwrap();
		repo.set_head("refs/heads/main").unwrap();
		let mut refer = repo.find_reference("refs/heads/main").unwrap();
		refer.set_target(fetch_commit.id(), "Fast-Forward").unwrap();
		repo.checkout_head(None)
			.expect("Unable to switch to latest commit");
		info!("Switched to latest commit");
		return;
	}

	let mut latest_version: Option<Version> = None;
	for tag in repo
		.tag_names(None)
		.expect("Unable to get SDK tags")
		.iter()
		.flatten()
	{
		if let Ok(version) = Version::parse(tag.strip_prefix('v').unwrap_or(tag)) {
			if latest_version.as_ref().is_none() || &version > latest_version.as_ref().unwrap() {
				latest_version = Some(version);
			}
		}
	}

	if latest_version.is_none() {
		warn!("No SDK tags found, unable to switch");
		return;
	}

	// Change head and checkout
	let refname = format!("refs/tags/v{}", latest_version.as_ref().unwrap());
	repo.set_head(&refname).unwrap();
	repo.checkout_head(None).unwrap();

	done!("Updated head to v{}", latest_version.unwrap());
}

fn install_binaries(config: &mut Config) {
	update(config, None);
	let release_tag: String;
	let target_dir: PathBuf;
	if config.sdk_nightly {
		info!("Installing nightly binaries");
		release_tag = "Nightly".into();
		target_dir = Config::sdk_path().join("bin/nightly");
	} else {
		let ver = get_version();
		info!("Installing binaries for {}", ver);
		release_tag = format!("v{}", ver);
		// remove any -beta or -alpha suffixes as geode cmake doesn't care about those
		let mut stripped_ver = ver.clone();
		stripped_ver.pre = Prerelease::EMPTY;
		target_dir = Config::sdk_path().join(format!("bin/{}", stripped_ver));
	}
	let url = format!(
		"https://api.github.com/repos/geode-sdk/geode/releases/tags/{}",
		release_tag
	);

	let mut headers = HeaderMap::new();
	headers.insert(USER_AGENT, HeaderValue::from_static("github_api/1.0"));

	let res = reqwest::blocking::Client::new()
		.get(&url)
		.headers(headers)
		.send()
		.expect("Unable to get download info from GitHub")
		.json::<GithubReleaseResponse>()
		.expect("Unable to parse GitHub response");

	let mut target_url: Option<String> = None;
	for asset in res.assets {
		#[cfg(target_os = "windows")]
		if asset.name.to_lowercase().contains("win") {
			target_url = Some(asset.browser_download_url);
			info!("Found binaries for platform Windows");
			break;
		}

		#[cfg(target_os = "macos")]
		if asset.name.to_lowercase().contains("mac") {
			target_url = Some(asset.browser_download_url);
			info!("Found binaries for platform MacOS");
			break;
		}
	}

	assert!(target_url.is_some(), "No binaries found for current platform!");

	fs::create_dir_all(&target_dir).expect("Unable to create directory for binaries");

	info!("Downloading");

	let temp_zip = target_dir.join("temp.zip");
	download_url(target_url.unwrap(), &temp_zip).expect("Downloading binaries failed");

	let file = fs::File::open(&temp_zip).expect("Unable to read downloaded ZIP");
	let mut zip = zip::ZipArchive::new(file).expect("Downloaded ZIP appears to be corrupted");
	zip.extract(target_dir)
		.expect("Unable to unzip downloaded binaries");

	fs::remove_file(temp_zip).expect("Unable to clean up downloaded ZIP");

	done!("Binaries installed");
}

fn set_sdk_path(path: PathBuf, do_move: bool) {
	if do_move {
		let old = std::env::var("GEODE_SDK").map(PathBuf::from)
			.expect("Cannot locate SDK.");

		assert!(old.is_dir(), "Internal Error: $GEODE_SDK doesn't point to a directory. Please reinstall the Geode SDK");
		assert!(old.join("VERSION").exists(), "Internal Error: $GEODE_SDK/VERSION not found. Please reinstall the Geode SDK.");
		assert!(!path.exists(), "Cannot move SDK to existing path {}", path.to_str().unwrap());

		fs::rename(old, &path).expect("Unable to move SDK");
	} else {
		assert!(path.exists(), "Cannot set SDK path to nonexistent directory {}", path.to_str().unwrap());
		assert!(path.is_dir(), "Cannot set SDK path to non-directory {}", path.to_str().unwrap());
		assert!(path.join("VERSION").exists(), "{} is either malformed or not a Geode SDK installation", path.to_str().unwrap());
	}

	if set_sdk_env(&path) {
		done!("Successfully set SDK path to {}", path.to_str().unwrap());
	} else {
		fatal!("Unable to change SDK path");
	}
}

pub fn get_version() -> Version {
	Version::parse(
		fs::read_to_string(Config::sdk_path().join("VERSION"))
			.expect("Unable to read SDK version, make sure you are using SDK v0.4.2 or later")
			.as_str(),
	)
	.expect("Invalid SDK version")
}

pub fn subcommand(config: &mut Config, cmd: Sdk) {
	match cmd {
		Sdk::Install { reinstall, force, path } => {
			if reinstall && !uninstall() && !force {
				return;
			}

			let actual_path = match path {
				Some(p) => p,
				None => {
					let default_path = if cfg!(target_os = "macos") {
						PathBuf::from("/Users/Shared/Geode/sdk")
					} else {
						dirs::document_dir()
							.expect(
								"No default path available! \
								Please provide the path manually as an\
								argument to `geode sdk install`",
							)
							.join("Geode")
					};
					if !confirm!(
						"Installing at default path {}. Is this okay?",
						&default_path.to_str().unwrap()
					) {
						fatal!(
							"Please provide the path as an argument \
							to `geode sdk install`"
						);
					}
					default_path
				}
			};

			install(config, actual_path, force);
		}
		Sdk::Uninstall => {
			uninstall();
		}
		Sdk::SetPath { path, r#move } => set_sdk_path(path, r#move),
		Sdk::Update { branch } => update(config, branch),
		Sdk::Version => info!("Geode SDK version: {}", get_version()),
		Sdk::InstallBinaries => install_binaries(config),
	}
}
