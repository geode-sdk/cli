use image::GenericImageView;
use std::fs::File;
use colored::Colorize;

use crate::print_error;

use serde::Serialize;

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

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

pub fn pack_sprites(in_dir: &Path, out_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = TexturePackerConfig {
        max_width: 0,
        max_height: 0,
        allow_rotation: true,
        texture_outlines: false,
        border_padding: 1,
        ..Default::default()
    };

    let mut frames = Vec::<(PathBuf, String)>::new();
    for walk in walkdir::WalkDir::new(in_dir) {
        let s = walk?;

        if s.metadata()?.is_dir() {
            continue;
        }

        let sprite = PathBuf::from(s.path());
        let framename = sprite.file_name().unwrap().to_str().unwrap_or("").to_string();

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
        if config.max_height < dim.1 {
            config.max_height = dim.1;
        }
    }
    config.max_width = (config.max_width as f64 * config.max_height as f64).sqrt() as u32;
    config.max_height = u32::MAX;

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

    plist::to_file_xml(out_dir.join("spritesheet.plist"), &sheet)?;

    let exporter = ImageExporter::export(&packer).unwrap();
    let mut f = File::create(out_dir.join("spritesheet.png")).unwrap();
    exporter.write_to(&mut f, image::ImageFormat::Png)?;

    Ok(())
}
