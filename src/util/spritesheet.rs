use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::fs;
use crate::mod_file::SpriteSheet;
use crate::cache::CacheBundle;

fn create_spritesheet(sheet: &SpriteSheet, working_dir: &Path) -> PathBuf {
	todo!()
}

pub fn get_spritesheet(sheet: &SpriteSheet, working_dir: &Path, cache: &mut Option<CacheBundle>) -> PathBuf {
	if let Some(bundle) = cache {
		// Cache found
		if let Some(p) = bundle.cache.fetch_spritesheet(sheet) {
			let mut cached_file = bundle.archive.by_name(p.to_str().unwrap()).unwrap();

			// Read cached file to buffer
			let mut buf = String::new();
			cached_file.read_to_string(&mut buf).unwrap();

			// Write buffer into working directory, same file name
			let out_path = working_dir.join(p.file_name().unwrap().to_str().unwrap());
			fs::write(&out_path, buf).unwrap();

			return out_path;
		}
	}

	create_spritesheet(sheet, working_dir)
}
