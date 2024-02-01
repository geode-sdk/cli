use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::mod_file::BitmapFont;
use crate::spritesheet::SpriteSheet;
use crate::{warn, NiceUnwrap};

#[derive(Serialize, Deserialize)]
pub struct ResourceCache {
	pub spritesheets: HashMap<String, PathBuf>,
	pub fonts: HashMap<String, PathBuf>,
}

pub struct CacheBundle {
	pub cache: ResourceCache,
	pub src: CacheBundleSource,
}

impl CacheBundle {
	pub fn try_extract_cached_into(&mut self, name: &str, output: &PathBuf) -> bool {
		match &mut self.src {
			CacheBundleSource::Archive(archive) => {
				let Ok(mut cached_file) = archive.by_name(name) else {
					return false;
				};

				// Read cached file to buffer
				let mut buf: Vec<u8> = Vec::new();
				let Ok(_) = cached_file.read_to_end(&mut buf) else {
					return false;
				};

				// Write buffer into output directory, same file name
				std::fs::write(output, buf).is_ok()
			}

			CacheBundleSource::Directory(dir) => {
				if dir.join(name) != *output {
					std::fs::copy(dir.join(name), output).is_ok()
				} else {
					false
				}
			}
		}
	}
}

pub enum CacheBundleSource {
	Archive(zip::ZipArchive<File>),
	Directory(PathBuf),
}

fn hash_sheet(sheet: &SpriteSheet) -> String {
	let mut hashes: Vec<String> = sheet
		.files
		.iter()
		.map(|x| sha256::try_digest(x).unwrap())
		.collect();
	hashes.sort();
	sha256::digest(hashes.into_iter().collect::<String>())
}

fn hash_font(font: &BitmapFont) -> String {
	sha256::digest(format!(
		"{}|{}|{}|{}",
		font.size,
		font.outline,
		font.charset.clone().unwrap_or_default(),
		sha256::try_digest(font.path.clone()).unwrap()
	))
}

pub fn get_cache_bundle_from_dir(path: &Path) -> Option<CacheBundle> {
	path.join(".geode_cache")
		.exists()
		.then(|| {
			let cache = ResourceCache::load(
				fs::read_to_string(path.join(".geode_cache")).nice_unwrap("Unable to read cache"),
			);
			Some(CacheBundle {
				cache,
				src: CacheBundleSource::Directory(path.to_path_buf()),
			})
		})
		.flatten()
}

pub fn get_cache_bundle(path: &Path) -> Option<CacheBundle> {
	path.exists()
		.then(|| {
			match zip::ZipArchive::new(
				File::open(path).nice_unwrap("Unable to open cached package"),
			) {
				Ok(mut archive) => {
					let cache: ResourceCache = if archive.by_name(".geode_cache").is_ok() {
						let mut cache_data = String::new();
						if archive
							.by_name(".geode_cache")
							.unwrap()
							.read_to_string(&mut cache_data)
							.is_err()
						{
							return None;
						}

						ResourceCache::load(cache_data)
					} else {
						ResourceCache::new()
					};

					Some(CacheBundle {
						cache,
						src: CacheBundleSource::Archive(archive),
					})
				}

				Err(e) => {
					warn!("Error reading cache from previous build: {}. Disabling cache for this build", e);
					None
				}
			}
		})
		.flatten()
}

impl ResourceCache {
	pub fn new() -> ResourceCache {
		ResourceCache {
			spritesheets: HashMap::new(),
			fonts: HashMap::new(),
		}
	}

	pub fn load(cache_data: String) -> ResourceCache {
		serde_json::from_str::<ResourceCache>(&cache_data).nice_unwrap("Unable to parse cache file")
	}

	pub fn save(&self, path: &Path) {
		std::fs::write(
			path.join(".geode_cache"),
			serde_json::to_string(self).unwrap(),
		)
		.unwrap()
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
		self.spritesheets.get(&hash_sheet(sheet)).map(|x| &**x)
	}

	pub fn fetch_font_bundles(&self, font: &BitmapFont) -> Option<&Path> {
		self.fonts.get(&hash_font(font)).map(|x| &**x)
	}
}
