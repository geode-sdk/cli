use std::io::Write;
use std::fs::File;
use colored::Colorize;

use std::io::BufReader;
use std::io::Read;

use crate::resources::{ModResources, parse_resources, create_resources};
use crate::{throw_error, throw_unwrap, spritesheet};

use serde_json::Value;

use std::fs;
use std::path::{Path, PathBuf};

use std::collections::HashMap;

struct ModInfo {
    //name: String,
    bin_list: Vec<String>,
    id: String,
    resources: ModResources,
}

fn get_extension(platform: &str) -> &'static str {
    if platform == "windows" {
        ".dll"
    } else if platform == "macos" || platform == "ios" {
        ".dylib"
    } else if platform == "android" {
        ".so"
    } else {
        unimplemented!("Unsupported platform");
    }
}

pub fn platform_string() -> &'static str {
    if cfg!(windows) || cfg!(target_os = "linux") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "ios") {
        "ios"
    } else if cfg!(target_os = "android") {
        "android"
    } else {
        unimplemented!("Unsupported platform");
    }
}

pub fn platform_extension() -> &'static str {
    get_extension(platform_string())
}

fn extract_mod_info(mod_json: &Value, mod_json_location: &Path) -> Result<ModInfo, Box<dyn std::error::Error>> {
    let mut bin_list = Vec::new();

    match &mod_json["binary"] {
        Value::String(s) => {
            let mut filename = s.to_string();
            if !filename.ends_with(platform_extension()) {
                filename += platform_extension();
            }
            bin_list.push(filename);
        },
        Value::Object(bin_object) => {
            for i in ["windows", "macos", "android", "ios", "*"] {
                match &bin_object[i] {
                    Value::Null => (),
                    Value::String(s) => {
                        if s == "*" { continue; }
                        let mut filename = s.to_string();
                        if !filename.ends_with(get_extension(i)) {
                            filename += get_extension(i);
                        }
                        bin_list.push(filename);
                    },
                    _ => throw_error!("[mod.json].binary.{} is not a string!", i)
                }
            }

            if bin_list.is_empty() && !bin_object["*"].is_null() {
                match &bin_object["*"] {
                    Value::String(s) => {
                        let mut filename = s.to_string();
                        if !filename.ends_with(platform_extension()) {
                            filename += platform_extension();
                        }
                        bin_list.push(filename);
                    },
                    _ => throw_error!("[mod.json].binary.* is not a string!")
                }
            }
        },
        _ => throw_error!("[mod.json].binary is not a string nor an object!"),
    }

    if bin_list.is_empty() {
        throw_error!("[mod.json].binary is empty!");
    }

    let _name = match &mod_json["name"] {
        Value::String(n) => n,
        Value::Null => throw_error!("[mod.json].name is empty!"),
        _ => throw_error!("[mod.json].name is not a string!")
    };

    let id = match &mod_json["id"] {
        Value::String(n) => n,
        Value::Null => throw_error!("[mod.json].id is empty!"),
        _ => throw_error!("[mod.json].id is not a string!")
    };

    let resources = match &mod_json["resources"] {
        Value::Object(res_object) => parse_resources(res_object, &mod_json_location)?,
        _ => {
            ModResources {
                raw_files: vec!(),
                prefixed_files: vec!(),
                sheets: vec!(),
                fonts: vec!(),
                font_jsons: HashMap::new(),
            }
        }
    };

    Ok(ModInfo {
        //name: name.to_string(),
        bin_list,
        id: id.clone(),
        resources
    })
}

pub fn install_geode_file(
    install_path: &Path,
    out_file: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut target_path = install_path.join("geode");
    target_path = target_path.join("mods");
    if !target_path.exists() {
        fs::create_dir_all(&target_path)?;
    }
    target_path = target_path.join(out_file.to_path_buf().file_name().unwrap());
    fs::copy(out_file, target_path)?;
    println!("{}", 
        format!("Succesfully installed {}", 
            out_file.file_name().unwrap().to_str().unwrap()
        ).cyan().bold()
    );
    Ok(())
}

pub fn create_geode(
    mod_src_dir: &Path,
    exec_dir: &Path,
    out_file: &Path,
    log: bool,
    use_cached_resources: bool,
) -> Result<(), Box<dyn std::error::Error>> {
	let mod_json = serde_json::from_str(&fs::read_to_string(mod_src_dir.join("mod.json"))?)?;
    let modinfo = extract_mod_info(&mod_json, mod_src_dir)?;
    
    println!("{}", 
        format!("Packaging {}", 
            modinfo.id
        ).yellow().bold()
    );

    let tmp_pkg = &dirs::cache_dir().unwrap().join(format!("geode_pkg_{}", modinfo.id));

    if !use_cached_resources {
        fs::remove_dir_all(tmp_pkg).unwrap_or(());
    }

    if !tmp_pkg.exists() {
        fs::create_dir_all(tmp_pkg).unwrap();
    }

    for ref f in modinfo.bin_list {
        if !exec_dir.join(f).exists() {
            throw_error!("Unable to find binary {}, defined in [mod.json].binary", f);
        }
        fs::copy(exec_dir.join(f), tmp_pkg.join(f))?;
    }

    fs::copy(mod_src_dir.join("mod.json"), tmp_pkg.join("mod.json"))?;

    if !tmp_pkg.join("resources").exists() {
        fs::create_dir_all(tmp_pkg.join("resources"))?;
    }

    if mod_src_dir.join("logo.png").exists() {
        println!("Creating variants of logo.png");
        fs::copy(mod_src_dir.join("logo.png"), tmp_pkg.join(modinfo.id.clone() + ".png"))?;
        throw_unwrap!(spritesheet::create_variants_of_sprite(
            &tmp_pkg.join(modinfo.id.clone() + ".png"), tmp_pkg, None
        ), "Could not create sprite variants");
    }

    if mod_src_dir.join("about.md").exists() {
        println!("Found about.md, adding to package");
        fs::copy(mod_src_dir.join("about.md"), tmp_pkg.join("about.md"))?;
    }

    create_resources(
        &modinfo.resources,
        use_cached_resources,
        &modinfo.id,
        &tmp_pkg.join("resources"),
        log
    ).unwrap();

    println!("Zipping...");

    let mut zip = zip::ZipWriter::new(File::create(out_file)?);

    let cwd = std::env::current_dir()?;
    std::env::set_current_dir(tmp_pkg)?;

    let zopts = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for item in walkdir::WalkDir::new(".") {
        let item = item?;

        if !item.metadata()?.is_dir() && item.file_name() != "cache_data.json" {
            let mut file_path = item.path().strip_prefix("./")?.to_str().unwrap().to_string();
            if cfg!(windows) {
                file_path = file_path.replace('/', "\\");
            }
            zip.start_file(file_path, zopts)?;
            zip.write_all(&fs::read(item.path())?)?;
        }
    }

    zip.finish().expect("Unable to package .geode file");
    std::env::set_current_dir(cwd)?;

    println!("{}", 
        format!("Successfully packaged {}", 
            out_file.file_name().unwrap().to_str().unwrap()
        ).yellow().bold()
    );

    Ok(())
}

pub fn amend_geode(
    geode_file: &Path,
    file_to_add: &Path,
    dir_in_zip: &Path
) -> Result<(), Box<dyn std::error::Error>> {
    let mut zip = zip::ZipWriter::new_append(
        File::options().read(true).write(true).open(geode_file)?
    )?;
    
    zip.start_file(
        dir_in_zip.join(file_to_add.file_name().unwrap()).to_str().unwrap(),
        Default::default()
    )?;
    
    let f = File::open(file_to_add)?;
    let mut reader = BufReader::new(f);
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    zip.write_all(&buf)?;

    zip.finish()?;

    Ok(())
}

pub fn edit_geode_interactive(
    geode_file: &Path,
    tmp_folder: Option<PathBuf>
) -> Result<(), Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir().unwrap();

    let tmp_folder = tmp_folder.unwrap_or(cwd.join("geode_interactive"));

    if !tmp_folder.exists() {
        fs::create_dir(&tmp_folder)?;
    }

    let reader = BufReader::new(File::open(geode_file).unwrap());
    let mut zipfile = zip::ZipArchive::new(reader).unwrap();

    zipfile.extract(&tmp_folder)?;


    let mut _b = String::new();

    println!("{}", "Currently unzipped, press enter to repackage".bright_cyan());
    std::io::stdin().read_line(&mut _b).unwrap();

    let tmp_zip = std::env::temp_dir().join(PathBuf::from(geode_file.file_stem().unwrap()));

    let mut zipfile = zip::ZipWriter::new(File::create(&tmp_zip).unwrap());

    std::env::set_current_dir(&tmp_folder).unwrap();

    let zopts = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for item in walkdir::WalkDir::new(".") {
        let item = item.unwrap();

        if !item.metadata().unwrap().is_dir() && item.file_name() != "cache_data.json" {
            let mut file_path = item.path().strip_prefix("./").unwrap().to_str().unwrap().to_string();
            if cfg!(windows) {
                file_path = file_path.replace('/', "\\");
            }

            zipfile.start_file(file_path, zopts).unwrap();
            zipfile.write_all(&fs::read(item.path()).unwrap()).unwrap();
        }
    }

    zipfile.finish().expect("Unable to repackage .geode file");

    std::env::set_current_dir(cwd).unwrap();
    fs::copy(tmp_zip, geode_file)?;

    Ok(())
}

