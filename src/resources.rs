use path_absolutize::Absolutize;
use std::fs;
use std::path::{Path, PathBuf};
use colored::Colorize;
use std::collections::HashMap;
use serde_json::{Map, Value, json};
use glob::glob;
use std::time::{Duration, SystemTime};
use crate::{throw_error, throw_unwrap, spritesheet, font};

pub struct GameSheet {
    pub name: String,
    pub files: Vec<PathBuf>,
}

pub struct BMFont {
    pub name: String,
    pub ttf_src: PathBuf,
    pub charset: Option<String>,
    pub fontsize: u32,
    pub outline: u32,
}

pub struct ModResources {
    pub raw_files: Vec<PathBuf>,
    pub prefixed_files: Vec<PathBuf>,
    pub sheets: Vec<GameSheet>,
    pub fonts: Vec<BMFont>,
    pub font_jsons: HashMap<String, Value>,
}

pub struct CacheData {
    latest_file: HashMap<String, Duration>,
    latest_json: HashMap<String, Value>,
}

impl CacheData {
    pub fn parse_json(&mut self, file: &Path) -> Result<(), Box<dyn std::error::Error>> {
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

    pub fn to_json_string(&self) -> String {
        let mut json = json!({});
        for (k, v) in &self.latest_file {
            json[k] = serde_json::to_value(v.as_secs()).unwrap();
        }
        for (k, v) in &self.latest_json {
            json[k] = v.clone();
        }
        json.to_string()
    }

    pub fn check_json_different_or_file_later(&mut self, json: &Value, key: &str, file: &Path)
        -> Result<bool, Box<dyn std::error::Error>> {
        if file.exists() {
            let modified_date = fs::metadata(file)?.modified()?.duration_since(SystemTime::UNIX_EPOCH)?;
            let mut latest_json_key = key.to_string();
            latest_json_key.push_str("_json");
            if let std::collections::hash_map::Entry::Vacant(e) = self.latest_json.entry(latest_json_key.clone()) {
                e.insert(json.clone());
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

    pub fn are_any_of_these_later(&mut self, sheet: &str, files: &[PathBuf])
        -> Result<bool, Box<dyn std::error::Error>> {
        if files.is_empty() {
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

pub fn parse_resources(
    json: &Map<String, Value>,
    start_search_path: &Path
) -> Result<ModResources, Box<dyn std::error::Error>> {
    let mut raw_files: Vec<PathBuf> = vec![];
    let mut prefixed: Vec<PathBuf> = vec![];
    let mut sheets: Vec<GameSheet> = vec![];
    let mut fonts: Vec<BMFont> = vec![];
    let mut font_jsons = HashMap::new();

    for (key, value) in json {
        match key.as_str() {
            "raw" => {
                for path in value.as_array().ok_or("[mod.json].resources.raw is not an array!")? {
                    if path.is_string() {
                        let mut search_path = Path::new(&path.as_str().unwrap()).to_path_buf();
                        if search_path.is_relative() {
                            search_path = start_search_path.join(search_path);
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
                            search_path = start_search_path.join(search_path);
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

                    #[allow(clippy::or_fun_call)]
                    for path in sheet_files.as_array().ok_or(
                        format!("[mod.json].resources.spritesheets.{} is not an array!", sheet_name).as_str()
                    )? {
                        if path.is_string() {
                            let mut search_path = Path::new(&path.as_str().unwrap()).to_path_buf();
                            if search_path.is_relative() {
                                search_path = start_search_path.join(search_path);
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
                        ttf_path = start_search_path.join(ttf_path);
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
                        charset,
                        fontsize,
                        outline,
                    });
                }
            },
            _ => {
                throw_error!("[mod.json].resources: Unknown key {}", key);
            }
        }
    }

    Ok(ModResources {
        raw_files,
        prefixed_files: prefixed,
        sheets,
        fonts,
        font_jsons,
    })
}

pub fn create_resources(
    resources: &ModResources,
    use_cache: bool,
    mod_id: &String,
    dir: &Path,
    log: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    
    let mut cache_data = CacheData {
        latest_file: HashMap::new(),
        latest_json: HashMap::new(),
    };

    if dir.join("cache_data.json").exists() && use_cache {
        cache_data.parse_json(&dir.join("cache_data.json"))?;
    }

    for file in &resources.raw_files {
        let file_name = &file.file_name().unwrap().to_str().unwrap();
        if !cache_data.are_any_of_these_later(file_name, &[file.clone()])? {
            println!("Skipping {} as no changes were detected", file_name.yellow().bold());
            continue;
        }
        fs::copy(&file, &dir.join(&file_name))?;
    }

    for file in &resources.prefixed_files {
        let file_name = &file.file_name().unwrap().to_str().unwrap();
        if !cache_data.are_any_of_these_later(file_name, &[file.clone()])? {
            println!("Skipping {} as no changes were detected", file_name.yellow().bold());
            continue;
        }

        if spritesheet::is_image(&file) {
            println!("Creating variants of {}", &file_name);
            throw_unwrap!(spritesheet::create_variants_of_sprite(
                &file, &dir, Some(&(mod_id.clone() + "_"))
            ), "Could not create sprite variants");
        } else {
            fs::copy(&file, &dir.join(mod_id.clone() + "_" + file_name))?;
        }
    }

    for sheet in &resources.sheets {
        if !cache_data.are_any_of_these_later(&sheet.name, &sheet.files)? {
            println!("Skipping packing {} as no changes were detected", sheet.name.yellow().bold());
            continue;
        }
        if log {
            println!("Packing {}", sheet.name.yellow().bold());
        }
        throw_unwrap!(spritesheet::pack_sprites(
            sheet.files.clone(),
            &dir,
            true,
            Some(&(mod_id.clone() + "_" + &sheet.name)),
            Some(&(mod_id.clone() + "_")),
        ), "Could not pack sprites");
    }

    for font in &resources.fonts {
        if !cache_data.check_json_different_or_file_later(
            &resources.font_jsons[&font.name], font.name.as_str(), &font.ttf_src
        )? {
            println!("Skipping processing {} as no changes were detected", font.name.yellow().bold());
            continue;
        }
        if log {
            println!("Creating bitmap font from {}", font.name.yellow().bold());
        }
        throw_unwrap!(font::create_bitmap_font_from_ttf(
            Path::new(&font.ttf_src),
            &dir,
            Some(&font.name),
            font.fontsize,
            Some(&(mod_id.clone() + "_")),
            true,
            font.charset.as_deref(),
            font.outline,
        ), "Could not create bitmap font");
    }

    if use_cache {
        fs::write(
            dir.join("cache_data.json"),
            cache_data.to_json_string()
        )?;
    }

    Ok(())
}
