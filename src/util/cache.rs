use std::collections::HashMap;
use std::io::Read;
use std::fs::File;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::mod_file::BitmapFont;
use crate::spritesheet::SpriteSheet;
use crate::{warn, fatal};

#[derive(Serialize, Deserialize)]
pub struct ResourceCache {
	pub spritesheets: HashMap<String, PathBuf>,
	pub fonts: HashMap<String, PathBuf>
}

pub struct CacheBundle {
	pub cache: ResourceCache,
	pub archive: zip::ZipArchive<File>
}

fn hash_sheet(sheet: &SpriteSheet) -> String {
	let mut hashes: Vec<String> = sheet.files.iter().map(|x| sha256::digest_file(x).unwrap()).collect();
	hashes.sort();
	sha256::digest(hashes.into_iter().collect::<String>())
}

fn hash_font(font: &BitmapFont) -> String {
	sha256::digest(format!("{}|{}|{}|{}",
		font.size,
		font.outline,
		font.charset.clone().unwrap_or(String::new()),
		sha256::digest_file(font.path.clone()).unwrap()
	))
}

pub fn get_cache_bundle(path: &Path) -> Option<CacheBundle> {
	path.exists().then(|| {
		match zip::ZipArchive::new(File::open(path).unwrap()) {
			Ok(mut archive) => {
				let cache: ResourceCache;

				if archive.by_name(".geode_cache").is_ok() {
					let mut cache_data = String::new();
					if archive.by_name(".geode_cache").unwrap().read_to_string(&mut cache_data).is_err() {
						return None;
					}

					cache = ResourceCache::load(cache_data);
				} else {
					cache = ResourceCache::new();
				}

				Some(CacheBundle {
					cache,
					archive
				})			
			},

			Err(e) => {
				warn!("Error reading cache from previous build: {}. Disabling cache for this build", e);
				None
			}
		}
	}).flatten()
}

impl ResourceCache {
	pub fn new() -> ResourceCache {
		ResourceCache {
			spritesheets: HashMap::new(),
			fonts: HashMap::new()
		}
	}

	pub fn load(cache_data: String) -> ResourceCache {
		serde_json::from_str::<ResourceCache>(&cache_data)
			.unwrap_or_else(|e| fatal!("Unable to parse cache file: {}", e))
	}

	pub fn save(&self, path: &Path) {
		std::fs::write(
			path.join(".geode_cache"),
			serde_json::to_string(self).unwrap()
		).unwrap()
	}

	pub fn add_sheet(&mut self, sheet: &SpriteSheet, path: PathBuf) {
		if !path.is_relative() {
			unreachable!("Contact geode developers: {}", path.display());
		}
		self.spritesheets.insert(hash_sheet(sheet), path);
	}

	pub fn add_font(&mut self, font: &BitmapFont, path: PathBuf) {
		if !path.is_relative() {
			unreachable!("Contact geode developers: {}", path.display());
		}
		self.fonts.insert(hash_font(font), path);
	}

	pub fn fetch_spritesheet_bundles(&self, sheet: &SpriteSheet) -> Option<&Path> {
		self.spritesheets.get(&hash_sheet(sheet)).and_then(|x| Some(&**x))
	}

	pub fn fetch_font(&self, font: &BitmapFont) -> Option<&Path> {
		self.fonts.get(&hash_font(font)).and_then(|x| Some(&**x))
	}
}