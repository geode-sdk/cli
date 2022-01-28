use colored::Colorize;
use serde_json::Value;
use crate::print_error;
use std::fs;

pub fn platform_extension() -> &'static str {
    if cfg!(windows) {
        ".dll"
    } else if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
        ".dylib"
    } else if cfg!(target_os = "android") {
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

fn extract_binary_name(mod_json: &Value) -> String {
    let bin_val: serde_json::value::Value;
    let mut has_extension = true;

    if mod_json["binary"].is_string() {
        bin_val = mod_json["binary"].clone();
    } else if mod_json["binary"].is_object() {
        let bin_object = &mod_json["binary"];

        bin_val = match &bin_object[platform_string()] {
            Value::Null => bin_object["*"].clone(),
            Value::String(s) => Value::String(s.to_string()),
            a => a.clone()
        };

        has_extension = bin_object["auto"].as_bool().unwrap_or(true);
    } else {
        print_error!("[mod.json].binary is not a string nor an object!");
    }

    let mut binary_name = match bin_val {
        Value::String(s) => s,
        Value::Null => print_error!("[mod.json].binary is empty!"),
        a => a.to_string()
    };

    if has_extension {
        binary_name.push_str(platform_extension());
    }

    binary_name
}

pub fn create_geode(build_path: String) {
	let raw = fs::read_to_string(build_path).unwrap();
	let mod_json: Value = match serde_json::from_str(&raw) {
	    Ok(p) => p,
	    Err(_) => print_error!("mod.json is not a valid JSON file!")
	};

	let binary = extract_binary_name(&mod_json);

	println!("binary name: {}", binary);
}
