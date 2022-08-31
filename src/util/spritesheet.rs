use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use image::{RgbaImage, imageops, ImageFormat};
use serde_json::json;
use texture_packer::{TexturePacker, TexturePackerConfig};
use texture_packer::exporter::ImageExporter;

use crate::cache::CacheBundle;
use crate::rgba4444::RGBA4444;
use crate::{info, done};
use crate::NiceUnwrap;

struct Sprite {
	pub name: String,
	pub image: RgbaImage
}

pub struct SheetBundle {
	pub png: PathBuf,
	pub plist: PathBuf
}

pub struct SpriteSheet {
	pub name: String,
	pub files: Vec<PathBuf>,
}

pub struct SheetBundles {
	pub sd: SheetBundle,
	pub hd: SheetBundle,
	pub uhd: SheetBundle
}

impl SheetBundles {
	fn new_file(base: PathBuf) -> SheetBundle {
		let mut plist = base.to_owned();
		plist.set_extension("plist");

		SheetBundle {
			png: base,
			plist
		}
	}

	pub fn new(base: PathBuf) -> SheetBundles {
		let base_name = base.file_stem().unwrap().to_str().unwrap().to_string();

		let hd = base.with_file_name(base_name.to_string() + "-hd.png");
		let uhd = base.with_file_name(base_name + "-uhd.png");

		SheetBundles {
			sd: SheetBundles::new_file(base),
			hd: SheetBundles::new_file(hd),
			uhd: SheetBundles::new_file(uhd)
		}
	}

	pub fn cache_name(&self, working_dir: &Path) -> PathBuf {
		if self.sd.png.is_relative() {
			self.sd.png.to_path_buf()
		} else {
			self.sd.png.strip_prefix(working_dir).unwrap().to_path_buf()
		}
	}
}

fn initialize_spritesheet_bundle(bundle: &SheetBundle, sheet: &SpriteSheet, downscale: u32) {
	// Convert all files to sprites
	let mut sprites: Vec<Sprite> = sheet.files.iter().map(|x| {
		Sprite {
			name: x.file_stem().unwrap().to_str().unwrap().to_string(),
			image: image::io::Reader::open(x)
				       .nice_unwrap(format!("Error reading sprite '{}'", x.display()))
				       .decode()
				       .nice_unwrap(format!("Error decoding sprite '{}'", x.display()))
				       .to_rgba8()

		}
	}).collect();

	// Resize
	for sprite in &mut sprites {
		sprite.image = imageops::resize(
			&sprite.image, 
			sprite.image.width() / downscale,
			sprite.image.height() / downscale,
			imageops::FilterType::Lanczos3
		);

		// Dither
		imageops::dither(&mut sprite.image, &RGBA4444);
	}

	// Determine maximum dimensions of sprite sheet
	let largest_width: u32 = sprites.iter().map(|x| x.image.width()).max().unwrap();
	let width_sum: u32 = sprites.iter().map(|x| x.image.width()).sum();
	let height_sum: u32 = sprites.iter().map(|x| x.image.height()).sum();

	let mut max_width = ((width_sum * height_sum) as f64).sqrt() as u32;
	if max_width < largest_width {
	    max_width = largest_width;
	}

	// Setup texture packer
	let config = TexturePackerConfig {
	    max_width,
	    max_height: u32::MAX,
	    allow_rotation: false,
	    texture_outlines: false,
	    border_padding: 1,
	    ..Default::default()
	};
	let mut texture_packer = TexturePacker::new_skyline(config);

	// Pack textures
	info!("Packing sprites");
	sprites.iter().for_each(|x| texture_packer.pack_ref(&x.name, &x.image).unwrap());
	done!("Packed sprites");

	// Initialize the plist file
	let frame_info = texture_packer.get_frames().iter().map(|(name, frame)| {
		(name.to_string(), json!({
			"textureRotated": frame.rotated,
			"spriteSourceSize": format!("{{{}, {}}}", frame.source.w, frame.source.h),
			"spriteSize": format!("{{{}, {}}}", frame.frame.w, frame.frame.h),
			"textureRect": format!("{{{{{}, {}}}, {{{}, {}}}}}", frame.frame.x, frame.frame.y, frame.frame.w, frame.frame.h),
			"spriteOffset": format!("{{{}, {}}}", frame.source.x, -(frame.source.y as i32)),
		}))
	}).collect::<HashMap<_, _>>();

	// Write plist
	let plist_file = json!({
		"frames": frame_info,
		"metadata": {
			"format": 3
		}
	});

	plist::to_file_xml(&bundle.plist, &plist_file).nice_unwrap("Unable to write to plist file");

	// Write png
	let mut file = std::fs::File::create(&bundle.png).unwrap();

	let exporter = ImageExporter::export(&texture_packer).unwrap();
	exporter.write_to(&mut file, ImageFormat::Png).nice_unwrap("Unable to write to png file");

	done!("Successfully packed {}", bundle.png.with_extension("").file_name().unwrap().to_str().unwrap().bright_yellow());
}

fn extract_from_cache(path: &Path, working_dir: &Path, cache_bundle: &mut CacheBundle) {
	let path_name = path.to_str().unwrap();

	info!("Extracting '{}' from cache", path_name);
	let mut cached_file = cache_bundle.archive.by_name(path_name).unwrap();

	// Read cached file to buffer
	let mut buf: Vec<u8> = Vec::new();
	cached_file.read_to_end(&mut buf).unwrap();

	// Write buffer into working directory, same file name
	let out_path = working_dir.join(path.file_name().unwrap().to_str().unwrap());
	std::fs::write(&out_path, buf).unwrap();
}

pub fn get_spritesheet_bundles(sheet: &SpriteSheet, working_dir: &Path, cache: &mut Option<CacheBundle>) -> SheetBundles {
	info!("Fetching spritesheet {}", sheet.name.bright_yellow());

	if let Some(cache_bundle) = cache {
		// Cache found
		if let Some(p) = cache_bundle.cache.fetch_spritesheet_bundles(sheet) {
			info!("Using cached files");
			let bundles = SheetBundles::new(p.to_path_buf());

			// Extract all files
			extract_from_cache(&bundles.sd.png, working_dir, cache_bundle);
			extract_from_cache(&bundles.sd.plist, working_dir, cache_bundle);
			extract_from_cache(&bundles.hd.png, working_dir, cache_bundle);
			extract_from_cache(&bundles.hd.plist, working_dir, cache_bundle);
			extract_from_cache(&bundles.uhd.png, working_dir, cache_bundle);
			extract_from_cache(&bundles.uhd.plist, working_dir, cache_bundle);

			done!("Fetched {} from cache", sheet.name.bright_yellow());
			return bundles;
		}
	}

	info!("Sheet is not cached, building from scratch");
	let mut bundles = SheetBundles::new(working_dir.join(sheet.name.to_string() + ".png"));
	
	// Initialize all files

	info!("Creating normal sheet");
	initialize_spritesheet_bundle(&mut bundles.sd, sheet, 4);

	info!("Creating HD sheet");
	initialize_spritesheet_bundle(&mut bundles.hd, sheet, 2);

	info!("Creating UHD sheet");
	initialize_spritesheet_bundle(&mut bundles.uhd, sheet, 1);

	done!("Built spritesheet {}", sheet.name.bright_yellow());
	bundles
}
