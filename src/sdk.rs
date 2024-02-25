use crate::config::Config;
use crate::util::logging::ask_confirm;
use clap::Subcommand;
use colored::Colorize;
use git2::build::{CheckoutBuilder, RepoBuilder};
use git2::{FetchOptions, RemoteCallbacks, Repository};
use path_absolutize::Absolutize;
use reqwest::header::{AUTHORIZATION, USER_AGENT};
use semver::{Prerelease, Version};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(target_os = "macos")]
use crate::launchctl;

#[cfg(windows)]
use winreg::RegKey;

use crate::confirm;
use crate::{done, fail, fatal, info, warn, NiceUnwrap};

#[derive(Deserialize)]
struct GithubReleaseAsset {
	name: String,
	browser_download_url: String,
}

#[derive(Deserialize)]
struct GithubReleaseResponse {
	assets: Vec<GithubReleaseAsset>,
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
	InstallBinaries {
		/// Force platform to install binaries for
		#[clap(long, short)]
		platform: Option<String>,
	},

	/// Uninstall SDK
	Uninstall,

	/// Update SDK
	Update {
		/// Set update branch, can be nightly, stable, or any specific version
		branch: Option<String>,
	},

	/// Change SDK path.
	SetPath {
		/// Move old SDK to new directory
		#[clap(long)]
		r#move: bool,

		/// New SDK path
		path: PathBuf,
	},

	/// Get SDK version
	Version,
}

fn uninstall() -> bool {
	let sdk_path = Config::sdk_path();

	if !ask_confirm(
		&format!("Are you sure you want to uninstall Geode SDK? (Installed at {sdk_path:?})"),
		false,
	) {
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

fn set_sdk_env(path: &Path) -> bool {
	let env_success: bool;

	#[cfg(windows)]
	{
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

			use std::ffi::c_void;

			#[link(name = "user32")]
			extern "system" {
				fn SendMessageTimeoutW(
					hwnd: *const c_void,
					msg: u32,
					wparam: *const c_void,
					lparam: *const c_void,
					flags: u32,
					timeout: u32,
					result: *mut c_void,
				) -> i32;
			}
			unsafe {
				const HWND_BROADCAST: *const c_void = 0xffff as *const c_void;
				const WM_SETTINGCHANGE: u32 = 0x1a;
				const SMTO_ABORTIFHUNG: u32 = 0x2;

				let param =
					['E', 'n', 'v', 'i', 'r', 'o', 'n', 'm', 'e', 'n', 't', '\0'].map(|x| x as i8);
				let param_wide = param.map(|x| x as i16);

				// This should properly update the enviroment variable change to new cmd instances for example,
				// existing terminals will still have to reload though
				// Do it for both narrow and wide because i saw it on stackoverflow idk
				SendMessageTimeoutW(
					HWND_BROADCAST,
					WM_SETTINGCHANGE,
					std::ptr::null(),
					param.as_ptr() as *const c_void,
					SMTO_ABORTIFHUNG,
					1000,
					std::ptr::null_mut(),
				);
				SendMessageTimeoutW(
					HWND_BROADCAST,
					WM_SETTINGCHANGE,
					std::ptr::null(),
					param_wide.as_ptr() as *const c_void,
					SMTO_ABORTIFHUNG,
					1000,
					std::ptr::null_mut(),
				);
			}
		}
	}

	#[cfg(any(target_os = "linux", target_os = "android"))]
	{
		warn!("set_sdk_env is not implemented on linux");
		warn!("Please set the GEODE_SDK enviroment variable yourself to:");
		warn!("{}", path.to_str().unwrap());
		env_success = false;
	}

	#[cfg(target_os = "macos")]
	{
		env_success = launchctl::set_sdk_env(path.to_str().unwrap());
	}

	env_success
}

fn get_sdk_path() -> Option<PathBuf> {
	if std::env::var("GEODE_SDK").is_ok() && Config::try_sdk_path().is_ok() {
		Some(Config::sdk_path())
	} else {
		None
	}
}

fn install(config: &mut Config, path: PathBuf, force: bool) {
	let path = path.absolutize().nice_unwrap("Failed to get absolute path");
	let parent = path.parent().unwrap();

	if !force && std::env::var("GEODE_SDK").is_ok() {
		if Config::try_sdk_path().is_ok() {
			fail!(
				"SDK is already installed at {}",
				Config::sdk_path().display()
			);
			info!("Use --reinstall if you want to remove the existing installation");
			return;
		} else {
			let env_sdk_path = std::env::var("GEODE_SDK").unwrap();
			info!("GEODE_SDK ({env_sdk_path}) is already set, but seems to point to an invalid sdk installation.");
			if !crate::logging::ask_confirm("Do you wish to proceed?", true) {
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
		.nice_unwrap("Could not download SDK");

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

	fetch_repo_info(&repo);

	switch_to_tag(config, &repo);

	done!("Successfully installed SDK");
	info!("Please restart your command line to have the GEODE_SDK enviroment variable set.");
	info!("Use `geode sdk install-binaries` to install pre-built binaries");
}

fn fetch_repo_info(repo: &git2::Repository) -> git2::MergeAnalysis {
	let mut remote = repo.find_remote("origin").unwrap();

	let mut callbacks = RemoteCallbacks::new();
	callbacks.sideband_progress(|x| {
		print!(
			"{} {}",
			"| Info |".bright_cyan(),
			std::str::from_utf8(x).unwrap()
		);
		true
	});

	let res = remote.fetch(
		&["main"],
		Some(FetchOptions::new().remote_callbacks(callbacks)),
		None,
	);
	if res.as_ref().is_err_and(|e| {
		e.message()
			.contains("authentication required but no callback set")
	}) {
		// Setting the authentication callback is kinda jank, just call the git process lmao
		Command::new("git")
			.args(&["fetch", "origin", "main"])
			.current_dir(Config::sdk_path())
			.spawn()
			.nice_unwrap("Could not fetch latest update")
			.wait()
			.nice_unwrap("Could not fetch latest update");
	} else {
		res.nice_unwrap("Could not fetch latest update");
	}

	// Check if can fast-forward
	let fetch_head = repo.find_reference("FETCH_HEAD").unwrap();
	let fetch_commit = repo.reference_to_annotated_commit(&fetch_head).unwrap();

	repo.merge_analysis(&[&fetch_commit]).unwrap().0
}

fn update(config: &mut Config, branch: Option<String>) {
	// Switch branch if necessary
	match branch.as_deref().unwrap_or(if config.sdk_nightly {
		"nightly"
	} else {
		"stable"
	}) {
		"nightly" => {
			info!("Switching to nightly");
			config.sdk_nightly = true;
			config.sdk_version = None;
		}
		"stable" => {
			info!("Switching to stable");
			config.sdk_nightly = false;
			config.sdk_version = None;
		}
		ver => {
			info!("Switching to {}", ver);
			config.sdk_nightly = false;
			config.sdk_version = Some(ver.into());
		}
	};

	info!("Updating SDK");

	// Initialize repository
	let repo = Repository::open(Config::sdk_path())
		.nice_unwrap("Could not initialize local SDK repository");

	// Fetch
	let merge_analysis = fetch_repo_info(&repo);

	if merge_analysis.is_up_to_date() {
		switch_to_tag(config, &repo);

		done!("SDK is up to date");
	} else if merge_analysis.is_fast_forward() {
		// Change head and checkout

		switch_to_tag(config, &repo);

		done!("Successfully updated SDK.");
	} else {
		fail!("Cannot update SDK, it has local changes");
		info!(
			"Go into the repository at {} and manually run `git pull`",
			Config::sdk_path().to_str().unwrap()
		);
	}
}

fn switch_to_ref(repo: &Repository, name: &str) {
	let mut reference = repo.find_reference("refs/heads/main").unwrap();
	let fetch_head = repo.find_reference("FETCH_HEAD").unwrap();
	let fetch_commit = repo.reference_to_annotated_commit(&fetch_head).unwrap();

	reference
		.set_target(fetch_commit.id(), "Fast-Forward")
		.unwrap();
	repo.set_head("refs/heads/main").unwrap();
	repo.checkout_head(Some(CheckoutBuilder::default().force()))
		.nice_unwrap("Failed to checkout main");

	let (obj, refer) = repo.revparse_ext(name).unwrap();
	repo.checkout_tree(&obj, None)
		.nice_unwrap("Unable to checkout tree");
	match refer {
		Some(refer) => repo.set_head(refer.name().unwrap()),
		None => repo.set_head_detached(obj.id()),
	}
	.nice_unwrap("Failed to update head");
}

fn switch_to_tag(config: &mut Config, repo: &Repository) {
	info!("Updating head");

	if config.sdk_nightly {
		switch_to_ref(repo, "refs/heads/main");
		info!("Switched to latest commit");
		return;
	} else if let Some(ver) = config.sdk_version.clone() {
		let ref_str = format!("refs/tags/{ver}");
		if repo.find_reference(ref_str.as_str()).is_err() {
			config.sdk_version = None;
			fatal!("Unable to find tag {ver}");
		}
		switch_to_ref(repo, ref_str.as_str());
		info!("Switched to {ver}");
		return;
	}

	let mut latest_version: Option<Version> = None;
	for tag in repo
		.tag_names(None)
		.nice_unwrap("Unable to get SDK tags")
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

	let tag_name = latest_version.as_ref().unwrap().to_string();
	switch_to_ref(repo, &format!("refs/tags/v{}", tag_name));
	done!("Updated head to v{}", latest_version.unwrap());
}

fn install_binaries(config: &mut Config, platform: Option<String>) {
	let release_tag: String;
	let target_dir: PathBuf;
	if config.sdk_nightly {
		info!("Installing nightly binaries");
		release_tag = "nightly".into();
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

	let res = reqwest::blocking::Client::new()
		.get(format!(
			"https://api.github.com/repos/geode-sdk/geode/releases/tags/{}",
			release_tag
		))
		.header(USER_AGENT, "github_api/1.0")
		.header(
			AUTHORIZATION,
			std::env::var("GITHUB_TOKEN").map_or("".into(), |token| format!("Bearer {token}")),
		)
		.send()
		.nice_unwrap("Unable to get download info from GitHub")
		.json::<GithubReleaseResponse>()
		.nice_unwrap(format!("Could not parse Geode release \"{}\"", release_tag));

	let mut target_url: Option<String> = None;
	let platform = platform
		.as_deref()
		.unwrap_or(env::consts::OS)
		.to_lowercase();
	for asset in res.assets {
		// skip installers
		if asset.name.to_lowercase().contains("installer") {
			continue;
		}

		// skip resources
		if !asset.name.to_lowercase().contains("geode") {
			continue;
		}

		match platform.as_str() {
			"windows" | "linux" | "win" => {
				if asset.name.to_lowercase().contains("-win") {
					target_url = Some(asset.browser_download_url);
					info!("Found binaries for platform Windows");
					break;
				}
			}
			"macos" | "mac" => {
				if asset.name.to_lowercase().contains("-mac") {
					target_url = Some(asset.browser_download_url);
					info!("Found binaries for platform MacOS");
					break;
				}
			}
			os => {
				if asset.name.to_lowercase().contains(&format!("-{os}")) {
					target_url = Some(asset.browser_download_url);
					info!("Found binaries for platform \"{os}\"");
					break;
				}
			}
		}
	}

	if target_url.is_none() {
		fatal!("No binaries found for current platform! ({platform})");
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

fn set_sdk_path(path: PathBuf, do_move: bool) {
	if do_move {
		let old = std::env::var("GEODE_SDK")
			.map(PathBuf::from)
			.nice_unwrap("Cannot locate SDK.");

		assert!(
			old.is_dir(),
			"Internal Error: GEODE_SDK doesn't point to a directory ({}). This \
			might be caused by having run `geode sdk set-path` - try restarting \
			your terminal / computer, or reinstall using `geode sdk install --reinstall`",
			old.display()
		);
		assert!(
			old.join("VERSION").exists(),
			"Internal Error: $GEODE_SDK/VERSION not found. Please reinstall the Geode SDK."
		);
		assert!(
			!path.exists(),
			"Cannot move SDK to existing path {}",
			path.to_str().unwrap()
		);

		fs::rename(old, &path).nice_unwrap("Unable to move SDK");
	} else {
		assert!(
			path.exists(),
			"Cannot set SDK path to nonexistent directory {}",
			path.to_str().unwrap()
		);
		assert!(
			path.is_dir(),
			"Cannot set SDK path to non-directory {}",
			path.to_str().unwrap()
		);
		assert!(
			path.join("VERSION").exists(),
			"{} is either malformed or not a Geode SDK installation",
			path.to_str().unwrap()
		);
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
			.nice_unwrap("Unable to read SDK version, make sure you are using SDK v0.4.2 or later")
			.as_str()
			.trim(),
	)
	.nice_unwrap("Invalid SDK version")
}

pub fn subcommand(config: &mut Config, cmd: Sdk) {
	match cmd {
		Sdk::Install {
			reinstall,
			force,
			path,
		} => {
			if reinstall && !uninstall() && !force {
				return;
			}

			if !force {
				if let Some(path) = get_sdk_path() {
					fatal!(
						"SDK is already installed at {} - if you meant to \
						update the SDK, use `geode sdk update`, or if you \
						want to change the install location use the --reinstall \
						option",
						path.display()
					);
				}
			}

			let actual_path = match path {
				Some(p) => p,
				None => {
					let default_path = if cfg!(target_os = "macos") {
						PathBuf::from("/Users/Shared/Geode/sdk")
					} else {
						dirs::document_dir()
							.nice_unwrap(
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
		Sdk::InstallBinaries { platform } => install_binaries(config, platform),
	}
}
