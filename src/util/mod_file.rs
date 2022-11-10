use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::spritesheet::SpriteSheet;
use crate::NiceUnwrap;
use crate::{geode_assert, fatal, warn};

pub struct BitmapFont {
	pub name: String,
	pub path: PathBuf,
	pub charset: Option<String>,
	pub size: u32,
	pub outline: u32,
	pub color: [u8; 3],
}

pub struct ModResources {
	pub files: Vec<PathBuf>,
	pub spritesheets: HashMap<String, SpriteSheet>,
	pub sprites: Vec<PathBuf>,
	pub fonts: HashMap<String, BitmapFont>,
}

pub struct ModFileInfo {
	pub name: String,
	pub id: String,
	pub resources: ModResources,
}

/// Reusability for get_mod_resources
fn collect_globs(value: &Value, value_name: &str, root_path: &Path, out: &mut Vec<PathBuf>) {
	geode_assert!(value.is_array(), "{}: Expected array", value_name);

	// Iterate paths
	for (i, entry) in value.as_array().unwrap().iter().enumerate() {
		// Ensure path is a string
		let mut path = PathBuf::from(
			entry
				.as_str()
				.nice_unwrap(format!("{}[{}]: Expected string", value_name, i)),
		);

		// Absolutize
		if path.is_relative() {
			path = root_path.join(path);
		}

		// Reusability for next
		let glob_err = format!("{}[{}]: Could not parse glob pattern", value_name, i);

		// Evaluate glob pattern
		let glob_results = glob::glob(path.to_str().unwrap())
			.nice_unwrap(&glob_err)
			.into_iter()
			.collect::<Result<Vec<_>, _>>()
			.nice_unwrap(glob_err);

		// Add to files
		out.extend(glob_results);
	}
}

fn get_mod_resources(root: &Value, root_path: &Path) -> ModResources {
	let mut out = ModResources {
		files: vec![],
		sprites: vec![],
		spritesheets: HashMap::new(),
		fonts: HashMap::new(),
	};

	if let Value::Object(ref resources) = root["resources"] {
		// Iterate resource attributes
		for (key, value) in resources {
			match key.as_str() {
				"files" => {
					collect_globs(
						value,
						"[mod.json].resources.files",
						root_path,
						&mut out.files,
					);
				}

				"sprites" => {
					collect_globs(
						value,
						"[mod.json].resources.files",
						root_path,
						&mut out.sprites,
					);
				}

				"spritesheets" => {
					geode_assert!(value.is_object(), "[mod.json].resources.spritesheets: Expected object");

					// Iterate spritesheets
					for (name, files) in value.as_object().unwrap() {
						geode_assert!(
							out.spritesheets.get(name).is_none(),
							"[mod.json].resources.spritesheets: Duplicate name '{}'",
							name
						);

						let mut sheet_files = Vec::<PathBuf>::new();

						collect_globs(
							files,
							&format!("[mod.json].resources.spritesheets.{}", name),
							root_path,
							&mut sheet_files,
						);

						for (i, file) in sheet_files.iter().enumerate() {
							if file.extension().and_then(|x| x.to_str()).unwrap_or("") != "png" {
								warn!("[mod.json].resources.sprites.{}[{}]: File extension is not png. Extension will change", name, i);
							}
						}

						out.spritesheets.insert(
							name.to_string(),
							SpriteSheet {
								name: name.to_string(),
								files: sheet_files,
							},
						);
					}
				}

				"fonts" => {
					// Iterate fonts
					for (name, info) in value
						.as_object()
						.nice_unwrap("[mod.json].resources.font: Expected object")
					{
						geode_assert!(out.fonts.get(name).is_none(), "[mod.json].resources.fonts: Duplicate name '{}'", name);

						// Convenience variable
						let info_name = format!("[mod.json].resources.font.{}", name);

						geode_assert!(info.is_object(), "{}: Expected object", info_name);

						let mut font = BitmapFont {
							name: name.to_string(),
							path: PathBuf::new(),
							charset: None,
							size: 0,
							outline: 0,
							color: [255, 255, 255],
						};

						// Iterate font attributes
						for (key, value) in info.as_object().unwrap() {
							match key.as_str() {
								"path" => {
									font.path = PathBuf::from(value.as_str().nice_unwrap(format!(
										"{}.path: Expected string",
										info_name
									)));

									// Absolutize
									if font.path.is_relative() {
										font.path = root_path.join(font.path);
									}
								}

								"size" => {
									font.size = value.as_u64().nice_unwrap(format!(
										"{}.size: Expected unsigned integer",
										info_name
									)) as u32;

									geode_assert!(font.size != 0, "{}.size: Font size cannot be 0", info_name);
								}

								"outline" => {
									font.outline = value.as_u64().nice_unwrap(format!(
										"{}.outline: Expected unsigned integer",
										info_name
									)) as u32;
								}

								"color" => {
									let color = value
										.as_str()
										.nice_unwrap(format!("{}.color: Expected string", info_name));

									let col = u32::from_str_radix(color, 16).nice_unwrap(format!(
										"{}.color: Expected hexadecimal color",
										info_name
									));

									font.color = [
										((col >> 16) & 0xFF) as u8,
										((col >> 8) & 0xFF) as u8,
										(col & 0xFF) as u8,
									];
								}

								"charset" => {
									font.charset = Some(
										value
											.as_str()
											.nice_unwrap(format!(
												"{}.charset: Expected string",
												info_name
											))
											.to_string(),
									);
								}

								_ => fatal!("{}: Unknown key {}", info_name, key),
							}
						}

						// Ensure required attributes are filled in
						geode_assert!(!font.path.as_os_str().is_empty(), "{}: Missing required key 'path'", info_name);
						geode_assert!(font.size != 0, "{}: Missing required key 'size'", info_name);

						out.fonts.insert(name.to_string(), font);
					}
				}

				_ => fatal!("[mod.json].resources: Unknown key {}", key),
			}
		}
	}
	out
}

pub fn get_mod_file_info(root: &Value, root_path: &Path) -> ModFileInfo {
	let name = root
		.get("name")
		.nice_unwrap("[mod.json]: Missing required key 'name'")
		.as_str()
		.nice_unwrap("[mod.json].name: Expected string")
		.to_string();

	let id = root
		.get("id")
		.nice_unwrap("[mod.json]: Missing required key 'id'")
		.as_str()
		.nice_unwrap("[mod.json].id: Expected string")
		.to_string();

	ModFileInfo {
		name,
		id,
		resources: get_mod_resources(root, root_path),
	}
}
