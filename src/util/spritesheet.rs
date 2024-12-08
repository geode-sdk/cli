use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use image::{imageops, ImageFormat, Pixel, Rgba, Rgba32FImage, RgbaImage};
use serde_json::json;
use texture_packer::exporter::ImageExporter;
use texture_packer::texture::Texture;
use texture_packer::{TexturePacker, TexturePackerConfig};

use crate::cache::CacheBundle;
use crate::{done, info, NiceUnwrap};

use super::mod_file::ModFileInfo;

pub struct Sprite {
	pub name: String,
	pub image: RgbaImage,
}

pub struct SheetBundle {
	pub png: PathBuf,
	pub plist: PathBuf,
}

#[derive(PartialEq)]
pub struct SpriteSheet {
	pub name: String,
	pub files: Vec<PathBuf>,
}

pub struct SheetBundles {
	pub sd: SheetBundle,
	pub hd: SheetBundle,
	pub uhd: SheetBundle,
}

impl SheetBundles {
	fn new_file(base: PathBuf) -> SheetBundle {
		let mut plist = base.to_owned();
		plist.set_extension("plist");

		SheetBundle { png: base, plist }
	}

	pub fn new(mut base: PathBuf) -> SheetBundles {
		base.set_extension("png");

		let base_name = base.file_stem().unwrap().to_str().unwrap().to_string();

		let hd = base.with_file_name(base_name.to_string() + "-hd.png");
		let uhd = base.with_file_name(base_name + "-uhd.png");

		SheetBundles {
			sd: SheetBundles::new_file(base),
			hd: SheetBundles::new_file(hd),
			uhd: SheetBundles::new_file(uhd),
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

pub fn read_to_image(path: &Path) -> RgbaImage {
	image::ImageReader::open(path)
		.nice_unwrap(format!("Error reading sprite '{}'", path.display()))
		.decode()
		.nice_unwrap(format!("Error decoding sprite '{}'", path.display()))
		.to_rgba8()
}

pub fn downscale(img: &mut RgbaImage, factor: u32) {
	if factor == 1 {
		return;
	}

	// premultiply alpha before resampling to fix black halo around transparent edges
	let mut tmp: Rgba32FImage = imageproc::map::map_colors(img, |x| {
		let ch = x.channels();
		let a = ch[3] as f32 / 255.0;
		let r = ch[0] as f32 / 255.0 * a;
		let g = ch[1] as f32 / 255.0 * a;
		let b = ch[2] as f32 / 255.0 * a;
		Rgba([r, g, b, a])
	});

	tmp = imageops::resize(
		&tmp,
		tmp.width() / factor,
		tmp.height() / factor,
		imageops::FilterType::Lanczos3,
	);

	*img = imageproc::map::map_colors(&tmp, |x| {
		let ch = x.channels();
		let a = ch[3];
		let r: u8;
		let g: u8;
		let b: u8;
		if a == 0.0 {
			r = (ch[0] * 255.0).round().clamp(0.0, 255.0) as u8;
			g = (ch[1] * 255.0).round().clamp(0.0, 255.0) as u8;
			b = (ch[2] * 255.0).round().clamp(0.0, 255.0) as u8;
		} else {
			r = (ch[0] / a * 255.0).round().clamp(0.0, 255.0) as u8;
			g = (ch[1] / a * 255.0).round().clamp(0.0, 255.0) as u8;
			b = (ch[2] / a * 255.0).round().clamp(0.0, 255.0) as u8;
		}
		Rgba([r, g, b, (a * 255.0).round().clamp(0.0, 255.0) as u8])
	});
}

fn initialize_spritesheet_bundle(
	bundle: &SheetBundle,
	sheet: &SpriteSheet,
	factor: u32,
	mod_info: &ModFileInfo,
) {
	// Convert all files to sprites
	let mut sprites: Vec<Sprite> = sheet
		.files
		.iter()
		.map(|x| Sprite {
			name: x.file_stem().unwrap().to_str().unwrap().to_string(),
			image: read_to_image(x),
		})
		.collect();

	// Resize
	for sprite in &mut sprites {
		downscale(&mut sprite.image, factor);
	}

	// Determine maximum dimensions of sprite sheet
	let largest_width: u32 = sprites.iter().map(|x| x.image.width()).max().unwrap();

	let mean_height =
		sprites.iter().map(|x| x.image.height() as f64).sum::<f64>() / sprites.len() as f64;
	let width_sum = sprites.iter().map(|x| x.image.width()).sum::<u32>() as f64;

	let mut max_width = (width_sum * mean_height).sqrt() as u32;

	if max_width < largest_width || sprites.len() == 1 {
		max_width = largest_width + 2;
	}

	// Setup texture packer
	let config = TexturePackerConfig {
		max_width,
		max_height: u32::MAX,
		..Default::default()
	};
	let mut texture_packer = TexturePacker::new_skyline(config);

	// Pack textures
	info!("Packing sprites");
	sprites
		.iter()
		.for_each(|x| texture_packer.pack_ref(&x.name, &x.image).unwrap());
	done!("Packed sprites");

	let sprite_name_in_sheet = |name: &String| {
		// `mod.id/sprite.png`
		mod_info.id.to_owned()
			+ "/" + name
			.strip_suffix("-uhd")
			.or_else(|| name.strip_suffix("-hd"))
			.unwrap_or(name)
			+ ".png"
	};

	// Initialize the plist file
	let frame_info = texture_packer.get_frames().iter().map(|(name, frame)| {
		// when the texture is rotated frame width and height are supposed to still be un-rotated
		let real_frame_w: u32 = if frame.rotated { frame.frame.h } else { frame.frame.w };
		let real_frame_h: u32 = if frame.rotated { frame.frame.w } else { frame.frame.h };

		// subtract original center from new center to get the offset
		let offset_x: i32 = (frame.source.x + real_frame_w / 2) as i32 - (frame.source.w / 2) as i32;
		let offset_y: i32 = (frame.source.y + real_frame_h / 2) as i32 - (frame.source.h / 2) as i32;

		(sprite_name_in_sheet(name), json!({
			"spriteOffset": format!("{{{},{}}}", offset_x, -offset_y),
			"spriteSize": format!("{{{},{}}}", real_frame_w, real_frame_h),
			"spriteSourceSize": format!("{{{},{}}}", frame.source.w, frame.source.h),
			"textureRect": format!("{{{{{},{}}},{{{},{}}}}}", frame.frame.x, frame.frame.y, real_frame_w, real_frame_h),
			"textureRotated": frame.rotated,
		}))
	}).collect::<BTreeMap<_, _>>();
	// Using BTreeMap to make sure all packings for the same input produce
	// identical output via sorted keys

	let texture_file_name =
		mod_info.id.to_owned() + "/" + bundle.png.file_name().unwrap().to_str().unwrap();

	// Write plist
	let plist_file = json!({
		"frames": frame_info,
		"metadata": {
			"format": 3,
			"realTextureFileName": texture_file_name,
			"size": format!("{{{},{}}}", texture_packer.width(), texture_packer.height()),
			"textureFileName": texture_file_name
		}
	});

	plist::to_file_xml(&bundle.plist, &plist_file).nice_unwrap("Unable to write to plist file");

	// Write png
	let mut file = std::fs::File::create(&bundle.png).unwrap();

	info!("Exporting");

	let exporter = ImageExporter::export(&texture_packer, None).unwrap();
	exporter
		.write_to(&mut file, ImageFormat::Png)
		.nice_unwrap("Unable to write to png file");

	done!(
		"Successfully packed {}",
		bundle
			.png
			.with_extension("")
			.file_name()
			.unwrap()
			.to_str()
			.unwrap()
			.bright_yellow()
	);
}

fn try_extract_from_cache(
	path: &Path,
	working_dir: &Path,
	cache_bundle: &mut CacheBundle,
	shut_up: bool,
) -> bool {
	let path_name = path.to_str().unwrap();
	if !shut_up {
		info!("Extracting '{}' from cache", path_name);
	}
	cache_bundle.try_extract_cached_into(
		path_name,
		&working_dir.join(path.file_name().unwrap().to_str().unwrap()),
	)
}

fn try_extract_bundles_from_cache(
	sheet: &SpriteSheet,
	working_dir: &Path,
	cache: &mut Option<CacheBundle>,
	shut_up: bool,
) -> Option<SheetBundles> {
	if let Some(cache_bundle) = cache {
		// Cache found
		if let Some(p) = cache_bundle.cache.fetch_spritesheet_bundles(sheet) {
			if !shut_up {
				info!("Using cached files");
			}
			let bundles = SheetBundles::new(p.to_path_buf());

			// Extract all files
			try_extract_from_cache(&bundles.sd.png, working_dir, cache_bundle, shut_up)
				.then_some(())?;
			try_extract_from_cache(&bundles.sd.plist, working_dir, cache_bundle, shut_up)
				.then_some(())?;
			try_extract_from_cache(&bundles.hd.png, working_dir, cache_bundle, shut_up)
				.then_some(())?;
			try_extract_from_cache(&bundles.hd.plist, working_dir, cache_bundle, shut_up)
				.then_some(())?;
			try_extract_from_cache(&bundles.uhd.png, working_dir, cache_bundle, shut_up)
				.then_some(())?;
			try_extract_from_cache(&bundles.uhd.plist, working_dir, cache_bundle, shut_up)
				.then_some(())?;

			done!("Fetched {} from cache", sheet.name.bright_yellow());
			return Some(bundles);
		}
	}
	None
}

pub fn get_spritesheet_bundles(
	sheet: &SpriteSheet,
	working_dir: &Path,
	cache: &mut Option<CacheBundle>,
	mod_info: &ModFileInfo,
	shut_up: bool,
) -> SheetBundles {
	if !shut_up {
		info!("Fetching spritesheet {}", sheet.name.bright_yellow());
	}

	if let Some(cached) = try_extract_bundles_from_cache(sheet, working_dir, cache, shut_up) {
		return cached;
	}

	if !shut_up {
		info!("Sheet is not cached, building from scratch");
	}
	let bundles = SheetBundles::new(working_dir.join(sheet.name.to_string() + ".png"));

	// Initialize all files

	info!("Creating normal sheet");
	initialize_spritesheet_bundle(&bundles.sd, sheet, 4, mod_info);

	info!("Creating HD sheet");
	initialize_spritesheet_bundle(&bundles.hd, sheet, 2, mod_info);

	info!("Creating UHD sheet");
	initialize_spritesheet_bundle(&bundles.uhd, sheet, 1, mod_info);

	done!("Built spritesheet {}", sheet.name.bright_yellow());
	bundles
}
