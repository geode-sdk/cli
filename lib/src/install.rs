use std::fs;
use std::path::Path;

use reqwest::blocking::get;
use crate::VersionInfo;
use crate::InstallInfo;
use serde_json;

use crate::ProgressCallback;
use crate::string2c;

pub fn install_geode(
	exe: &Path,
	nightly: bool,
	api: bool,
	callback: ProgressCallback
) -> Result<InstallInfo, Box<dyn std::error::Error>> {

	let url = if nightly {
		"https://github.com/geode-sdk/suite/archive/refs/heads/nightly.zip"
	} else {
		"https://github.com/geode-sdk/suite/archive/refs/heads/main.zip"
	};

	let mut src_dir = std::env::temp_dir().join("Geode");
	if src_dir.exists() {
		fs::remove_dir_all(&src_dir).unwrap();
	}
	fs::create_dir(&src_dir).unwrap();

	let mod_dir = if cfg!(windows) {
		exe.parent().unwrap().to_path_buf()
	} else {
		exe.join("Contents")
	}.join("geode").join("mods");

	let loader_dir = if cfg!(windows) {
		exe.parent().unwrap().to_path_buf()
	} else {
		exe.join("Contents").join("Frameworks")
	};

	unsafe {
		callback(string2c("Downloading"), 0);
	}

	// todo: figure out some way to gauge get progress
	let resp = get(url)?.bytes()?;

	unsafe {
		callback(string2c("Installing"), 96);
	}
	
	fs::create_dir_all(&mod_dir)?;

	let mut archive = zip::ZipArchive::new(std::io::Cursor::new(resp))?;
	archive.extract(&src_dir).unwrap();
	
	// idk how to access first element of 
	// iterator in rust and frankly idc
	for entry in src_dir.read_dir()? {
		src_dir.push(entry?.file_name());
	}

	if cfg!(windows) {
		src_dir.push("windows");
	} else if cfg!(target_os = "macos") {
		src_dir.push("macos");
	} else {
		panic!("Not implemented for this platform");
	}

	unsafe {
		callback(string2c("Copying files"), 97);
	}

	if api {
		fs::copy(src_dir.join("GeodeAPI.geode"), &mod_dir.join("GeodeAPI.geode"))?;
	}

	if cfg!(windows) {
		fs::copy(src_dir.join("geode.dll"), &loader_dir.join("geode.dll"))?;
		fs::copy(src_dir.join("XInput9_1_0.dll"), &loader_dir.join("XInput9_1_0.dll"))?;
		fs::write(loader_dir.join("steam_appid.txt"), "322170")?;
	} else {
		fs::copy(src_dir.join("Geode.dylib"), &loader_dir.join("Geode.dylib"))?;

		if !loader_dir.join("dontdelete_fmod.dylib").exists() {
			fs::copy(loader_dir.join("libfmod.dylib"), loader_dir.join("dontdelete_fmod.dylib"))?;
		}

		fs::copy(src_dir.join("libfmod.dylib"), &loader_dir.join("libfmod.dylib"))?;
	}

	unsafe {
		callback(string2c("Finishing"), 98);
	}

	src_dir.pop();
	let versions_json = match serde_json::from_str(
		&fs::read_to_string(src_dir.join("versions.json")).unwrap()
	) {
		Ok(p) => p,
		Err(_) => serde_json::Value::default()
	};

	unsafe {
		callback(string2c("Cleaning up"), 99);
	}

	fs::remove_dir_all(std::env::temp_dir().join("Geode"))?;

	Ok(InstallInfo {
		loader_version: VersionInfo::from_string(
			&versions_json["loader"].as_str().unwrap().to_string()
		),
		api_version: VersionInfo::from_string(
			&versions_json["api"].as_str().unwrap().to_string()
		),
	})
}

#[cfg(windows)]
pub fn uninstall_geode(exe: &Path) -> std::io::Result<()> {
	if exe.join("XInput9_1_0.dll").exists() {
		fs::remove_file(exe.join("XInput9_1_0.dll"))?;
	}
	if exe.join("geode.dll").exists() {
		fs::remove_file(exe.join("geode.dll"))?;
	}
	if exe.join("geode").exists() {
		fs::remove_dir_all(exe.join("geode"))?;
	}

	Ok(())
}

#[cfg(not(windows))]
pub fn uninstall_geode(exe: &Path) -> std::io::Result<()> {
	let frameworks = exe.join("Contents").join("Frameworks");
	let contents = exe.join("Contents");

	if !frameworks.join("dontdelete_fmod.dylib").exists() {
		return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Can't find backup libfmod.dylib, unable to install!"));
	}
	if frameworks.join("Geode.dylib").exists() {
		fs::remove_file(frameworks.join("geode.dll"))?;
	}
	if contents.join("geode").exists() {
		fs::remove_dir_all(contents.join("geode"))?;
	}

	fs::copy(frameworks.join("dontdelete_fmod.dylib"), frameworks.join("libfmod.dylib"))?;

	Ok(())
}
