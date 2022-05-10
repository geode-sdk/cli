use std::io::Write;
use std::fs::File;
use colored::Colorize;
use glob::glob;
use std::time::{Duration, SystemTime};
use path_absolutize::Absolutize;

use crate::{throw_error, throw_unwrap, spritesheet, font};

use serde_json::{Value, json};

use std::fs;
use std::path::{Path, PathBuf};

use std::collections::HashMap;

struct GameSheet {
    name: String,
    files: Vec<PathBuf>,
}

struct BMFont {
    name: String,
    ttf_src: PathBuf,
    charset: Option<String>,
    fontsize: u32,
    outline: u32,
}

struct ModResources {
    raw_files: Vec<PathBuf>,
    prefixed_files: Vec<PathBuf>,
    sheets: Vec<GameSheet>,
    fonts: Vec<BMFont>,
    font_jsons: HashMap<String, Value>,
}

struct ModInfo {
    //name: String,
    bin_list: Vec<String>,
    id: String,
    resources: ModResources,
}

struct CacheData {
    latest_file: HashMap<String, Duration>,
    latest_json: HashMap<String, Value>,
}

impl CacheData {
    fn parse_json(&mut self, file: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let json: Value = serde_json::from_str(&fs::read_to_string(file)?)?;

        for (k, v) in json.as_object().unwrap() {
            if v.is_u64() {
                let time = Duration::from_secs(v.as_u64().unwrap());
                self.latest_file.insert(k.to_string(), time);
            } else if v.is_object() {
                self.latest_json.insert(k.to_string(), v.clone());
            }
        }

        Ok(())
    }

    fn to_json_string(&self) -> String {
        let mut json = json!({});
        for (k, v) in &self.latest_file {
            json[k] = serde_json::to_value(v.as_secs()).unwrap();
        }
        for (k, v) in &self.latest_json {
            json[k] = v.clone();
        }
        json.to_string()
    }

    fn is_this_json_different_or_file_later(&mut self, json: &Value, key: &str, file: &PathBuf)
        -> Result<bool, Box<dyn std::error::Error>> {
        if file.exists() {
            let modified_date = fs::metadata(file)?.modified()?.duration_since(SystemTime::UNIX_EPOCH)?;
            let mut latest_json_key = key.to_string();
            latest_json_key.push_str("_json");
            if !self.latest_json.contains_key(&latest_json_key) {
                self.latest_json.insert(latest_json_key, json.clone());
                self.latest_file.insert(key.to_string(), modified_date);
                return Ok(true);
            }
            let cached_json = &self.latest_json[&latest_json_key];
            if *cached_json != *json {
                self.latest_json.insert(latest_json_key, json.clone());
                self.latest_file.insert(key.to_string(), modified_date);
                return Ok(true);
            }
            if !self.latest_file.contains_key(key) ||
               modified_date.as_secs() > self.latest_file[key].as_secs() {
                self.latest_json.insert(latest_json_key, json.clone());
                self.latest_file.insert(key.to_string(), modified_date);
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn are_any_of_these_later(&mut self, sheet: &str, files: &[PathBuf])
        -> Result<bool, Box<dyn std::error::Error>> {
        if files.len() == 0 {
            return Ok(true);
        }
        let mut res = false;
        for file in files {
            if !file.exists() {
                throw_error!("File {} does not exist (from cache check)", file.absolutize().unwrap().to_str().unwrap());
            }
            let modified_date = fs::metadata(file)?.modified()?.duration_since(SystemTime::UNIX_EPOCH)?;

            if !self.latest_file.contains_key(sheet) ||
                modified_date.as_secs() > self.latest_file[sheet].as_secs()
            {
                self.latest_file.insert(sheet.to_string(), modified_date);
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
        Value::Object(res_object) => {
            let mut raw_files: Vec<PathBuf> = vec![];
            let mut prefixed: Vec<PathBuf> = vec![];
            let mut sheets: Vec<GameSheet> = vec![];
            let mut fonts: Vec<BMFont> = vec![];
            let mut font_jsons = HashMap::new();

            for (key, value) in res_object {
                match key.as_str() {
                    "raw" => {
                        for path in value.as_array().ok_or("[mod.json].resources.raw is not an array!")? {
                            if path.is_string() {
                                let mut search_path = Path::new(&path.as_str().unwrap()).to_path_buf();
                                if search_path.is_relative() {
                                    search_path = mod_json_location.join(search_path);
                                }
                                raw_files.extend(
                                    glob(search_path.to_str().unwrap())
                                    ?.map(|x| x.unwrap())
                                );
                            } else {
                                throw_error!("[mod.json].resources.raw: Expected item to be 'string', but it was not");
                            }
                        }
                    },
                    "files" => {
                        for path in value.as_array().ok_or("[mod.json].resources.files is not an array!")? {
                            if path.is_string() {
                                let mut search_path = Path::new(&path.as_str().unwrap()).to_path_buf();
                                if search_path.is_relative() {
                                    search_path = mod_json_location.join(search_path);
                                }
                                prefixed.extend(
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
                            for path in sheet_files.as_array().ok_or(
                                format!("[mod.json].resources.spritesheets.{} is not an array!", sheet_name).as_str()
                            )? {
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
                                    throw_error!(
                                        "[mod.json].resources.spritesheets.{}: Expected item to be 'string', but it was not",
                                        sheet_name
                                    );
                                }
                            }
                            sheets.push(GameSheet {
                                name: sheet_name.clone(),
                                files: sheet_paths,
                            });
                        }
                    },
                    "fonts" => {
                        for (bm_name, bm_json) in value.as_object().unwrap() {
                            let bm_obj = bm_json.as_object().unwrap();
                            let mut ttf_path = Path::new(match &bm_obj["path"] {
                                Value::String(n) => n,
                                Value::Null => throw_error!("[mod.json].resources.fonts.{}.path is empty!", bm_name),
                                _ => throw_error!("[mod.json].resources.fonts.{}.id is not a string!", bm_name)
                            }).to_path_buf();
                            if ttf_path.is_relative() {
                                ttf_path = mod_json_location.join(ttf_path);
                            }
                            let fontsize: u32 = match &bm_obj["size"] {
                                Value::Number(n) => n.as_u64().unwrap() as u32,
                                Value::Null => throw_error!("[mod.json].resources.fonts.{}.size is null!", bm_name),
                                _ => throw_error!("[mod.json].resources.fonts.{}.size is not an int!", bm_name)
                            };
                            let mut charset: Option<String> = None;
                            let mut outline = 0u32;
                            for (key, val) in bm_obj {
                                match key.as_str() {
                                    "charset" => {
                                        if val.is_string() {
                                            charset = Some(val.as_str().unwrap().to_string());
                                        } else {
                                            throw_error!("[mod.json].resources.fonts.{}.charset is not a string!", bm_name);
                                        }
                                    },
                                    "outline" => {
                                        if val.is_u64() {
                                            outline = val.as_u64().unwrap() as u32;
                                        } else {
                                            throw_error!("[mod.json].resources.fonts.{}.outline is not an integer!", bm_name);
                                        }
                                    },
                                    "size" => {},
                                    "path" => {},
                                    _ => {
                                        throw_error!("[mod.json].resources.fonts.{}: Unknown key {}", bm_name, key);
                                    }
                                }
                            }
                            font_jsons.insert(bm_name.clone(), bm_json.clone());
                            fonts.push(BMFont {
                                name: bm_name.clone(),
                                ttf_src: ttf_path,
                                charset: charset,
                                fontsize: fontsize,
                                outline: outline,
                            });
                        }
                    },
                    _ => {
                        throw_error!("[mod.json].resources: Unknown key {}", key);
                    }
                }
            }

            ModResources {
                raw_files: raw_files,
                prefixed_files: prefixed,
                sheets: sheets,
                fonts: fonts,
                font_jsons: font_jsons,
            }
        },
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
    mod_src_dir: &Path,
    exec_dir: &Path,
    out_file: &Path,
    log: bool,
    use_cached_resources: bool,
) -> Result<(), Box<dyn std::error::Error>> {
	let mod_json = serde_json::from_str(&fs::read_to_string(mod_src_dir.join("mod.json"))?)?;
    let modinfo = extract_mod_info(&mod_json, &mod_src_dir.to_path_buf())?;

    println!("{}", 
        format!("Packaging {}", 
            modinfo.id
        ).yellow().bold()
    );

    let tmp_pkg = &std::env::temp_dir().join(format!("geode_pkg_{}", modinfo.id));

    let mut cache_data = CacheData {
        latest_file: HashMap::new(),
        latest_json: HashMap::new(),
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

    fs::copy(mod_src_dir.join("mod.json"), tmp_pkg.join("mod.json"))?;

    if !tmp_pkg.join("resources").exists() {
        fs::create_dir_all(tmp_pkg.join("resources"))?;
    }

    if mod_src_dir.join("logo.png").exists() {
        println!("Creating variants of logo.png");
        fs::copy(mod_src_dir.join("logo.png"), tmp_pkg.join(modinfo.id.clone() + ".png"))?;
        throw_unwrap!(spritesheet::create_variants_of_sprite(
            &tmp_pkg.join(modinfo.id.clone() + ".png"), &tmp_pkg, None
        ), "Could not create sprite variants");
    }

    if mod_src_dir.join("about.md").exists() {
        println!("Found about.md, adding to package");
        fs::copy(mod_src_dir.join("about.md"), tmp_pkg.join("about.md"))?;
    }

    for file in modinfo.resources.raw_files {
        let file_name = &file.file_name().unwrap().to_str().unwrap();
        if !cache_data.are_any_of_these_later(&file_name, &[file.clone()])? {
            println!("Skipping {} as no changes were detected", file_name.yellow().bold());
            continue;
        }
        fs::copy(&file, &tmp_pkg.join("resources").join(&file_name))?;
    }

    for file in modinfo.resources.prefixed_files {
        let file_name = &file.file_name().unwrap().to_str().unwrap();
        if !cache_data.are_any_of_these_later(&file_name, &[file.clone()])? {
            println!("Skipping {} as no changes were detected", file_name.yellow().bold());
            continue;
        }

        if spritesheet::is_image(&file) {
            println!("Creating variants of {}", &file_name);
            throw_unwrap!(spritesheet::create_variants_of_sprite(
                &file, &tmp_pkg.join("resources"), Some(&(modinfo.id.clone() + "_"))
            ), "Could not create sprite variants");
        } else {
            fs::copy(&file, &tmp_pkg.join("resources").join(modinfo.id.clone() + "_" + file_name))?;
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
            Some(&(modinfo.id.clone() + "_" + &sheet.name)),
            Some(&(modinfo.id.clone() + "_")),
        ), "Could not pack sprites");
    }

    for font in modinfo.resources.fonts {
        if !cache_data.is_this_json_different_or_file_later(
            &modinfo.resources.font_jsons[&font.name], font.name.as_str(), &font.ttf_src
        )? {
            println!("Skipping processing {} as no changes were detected", font.name.yellow().bold());
            continue;
        }
        if log {
            println!("Creating bitmap font from {}", font.name.yellow().bold());
        }
        throw_unwrap!(font::create_bitmap_font_from_ttf(
            &Path::new(&font.ttf_src),
            &tmp_pkg.join("resources"),
            Some(&font.name),
            font.fontsize,
            Some(&(modinfo.id.clone() + "_")),
            true,
            font.charset.as_deref(),
            font.outline,
        ), "Could not create bitmap font");
    }

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
