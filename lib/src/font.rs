use std::path::Path;
use std::fs::{self, File, create_dir_all};
use crate::throw_error;
use std::vec;
use texture_packer::exporter::ImageExporter;
use texture_packer::{TexturePacker, TexturePackerConfig};
use image::{self, Pixel, GenericImage, GenericImageView, DynamicImage};
use texture_packer::texture::Texture;

fn point_in_circle(x: i32, y: i32, r: u32) -> bool {
    return ((x.pow(2) + y.pow(2)) as f64).sqrt() < r as f64;
}

fn create_resized_bitmap_font_from_ttf(
    ttf_path: &Path,
    out_dir: &Path,
    name: &str,
    fontsize: u32,
    charset: Option<&str>,
    outline_dia: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    create_dir_all(out_dir).unwrap();

    let true_charset = match charset {
        Some(s) => s,
        None => "32-126,8226" // same as bigFont.fnt
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
        trim: false,
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

    fn render_char_blend(
        metrics: &fontdue::Metrics,
        bitmap: &Vec<u8>,
        offset_x: u32,
        offset_y: u32,
        luma: u8,
        texture: &mut DynamicImage,
    ) -> () {
        for x in 0..metrics.width {
            for y in 0..metrics.height {
                texture.blend_pixel(x as u32 + offset_x, y as u32 + offset_y, image::Rgba([
                    luma, luma, luma,
                    bitmap[x + y * metrics.width]
                ]));
            }
        }
    }

    fn render_char(
        metrics: &fontdue::Metrics,
        bitmap: &Vec<u8>,
        offset_x: u32,
        offset_y: u32,
        luma: u8,
        texture: &mut DynamicImage,
    ) -> () {
        for x in 0..metrics.width {
            for y in 0..metrics.height {
                texture.put_pixel(x as u32 + offset_x, y as u32 + offset_y, image::Rgba([
                    luma, luma, luma,
                    bitmap[x + y * metrics.width]
                ]));
            }
        }
    }

    let outline = outline_dia / 2;
    let use_shadow = outline > 0;
    let shadow_offset = if use_shadow { outline * 2 + 2 } else { 0 };
    let shadow_pad = if use_shadow { 2 } else { 0 };

    let mut largest_height = 0;

    for (ch, metrics, bitmap) in &rendered_chars {
        if metrics.width == 0 || metrics.height == 0 {
            continue;
        }
        let texture_width = metrics.width as u32 + outline * 2 + shadow_offset + shadow_pad;
        let texture_height = metrics.height as u32 + outline * 2 + shadow_offset + shadow_pad;
        let mut texture = DynamicImage::new_rgba8(texture_width, texture_height);
        let mut outline_texture = DynamicImage::new_rgba8(texture_width, texture_height);
        if texture_height > largest_height {
            largest_height = texture_height;
        }
        if outline > 0 {
            if use_shadow {
                // draw buldged version for shadow
                for x in 0..outline*2 {
                    for y in 0..outline*2 {
                        if point_in_circle(
                            x as i32 - outline as i32,
                            y as i32 - outline as i32,
                            outline
                        ) {
                            render_char_blend(
                                &metrics, &bitmap,
                                x + shadow_offset, y + shadow_offset,
                                0, &mut texture
                            );
                        }
                    }
                }
                // lower opacity of drawn pixels
                for x in 0..GenericImageView::width(&texture) {
                    for y in 0..GenericImageView::height(&texture) {
                        let mut px = texture.get_pixel(x, y);
                        px.channels_mut()[3] = (px.channels()[3] as f32 / 2.7f32) as u8;
                        texture.put_pixel(x, y, px);
                    }
                }
                // if you look at bigFont, you can see the 
                // shadow is slightly blurred
                texture = texture.blur(1.5);
            }
            // draw character itself
            render_char_blend(&metrics, &bitmap, outline, outline, 255, &mut texture);
            // draw outline
            for x in 0..metrics.width {
                for y in 0..metrics.height {
                    let ix = x + y * metrics.width;
                    let alpha = bitmap[ix];
                    let next_alpha = if ix + 1 < metrics.width * metrics.height { bitmap[ix + 1] } else { 0 };
                    let prev_alpha = if ix > 0 { bitmap[ix - 1] } else { 0 };
                    let above_alpha = if ix >= metrics.width { bitmap[ix - metrics.width] } else { 0 };
                    let below_alpha = if ix < metrics.width * (metrics.height - 1) { bitmap[ix + metrics.width] } else { 0 };
                    let on_edge: bool;
                    if alpha == 255 {
                        on_edge =
                            prev_alpha != 255 ||
                            next_alpha != 255 ||
                            above_alpha != 255 ||
                            below_alpha != 255;
                    } else {
                        on_edge =
                            prev_alpha == 255 ||
                            next_alpha == 255 ||
                            above_alpha == 255 ||
                            below_alpha == 255;
                    }
                    if on_edge {
                        imageproc::drawing::draw_filled_circle_mut(
                            &mut outline_texture,
                            (
                                x as i32 + outline as i32,
                                y as i32 + outline as i32
                            ),
                            outline as i32,
                            image::Rgba([0u8, 0u8, 0u8, 255u8])
                        );
                    }
                }
            }
            // https://stackoverflow.com/questions/485800/algorithm-for-drawing-an-anti-aliased-circle
            // anti_alised_matrix[x][y] = point[x][y] / 2 + point[x+1][y]/8 + point[x-1][y]/8 + point[x][y-1]/8 + point[x][y+1]/8;
            let mut antialised_outline_texture = DynamicImage::new_rgba8(
                metrics.width as u32 + outline * 2 + shadow_offset + shadow_pad,
                metrics.height as u32 + outline * 2 + shadow_offset + shadow_pad
            );
            for x in 0..GenericImageView::width(&antialised_outline_texture) {
                for y in 0..GenericImageView::height(&antialised_outline_texture) {
                    antialised_outline_texture.put_pixel(
                        x as u32, y as u32, 
                        image::Rgba([
                            0u8, 0u8, 0u8,
                            outline_texture.get_pixel(x, y).channels()[3] / 2 +
                            if x < GenericImageView::width(&antialised_outline_texture) - 1 {
                                outline_texture.get_pixel(x + 1, y).channels()[3] / 8
                            } else { 0 } +
                            if x > 0 {
                                outline_texture.get_pixel(x - 1, y).channels()[3] / 8
                            } else { 0 } +
                            if y < GenericImageView::height(&antialised_outline_texture) - 1 {
                                outline_texture.get_pixel(x, y + 1).channels()[3] / 8
                            } else { 0 } +
                            if y > 0 {
                                outline_texture.get_pixel(x, y - 1).channels()[3] / 8
                            } else { 0 }
                        ])
                    );
                }
            }
            image::imageops::overlay(&mut texture, &antialised_outline_texture, 0, 0);
        } else {
            render_char(&metrics, &bitmap, outline, outline, 255, &mut texture);
        }
        packer.pack_own(ch, texture).expect("Internal error packing font characters");
    }

    let line_metrics = font.horizontal_line_metrics(fontsize as f32).unwrap();
    let mut fnt_data = format!(
        concat!(
            "info face=\"{font_name}\" size={font_size} bold=0 italic=0 ",
            "charset=\"\" unicode=1 stretchH=100 smooth=1 aa=1 padding=0,0,0,0 spacing=1,1\n",
            "common lineHeight={common_line_height} base={font_base} ",
            "scaleW={scale_w} scaleH={scale_h} pages=1 packed=0\n",
            "page id=0 file=\"{sprite_file_name}\"\n"
        ),
        font_name = ttf_path.file_name().unwrap().to_str().unwrap(),
        font_size = fontsize,
        common_line_height = largest_height as u32,
        font_base = (-line_metrics.descent + line_metrics.line_gap) as i32,
        scale_w = packer.width(),
        scale_h = packer.height(),
        sprite_file_name = format!("{}.png", name)
    );
    let mut fnt_chars_data: Vec<String> = vec!();
    let mut fnt_kernings_data: Vec<String> = vec!();

    fnt_chars_data.push(format!(
        "char id=32 x=0 y=0 width=0 height=0 xoffset=0 yoffset=0 xadvance={} page=0 chnl=0\n",
        font.metrics(' ', fontsize as f32).advance_width
    ));

    for (name, frame) in packer.get_frames() {
        let metrics = font.metrics(std::char::from_u32(**name).unwrap(), fontsize as f32);
        fnt_chars_data.push(format!(
            "char id={} x={} y={} width={} height={} xoffset={} yoffset={} xadvance={} page=0 chnl=0\n",
            name,
            frame.frame.x as i32,
            frame.frame.y as i32,
            frame.frame.w as i32,
            frame.frame.h as i32,
            metrics.xmin,
            fontsize as i32 - metrics.height as i32 - metrics.ymin,
            metrics.advance_width as i32
        ));
    }

    fnt_data.push_str(&format!("chars count={}\n", fnt_chars_data.len()));
    for char_data in fnt_chars_data {
        fnt_data.push_str(&char_data);
    }

    for (ch_left, _, _) in &rendered_chars {
        for (ch_right, _, _) in &rendered_chars {
            let kern = font.horizontal_kern(
                std::char::from_u32(*ch_left).unwrap(),
                std::char::from_u32(*ch_right).unwrap(),
                fontsize as f32
            );
            if kern.is_some() {
                fnt_kernings_data.push(format!(
                    "kerning first={} second={} amount={}\n",
                    ch_left,
                    ch_right,
                    kern.unwrap() as i32
                ));
            }
        }
    }

    fnt_data.push_str(&format!("kernings count={}\n", fnt_kernings_data.len()));
    for kerning_data in fnt_kernings_data {
        fnt_data.push_str(&kerning_data);
    }
    
    let exporter = ImageExporter::export(&packer).unwrap();
    let mut f = File::create(out_dir.join(format!("{}.png", name))).unwrap();
    exporter.write_to(&mut f, image::ImageFormat::Png)?;

    fs::write(out_dir.join(format!("{}.fnt", name)), fnt_data)?;

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
    outline: u32,
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
            ttf_path, out_dir, (true_name.clone() + "-uhd").as_str(), fontsize, charset, outline
        ).unwrap();
        create_resized_bitmap_font_from_ttf(
            ttf_path, out_dir, (true_name.clone() + "-hd").as_str(), fontsize / 2, charset, outline / 2
        ).unwrap();
        create_resized_bitmap_font_from_ttf(
            ttf_path, out_dir, true_name.as_str(), fontsize / 4, charset, outline / 4
        ).unwrap();
        Ok(())
    } else {
        create_resized_bitmap_font_from_ttf(
            ttf_path, out_dir, (true_name + "-uhd").as_str(), fontsize, charset, outline
        )
    }
}
