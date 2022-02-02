use std::io::Write;
use std::fs::File;
use colored::Colorize;
use glob::glob;

use crate::{print_error, spritesheet, Configuration};

use fs_extra::dir as fs_dir;

use serde_json::Value;

use std::fs;
use std::path::{Path, PathBuf};
use path_slash::*;

struct GameSheet {
    name: String,
    files: Vec<PathBuf>,
}

struct ModResources {
    files: Vec<PathBuf>,
    sheets: Vec<GameSheet>,
}

struct ModInfo {
    name: String,
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
        print_error!("You are not on a supported platform :(");
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
        print_error!("You are not on a supported platform :(");
    }
}

pub fn platform_extension() -> &'static str {
    get_extension(platform_string())
}

fn extract_mod_info(mod_json: &Value, mod_json_location: &PathBuf) -> ModInfo {
    let mut bin_list = Vec::new();

    if mod_json["binary"].is_string() {
        match mod_json["binary"].clone() {
            Value::String(s) => {
                let mut filename = s.to_string();
                if !filename.ends_with(platform_extension()) {
                    filename += platform_extension();
                }
                bin_list.push(filename);
            },
            _ => unreachable!()
        }
    } else if mod_json["binary"].is_object() {
        let bin_object = &mod_json["binary"];

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
                _ => print_error!("[mod.json].binary.{} is not a string!", i)
            }
        }

        if bin_list.is_empty() && !bin_object["*"].is_null() {
            match bin_object["*"].clone() {
                Value::String(s) => {
                    let mut filename = s.to_string();
                    if !filename.ends_with(platform_extension()) {
                        filename += platform_extension();
                    }
                    bin_list.push(filename);
                },
                _ => print_error!("[mod.json].binary.* is not a string!")
            }
        }
    } else {
        print_error!("[mod.json].binary is not a string nor an object!");
    }

    if bin_list.is_empty() {
        print_error!("[mod.json].binary is empty!");
    }

    let name = match &mod_json["name"] {
        Value::String(n) => n,
        Value::Null => print_error!("[mod.json].name is empty!"),
        _ => print_error!("[mod.json].name is not a string!")
    };

    let id = match &mod_json["id"] {
        Value::String(n) => n,
        Value::Null => print_error!("[mod.json].id is empty!"),
        _ => print_error!("[mod.json].id is not a string!")
    };

    let resources: ModResources;
    
    if mod_json["resources"].is_object() {
        let res_object = mod_json["resources"].as_object().unwrap();
        
        let mut files: Vec<PathBuf> = vec!();
        let mut sheets: Vec<GameSheet> = vec!();
        for (key, value) in res_object {
            match key.as_str() {
                "*" => {
                    for path in value.as_array().expect("[mod.json].resources.* is not an array!") {
                        if path.is_string() {
                            let mut a_path = Path::new(&path.as_str().unwrap()).to_path_buf();
                            if a_path.is_relative() {
                                a_path = mod_json_location.join(a_path);
                            }
                            files.append(
                                &mut glob(a_path.to_str().unwrap())
                                    .unwrap().map(|x| x.unwrap())
                                    .collect()
                            );
                        } else {
                            print_error!("[mod.json].resources.*: Expected item to be 'string', but it was not");
                        }
                    }
                },
                _ => {
                    let mut sheet_files: Vec<PathBuf> = vec!();
                    for path in value.as_array().expect("[mod.json].resources.* is not an array!") {
                        if path.is_string() {
                            let mut a_path = Path::new(&path.as_str().unwrap()).to_path_buf();
                            if a_path.is_relative() {
                                a_path = mod_json_location.join(a_path);
                            }
                            sheet_files.append(
                                &mut glob(a_path.to_str().unwrap())
                                    .unwrap().map(|x| x.unwrap())
                                    .collect()
                            );
                        } else {
                            print_error!("[mod.json].resources.*: Expected item to be 'string', but it was not");
                        }
                    }
                    sheets.push(GameSheet {
                        name: key.clone(),
                        files: sheet_files,
                    });
                },
            }
        }

        resources = ModResources {
            files: files,
            sheets: sheets,
        };
    } else {
        resources = ModResources {
            files: vec!(),
            sheets: vec!(),
        };
    }

    ModInfo {
        name: name.to_string(),
        bin_list: bin_list,
        id: id.clone(),
        resources: resources
    }
}

pub fn create_geode(resource_dir: &Path, exec_dir: &Path, out_file: &Path, install: bool, api: bool) {
	let raw = fs::read_to_string(resource_dir.join("mod.json")).unwrap();
	let mod_json: Value = match serde_json::from_str(&raw) {
	    Ok(p) => p,
	    Err(_) => print_error!("mod.json is not a valid JSON file!")
	};

    let modinfo = extract_mod_info(&mod_json, &resource_dir.to_path_buf());

    let tmp_pkg_name = format!("geode_pkg_{}", modinfo.id);
    let tmp_pkg = &std::env::temp_dir().join(tmp_pkg_name);

    if tmp_pkg.exists() {
        fs_dir::remove(tmp_pkg).unwrap();
    }

    fs::create_dir(tmp_pkg).unwrap();

    let mut output_name = String::new();

    let try_copy = || -> Result<(), Box<dyn std::error::Error>> {
        output_name = modinfo.name;
        for ref f in modinfo.bin_list {
            if !exec_dir.join(f).exists() {
                print_error!("Unable to find binary {}, defined in [mod.json].binary", f);
            }

            fs::copy(exec_dir.join(f), tmp_pkg.join(f))?;
        }

        fs::copy(resource_dir.join("mod.json"), tmp_pkg.join("mod.json"))?;

        fs::create_dir_all(tmp_pkg.join("resources")).unwrap();

        for file in modinfo.resources.files {
            fs::copy(&file, tmp_pkg.join("resources").join(file.file_name().unwrap())).unwrap();
        }

        for sheet in modinfo.resources.sheets {
            spritesheet::pack_sprites(&sheet.files, &tmp_pkg.join("resources"), false, Some(sheet.name))?;
        }

        Ok(())
    };

    try_copy().expect("Unable to copy files");

    let outfile = File::create(out_file).unwrap();
    let mut zip = zip::ZipWriter::new(outfile);


    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(tmp_pkg).unwrap();

    let zopts = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for walk in walkdir::WalkDir::new(".") {
        let item = walk.unwrap();
        if !item.metadata().unwrap().is_dir() {
            zip.start_file(item.path().strip_prefix("./").unwrap().to_slash().unwrap().as_str(), zopts).unwrap();
            zip.write_all(&fs::read(item.path()).unwrap()).unwrap();
        }
    }

    zip.finish().expect("Unable to package .geode file");
    std::env::set_current_dir(cwd).unwrap();

    println!("{}", 
        format!("Successfully packaged {}", 
            out_file.file_name().unwrap().to_str().unwrap()
        ).yellow().bold()
    );

    if install {
        let mut target_path = Configuration::install_path().join("geode");
        target_path = if api { target_path.join("api") } else { target_path.join("mods") };
        target_path =target_path.join(out_file.to_path_buf().file_name().unwrap());
        fs::copy(out_file, target_path).unwrap();
        println!("{}", 
            format!("Succesfully installed {}", 
                out_file.file_name().unwrap().to_str().unwrap()
            ).cyan().bold()
        );
    }
}
