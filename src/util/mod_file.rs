use semver::{VersionReq, Version};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{PathBuf, Path};
use crate::spritesheet::SpriteSheet;
use crate::NiceUnwrap;

trait Glob {
	fn glob(self) -> Self;
}

impl Glob for Vec<PathBuf> {
	fn glob(self) -> Self {
		self
			.into_iter()
			.flat_map(|src|
				glob::glob(
					std::env::current_dir().unwrap()
						.join(&src)
						.to_str()
						.unwrap()
				)
				.nice_unwrap(format!("Invalid glob pattern {}", src.to_str().unwrap()))
				.map(|g| g.unwrap())
			).collect()
	}
}

fn parse_glob<'de, D>(deserializer: D) -> Result<Vec<PathBuf>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Vec::<PathBuf>::deserialize(deserializer)?.glob())
}

fn parse_glob_rel<'de, D>(deserializer: D) -> Result<Vec<PathBuf>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Vec::<PathBuf>::deserialize(deserializer)?
		.glob()
		.into_iter()
		.map(|p| p.strip_prefix(std::env::current_dir().unwrap()).unwrap_or(&p).to_path_buf())
		.collect()
	)
}

fn parse_spritesheets<'de, D>(deserializer: D) -> Result<HashMap<String, SpriteSheet>, D::Error>
where
    D: Deserializer<'de>,
{
	Ok(HashMap::<String, Vec<PathBuf>>::deserialize(deserializer)?
		.into_iter()
        .map(|(name, srcs)| {
			(name.clone(), SpriteSheet {
				name,
				files: srcs.glob()
			})
        })
		.collect()
	)
}

fn parse_version<'de, D>(deserializer: D) -> Result<Version, D::Error>
where
    D: Deserializer<'de>,
{
	// semver doesn't accept "v" prefixes and the string will be validated at 
	// runtime by Geode anyway so let's just crudely remove all 'v's for now
	Ok(Version::parse(&<String>::deserialize(deserializer)?.replace("v", ""))
		.map_err(serde::de::Error::custom)?
	)
}

fn parse_comparable_version<'de, D>(deserializer: D) -> Result<VersionReq, D::Error>
where
    D: Deserializer<'de>,
{
	// semver doesn't accept "v" prefixes and the string will be validated at 
	// runtime by Geode anyway so let's just crudely remove all 'v's for now
	Ok(VersionReq::parse(&<String>::deserialize(deserializer)?.replace("v", ""))
		.map_err(serde::de::Error::custom)?
	)
}

fn parse_fonts<'de, D>(deserializer: D) -> Result<HashMap<String, BitmapFont>, D::Error>
where
    D: Deserializer<'de>,
{
	Ok(<HashMap<String, BitmapFont>>::deserialize(deserializer)?
		.into_iter()
		.map(|(name, mut font)| {
			font.name = name.clone();
			font.path = std::env::current_dir().unwrap().join(font.path);
			(name, font)
		})
		.collect()
	)
}

fn parse_color<'de, D>(deserializer: D) -> Result<Color, D::Error>
where
    D: Deserializer<'de>,
{
	Ok(Color::parse_hex(
		&<String>::deserialize(deserializer)?
	).map_err(serde::de::Error::custom)?)
}

#[derive(Clone, PartialEq, Debug)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Color {
    pub fn parse_hex(value: &str) -> Result<Self, String> {
        let value = if let Some(stripped) = value.strip_prefix('#') {
            stripped
        } else {
            value
        };
        match value.len() {
            // RRGGBB
            6 => Ok(Self {
                red: u8::from_str_radix(&value[0..2], 16).or(Err("Invalid red value".to_string()))?,
                green: u8::from_str_radix(&value[2..4], 16)
                    .or(Err("Invalid green value".to_string()))?,
                blue: u8::from_str_radix(&value[4..6], 16)
                    .or(Err("Invalid blue value".to_string()))?,
            }),
            // RGB
            3 => Ok(Self {
                red: u8::from_str_radix(&value[0..1], 16).or(Err("Invalid red value".to_string()))?
                    * 17,
                green: u8::from_str_radix(&value[1..2], 16)
                    .or(Err("Invalid green value".to_string()))?
                    * 17,
                blue: u8::from_str_radix(&value[2..3], 16)
                    .or(Err("Invalid blue value".to_string()))?
                    * 17,
            }),
            _ => {
                Err("Invalid length for hex string, expected RGB or RRGGBB".into())
            }
        }
    }

    pub fn white() -> Self {
        Self {
            red: 255,
            green: 255,
            blue: 255,
        }
    }
}

#[derive(Deserialize, PartialEq)]
pub struct BitmapFont {
	#[serde(skip)]
	pub name: String,
	pub path: PathBuf,
	pub charset: Option<String>,
	pub size: u32,
	#[serde(default)]
	pub outline: u32,
	#[serde(default = "Color::white", deserialize_with = "parse_color")]
	pub color: Color,
}

#[derive(Default, Deserialize, PartialEq)]
pub struct ModResources {
	#[serde(deserialize_with = "parse_glob", default = "Vec::new")]
	pub libraries: Vec<PathBuf>,

	#[serde(deserialize_with = "parse_glob", default = "Vec::new")]
	pub files: Vec<PathBuf>,

	#[serde(deserialize_with = "parse_spritesheets", default = "HashMap::new")]
	pub spritesheets: HashMap<String, SpriteSheet>,

	#[serde(deserialize_with = "parse_glob", default = "Vec::new")]
	pub sprites: Vec<PathBuf>,

	#[serde(deserialize_with = "parse_fonts", default = "HashMap::new")]
	pub fonts: HashMap<String, BitmapFont>,
}

#[derive(Default, Deserialize, PartialEq)]
pub struct Dependency {
	pub id: String,
	#[serde(deserialize_with = "parse_comparable_version")]
	pub version: VersionReq,
	#[serde(default)]
	pub required: bool,
}

#[derive(Default, Deserialize, PartialEq)]
pub struct ModApi {
	#[serde(deserialize_with = "parse_glob_rel")]
	pub include: Vec<PathBuf>,
}

#[derive(Deserialize, PartialEq)]
pub struct ModFileInfo {
	#[serde(deserialize_with = "parse_version")]
	pub geode: Version,
	pub id: String,
	pub name: String,
	#[serde(deserialize_with = "parse_version")]
	pub version: Version,
	pub developer: String,
	pub description: String,
	#[serde(default)]
	pub resources: ModResources,
	#[serde(default)]
	pub dependencies: Vec<Dependency>,
	pub api: Option<ModApi>,
}

pub fn try_parse_mod_info(root_path: &Path) -> Result<ModFileInfo, String> {
	let data = if root_path.is_dir() {
		std::fs::read_to_string(root_path.join("mod.json"))
		.map_err(|e| format!("Unable to read mod.json: {e}"))?
	} else {
		let mut out = String::new();

		zip::ZipArchive::new(fs::File::open(root_path).unwrap())
			.map_err(|e| format!("Unable to unzip: {e}"))?
			.by_name("mod.json")
			.map_err(|e| format!("Unable to find mod.json in package: {e}"))?
			.read_to_string(&mut out)
			.map_err(|e| format!("Unable to read mod.json: {e}"))?;

		out
	};

	// to make globs work, relink current directory to the one mod.json is in
	let old = std::env::current_dir().or(Err("Unable to get current directory"))?;

	std::env::set_current_dir(
		if root_path.is_dir() {
			root_path
		} else {
			root_path.parent().unwrap()
		}
	).or(Err("Unable to relink working directory"))?;
	
	let res = serde_json::from_str(&data)
		.map_err(|e| format!("Could not parse mod.json: {e}"))?;
	
	// then link it back to where-ever it was
	std::env::set_current_dir(old).or(Err("Unable to reset working directory"))?;

	Ok(res)
}

pub fn parse_mod_info(root_path: &Path) -> ModFileInfo {
	try_parse_mod_info(root_path).nice_unwrap("Failed to parse mod.json")
}
