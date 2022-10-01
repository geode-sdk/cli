use crate::cache::CacheBundle;
use crate::mod_file::BitmapFont;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use texture_packer::exporter::ImageExporter;
use texture_packer::TexturePacker;
use texture_packer::TexturePackerConfig;

use crate::{fatal, NiceUnwrap};
use image::{Rgba, Rgb, RgbaImage, LumaA, EncodableLayout, Pixel, GenericImageView, DynamicImage, GrayAlphaImage};
use signed_distance_field::prelude::*;

struct RenderedChar {
	id: u32,
	img: RgbaImage
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

fn generate_char(font: &BitmapFont, metrics: fontdue::Metrics, data: Vec<u8>) -> Option<RgbaImage> {
	if data.len() == 0 {
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

	Some(RgbaImage::from_fn(
		width,
		height,
		|x, y| {
			Rgba::<u8>([255, 255, 255, data[(x + width*y) as usize]])
		}
	))
}

fn create_font(font: &BitmapFont, working_dir: &Path) -> PathBuf {
	// Destination paths
	let fnt_dst = working_dir.join(font.name.to_owned() + ".fnt");
	let png_dst = working_dir.join(font.name.to_owned() + ".png");

	// Get all characters from the charset format
	let chars: Vec<char> = font.charset
		.as_deref()
		.unwrap_or("32-126,8226")
		.split(',')
		.map(|x| x.split('-').map(|x| x.parse().unwrap()).collect::<Vec<u32>>())
		.map(|x| {
			if x.len() <= 2 {
				*x.first().unwrap() .. *x.last().unwrap() + 1
			} else {
				fatal!("Invalid charset '{}'", font.charset.as_ref().unwrap());
			}
		})
		.flatten()
		.map(|c| char::from_u32(c).unwrap())
		.collect();

	// Read & parse source .ttf file
	let ttf_font = fontdue::Font::from_bytes(
		fs::read(&font.path).unwrap(),
		fontdue::FontSettings::default()
	).unwrap();

	// Rasterize characters from charset using the source font
	let rasterized_chars: Vec<_> = chars.iter().filter_map(|c| {
		let (metrics, data) = ttf_font.rasterize(
			*c,
			font.size as f32
		);

		if let Some(img) = generate_char(font, metrics, data) {
			Some(RenderedChar {
				id: *c as u32,
				img
			})
		} else {
			None
		}	
	}).collect();

	// Determine bounds to create the most efficient packing
	let char_widths = rasterized_chars.iter().map(|c| c.img.width());

	let widest_char: u32 = char_widths.clone().max().unwrap();
	let width_sum: u32 = char_widths.sum();
	let mean_height: f64 = (rasterized_chars.iter().map(|c| c.img.height()).sum::<u32>() as f64) / rasterized_chars.len() as f64;

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
	
	rasterized_chars.iter().for_each(|x| packer.pack_ref(x.id, &x.img).unwrap());

	// test!
	let exporter = ImageExporter::export(&packer).unwrap();
	let mut f = fs::File::create("/Users/jakrillis/cock.png").unwrap();
	exporter.write_to(&mut f, image::ImageFormat::Png).unwrap();

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
