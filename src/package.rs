use std::io::Write;
use std::fs::File;
use colored::Colorize;

use crate::{print_error, spritesheet, Configuration};

use fs_extra::dir as fs_dir;

use serde_json::Value;

use std::fs;
use std::path::Path;

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

fn extract_mod_info(mod_json: &Value) -> (String, Vec<String>, String) {
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

    (name.to_string(), bin_list, id.clone())
}

pub fn create_geode(resource_dir: &Path, exec_dir: &Path, out_file: &Path, install: bool) {
	let raw = fs::read_to_string(resource_dir.join("mod.json")).unwrap();
	let mod_json: Value = match serde_json::from_str(&raw) {
	    Ok(p) => p,
	    Err(_) => print_error!("mod.json is not a valid JSON file!")
	};

    let modinfo = extract_mod_info(&mod_json);

    let tmp_pkg_name = format!("geode_pkg_{}", modinfo.2);
    let tmp_pkg = &std::env::temp_dir().join(tmp_pkg_name);

    if tmp_pkg.exists() {
        fs_dir::remove(tmp_pkg).unwrap();
    }

    fs::create_dir(tmp_pkg).unwrap();

    let mut output_name = String::new();

    let try_copy = || -> Result<(), Box<dyn std::error::Error>> {
        output_name = modinfo.0;
        for ref f in modinfo.1 {
            if !exec_dir.join(f).exists() {
                print_error!("Unable to find binary {}, defined in [mod.json].binary", f);
            }

            fs::copy(exec_dir.join(f), tmp_pkg.join(f))?;
        }

        fs::copy(resource_dir.join("mod.json"), tmp_pkg.join("mod.json"))?;

        let options = fs_dir::CopyOptions::new();

        if resource_dir.join("resources").exists() {

            fs_dir::copy(resource_dir.join("resources"), tmp_pkg, &options)?;
        }
        if resource_dir.join("sprites").exists() {
            spritesheet::pack_sprites(&resource_dir.join("sprites"), tmp_pkg, false, None)?;
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
            zip.start_file(item.path().strip_prefix("./").unwrap().as_os_str().to_str().unwrap(), zopts).unwrap();
            zip.write(&fs::read(item.path()).unwrap()).unwrap();
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
        let target_path = Configuration::install_path().join("geode").join("mods").join(out_file.to_path_buf().file_name().unwrap());
        fs::copy(out_file, target_path).unwrap();
        println!("{}", 
            format!("Succesfully installed {}", 
                out_file.file_name().unwrap().to_str().unwrap()
            ).cyan().bold()
        );
    }
}
