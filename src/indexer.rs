use crate::config::geode_root;
use crate::fatal;
use colored::Colorize;
use std::{io, fs};
use std::path::PathBuf;

pub fn indexer_path() -> PathBuf {
	geode_root().join("indexer")
}

pub fn is_initialized() -> bool {
	indexer_path().exists()
}

pub fn list_mods() -> io::Result<()> {
	if !is_initialized() {
		fatal!("Indexer has not been set up - use `geode indexer init` to set it up");
	}

	println!("Published mods:");

	for dir in fs::read_dir(indexer_path()).unwrap() {
		let path = dir.unwrap().path();

		if path.is_dir() && path.join("mod.geode").exists() {
			println!(
				"    - {}",
				path.file_name().unwrap().to_str().unwrap().bright_green()
			);
		}
	}

    Ok(())
}

#[cfg(test)]
mod tests {
	use crate::indexer::indexer_path;

	#[test]
	fn can_not_find_indexer_path() {
		assert_eq!(indexer_path().is_dir(), false);
	}

    #[cfg(target_os = "windows")]
	#[test]
	fn can_find_indexer_path() {
		assert_eq!(indexer_path().is_dir(), true);
	}
}
