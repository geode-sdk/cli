use crate::cache::CacheBundle;
use crate::mod_file::BitmapFont;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use texture_packer::exporter::ImageExporter;
use texture_packer::texture::Texture;
use texture_packer::TexturePacker;
use texture_packer::TexturePackerConfig;

use crate::{done, geode_assert, info, NiceUnwrap};
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

fn generate_char(
	font: &BitmapFont,
	metrics: fontdue::Metrics,
	data: Vec<u8>,
) -> Option<RgbaImage> {
	if data.is_empty() {
		return None;
	}

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

	Some(RgbaImage::from_fn(width, height, |x, y| {
		Rgba::<u8>([font.color[0], font.color[1], font.color[2], data[(x + width * y) as usize]])
	}))
}

fn initialize_font_bundle(
	bundle: &FontBundle,
	font: &BitmapFont,
	factor: u32,
	_mod_info: &ModFileInfo,
) -> PathBuf {
	// Get all characters from the charset format
	let chars: Vec<char> = font
		.charset
		.as_deref()
		.unwrap_or("32-126,8226")
		.split(',')
		.map(|x| {
			x.split('-')
				.map(|x| x.parse().unwrap())
				.collect::<Vec<u32>>()
		})
		.flat_map(|x| {
			geode_assert!(x.len() <= 2, "Invalid charset '{}'", font.charset.as_ref().unwrap());
			*x.first().unwrap()..*x.last().unwrap() + 1
		})
		.map(|c| char::from_u32(c).unwrap())
		.collect();

	// Scaled font size
	let scaled_size = font.size / factor;

	// Read & parse source .ttf file
	let ttf_font = fontdue::Font::from_bytes(
		fs::read(&font.path).unwrap(),
		fontdue::FontSettings::default(),
	)
	.unwrap();

	// Rasterize characters from charset using the source font
	let rasterized_chars: Vec<_> = chars
		.iter()
		.filter_map(|c| {
			let (metrics, data) = ttf_font.rasterize(*c, scaled_size as f32);

			generate_char(font, metrics, data).map(|img| RenderedChar { id: *c, img })
		})
		.collect();

	// Determine bounds to create the most efficient packing
	let char_widths = rasterized_chars.iter().map(|c| c.img.width());

	let widest_char: u32 = char_widths.clone().max().unwrap();
	let width_sum: u32 = char_widths.sum();
	let mean_height: f64 = (rasterized_chars.iter().map(|c| c.img.height()).sum::<u32>() as f64)
		/ rasterized_chars.len() as f64;

	let mut max_width = (width_sum as f64 * mean_height).sqrt() as u32;

	if max_width < widest_char {
		max_width = widest_char + 2;
	}

	// Configuration for texture packer
	let config = TexturePackerConfig {
		max_width,
		max_height: u32::MAX,
		allow_rotation: false,
		texture_outlines: false,
		border_padding: 20,
		trim: false,
		..Default::default()
	};
	let mut packer = TexturePacker::new_skyline(config);

	rasterized_chars
		.iter()
		.for_each(|x| packer.pack_ref(x.id, &x.img).unwrap());

	// Create .png file
	let exporter = ImageExporter::export(&packer).unwrap();
	let mut f = fs::File::create(&bundle.png).nice_unwrap("Unable to write font .png file");
	exporter.write_to(&mut f, image::ImageFormat::Png).unwrap();

	// Get all characters and their metrics (positions in the png)
	// Add space explicitly because it's empty and not in the frames
	// todo: figure out why space isn't there and how to make sure
	// other space characters don't get omitted
	let mut all_chars = vec![format!(
		"char id=32 x=0 y=0 width=0 height=0 xoffset=0 yoffset=0 xadvance={} page=0 chln=0",
		ttf_font.metrics(' ', scaled_size as f32).advance_width
	)];
	for (name, frame) in packer.get_frames() {
		let metrics = ttf_font.metrics(*name, scaled_size as f32);
		all_chars.push(format!(
			"char id={} x={} y={} width={} height={} xoffset={} yoffset={} xadvance={} page=0 chnl=0",
			*name as i32,
			frame.frame.x as i32,
			frame.frame.y as i32,
			frame.frame.w as i32,
			frame.frame.h as i32,
			metrics.xmin,
			scaled_size as i32 - metrics.height as i32 - metrics.ymin,
			metrics.advance_width as i32
		));
	}
	// Make sure all packings for the same input produce identical output by
	// sorting
	all_chars.sort();

	// Get all kerning pairs
	let mut all_kerning_pairs = rasterized_chars
		.iter()
		.flat_map(|left| {
			rasterized_chars.iter().filter_map(|right| {
				ttf_font
					.horizontal_kern(left.id, right.id, scaled_size as f32)
					.map(|kern| {
						format!(
							"kerning first={} second={} amount={}",
							left.id, right.id, kern as i32
						)
					})
			})
		})
		.collect::<Vec<_>>();
	// Make sure all packings for the same input produce identical output by
	// sorting
	all_kerning_pairs.sort();

	// Create .fnt file
	let line_metrics = ttf_font
		.horizontal_line_metrics(scaled_size as f32)
		.unwrap();
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
		font_base = (-line_metrics.descent + line_metrics.line_gap) as i32,
		scale_w = packer.width(),
		scale_h = packer.height(),
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
) {
	let path_name = path.to_str().unwrap();
	if !shut_up {
		info!("Extracting '{}' from cache", path_name);
	}
	cache_bundle.extract_cached_into(
		path_name,
		&working_dir.join(path.file_name().unwrap().to_str().unwrap()),
	);
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
			if !shut_up {
				info!("Using cached files");
			}
			let bundles = FontBundles::new(p.to_path_buf());

			// Extract all files
			extract_from_cache(&bundles.sd.png, working_dir, cache_bundle, shut_up);
			extract_from_cache(&bundles.sd.fnt, working_dir, cache_bundle, shut_up);
			extract_from_cache(&bundles.hd.png, working_dir, cache_bundle, shut_up);
			extract_from_cache(&bundles.hd.fnt, working_dir, cache_bundle, shut_up);
			extract_from_cache(&bundles.uhd.png, working_dir, cache_bundle, shut_up);
			extract_from_cache(&bundles.uhd.fnt, working_dir, cache_bundle, shut_up);

			done!("Fetched {} from cache", font.name.bright_yellow());
			return bundles;
		}
	}

	if !shut_up {
		info!("Font is not cached, building from scratch");
	}
	let bundles = FontBundles::new(working_dir.join(font.name.to_string() + ".png"));

	// Create new font

	info!("Creating normal font");
	initialize_font_bundle(&bundles.sd, font, 4, mod_info);

	info!("Creating HD font");
	initialize_font_bundle(&bundles.hd, font, 2, mod_info);

	info!("Creating UHD font");
	initialize_font_bundle(&bundles.uhd, font, 1, mod_info);

	done!("Built font {}", font.name.bright_yellow());
	bundles
}
