use std::collections::{BTreeSet, HashMap};
use crate::cache::CacheBundle;
use crate::mod_file::BitmapFont;
use std::fs;
use std::hash::{DefaultHasher, Hasher};
use std::path::Path;
use std::path::PathBuf;
use texture_packer::texture::Texture;
use texture_packer::TexturePacker;
use texture_packer::TexturePackerConfig;

use crate::{done, info, NiceUnwrap, warn};
use image::{Rgba, RgbaImage};

use super::mod_file::ModFileInfo;

struct RenderedChar {
	id: char,
	img: RgbaImage,
}

/*fn smoothstep(start: f32, end: f32, x: f32) -> f32 {
	let x = ((x - start) / (end - start)).clamp(0.0, 1.0);
	x * x * (3.0 - 2.0 * x)
}

fn graya(value: u8) -> Rgba<u8> {
	Rgba::from([value, value, value, 255])
}

fn white_alpha(value: u8) -> Rgba<u8> {
	Rgba::from([0, 0, 0, value])
}

fn gray(value: u8) -> Rgb<u8> {
	Rgb::from([value, value, value])
}

fn gen_sdf(img: &image::DynamicImage) -> SignedDistanceField<F32DistanceStorage> {
	let mut img = img.to_luma_alpha8();
	img.pixels_mut().for_each(|pixel| *pixel = image::LumaA::from([(pixel.0[0] as f32 / pixel.0[1] as f32) as u8, 255]));
	let img = image::DynamicImage::from(img).to_luma8();

	let img2 = binary_image::of_byte_slice(
		img.as_bytes(), img.width() as u16, img.height() as u16);

	let sdf = compute_f32_distance_field(&img2);

	sdf
}

fn gen_outline<T: DistanceStorage>(sdf: SignedDistanceField<T>, size: f32) -> image::RgbaImage {
	let mut img = image::RgbaImage::new(sdf.width.into(), sdf.height.into());

	let ramp = 1.5;

	for y in 0..sdf.height {
		for x in 0..sdf.width {
			let dist = sdf.get_distance(x, y);

			let x = x as u32;
			let y = y as u32;

			let value =
			smoothstep(0.0 - size - ramp, 0.0 - size, dist) -
			smoothstep(0.0 + size, 0.0 + size + ramp, dist);
			// let value = smoothstep(-10.0, 10.0, dist);

			img.put_pixel(x, y, white_alpha((value * 255.0) as u8));
		}
	}

	img
}*/

fn generate_char(font: &BitmapFont, metrics: fontdue::Metrics, data: &[u8]) -> RgbaImage {
	/*let width = metrics.width as u32;
	let height = metrics.height as u32;

	let tmp_char = GrayAlphaImage::from_fn(
		width,
		height,
		|x, y| {
			LumaA::<u8>([255, data[(x + width*y) as usize]])
		}
	);

	let mut input_buf = GrayAlphaImage::new(width + font.outline, height + font.outline);
	image::imageops::overlay(&mut input_buf, &tmp_char, font.outline as i64/ 2, font.outline as i64/ 2);

	let outline = gen_outline(gen_sdf(&DynamicImage::ImageLumaA8(input_buf.clone())), font.outline as f32);
	image::imageops::overlay(&mut input_buf, &outline, 0, 0);

	Some(input_buf)*/

	let width = metrics.width as u32;
	let height = metrics.height as u32;

	RgbaImage::from_fn(width, height, |x, y| {
		Rgba::<u8>([
			font.color.red,
			font.color.green,
			font.color.blue,
			data[(x + width * y) as usize],
		])
	})
}

fn parse_charset(charset: &str) -> Vec<char> {
	let mut chars = BTreeSet::new();

	for token in charset.split(',') {
		let token = token.trim();
		if token.is_empty() {
			continue;
		}

		let mut parts = token.split('-');
		let start = match parts.next().and_then(|part| part.trim().parse::<u32>().ok()) {
			Some(start) => start,
			None => {
				warn!("Invalid charset range '{}'", token);
				continue
			}
		};

		let end = match parts.next() {
			None => start,
			Some(part) => match part.trim().parse::<u32>().ok() {
				Some(end) => end,
				None => {
					warn!("Invalid charset range '{}'", token);
					continue
				}
			}
		};

		if parts.next().is_some() || end < start {
			warn!("Invalid charset range '{}'", token);
			continue;
		}

		chars.extend((start..=end).filter_map(char::from_u32));
	}

	chars.into_iter().collect()
}

fn collect_kerning_pairs(
	ttf_font: &fontdue::Font,
	left_chars: &[(char, u16)],
	all_chars: &[(char, u16)],
	px: f32,
	pairs: &mut Vec<(u32, u32, i32)>,
) {
	for (left, left_idx) in left_chars {
		for (right, right_idx) in all_chars {
			if let Some(kern) = ttf_font.horizontal_kern_indexed(*left_idx, *right_idx, px) {
				let amount = kern.round() as i32;
				if amount != 0 {
					pairs.push((*left as u32, *right as u32, amount));
				}
			}
		}
	}
}

fn compute_kerning_pairs(ttf_font: &fontdue::Font, chars: &[char], px: f32) -> Vec<(u32, u32, i32)> {
	let mut pairs = Vec::new();
	let count = chars.len();
	if count == 0 {
		return pairs;
	}

	let chars: Vec<(char, u16)> = chars
		.iter()
		.map(|c| (*c, ttf_font.lookup_glyph_index(*c)))
		.collect();
	let chars = chars.as_slice();

	let threads = std::thread::available_parallelism()
		.map(|threads| threads.get())
		.unwrap_or(1)
		.min(count.div_ceil(256))
		.max(1);

	if threads <= 1 {
		collect_kerning_pairs(ttf_font, chars, chars, px, &mut pairs);
	} else {
		std::thread::scope(|scope| {
			let mut handles = Vec::new();
			for chunk in chars.chunks(count.div_ceil(threads)) {
				handles.push(scope.spawn(move || {
					let mut pairs = Vec::new();
					collect_kerning_pairs(ttf_font, chunk, chars, px, &mut pairs);
					pairs
				}));
			}

			for handle in handles {
				pairs.extend(handle.join().unwrap());
			}
		});
	}

	pairs.sort();
	pairs
}

fn initialize_font_bundle(
	bundle: &FontBundle,
	font: &BitmapFont,
	ttf_font: &fontdue::Font,
	chars: &[char],
	factor: u32,
	_mod_info: &ModFileInfo,
) -> PathBuf {
	// Scaled font size
	let scaled_size = font.size / factor;
	let px = font.size as f32 / factor as f32;

	// Rasterize characters from charset using the source font
	let mut rasterized_chars: Vec<RenderedChar> = Vec::with_capacity(chars.len());
	let mut empty_glyphs = Vec::new();
	let mut glyphs = Vec::with_capacity(chars.len());

	// Dedup copied glyphs (i.e. Latin 'A' vs Greek 'A' vs Cyrillic 'A')
	let mut glyph_map: HashMap<u64, usize> = HashMap::new();
	for c in chars {
		let (metrics, data) = ttf_font.rasterize(*c, px);
		if data.is_empty() {
			empty_glyphs.push((*c, metrics));
			continue;
		}

		let img = generate_char(font, metrics, &data);
		let mut hasher = DefaultHasher::new();
		hasher.write_u32(img.width());
		hasher.write_u32(img.height());
		hasher.write(img.as_raw());

		let hash = hasher.finish();
		let glyph = match glyph_map.get(&hash).copied() {
			Some(idx) if rasterized_chars[idx].img == img => rasterized_chars[idx].id,
			found => {
				if found.is_none() {
					glyph_map.insert(hash, rasterized_chars.len());
				}
				rasterized_chars.push(RenderedChar { id: *c, img });
				*c
			}
		};

		glyphs.push((*c, metrics, glyph));
	}

	// Determine bounds to create the most efficient packing
	let widest_char: u32 = rasterized_chars.iter().map(|c| c.img.width()).max().unwrap_or(1);
	let total_area: u64 = rasterized_chars
		.iter()
		.map(|c| c.img.width() as u64 * c.img.height() as u64)
		.sum();

	let mut to_pack: Vec<&RenderedChar> = rasterized_chars.iter().collect();
	to_pack.sort_by(|a, b| {
		b.img.height()
			.cmp(&a.img.height())
			.then_with(|| b.img.width().cmp(&a.img.width()))
	});

	let padding: u32 = 2;
	let min_width = widest_char + 2 * padding;

	let estimate = ((total_area as f64 * 1.15).sqrt() as u32).max(min_width);

	let width_candidates = [
		estimate * 8 / 10,
		estimate * 9 / 10,
		estimate,
		estimate * 11 / 10,
		estimate * 12 / 10,
		estimate * 14 / 10,
	];

	let build_packer = |width: u32| {
		let config = TexturePackerConfig {
			max_width: width,
			max_height: u32::MAX,
			allow_rotation: false,
			texture_outlines: false,
			border_padding: padding,
			trim: false,
			..Default::default()
		};

		let mut packer = TexturePacker::new_skyline(config);
		for c in &to_pack {
			packer.pack_ref(c.id, &c.img).unwrap();
		}

		packer
	};


	// Guess best width
	let mut best_packer = None;
	let mut best_score = f64::MIN;
	for w in width_candidates.map(|w| w.max(min_width)) {
		let packer = build_packer(w);

		let pw = packer.width() as f64;
		let ph = packer.height() as f64;
		let fill = total_area as f64 / (pw * ph);
		let aspect = pw.max(ph) / pw.min(ph).max(1.0);
		let score = fill / aspect;
		if score > best_score {
			best_score = score;
			best_packer = Some(packer);
		}
	}

	let packer = best_packer.unwrap_or_else(|| build_packer(estimate));

	let atlas_width = packer.width().max(1);
	let atlas_height = packer.height().max(1);

	// Create .png file
	let mut atlas = RgbaImage::new(atlas_width, atlas_height);
	for c in &rasterized_chars {
		if let Some(frame) = packer.get_frames().get(&c.id) {
			image::imageops::replace(
				&mut atlas,
				&c.img,
				frame.frame.x as i64,
				frame.frame.y as i64,
			);
		}
	}

	let mut file = fs::File::create(&bundle.png).nice_unwrap("Unable to write font .png file");
	image::DynamicImage::ImageRgba8(atlas)
		.write_to(&mut file, image::ImageFormat::Png)
		.unwrap();

	let line_metrics = ttf_font
		.horizontal_line_metrics(scaled_size as f32)
		.unwrap();
	let base = line_metrics.ascent;

	// Get all characters and their metrics (positions in the png)
	let mut all_chars = Vec::with_capacity(glyphs.len() + empty_glyphs.len() + 1);
	for (c, metrics, owner) in &glyphs {
		let frame = packer.get_frame(owner).unwrap();
		all_chars.push((*c as u32, format!(
			"char id={} x={} y={} width={} height={} xoffset={} yoffset={} xadvance={} page=0 chnl=0",
			*c as i32,
			frame.frame.x as i32,
			frame.frame.y as i32,
			frame.frame.w as i32,
			frame.frame.h as i32,
			metrics.xmin,
			base - metrics.ymin as f32 - metrics.height as f32,
			metrics.advance_width,
		)));
	}
	for (c, metrics) in &empty_glyphs {
		all_chars.push((*c as u32, format!(
			"char id={} x=0 y=0 width=0 height=0 xoffset=0 yoffset=0 xadvance={} page=0 chnl=0",
			*c as i32,
			metrics.advance_width
		)));
	}
	// Make sure space is always present even if it's not in the charset
	if !chars.contains(&' ') {
		all_chars.push((32, format!(
			"char id=32 x=0 y=0 width=0 height=0 xoffset=0 yoffset=0 xadvance={} page=0 chnl=0",
			ttf_font.metrics(' ', px).advance_width
		)));
	}
	// Make sure all packings for the same input produce identical output by
	// sorting
	all_chars.sort_unstable_by_key(|(id, _)| *id);
	let all_chars: Vec<String> = all_chars.into_iter().map(|(_, line)| line).collect();

	// Get all kerning pairs
	let all_kerning_pairs: Vec<String> = compute_kerning_pairs(ttf_font, chars, px)
		.into_iter()
		.map(|(first, second, amount)| {
			format!("kerning first={} second={} amount={}", first, second, amount)
		})
		.collect();

	// Create .fnt file
	let fnt_data = format!(
		"info face=\"{font_name}\" size={font_size} bold=0 italic=0 \
		charset=\"\" unicode=1 stretchH=100 smooth=1 aa=1 padding=0,0,0,0 spacing=1,1\n\
		common lineHeight={common_line_height} base={font_base} \
		scaleW={scale_w} scaleH={scale_h} pages=1 packed=0\n\
		page id=0 file=\"{sprite_file_name}.png\"\n\
		chars count={char_count}\n\
		{all_chars}\n\
		kernings count={kerning_count}\n\
		{all_kernings}\n",
		font_name = font.path.file_name().unwrap().to_str().unwrap(),
		font_size = scaled_size,
		common_line_height = line_metrics.new_line_size,
		font_base = base,
		scale_w = atlas_width,
		scale_h = atlas_height,
		sprite_file_name = font.name,
		char_count = all_chars.len(),
		all_chars = all_chars.join("\n"),
		kerning_count = all_kerning_pairs.len(),
		all_kernings = all_kerning_pairs.join("\n"),
	);
	fs::write(&bundle.fnt, fnt_data).nice_unwrap("Unable to write font .fnt file");

	PathBuf::from(font.name.to_owned() + ".png")
}

pub struct FontBundle {
	pub png: PathBuf,
	pub fnt: PathBuf,
}

pub struct FontBundles {
	pub sd: FontBundle,
	pub hd: FontBundle,
	pub uhd: FontBundle,
}

impl FontBundles {
	fn new_file(base: PathBuf) -> FontBundle {
		let mut fnt = base.to_owned();
		fnt.set_extension("fnt");

		FontBundle { png: base, fnt }
	}

	pub fn new(mut base: PathBuf) -> FontBundles {
		base.set_extension("png");

		let base_name = base.file_stem().unwrap().to_str().unwrap().to_string();

		let hd = base.with_file_name(base_name.to_string() + "-hd.png");
		let uhd = base.with_file_name(base_name + "-uhd.png");

		FontBundles {
			sd: FontBundles::new_file(base),
			hd: FontBundles::new_file(hd),
			uhd: FontBundles::new_file(uhd),
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

fn extract_from_cache(
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

fn format_char_ranges(chars: &[char]) -> String {
	let mut ranges: Vec<(u32, u32)> = Vec::new();

	for c in chars {
		let code = *c as u32;
		match ranges.last_mut() {
			Some((_, end)) if code == *end + 1 => *end = code,
			_ => ranges.push((code, code)),
		}
	}

	ranges
		.iter()
		.map(|(start, end)| {
			if start == end {
				format!("{start}")
			} else {
				format!("{start}-{end}")
			}
		})
		.collect::<Vec<String>>()
		.join(", ")
}

pub fn get_font_bundles(
	font: &BitmapFont,
	working_dir: &Path,
	cache: &mut Option<CacheBundle>,
	mod_info: &ModFileInfo,
	shut_up: bool,
) -> FontBundles {
	// todo: we really should add a global verbosity option and logging levels for that

	if !shut_up {
		info!("Fetching font {}", font.name.bright_yellow());
	}

	if let Some(cache_bundle) = cache {
		// Cache found
		if let Some(p) = cache_bundle.cache.fetch_font_bundles(font) {
			let bundles = FontBundles::new(p.to_path_buf());

			// Extract all files
			let success = extract_from_cache(&bundles.sd.png, working_dir, cache_bundle, shut_up)
				&& extract_from_cache(&bundles.sd.fnt, working_dir, cache_bundle, shut_up)
				&& extract_from_cache(&bundles.hd.png, working_dir, cache_bundle, shut_up)
				&& extract_from_cache(&bundles.hd.fnt, working_dir, cache_bundle, shut_up)
				&& extract_from_cache(&bundles.uhd.png, working_dir, cache_bundle, shut_up)
				&& extract_from_cache(&bundles.uhd.fnt, working_dir, cache_bundle, shut_up);

			if success {
				if !shut_up {
					info!("Using cached files");
				}

				done!("Fetched {} from cache", font.name.bright_yellow());
				return bundles;
			} else {
				info!("Failed to extract cached files");
			}
		}
	}

	if !shut_up {
		info!("Font is not cached, building from scratch");
	}
	let bundles = FontBundles::new(working_dir.join(font.name.to_string() + ".png"));

	// Create new font
	let ttf_font = fontdue::Font::from_bytes(
		fs::read(&font.path).nice_unwrap(format!(
			"Failed to read font file: {}",
			font.path.display()
		)),
		fontdue::FontSettings::default()
	)
	.nice_unwrap(format!(
		"Failed to create font from file: {}",
		font.path.display()
	));

	let (chars, missing): (Vec<char>, Vec<char>) = parse_charset(font.charset.as_deref().unwrap_or("32-126,8226"))
		.into_iter()
		.partition(|c| ttf_font.has_glyph(*c));

	if !missing.is_empty() {
		warn!(
			"Skipped {} characters not present in '{}': {}",
			missing.len(),
			font.path.display(),
			format_char_ranges(&missing)
		)
	}

	info!("Creating normal font");
	initialize_font_bundle(&bundles.sd, font, &ttf_font, &chars, 4, mod_info);

	info!("Creating HD font");
	initialize_font_bundle(&bundles.hd, font, &ttf_font, &chars, 2, mod_info);

	info!("Creating UHD font");
	initialize_font_bundle(&bundles.uhd, font, &ttf_font, &chars, 1, mod_info);

	done!("Built font {}", font.name.bright_yellow());
	bundles
}
