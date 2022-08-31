use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::fs;
use crate::mod_file::BitmapFont;
use crate::cache::CacheBundle;

fn create_font(_font: &BitmapFont, _working_dir: &Path) -> PathBuf {
	todo!()
}

pub fn get_font(font: &BitmapFont, working_dir: &Path, cache: &mut Option<CacheBundle>) -> PathBuf {
	if let Some(bundle) = cache {
		// Cache found
		if let Some(p) = bundle.cache.fetch_font(font) {
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

	create_font(font, working_dir)
}
