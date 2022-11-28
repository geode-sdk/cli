use crate::config::{Config, geode_root};
use crate::input::ask_value;
use std::fs;
use std::path::PathBuf;
use git2::{Repository, ResetType, IndexAddOption, Signature};
use clap::Subcommand;
use crate::logging::NiceUnwrap;
use crate::package::mod_json_from_archive;
use crate::{info, warn, done, fatal};
use colored::Colorize;

#[derive(Subcommand, Debug)]
#[clap(rename_all = "kebab-case")]
pub enum Database {
	/// Initializes your database
	Init,

	/// Lists all entries in your database
	List,

	/// Removes an entry from your database
	Remove {
		/// Mod ID that you want to remove
		id: String
	},

	/// Exports an entry to your database, updating if it always exists
	Export {
		/// Path to the .geode file
		package: PathBuf
	}
}

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
	
	let mut index = repo.index().expect("cannot get the Index file");
	index.add_all(["."].iter(), IndexAddOption::DEFAULT, None).nice_unwrap("Unable to add changes");
	index.write().nice_unwrap("Unable to write changes");

	let sig = Signature::now("Geode CLI", "ilaca314@gmail.com").unwrap();

	let tree = repo.find_tree(index.write_tree().nice_unwrap("Unable to get write tree")).unwrap();
	repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &[&commit]).nice_unwrap("Unable to commit");
}

fn initialize() {
	let database_path = geode_root().join("database");
	if database_path.exists() {
		warn!("Database is already initialized. Exiting.");
		return;
	}

	info!("Welcome to the Database Setup. Here, we will set up your database to be compatible with the Geode index.");
	info!("Before continuing, make a github fork of https://github.com/geode-sdk/database.");

	let fork_url = ask_value("Enter your forked URL", None, true);
	Repository::clone(&fork_url, database_path).nice_unwrap("Unable to clone your repository.");

	done!("Successfully initialized");
}

fn list_mods() {
	let database_path = geode_root().join("database");
	if !database_path.exists() {
		fatal!("Database has not yet been initialized.");
	}

	println!("Mod list:");

	for dir in fs::read_dir(database_path).unwrap() {
		let path = dir.unwrap().path();

		if path.is_dir() && path.join("mod.geode").exists() {
			println!("    - {}", path.file_name().unwrap().to_str().unwrap().bright_green());
		}
	}
}

fn remove_mod(id: String) {
	let database_path = geode_root().join("database");
	if !database_path.exists() {
		fatal!("Database has not yet been initialized.");
	}

	let mod_path = database_path.join(&id);
	if !mod_path.exists() {
		fatal!("Cannot remove mod {}: does not exist", id);
	}

	fs::remove_dir_all(mod_path).nice_unwrap("Unable to remove mod");

	let repo = Repository::open(&database_path).nice_unwrap("Unable to open repository");
	reset_and_commit(&repo, &format!("Remove {}", &id));

	done!("Succesfully removed {}\n", id);
	info!("You will need to force-push this commit yourself. Type: ");
	info!("git -C {} push -f", database_path.to_str().unwrap());
}

fn export_mod(package: PathBuf) {
	let database_path = geode_root().join("database");
	if !database_path.exists() {
		fatal!("Database has not yet been initialized.");
	}

	if !package.exists() {
		fatal!("Path not found");
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

	let mod_path = database_path.join(format!("{}@{}", &mod_id, &major_version));
	if !mod_path.exists() {
		fs::create_dir(&mod_path).nice_unwrap("Unable to create folder");
	}

	fs::copy(package, mod_path.join("mod.geode")).nice_unwrap("Unable to copy mod");

	let repo = Repository::open(&database_path).nice_unwrap("Unable to open repository");
	reset_and_commit(&repo, &format!("Add/Update {}", &mod_id));

	done!("Successfully exported {}@{} to your database\n", mod_id, major_version);
	
	info!("You will need to force-push this commit yourself. Type: ");
	info!("git -C {} push -f", database_path.to_str().unwrap());
}


pub fn subcommand(_config: &mut Config, cmd: Database) {
	match cmd {
		Database::Init => initialize(),
		
		Database::List => list_mods(),

		Database::Remove { id } => remove_mod(id),

		Database::Export { package } => export_mod(package)
	}
}