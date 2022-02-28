use std::io::Write;
use std::fs::File;
use colored::Colorize;
use glob::glob;
use std::time::{Duration, SystemTime};

use crate::{throw_error, throw_unwrap, spritesheet};

use serde_json::{Value, json};

use std::fs;
use std::path::{Path, PathBuf};

use std::collections::HashMap;

struct GameSheet {
    name: String,
    files: Vec<PathBuf>,
}

struct ModResources {
    files: Vec<PathBuf>,
    sheets: Vec<GameSheet>,
}

struct ModInfo {
    //name: String,
    bin_list: Vec<String>,
    id: String,
    resources: ModResources,
}

struct CacheData {
    latest_gamesheet_file: HashMap<String, Duration>,
}

impl CacheData {
    fn parse_json(&mut self, file: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let json: Value = serde_json::from_str(&fs::read_to_string(file)?)?;

        for (k, v) in json.as_object().unwrap() {
            let time = Duration::from_secs(v.as_u64().unwrap());
            self.latest_gamesheet_file.insert(k.to_string(), time);
        }

        Ok(())
    }

    fn to_json_string(&self) -> String {
        let mut json = json!({});
        for (k, v) in &self.latest_gamesheet_file {
            json[k] = serde_json::to_value(v.as_secs()).unwrap();
        }
        json.to_string()
    }

    fn are_any_of_these_later(&mut self, sheet: &str, files: &[PathBuf]) -> Result<bool, Box<dyn std::error::Error>> {
        if files.len() == 0 {
            return Ok(true);
        }
        let mut res = false;
        for file in files {
            let modified_date = fs::metadata(file)?.modified()?.duration_since(SystemTime::UNIX_EPOCH)?;

            if !self.latest_gamesheet_file.contains_key(sheet) ||
                modified_date.as_secs() > self.latest_gamesheet_file[sheet].as_secs()
            {
                self.latest_gamesheet_file.insert(sheet.to_string(), modified_date);
                res = true;
            }
        }
        Ok(res)
    }
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

fn extract_mod_info(mod_json: &Value, mod_json_location: &PathBuf) -> Result<ModInfo, Box<dyn std::error::Error>> {
    let mut bin_list = Vec::new();


    match mod_json["binary"].clone() {
        Value::String(s) => {
            let mut filename = s.to_string();
            if !filename.ends_with(platform_extension()) {
                filename += platform_extension();
            }
            bin_list.push(filename);
        },
        Value::Object(bin_object) => {
            for i in ["windows", "macos", "android", "ios"] {
                match &bin_object[i] {
                    Value::Null => (),
                    Value::String(s) => {
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

    let name = match &mod_json["name"] {
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
        Value::Object(res_object) => {
            let mut files: Vec<PathBuf> = vec![];
            let mut sheets: Vec<GameSheet> = vec![];

            for (key, value) in res_object {
                match key.as_str() {
                    "files" => {
                        for path in value.as_array().ok_or("[mod.json].resources.files is not an array!")? {
                            if path.is_string() {
                                let mut search_path = Path::new(&path.as_str().unwrap()).to_path_buf();
                                if search_path.is_relative() {
                                    search_path = mod_json_location.join(search_path);
                                }
                                files.extend(
                                    glob(search_path.to_str().unwrap())
                                    ?.map(|x| x.unwrap())
                                );
                            } else {
                                throw_error!("[mod.json].resources.files: Expected item to be 'string', but it was not");
                            }
                        }
                    },
                    "spritesheets" => {
                        for (sheet_name, sheet_files) in value.as_object().unwrap() {
                            let mut sheet_paths: Vec<PathBuf> = vec!();
                            for path in sheet_files.as_array().ok_or(format!("[mod.json].resources.spritesheets.{} is not an array!", sheet_name).as_str())? {
                                if path.is_string() {
                                    let mut search_path = Path::new(&path.as_str().unwrap()).to_path_buf();
                                    if search_path.is_relative() {
                                        search_path = mod_json_location.join(search_path);
                                    }
                                    sheet_paths.extend(
                                        glob(search_path.to_str().unwrap())
                                        ?.map(|x| x.unwrap())
                                    );
                                } else {
                                    throw_error!("[mod.json].resources.spritesheets.{}: Expected item to be 'string', but it was not", sheet_name);
                                }
                            }
                            sheets.push(GameSheet {
                                name: sheet_name.clone(),
                                files: sheet_paths,
                            });
                        }
                    },
                    _ => {
                        throw_error!("[mod.json].resources: Unknown key {}", key);
                    }
                }
            }

            ModResources {
                files: files,
                sheets: sheets,
            }
        },
        _ => {
            ModResources {
                files: vec!(),
                sheets: vec!(),
            }
        }
    };

    Ok(ModInfo {
        //name: name.to_string(),
        bin_list: bin_list,
        id: id.clone(),
        resources: resources
    })
}

pub fn install_geode(
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
    resource_dir: &Path,
    exec_dir: &Path,
    out_file: &Path,
    log: bool,
    use_cached_resources: bool
) -> Result<(), Box<dyn std::error::Error>> {
	let mod_json = serde_json::from_str(&fs::read_to_string(resource_dir.join("mod.json"))?)?;
    let modinfo = extract_mod_info(&mod_json, &resource_dir.to_path_buf())?;

    println!("{}", 
        format!("Packaging {}", 
            modinfo.id
        ).yellow().bold()
    );

    let tmp_pkg = &std::env::temp_dir().join(format!("geode_pkg_{}", modinfo.id));

    let mut cache_data = CacheData {
        latest_gamesheet_file: HashMap::new()
    };


    if !use_cached_resources {
        fs::remove_dir_all(tmp_pkg).unwrap_or(());
    }

    if tmp_pkg.exists() && use_cached_resources {
        for resource_entry in fs::read_dir(tmp_pkg)? {
            let entry_path = resource_entry?.path();
            if entry_path.is_dir() {
                if 
                    entry_path.file_name().unwrap() == "resources" &&
                    entry_path.join("cache_data.json").exists()
                {
                    cache_data.parse_json(&entry_path.join("cache_data.json"))?;
                    continue;
                }
                fs::remove_dir_all(entry_path)?;
            } else {
                fs::remove_file(entry_path)?;
            }
        }
    } else {
        fs::create_dir(tmp_pkg)?;
    }

    for ref f in modinfo.bin_list {
        if !exec_dir.join(f).exists() {
            throw_error!("Unable to find binary {}, defined in [mod.json].binary", f);
        }

        fs::copy(exec_dir.join(f), tmp_pkg.join(f))?;
    }

    fs::copy(resource_dir.join("mod.json"), tmp_pkg.join("mod.json"))?;

    if !tmp_pkg.join("resources").exists() {
        fs::create_dir_all(tmp_pkg.join("resources"))?;
    }

    if resource_dir.join("logo.png").exists() {
        println!("Creating variants of logo.png");
        fs::copy(resource_dir.join("logo.png"), tmp_pkg.join(modinfo.id.clone() + ".png"))?;
        throw_unwrap!(spritesheet::create_variants_of_sprite(&tmp_pkg.join(modinfo.id.clone() + ".png"), &tmp_pkg), "Could not create sprite variants");
    }

    for file in modinfo.resources.files {
        let file_name = &file.file_name().unwrap().to_str().unwrap();
        if !cache_data.are_any_of_these_later(&file_name, &[file.clone()])? {
            println!("Skipping {} as no changes were detected", file_name.yellow().bold());
            continue;
        }

        if spritesheet::is_image(&file) {
            println!("Creating variants of {}", &file_name);
            throw_unwrap!(spritesheet::create_variants_of_sprite(&file, &tmp_pkg.join("resources")), "Could not create sprite variants");
        } else {
            fs::copy(&file, &tmp_pkg.join("resources").join(&file_name))?;
        }
    }

    for sheet in modinfo.resources.sheets {
        if !cache_data.are_any_of_these_later(&sheet.name, &sheet.files)? {
            println!("Skipping packing {} as no changes were detected", sheet.name.yellow().bold());
            continue;
        }
        if log {
            println!("Packing {}", sheet.name.yellow().bold());
        }
        throw_unwrap!(spritesheet::pack_sprites(
            sheet.files.clone(),
            &tmp_pkg.join("resources"),
            true,
            Some(&sheet.name),
            Some(&(modinfo.id.clone() + "_")
        )), "Could not pack sprites");
    }

    let mut zip = zip::ZipWriter::new(File::create(out_file)?);

    let cwd = std::env::current_dir()?;
    std::env::set_current_dir(tmp_pkg)?;

    let zopts = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for item in walkdir::WalkDir::new(".") {
        let item = item?;

        if !item.metadata()?.is_dir() && item.file_name() != "cache_data.json" {
            let mut file_path = item.path().strip_prefix("./")?.to_str().unwrap().to_string();
            if cfg!(windows) {
                file_path = file_path.replace("/", "\\");
            }
            zip.start_file(file_path, zopts)?;
            zip.write_all(&fs::read(item.path())?)?;
        }
    }

    zip.finish().expect("Unable to package .geode file");
    std::env::set_current_dir(cwd)?;

    if use_cached_resources {
        fs::write(tmp_pkg.join("resources").join("cache_data.json"), cache_data.to_json_string())?;
    }

    println!("{}", 
        format!("Successfully packaged {}", 
            out_file.file_name().unwrap().to_str().unwrap()
        ).yellow().bold()
    );

    Ok(())
}
