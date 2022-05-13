use colored::Colorize;
use std::fs::{self, File, create_dir_all};

use std::vec;

use crate::throw_error;
use crate::dither::RGBA4444;

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
    texture_rect: String,
    sprite_offset: String
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

fn pack_sprites_to_file(in_files: Vec<PathBuf>, out_dir: &Path, name: &str) ->
    Result<PackResult, Box<dyn std::error::Error>>
{
    assert_ne!(in_files.len(), 0, "No files provided to pack_sprites_to_file for {}", name);

    let mut config = TexturePackerConfig {
        max_width: 0,
        max_height: 0,
        allow_rotation: false,
        texture_outlines: false,
        border_padding: 1,
        ..Default::default()
    };

    let mut heights = Vec::new();

    let mut frames = Vec::<(PathBuf, String)>::new();

    let mut suffix_removals = 0u32;

    let mut largest_width = 0;
    for path in in_files {
        if fs::metadata(&path)?.is_dir() {
            continue;
        }

        let mut framename = path.file_stem().unwrap().to_str().unwrap_or("").to_string();

        if update_suffix(&mut framename, "") {
            suffix_removals += 1;
        }

        let dim = match image::open(&path) {
            Ok(x) => x.dimensions(),
            Err(_) => continue
        };

        if frames.iter().any(|x| x.1 == framename) {
            throw_error!("Duplicate sprite name found: {}", framename);
        } else {
            frames.push((path, framename));
        }

        if dim.0 > largest_width {
            largest_width = dim.0 + 10;
        }

        config.max_width += dim.0;
        heights.push(dim.1 as f64);
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
            sprite_offset: format!("{{{}, {}}}", frame.source.x, -(frame.source.y as i32)),
        });
    }

    create_dir_all(out_dir).unwrap();

    plist::to_file_xml(out_dir.join(format!("{}.plist", name)), &sheet)?;

    let exporter = ImageExporter::export(&packer).unwrap();
    let mut f = File::create(out_dir.join(format!("{}.png", name))).unwrap();
    exporter.write_to(&mut f, image::ImageFormat::Png)?;
    Ok(PackResult {
        suffix_removals,
        created_files: vec!(format!("{}.plist", name))
    })
}

fn pack_sprites_with_suffix(in_files: Vec<PathBuf>, out_dir: &Path, name: Option<&str>, suffix: &str) -> 
    Result<PackResult, Box<dyn std::error::Error>> 
{
    let mut actual_name = name.unwrap_or("spritesheet").to_string();
    actual_name.push_str(suffix);
    pack_sprites_to_file(in_files, out_dir, &actual_name)
}

fn create_resized_sprites(
    in_files: &[PathBuf],
    out_dir: &Path,
    downscale: u32,
    prefix: Option<&str>,
    suffix: &str
) -> Result<(), Box<dyn std::error::Error>> {
    create_dir_all(out_dir).unwrap();

    for path in in_files {
        if fs::metadata(path)?.is_dir() {
            continue;
        }

        let mut framename = path.file_stem().unwrap().to_str().unwrap_or("").to_string();

        update_suffix(&mut framename, suffix);
        if let Some(p) = prefix {
            framename = p.to_string() + &framename;
        }

        let mut out_file = out_dir.to_path_buf();
        out_file.push(framename);

        let img = match image::io::Reader::open(path) {
            Ok(i) => match i.decode() {
                Ok(im) => im,
                Err(err) => throw_error!("Error decoding {}: {}", path.to_str().unwrap(), err)
            },
            Err(err) => throw_error!("Error resizing {}: {}", path.to_str().unwrap(), err)
        };

        let mut resized = img.resize(img.width() / downscale, img.height() / downscale, FilterType::Lanczos3).to_rgba8();

        image::imageops::dither(&mut resized, &RGBA4444);

        resized.save(&out_file).unwrap();
    }

    Ok(())
}

fn read_sprites(in_dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(in_dir).unwrap().map(|x| x.unwrap().path()).collect()
}

pub fn pack_sprites(
    in_files: Vec<PathBuf>,
    out_dir: &Path,
    create_variants: bool,
    name: Option<&str>,
    prefix: Option<&str>
) -> Result<PackResult, Box<dyn std::error::Error>>
{   
    if create_variants {
        println!("{}", " -> Creating UHD Textures".yellow().bold());
        create_resized_sprites(&in_files, Path::new(&out_dir.join("tmp_uhd")), 1, prefix, "-uhd").unwrap();
        println!("{}", " -> Creating HD Textures".yellow().bold());
        create_resized_sprites(&in_files, Path::new(&out_dir.join("tmp_hd")),  2, prefix, "-hd").unwrap();
        println!("{}", " -> Creating Low Textures".yellow().bold());
        create_resized_sprites(&in_files, Path::new(&out_dir.join("tmp_low")), 4, prefix, "").unwrap();
        
        println!("{}", " -> Creating UHD Spritesheet".yellow().bold());
        let mut res = pack_sprites_with_suffix(read_sprites(&out_dir.join("tmp_uhd")), out_dir, name, "-uhd").unwrap();
        println!("{}", " -> Creating HD Spritesheet".yellow().bold());
        res.merge(&pack_sprites_with_suffix(read_sprites(&out_dir.join("tmp_hd")), out_dir, name, "-hd").unwrap());
        println!("{}", " -> Creating Low Spritesheet".yellow().bold());
        res.merge(&pack_sprites_with_suffix(read_sprites(&out_dir.join("tmp_low")), out_dir, name, "").unwrap());

        fs::remove_dir_all(&out_dir.join("tmp_uhd")).unwrap();
        fs::remove_dir_all(&out_dir.join("tmp_hd")).unwrap();
        fs::remove_dir_all(&out_dir.join("tmp_low")).unwrap();
        
        Ok(res)
    } else {
        println!("{}", " -> Creating UHD Textures".yellow().bold());
        create_resized_sprites(&in_files, Path::new(&out_dir.join("tmp_uhd")), 1, prefix, "-uhd").unwrap();
        println!("{}", " -> Creating UHD Spritesheet".yellow().bold());
        let res = pack_sprites_with_suffix(read_sprites(&out_dir.join("tmp_uhd")), out_dir, name, "");
        fs::remove_dir_all(&out_dir.join("tmp_uhd")).unwrap();
        res
    }
}

pub fn pack_sprites_in_dir(
    in_dir: &Path,
    out_dir: &Path,
    create_variants: bool,
    name: Option<&str>,
    prefix: Option<&str>
) -> Result<PackResult, Box<dyn std::error::Error>>
{
    pack_sprites(read_sprites(in_dir), out_dir, create_variants, name, prefix)
}

pub fn create_variants_of_sprite(
    file: &Path,
    out_dir: &Path,
    prefix: Option<&str>
) -> Result<(), Box<dyn std::error::Error>> {
    let in_files = vec!(file.to_path_buf());
    create_resized_sprites(&in_files, out_dir, 1, prefix, "-uhd").unwrap();
    create_resized_sprites(&in_files, out_dir, 2, prefix, "-hd").unwrap();
    create_resized_sprites(&in_files, out_dir, 4, prefix, "").unwrap();
    Ok(())
}

pub fn is_image(file: &Path) -> bool {
    match image::io::Reader::open(file) {
        Ok(i) => i.decode().is_ok(),
        Err(_) => false
    }
}