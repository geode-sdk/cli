use std::fs;
use std::path::Path;

use reqwest::blocking::get;

pub fn install_geode(
	exe: &Path,
	nightly: bool
) -> Result<(), Box<dyn std::error::Error>> {
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
		src_dir.push("windows");
		exe.parent().unwrap().to_path_buf()
	} else {
		src_dir.push("macos");
		exe.join("Contents")
	}.join("geode").join("mods");

	let loader_dir = if cfg!(windows) {
		exe.parent().unwrap().to_path_buf()
	} else {
		exe.join("Contents").join("Frameworks")
	};


	let resp = get(url)?.bytes()?;

	let mut archive = zip::ZipArchive::new(std::io::Cursor::new(resp))?;
	archive.extract(&src_dir).unwrap();


	fs::copy(src_dir.join("GeodeAPI.geode"), mod_dir)?;

	if cfg!(windows) {
		fs::copy(src_dir.join("geode.dll"), &loader_dir)?;
		fs::copy(src_dir.join("XInput9_1_0.dll"), &loader_dir)?;
		fs::write(loader_dir.join("steam_appid.txt"), "322170")?;
	} else {
		fs::copy(src_dir.join("Geode.dylib"), &loader_dir)?;

		if !loader_dir.join("dontdelete_fmod.dylib").exists() {
			fs::copy(loader_dir.join("libfmod.dylib"), loader_dir.join("dontdelete_fmod.dylib"))?;
		}

		fs::copy(src_dir.join("libfmod.dylib"), &loader_dir)?;
	}

	Ok(())
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
