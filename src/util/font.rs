use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::fs;
use crate::mod_file::BitmapFont;
use crate::cache::CacheBundle;
use texture_packer::TexturePackerConfig;

struct RenderedChar {
	id: u32,
	metrics: fontdue::Metrics,
	data: Vec<u8>,
}

fn create_font(font: &BitmapFont, working_dir: &Path) -> PathBuf {
	// Destination paths
	let fnt_dst = working_dir.join(font.name.to_owned() + ".fnt");
	let png_dst = working_dir.join(font.name.to_owned() + ".png");

	// Font character set or default character set (same as bigFont)
	let charset = font.charset.as_deref().unwrap_or("32-126,8226");

	// Read & parse source .ttf file
	let ttf_font = fontdue::Font::from_bytes(
		fs::read(&font.path).unwrap(),
		fontdue::FontSettings::default()
	).unwrap();

	// Configuration for texture packer, mutable so 
	// max width and height can be figured out from 
	// characters (for optimal packing)
	let mut config = TexturePackerConfig {
		max_width: 0,
		max_height: 0,
		allow_rotation: false,
		texture_outlines: false,
		border_padding: 1,
		trim: false,
		..Default::default()
	};

	// Vector to store the rendered characters in
	let mut rendered_chars: Vec<RenderedChar> = vec!();

	// Load all character info from font with charset
	let mut widest_char: usize = 0;
	for range in charset.split(',') {
		let range_start: u32;
		let range_end: u32;

		// 'a-b'
		if range.contains('-') {
			let nums = range.split('-').collect::<Vec<_>>();

			// If someone writes 'a-b-c' then just let them 
			// as that's equivalent to 'a-c'
			// Note: We might want to change this to be more 
			// strict if someone writes 'a-b-c' accidentally, 
			// although the circumstances in which one would 
			// do that are lost to me

			range_start = nums.first().unwrap().parse().unwrap();
			range_end = nums.last().unwrap().parse().unwrap();
		}
		// Just 'a'
		else {
			range_start = range.parse().unwrap();
			range_end = range_start;
		}
		// Iterate provided range and load characters
		for i in range_start..(range_end + 1) {
			let (metrics, px) = ttf_font.rasterize(
				char::from_u32(i).unwrap(),
				font.size as f32
			);
			
			// Check if this is the widest character so far
			if metrics.width > widest_char {
				widest_char = metrics.width;
			}
			config.max_width += metrics.width as u32;
			
			rendered_chars.push(RenderedChar {
				id: i,
				metrics: metrics,
				data: px
			});
		}
	}

	// Coerce texture packer to make the texture as square-ish as possible
	let average_height =
		rendered_chars.iter().map(|c| c.metrics.height as f64).sum::<f64>() /
		rendered_chars.len() as f64;
	config.max_width = (config.max_width as f64 * average_height).sqrt() as u32;
	config.max_height = u32::MAX;

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

	// Create new font
	create_font(font, working_dir)
}
