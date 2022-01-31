use std::fs::{File, create_dir_all};
use colored::Colorize;
use std::vec;

use crate::print_error;

use serde::Serialize;

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use image::{self, GenericImageView};
use image::imageops::FilterType;

use texture_packer::importer::ImageImporter;
use texture_packer::exporter::ImageExporter;
use texture_packer::{TexturePacker, TexturePackerConfig};

// its 3, the format is 3
#[derive(Serialize)]
struct GameSheetMeta {
    format: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GameSheetData {
    texture_rotated: bool,
    sprite_size: String,
    sprite_source_size: String,
    texture_rect: String
}

#[derive(Serialize)]
struct GameSheet {
    frames: HashMap<String, GameSheetData>,
    metadata: GameSheetMeta
}

pub struct PackResult {
    pub suffix_removals: u32,
    pub created_files: Vec<String>,
}

impl PackResult {
    fn merge(&mut self, other: &PackResult) {
        self.created_files.append(&mut other.created_files.clone());
    }
}

fn update_suffix(name: &mut String, suffix: &str) -> bool {
    if name.ends_with("-uhd") {
        name.pop();
        name.pop();
        name.pop();
        name.pop();
        name.push_str(suffix);
        name.push_str(".png");
        return true;
    }
    if name.ends_with("-hd") {
        name.pop();
        name.pop();
        name.pop();
        name.push_str(suffix);
        name.push_str(".png");
        return true;
    }
    name.push_str(suffix);
    name.push_str(".png");
    false
}

fn pack_sprites_to_file(in_dir: &Path, out_dir: &Path, name: &String) ->
    Result<PackResult, Box<dyn std::error::Error>>
{
    let mut config = TexturePackerConfig {
        max_width: 0,
        max_height: u32::MAX,
        allow_rotation: true,
        texture_outlines: false,
        border_padding: 1,
        ..Default::default()
    };

    let mut frames = Vec::<(PathBuf, String)>::new();

    let mut suffix_removals = 0u32;

    for walk in walkdir::WalkDir::new(in_dir) {
        let s = walk?;

        if s.metadata()?.is_dir() {
            continue;
        }

        let sprite = PathBuf::from(s.path());
        let mut framename = sprite.file_stem().unwrap().to_str().unwrap_or("").to_string();

        if update_suffix(&mut framename, "") {
            suffix_removals += 1;
        }

        let dim = match image::open(&sprite) {
            Ok(x) => x.dimensions(),
            Err(_) => continue
        };

        if frames.iter().filter(|x| x.1 == framename).collect::<Vec<_>>().len() > 0 {
            print_error!("Duplicate sprite name found: {}", framename);
        } else {
            frames.push((sprite, framename));
        }

        config.max_width += dim.0;
    }

    config.max_width /= (frames.len()/5) as u32;

    let mut packer = TexturePacker::new_skyline(config);

    for (fpath, frame) in frames {
        let texture = match ImageImporter::import_from_file(&fpath) {
            Ok(t) => t,
            Err(_) => continue
        };

        packer.pack_own(frame, texture).expect("Internal error packing files");
    }

    let mut sheet = GameSheet {
        frames: HashMap::new(),
        metadata: GameSheetMeta { format: 3 }
    };

    for (name, frame) in packer.get_frames() {
        sheet.frames.insert(name.to_string(), GameSheetData {
            texture_rotated: frame.rotated,
            sprite_source_size: format!("{{{}, {}}}", frame.source.w, frame.source.h),
            sprite_size: format!("{{{}, {}}}", frame.frame.w, frame.frame.h),
            texture_rect: format!("{{{{{}, {}}}, {{{}, {}}}}}", frame.frame.x, frame.frame.y, frame.frame.w, frame.frame.h),
        });
    }

    create_dir_all(out_dir).unwrap();

    plist::to_file_xml(out_dir.join(format!("{}.plist", name)), &sheet)?;

    let exporter = ImageExporter::export(&packer).unwrap();
    let mut f = File::create(out_dir.join(format!("{}.png", name))).unwrap();
    exporter.write_to(&mut f, image::ImageFormat::Png)?;
    Ok(PackResult {
        suffix_removals: suffix_removals,
        created_files: vec!(format!("{}.plist", name))
    })
}

fn pack_sprites_with_suffix(in_dir: &Path, out_dir: &Path, name: &Option<String>, suffix: &str) -> 
    Result<PackResult, Box<dyn std::error::Error>> 
{
    let mut actual_name = match name {
        Some(s) => s.clone(),
        None => "spritesheet".to_string()
    };
    actual_name.push_str(suffix);
    return pack_sprites_to_file(in_dir, out_dir, &actual_name);
}

fn create_resized_sprites(in_dir: &Path, out_dir: &Path, downscale: u32, suffix: &str) -> Result<(), Box<dyn std::error::Error>> {
    create_dir_all(out_dir).unwrap();

    for walk in walkdir::WalkDir::new(in_dir) {
        let s = walk?;

        if s.metadata()?.is_dir() {
            continue;
        }

        let sprite = PathBuf::from(s.path());
        let mut framename = sprite.file_stem().unwrap().to_str().unwrap_or("").to_string();

        update_suffix(&mut framename, suffix);

        let mut out_file = out_dir.to_path_buf();
        out_file.push(framename);

        let img = match image::io::Reader::open(s.path()) {
            Ok(i) => match i.decode() {
                Ok(im) => im,
                Err(err) => print_error!("Error decoding {}: {}", s.path().to_str().unwrap(), err)
            },
            Err(err) => print_error!("Error resizing {}: {}", s.path().to_str().unwrap(), err)
        };

        let mut resized = img.resize(img.width() / downscale, img.height() / downscale, FilterType::Lanczos3).to_luma8();

        image::imageops::dither(&mut resized, &image::imageops::colorops::BiLevel);

        resized.save(&out_file).unwrap();
    }

    Ok(())
}

pub fn pack_sprites(in_dir: &Path, out_dir: &Path, create_variants: bool, name: Option<String>) -> 
    Result<PackResult, Box<dyn std::error::Error>>
{
    if create_variants {
        create_resized_sprites(in_dir, Path::new(&out_dir.join("tmp_hd")), 2, "-hd").unwrap();
        create_resized_sprites(in_dir, Path::new(&out_dir.join("tmp_low")), 4, "").unwrap();

        let mut res = pack_sprites_with_suffix(in_dir, out_dir, &name, "-uhd").unwrap();
        res.merge(&pack_sprites_with_suffix(Path::new(&out_dir.join("tmp_hd")), out_dir, &name, "-hd").unwrap());
        res.merge(&pack_sprites_with_suffix(Path::new(&out_dir.join("tmp_low")), out_dir, &name, "").unwrap());
        
        Ok(res)
    } else {
        pack_sprites_with_suffix(in_dir, out_dir, &name, "")
    }
}
