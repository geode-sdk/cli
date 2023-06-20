use crate::config::{geode_root};
use crate::util::logging::ask_value;
use std::fs;
use std::path::PathBuf;
use git2::{Repository, ResetType, IndexAddOption, Signature};
use crate::package::mod_json_from_archive;
use crate::{info, done, fatal, warn, NiceUnwrap};
use colored::Colorize;

fn reset_and_commit(repo: &Repository, msg: &str) {
	let head = repo.head().nice_unwrap("Broken repository, can't get HEAD");
	if !head.is_branch() {
		fatal!("Broken repository, detached HEAD");
	}

	let mut commit = head.peel_to_commit().unwrap();
	while commit.parent_count() > 0 {
		commit = commit.parent(0).unwrap();
	}

	repo.reset(commit.as_object(), ResetType::Soft, None).nice_unwrap("Unable to refresh repository");
	
	let mut index = repo.index().nice_unwrap("cannot get the Index file");
	index.add_all(["."].iter(), IndexAddOption::DEFAULT, None).nice_unwrap("Unable to add changes");
	index.write().nice_unwrap("Unable to write changes");

	let sig = Signature::now("GeodeBot", "hjfodgames@gmail.com").unwrap();

	let tree = repo.find_tree(index.write_tree().nice_unwrap("Unable to get write tree")).unwrap();
	repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &[&commit]).nice_unwrap("Unable to commit");
}

pub fn indexer_path() -> PathBuf {
	geode_root().join("indexer")
}

pub fn is_initialized() -> bool {
	indexer_path().exists()
}

pub fn initialize() {
	if is_initialized() {
		done!("Indexer is already initialized");
		return;
	}

	info!(
		"Before publishing mods on the Geode index, we need to make you a local \
		Indexer, which handles everything related to publishing mods."
	);
	info!(
		"The mod binaries will be hosted on your Indexer repository, which will \
		automatically request them to be added to the official Mods Index."
	);
	info!(
		"To get started, log in to Github using your account, and go to \
		https://github.com/geode-sdk/indexer/fork to make a fork of the Indexer."
	);

	let fork_url = ask_value("Enter the URL of your fork", None, true);
	Repository::clone(&fork_url, indexer_path()).nice_unwrap("Unable to clone your repository.");

	done!("Successfully initialized Indexer");
}

pub fn list_mods() {
	if !is_initialized() {
		fatal!("Indexer has not been set up - use `geode indexer init` to set it up");
	}

	println!("Published mods:");

	for dir in fs::read_dir(indexer_path()).unwrap() {
		let path = dir.unwrap().path();

		if path.is_dir() && path.join("mod.geode").exists() {
			println!("    - {}", path.file_name().unwrap().to_str().unwrap().bright_green());
		}
	}
}

pub fn remove_mod(id: String) {
	if !is_initialized() {
		fatal!("Indexer has not been set up - use `geode indexer init` to set it up");
	}
	let indexer_path = indexer_path();

	let mod_path = indexer_path.join(&id);
	if !mod_path.exists() {
		fatal!("Cannot remove mod {}: does not exist", id);
	}

	fs::remove_dir_all(mod_path).nice_unwrap("Unable to remove mod");

	let repo = Repository::open(&indexer_path).nice_unwrap("Unable to open repository");
	reset_and_commit(&repo, &format!("Remove {}", &id));

	done!("Succesfully removed {}\n", id);
	info!("You will need to force-push to sync your changes.");
	info!("Run `git -C {} push -f` to sync your changes", indexer_path.to_str().unwrap());
}

pub fn add_mod(package: PathBuf) {
	if !is_initialized() {
		fatal!("Indexer has not been set up - use `geode indexer init` to set it up");
	}
	let indexer_path = indexer_path();

	if !package.exists() {
		fatal!("Package path {} does not exist!", package.display());
	}

	let mut archive = zip::ZipArchive::new(fs::File::open(&package).unwrap()).nice_unwrap("Unable to read package");
	
	let mod_json = mod_json_from_archive(&mut archive);

	let major_version = mod_json
		.get("version")
		.nice_unwrap("[mod.json]: Missing key 'version'")
		.as_str()
		.nice_unwrap("[mod.json].version: Expected string")
		.split(".")
		.next()
		.unwrap()
		.chars()
		.filter(|x| *x != 'v')
		.collect::<String>();

	let mod_id = mod_json_from_archive(&mut archive)
		.get("id")
		.nice_unwrap("[mod.json]: Missing key 'id'")
		.as_str()
		.nice_unwrap("[mod.json].id: Expected string")
		.to_string();

	let mod_path = indexer_path.join(format!("{}@{}", &mod_id, &major_version));
	if !mod_path.exists() {
		fs::create_dir(&mod_path)
			.nice_unwrap("Unable to create directory in local indexer for mod");
	}

	fs::copy(package, mod_path.join("mod.geode"))
		.nice_unwrap("Unable to copy .geode package to local Indexer");

	let repo = Repository::open(&indexer_path)
			.nice_unwrap("Unable to open local Indexer repository");
	reset_and_commit(&repo, &format!("Add/Update {}", &mod_id));

	match repo.find_remote("origin").and_then(|mut o| o.push(&["main"], None)) {
		Ok(_) => {
			done!(
				"Succesfully added {}@{} to your indexer!",
				mod_id, major_version
			);
		},
		Err(_) => {	
			done!("Successfully added {}@{}\n", mod_id, major_version);
			warn!(
				"Unable to automatically sync the changes to Github. \
				You will need to push this commit yourself."
			);
			info!("Run `git -C {} push -f` to push the commit", indexer_path.to_str().unwrap());
		},
	}
	if let Some(url) = repo.find_remote("origin").unwrap().url() {
		info!(
			"To let us know you're ready to publish your mod, please open \
			a Pull Request on your repository: \
			{}/compare/geode-sdk:indexer:main...main",
			url
		);
	}
	else {
		info!(
			"To let us know you're ready to publish your mod, please open \
			a Pull Request on your Indexer fork repository."
		);
	};
}
