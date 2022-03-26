use std::path::Path;
use std::fs::{self, File, create_dir_all};
use crate::throw_error;
use std::vec;
use texture_packer::exporter::ImageExporter;
use texture_packer::{TexturePacker, TexturePackerConfig};
use image::{self, GenericImage, DynamicImage};

fn create_resized_bitmap_font_from_ttf(
    ttf_path: &Path,
    out_dir: &Path,
    name: &str,
    fontsize: u32,
    charset: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    create_dir_all(out_dir).unwrap();

    let true_charset = match charset {
        Some(s) => s,
        None => "32-126"
    };

    let ttf_data = fs::read(ttf_path).unwrap();
    let font = fontdue::Font::from_bytes(ttf_data, fontdue::FontSettings::default()).unwrap();

    let mut rendered_chars: Vec<(u32, fontdue::Metrics, Vec<u8>)> = vec!();

    let mut config = TexturePackerConfig {
        max_width: 0,
        max_height: 0,
        allow_rotation: false,
        texture_outlines: false,
        border_padding: 1,
        ..Default::default()
    };

    let mut heights = Vec::new();

    let mut largest_width = 0u32;
    for range in true_charset.split(",") {
        let start: u32;
        let end: u32;
        if range.contains("-") {
            let nums = range.split("-").collect::<Vec<_>>();
            if nums.len() > 2 {
                throw_error!("Some set in the font's specified charset has more than one '-' which makes no sense");
            }
            start = nums[0].parse().unwrap();
            end = if nums.len() == 2 { nums[1].parse().unwrap() } else { start }
        } else {
            start = range.parse().unwrap();
            end = start;
        }
        for i in start..end {
            let (metrics, px) = font.rasterize(std::char::from_u32(i).unwrap(), fontsize as f32);
            if metrics.width > largest_width as usize {
                largest_width = metrics.width as u32 + 10;
            }
            config.max_width += metrics.width as u32;
            heights.push(metrics.height as f64);
            rendered_chars.push((i, metrics, px));
        }
    }
    let av = heights.iter().sum::<f64>() / heights.len() as f64 + heights.len() as f64;
    config.max_width = (config.max_width as f64 * av).sqrt() as u32;
    config.max_height = u32::MAX;

    // make sure the texture is large enough to 
    // fit the largest input file
    if config.max_width < largest_width {
        // todo: make it create a power of 2
        config.max_width = largest_width;
    }

    let mut packer = TexturePacker::new_skyline(config);

    for (ch, metrics, bitmap) in rendered_chars {
        if metrics.width == 0 || metrics.height == 0 {
            continue;
        }
        let mut texture = DynamicImage::new_rgba8(metrics.width as u32, metrics.height as u32);
        for x in 0..metrics.width {
            for y in 0..metrics.height {
                texture.put_pixel(x as u32, y as u32, image::Rgba([
                    255, 255, 255,
                    bitmap[x + y * metrics.width]
                ]));
            }
        }
        packer.pack_own(ch, texture).expect("Internal error packing font characters");
    }
    
    let exporter = ImageExporter::export(&packer).unwrap();
    let mut f = File::create(out_dir.join(format!("{}.png", name))).unwrap();
    exporter.write_to(&mut f, image::ImageFormat::Png)?;

    Ok(())
}

pub fn create_bitmap_font_from_ttf(
    ttf_path: &Path,
    out_dir: &Path,
    name: Option<&str>,
    fontsize: u32,
    prefix: Option<&str>,
    create_variants: bool,
    charset: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let true_prefix = match prefix {
        Some(s) => s,
        None => ""
    }.to_string();
    let true_name = true_prefix + match name {
        Some(s) => s,
        None => ttf_path.file_name().unwrap().to_str().unwrap()
    };

    if create_variants {
        create_resized_bitmap_font_from_ttf(
            ttf_path, out_dir, (true_name.clone() + "-uhd").as_str(), fontsize, charset
        ).unwrap();
        create_resized_bitmap_font_from_ttf(
            ttf_path, out_dir, (true_name.clone() + "-hd").as_str(), fontsize / 2, charset
        ).unwrap();
        create_resized_bitmap_font_from_ttf(
            ttf_path, out_dir, true_name.as_str(), fontsize / 4, charset
        ).unwrap();
        Ok(())
    } else {
        create_resized_bitmap_font_from_ttf(
            ttf_path, out_dir, (true_name + "-uhd").as_str(), fontsize, charset
        )
    }
}
