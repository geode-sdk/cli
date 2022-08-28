use std::collections::HashMap;
use std::path::{Path, PathBuf};
use serde_json::{Value, json};

use crate::fatal;

pub struct SpriteSheet {
	pub name: String,
	pub files: Vec<PathBuf>,
}

pub struct BitmapFont {
    pub name: String,
    pub path: PathBuf,
    pub charset: Option<String>,
    pub size: u32,
    pub outline: u32,
}

pub struct ModResources {
	pub files: Vec<PathBuf>,
	pub spritesheets: HashMap<String, SpriteSheet>,
	pub fonts: HashMap<String, BitmapFont>
}

pub struct ModFileInfo {
    pub name: String,
    pub binary_names: HashMap<String, String>,
    pub id: String,
    pub resources: ModResources,
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

fn platform_string() -> &'static str {
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

fn platform_extension() -> &'static str {
    get_extension(platform_string())
}

/// Reusability for get_mod_resources
fn collect_globs(value: &Value, value_name: &str, root_path: &Path, out: &mut Vec<PathBuf>) {
	if !value.is_array() {
		fatal!("{}: Expected array", value_name);
	}

	// Iterate paths
	for (i, entry) in value.as_array().unwrap().iter().enumerate() {
		// Ensure path is a string
		let mut path = PathBuf::from(entry.as_str().unwrap_or_else(|| {
			fatal!("{}[{}]: Expected string", value_name, i);
		}));

		// Absolutize
		if path.is_relative() {
			path = root_path.join(path);
		}

		// Reusability for next 
		let glob_err = |e: String| -> ! {
			fatal!("{}[{}]: Could not parse glob pattern: {}", value_name, i, e);
		};

		// Evaluate glob pattern
		let glob_results = glob::glob(path.to_str().unwrap())
			.unwrap_or_else(|e| glob_err(e.to_string()))
			.into_iter()
			.collect::<Result<Vec<_>, _>>()
			.unwrap_or_else(|e| glob_err(e.to_string()));

		// Add to files
		out.extend(glob_results);
	}
}

fn get_mod_resources(root: &Value, root_path: &Path) -> ModResources {
	let mut out = ModResources {
		files: vec![],
		spritesheets: HashMap::new(),
		fonts: HashMap::new()
	};

	if let Value::Object(ref resources) = root["resources"] {
		// Iterate resource attributes
		for (key, value) in resources {
			match key.as_str() {
				"files" => {
					collect_globs(value, "[mod.json].resources.files", root_path, &mut out.files);
				},

				"spritesheets" => {
					if !value.is_object() {
						fatal!("[mod.json].resources.spritesheets: Expected object");
					}

					// Iterate spritesheets
					for (name, files) in value.as_object().unwrap() {
						if out.spritesheets.get(name).is_some() {
							fatal!("[mod.json].resources.spritesheets: Duplicate name '{}'", name);
						}

						let mut sheet_files = Vec::<PathBuf>::new();

						collect_globs(files, &format!("[mod.json].resources.spritesheets.{}", name), root_path, &mut sheet_files);

						out.spritesheets.insert(name.to_string(), SpriteSheet {
							name: name.to_string(),
							files: sheet_files
						});
					}
				}

				"fonts" => {
					if !value.is_object() {
						fatal!("[mod.json].resources.font: Expected object");
					}

					// Iterate fonts
					for (name, info) in value.as_object().unwrap() {
						if out.fonts.get(name).is_some() {
							fatal!("[mod.json].resources.fonts: Duplicate name '{}'", name);
						}

						// Convenience variable
						let info_name = format!("[mod.json].resources.font.{}", name);
						
						if !info.is_object() {
							fatal!("{}: Expected object", info_name);
						}

						let mut font = BitmapFont {
							name: name.to_string(),
							path: PathBuf::new(),
							charset: None,
							size: 0,
							outline: 0
						};

						// Iterate font attributes
						for (key, value) in info.as_object().unwrap() {
							match key.as_str() {
								"path" => {
									font.path = PathBuf::from(
										value.as_str()
											 .unwrap_or_else(|| fatal!("{}.path: Expected string", info_name))
									);

									// Absolutize
									if font.path.is_relative() {
										font.path = root_path.join(font.path);
									}
								},

								"size" => {
									font.size = value.as_u64()
										.unwrap_or_else(|| fatal!("{}.size: Expected unsigned integer", info_name)) as u32;

									if font.size == 0 {
										fatal!("{}.size: Font size cannot be 0", info_name);
									}
								},

								"outline" => {
									font.outline = value.as_u64()
										.unwrap_or_else(|| fatal!("{}.outline: Expected unsigned integer", info_name)) as u32;
								},

								"charset" => {
									font.charset = Some(
										value.as_str()
											 .unwrap_or_else(|| fatal!("{}.charset: Expected string", info_name))
											 .to_string()
									);
								},

								_ => fatal!("{}: Unknown key {}", info_name, key)
							}
						}

						// Ensure required attributes are filled in
						if font.path.as_os_str().is_empty() {
							fatal!("{}: Missing required key 'path'", info_name);
						}
						if font.size == 0 {
							fatal!("{}: Missing required key 'size'", info_name);
						}

						out.fonts.insert(name.to_string(), font);
					}
				},

				_ => fatal!("[mod.json].resources: Unknown key {}", key)
			}
		}
	}
	out
}

pub fn get_mod_file_info(root: &Value, root_path: &Path) -> ModFileInfo {
	let name = root.get("name")
		.unwrap_or_else(|| fatal!("[mod.json]: Missing required key 'name'"))
		.as_str()
		.unwrap_or_else(|| fatal!("[mod.json].name: Expected string"))
		.to_string();

	let id = root.get("id")
		.unwrap_or_else(|| fatal!("[mod.json]: Missing required key 'id'"))
		.as_str()
		.unwrap_or_else(|| fatal!("[mod.json].id: Expected string"))
		.to_string();

	let mut out = ModFileInfo {
		name,
		id,
		resources: get_mod_resources(root, root_path),
		binary_names: HashMap::new()
	};

	// Get binaries field
	let mut binaries_value = root.get("binary").unwrap_or_else(|| fatal!("[mod.json]: Missing required key 'binary'")).clone();
	
	// String is just wildcard
	if binaries_value.is_string() {
		binaries_value = json!({
			"*": binaries_value.as_str().unwrap()
		});
	}

	let binaries = binaries_value.as_object()
		.unwrap_or_else(|| fatal!("[mod.json].binaries: Expected string or object"));


	// Iterate through platforms
	for (platform, binary) in binaries {
		// Ensure string
		if !binary.is_string() {
			fatal!("[mod.json].binaries.{}: Expected string", platform);
		}

		match platform.as_str() {
			"*" => (),
			"macos" | "windows" | "ios" | "linux" => {
				let mut binary_name = binary.as_str().unwrap().to_string();

				// Add platform extension if not exist
				if binary_name.ends_with(get_extension(platform)) {
					binary_name += get_extension(platform);
				}

				out.binary_names.insert(platform.to_string(), binary_name);
			},
			_ => fatal!("[mod.json].binaries: Unknown key {}", platform)
		}

		// Wildcard
		if platform == "*" {
			// Wildcard is useless if all other platforms are defined
			if binaries.len() == 5 {
				fatal!("[mod.json].binaries: Cannot mix '*' with all other platforms");
			}

			for platform in ["macos", "windows", "ios", "linux"] {
				// Add all binary names not already referenced
				if out.binary_names.get(platform).is_none() {
					let mut binary_name = binary.as_str().unwrap().to_string();

					// Add platform extension if not exist
					if binary_name.ends_with(get_extension(platform)) {
						binary_name += get_extension(platform);
					}

					out.binary_names.insert(platform.to_string(), binary_name);
				}
			}
		}
	}

	if out.binary_names.len() == 0 {
		fatal!("[mod.json].binaries: Cannot be empty");
	}

	out
}
