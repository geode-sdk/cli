use crate::spritesheet::SpriteSheet;
use crate::NiceUnwrap;
use clap::ValueEnum;
use semver::{Version, VersionReq};
use serde::{Deserialize, Deserializer, de::Error};
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use vec1::Vec1;

trait Glob {
	fn glob(self) -> Self;
}

impl Glob for Vec<PathBuf> {
	fn glob(self) -> Self {
		self.into_iter()
			.flat_map(|src| {
				glob::glob(
					std::env::current_dir()
						.unwrap()
						.join(&src)
						.to_str()
						.unwrap(),
				)
				.nice_unwrap(format!("Invalid glob pattern {}", src.to_str().unwrap()))
				.map(|g| g.unwrap())
			})
			.collect()
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
		.map(|p| {
			p.strip_prefix(std::env::current_dir().unwrap())
				.unwrap_or(&p)
				.to_path_buf()
		})
		.collect())
}

fn parse_spritesheets<'de, D>(deserializer: D) -> Result<HashMap<String, SpriteSheet>, D::Error>
where
	D: Deserializer<'de>,
{
	Ok(HashMap::<String, Vec<PathBuf>>::deserialize(deserializer)?
		.into_iter()
		.map(|(name, srcs)| {
			(
				name.clone(),
				SpriteSheet {
					name,
					files: srcs.glob(),
				},
			)
		})
		.collect())
}

fn parse_version<'de, D>(deserializer: D) -> Result<Version, D::Error>
where
	D: Deserializer<'de>,
{
	// semver doesn't accept "v" prefixes and the string will be validated at
	// runtime by Geode anyway so let's just crudely remove all 'v's for now
	Version::parse(&<String>::deserialize(deserializer)?.replace('v', ""))
		.map_err(serde::de::Error::custom)
}

fn parse_comparable_version<'de, D>(deserializer: D) -> Result<VersionReq, D::Error>
where
	D: Deserializer<'de>,
{
	// semver doesn't accept "v" prefixes and the string will be validated at
	// runtime by Geode anyway so let's just crudely remove all 'v's for now
	let str = <String>::deserialize(deserializer)?.replace('v', "");
	// semver defaults to ^, geode defaults to >=
	let actual_equal = str.starts_with('=');
	VersionReq::parse(&str)
		.map_err(serde::de::Error::custom)
		.map(|mut v| {
			// in practice there should only be one comparator.. oh well
			for c in &mut v.comparators {
				if c.op == semver::Op::Caret && !actual_equal {
					c.op = semver::Op::GreaterEq;
				}
			}
			v
		})
}

pub trait ToGeodeString {
	fn to_geode_string(&self) -> String;
}

impl ToGeodeString for VersionReq {
	fn to_geode_string(&self) -> String {
		// geode uses = instead of ^ for exact version
		self.to_string().replace('^', "=")
	}
}

fn parse_fonts<'de, D>(deserializer: D) -> Result<HashMap<String, BitmapFont>, D::Error>
where
	D: Deserializer<'de>,
{
	Ok(<HashMap<String, BitmapFont>>::deserialize(deserializer)?
		.into_iter()
		.map(|(name, mut font)| {
			font.name.clone_from(&name);
			font.path = std::env::current_dir().unwrap().join(font.path);
			(name, font)
		})
		.collect())
}

fn parse_color<'de, D>(deserializer: D) -> Result<Color, D::Error>
where
	D: Deserializer<'de>,
{
	Color::parse_hex(&<String>::deserialize(deserializer)?).map_err(serde::de::Error::custom)
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
				red: u8::from_str_radix(&value[0..2], 16)
					.or(Err("Invalid red value".to_string()))?,
				green: u8::from_str_radix(&value[2..4], 16)
					.or(Err("Invalid green value".to_string()))?,
				blue: u8::from_str_radix(&value[4..6], 16)
					.or(Err("Invalid blue value".to_string()))?,
			}),
			// RGB
			3 => Ok(Self {
				red: u8::from_str_radix(&value[0..1], 16)
					.or(Err("Invalid red value".to_string()))?
					* 17,
				green: u8::from_str_radix(&value[1..2], 16)
					.or(Err("Invalid green value".to_string()))?
					* 17,
				blue: u8::from_str_radix(&value[2..3], 16)
					.or(Err("Invalid blue value".to_string()))?
					* 17,
			}),
			_ => Err("Invalid length for hex string, expected RGB or RRGGBB".into()),
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

#[derive(Debug, Deserialize, Hash, PartialEq, Eq, Clone, Copy, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum PlatformName {
	#[serde(rename = "win")]
	#[value(alias = "win")]
	Windows,
	#[value(alias = "mac")]
	MacOS,
	#[serde(rename = "mac-intel")]
	MacIntel,
	#[serde(rename = "mac-arm")]
	MacArm,
	Android,
	Android32,
	Android64,
}

impl PlatformName {
	pub fn current() -> Option<PlatformName> {
		if cfg!(target_os = "windows") {
			Some(PlatformName::Windows)
		} else if cfg!(target_os = "android") {
			Some(PlatformName::Android64)
		} else if cfg!(target_os = "linux") {
			Some(PlatformName::Windows)
		} else if cfg!(target_os = "macos") {
			Some(PlatformName::MacOS)
		} else {
			None
		}
	}
}

impl Display for PlatformName {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		use PlatformName as P;
		f.write_str(match self {
			P::Windows => "win",
			P::MacOS => "mac",
			P::MacIntel => "mac-intel",
			P::MacArm => "mac-arm",
			P::Android => "android",
			P::Android32 => "android32",
			P::Android64 => "android64",
		})
	}
}

fn all_platforms() -> HashSet<PlatformName> {
	use PlatformName as P;
	HashSet::from([
		P::Windows,
		P::MacOS,
		P::MacIntel,
		P::MacArm,
		P::Android,
		P::Android32,
		P::Android64,
	])
}

#[derive(Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum DependencyImportance {
	#[default]
	Required,
	Recommended,
	Suggested,
}

#[derive(Deserialize, PartialEq)]
pub struct Dependency {
	#[serde(skip)]
	pub id: String,
	#[serde(deserialize_with = "parse_comparable_version")]
	pub version: VersionReq,
	#[serde(default)]
	pub importance: DependencyImportance,
	#[serde(default = "all_platforms")]
	pub platforms: HashSet<PlatformName>,
}

#[derive(Deserialize, PartialEq)]
pub struct LegacyDependency {
	pub id: String,
	#[serde(deserialize_with = "parse_comparable_version")]
	pub version: VersionReq,
	#[serde(default)]
	pub importance: DependencyImportance,
	#[serde(default = "all_platforms")]
	pub platforms: HashSet<PlatformName>,
}

#[derive(PartialEq)]
pub struct Dependencies(HashMap<String, Dependency>);

impl Dependencies {
	pub fn is_empty(&self) -> bool {
		self.0.is_empty()
	}
}

impl<'a> IntoIterator for &'a Dependencies {
	type IntoIter = std::collections::hash_map::Values<'a, String, Dependency>;
	type Item = &'a Dependency;
	fn into_iter(self) -> Self::IntoIter {
		self.0.values()
	}
}

// No it can't clippy Dependency doesn't impl Default
#[allow(clippy::derivable_impls)]
impl Default for Dependencies {
	fn default() -> Self {
		Self(HashMap::new())
	}
}

fn parse_dependencies<'de, D>(deserializer: D) -> Result<Dependencies, D::Error>
where
	D: Deserializer<'de>,
{
	// This is all to avoid union types having terrible errors 
	// (they just log "failed to parse any variant of X")

	// This is needed because deserializer is moved
	let value = serde_json::Value::deserialize(deserializer)?;
	
	match <HashMap<String, serde_json::Value>>::deserialize(value.clone()) {
		Ok(deps) => Ok(Dependencies(
			deps.into_iter().map(|(id, json)| {
				// Shorthand is just "[mod.id]": "[version]"
				match parse_comparable_version(json.clone()) {
					Ok(version) => Ok(Dependency {
						id: id.clone(),
						version,
						importance: DependencyImportance::Required,
						platforms: all_platforms(),
					}),
					// Longhand is "[mod.id]": { ... }
					Err(_) => Dependency::deserialize(json)
						// The ID isn't parsed from the object itself but is the key
						.map(|mut d| { d.id.clone_from(&id); d })
						.map_err(D::Error::custom)
				}.map(|r| (id, r))
			}).collect::<Result<_, _>>()?
		)),
		Err(e) => {
			// Can be removed after Geode hits v5
			match <Vec<LegacyDependency>>::deserialize(value) {
				Ok(deps) => {
					let mut res = Dependencies::default();
					for dep in deps {
						res.0.insert(dep.id.clone(), Dependency {
							id: dep.id,
							version: dep.version,
							importance: dep.importance,
							platforms: dep.platforms
						});
					}
					Ok(res)
				}
				Err(_) => Err(D::Error::custom(e))
			}
		}
	}
}

#[derive(Default, Deserialize, PartialEq)]
pub struct ModApi {
	#[serde(deserialize_with = "parse_glob_rel")]
	pub include: Vec<PathBuf>,
}

#[derive(PartialEq)]
pub struct Developers {
	list: Vec1<String>,
}

impl<'de> Deserialize<'de> for Developers {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		#[derive(Deserialize)]
		struct Parse {
			pub developer: Option<String>,
			pub developers: Option<Vec1<String>>,
		}
		let parsed = Parse::deserialize(deserializer)?;
		match (parsed.developer, parsed.developers) {
			(Some(_), Some(_)) => Err(serde::de::Error::custom(
				"can not specify both \"developer\" and \"developers\"",
			))?,
			(Some(dev), None) => Ok(Self {
				list: Vec1::new(dev),
			}),
			(None, Some(list)) => Ok(Self { list }),
			(None, None) => Err(serde::de::Error::missing_field("developer"))?,
		}
	}
}

#[derive(Deserialize, PartialEq)]
pub struct ModFileInfo {
	#[serde(deserialize_with = "parse_version")]
	pub geode: Version,
	pub gd: GDVersion,
	pub id: String,
	pub name: String,
	#[serde(deserialize_with = "parse_version")]
	pub version: Version,
	#[serde(flatten)]
	pub developers: Developers,
	pub description: String,
	#[serde(default)]
	pub resources: ModResources,
	#[serde(default, deserialize_with = "parse_dependencies")]
	pub dependencies: Dependencies,
	pub api: Option<ModApi>,
}

#[derive(Deserialize, PartialEq)]
pub struct DetailedGDVersion {
	pub android: Option<String>,
	pub win: Option<String>,
	pub mac: Option<String>,
	pub ios: Option<String>,
}

#[derive(Deserialize, PartialEq)]
#[serde(untagged)]
pub enum GDVersion {
	Simple(String),
	Detailed(DetailedGDVersion),
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

	std::env::set_current_dir(if root_path.is_dir() {
		root_path
	} else {
		root_path.parent().unwrap()
	})
	.or(Err("Unable to relink working directory"))?;

	let res = serde_json::from_str(&data).map_err(|e| format!("Could not parse mod.json: {e}"))?;

	// then link it back to where-ever it was
	std::env::set_current_dir(old).or(Err("Unable to reset working directory"))?;

	Ok(res)
}

pub fn parse_mod_info(root_path: &Path) -> ModFileInfo {
	try_parse_mod_info(root_path).nice_unwrap("Failed to parse mod.json")
}
