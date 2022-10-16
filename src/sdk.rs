use crate::config::Config;
use clap::Subcommand;
use colored::Colorize;
use git2::build::RepoBuilder;
use git2::{FetchOptions, RemoteCallbacks, Repository};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use semver::Version;
use serde::Deserialize;
use std::fs;
use std::io::{stdin, stdout, Write};
use std::path::PathBuf;

use crate::NiceUnwrap;
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

	/// Get SDK version
	Version,
}

fn parse_version(str: &str) -> Result<Version, semver::Error> {
	if str.starts_with('v') {
		Version::parse(&str[1..])
	} else {
		Version::parse(str)
	}
}

fn uninstall(config: &mut Config) -> bool {
	let sdk_path = Config::sdk_path();

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

	done!("Uninstalled Geode SDK");
	true
}

fn install(config: &mut Config, path: PathBuf) {
	let parent = path.parent().unwrap();

	if std::env::var("GEODE_SDK").is_ok() {
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
			.nice_unwrap("Could not download SDK");

		// TODO: set GEODE_SDK enviroment var

		info!(
			"Please set the GEODE_SDK enviroment variable to {}",
			path.to_str().unwrap()
		);

		switch_to_tag(config, &repo);

		done!("Successfully installed SDK");
		info!("Use `geode sdk install-binaries` to install pre-built binaries");
	}
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
		.nice_unwrap("Could not initialize local SDK repository");

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
		.nice_unwrap("Could not fetch latest update");

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
			.nice_unwrap("Unable to switch to latest commit");
		info!("Switched to latest commit");
		return;
	}

	let mut latest_version: Option<Version> = None;
	for tag in repo
		.tag_names(None)
		.nice_unwrap("Unable to get SDK tags")
		.iter()
		.flatten()
	{
		if let Ok(version) = parse_version(tag) {
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
	let refname = format!("refs/tags/v{}", latest_version.unwrap());
	repo.set_head(&refname).unwrap();
	repo.checkout_head(None).unwrap();

	done!("Updated head");
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
		target_dir = Config::sdk_path().join(format!("bin/{}", ver));
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
		.nice_unwrap("Unable to get download info from GitHub")
		.json::<GithubReleaseResponse>()
		.nice_unwrap("Unable to parse GitHub response");

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

	if target_url.is_none() {
		fatal!("No binaries found for current platform");
	}

	fs::create_dir_all(&target_dir).nice_unwrap("Unable to create directory for binaries");

	info!("Downloading");

	let temp_zip = target_dir.join("temp.zip");
	download_url(target_url.unwrap(), &temp_zip).nice_unwrap("Downloading binaries failed");

	let file = fs::File::open(&temp_zip).nice_unwrap("Unable to read downloaded ZIP");
	let mut zip = zip::ZipArchive::new(file).nice_unwrap("Downloaded ZIP appears to be corrupted");
	zip.extract(target_dir)
		.nice_unwrap("Unable to unzip downloaded binaries");

	fs::remove_file(temp_zip).nice_unwrap("Unable to clean up downloaded ZIP");

	done!("Binaries installed");
}

pub fn get_version() -> Version {
	Version::parse(
		fs::read_to_string(Config::sdk_path().join("VERSION"))
			.nice_unwrap("Unable to read SDK version, make sure you are using SDK v0.4.2 or later")
			.as_str(),
	)
	.nice_unwrap("Invalid SDK version")
}

pub fn subcommand(config: &mut Config, cmd: Sdk) {
	match cmd {
		Sdk::Install { reinstall, path } => {
			if reinstall && !uninstall(config) {
				return;
			}

			install(
				config,
				path.unwrap_or(if cfg!(target_os = "macos") {
					PathBuf::from("/Users/Shared/Geode/sdk")
				} else {
					dirs::document_dir()
						.nice_unwrap(
							"No default path available! \
						Please provide a path manually",
						)
						.join("Geode")
				}),
			);
		}
		Sdk::Uninstall => {
			uninstall(config);
		}
		Sdk::Update { branch } => update(config, branch),
		Sdk::Version => info!("Geode SDK version: {}", get_version()),
		Sdk::InstallBinaries => install_binaries(config),
	}
}
