use crate::config::geode_root;
use std::fs;
use std::path::PathBuf;
use crate::fatal;
use colored::Colorize;

pub fn indexer_path() -> PathBuf {
	geode_root().join("indexer")
}

pub fn is_initialized() -> bool {
	indexer_path().exists()
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
