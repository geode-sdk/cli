use std::path::Path;
use colored::Colorize;

use git2::Repository;

use std::io::{Result, Error, ErrorKind};
use std::path::PathBuf;
use std::fs;
use crate::package;

fn geode_library(install_path: &Path) -> PathBuf {
	if cfg!(target_os = "macos") {
		install_path.join("Frameworks")
	} else {
		install_path.to_path_buf()
	}
}

fn init_bin_repo() -> std::result::Result<(PathBuf, Repository), Box<dyn std::error::Error>> {
	let bin_path = home::home_dir().unwrap().join(".geode_bin");

	let repo = if !bin_path.exists() {
		Repository::clone("https://github.com/geode-sdk/bin", &bin_path)?
	} else {
		Repository::open(&bin_path)?
	};

    repo.find_remote("origin").unwrap().fetch(&["main"], None, None).unwrap();
    let fetch_head = repo.find_reference("FETCH_HEAD").unwrap();

    repo.set_head_detached(fetch_head.target().unwrap()).unwrap();
    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force())).unwrap();

    drop(fetch_head);
    Ok((bin_path, repo))
}

fn check_update_needed(specific_version: Option<&str>, install_path: &Path) -> Result<Option<(String, PathBuf)>> {
	let (bin_dir, bin_repo) = match init_bin_repo() {
	    Ok(r) => r,
	    Err(e) => {
	    	return Err(Error::new(ErrorKind::Other, format!("failed to fetch update: {}", e)))
	    },
	};
	
	let mut last_name = String::new();

	match specific_version {
		Some(s) => { last_name = s.to_string(); },
		None => {
			bin_repo.tag_foreach(|_, name| {
				last_name = String::from_utf8(name.to_vec()).unwrap();
				true
			}).unwrap();
		}
	}

	if last_name != "latest" {
		let (object, _) = bin_repo.revparse_ext(&last_name).unwrap();
		bin_repo.checkout_tree(&object, None).unwrap();
		bin_repo.set_head(&last_name).unwrap();
	}

	let new_library_path = bin_dir.join(package::platform_string().to_string()).join("geode".to_string() + package::platform_extension());
	let old_library_path = geode_library(install_path).join("geode".to_string() + package::platform_extension());

	if 
		!old_library_path.exists()
		|| (sha256::digest_file(&new_library_path).unwrap() != sha256::digest_file(&old_library_path).unwrap())
	{
		return Ok(Some((last_name, new_library_path.parent().unwrap().to_path_buf())));
	}
	Ok(None)
}

fn copy_files(src_path: &Path, dest_path: &Path) -> Result<()> {

	let geode_file = "Geode".to_string() + package::platform_extension();
	fs::copy(src_path.join(&geode_file), dest_path.join(&geode_file))?;

	let geode_injector = if cfg!(target_os = "macos") {
		"libfmod.dylib"
	} else if cfg!(windows) {
		"XInput9_1_0.dll"
	} else {
		unimplemented!("injector file");
	};

	fs::copy(src_path.join(&geode_injector), dest_path.join(&geode_injector))?;
	Ok(())
}

pub fn check_update(version: Option<&str>, install_path: &Path) -> Result<bool> {
	let b = check_update_needed(version, install_path)?;

	match b {
		Some(_) => {
			//println!("{} {}", "Update available: ".bright_magenta().bold(), name.blue().bold());
			Ok(true)
		}
		None => Ok(false)//Err(Error::new(ErrorKind::Other, "No update found."))
	}
}

pub fn update_geode(version: Option<&str>, install_path: &Path) -> Result<()> {
	let b = check_update_needed(version, install_path)?;

	match b {
		Some((n, ref p)) => {
			println!("{} {}", "Downloaded update ".bright_cyan(), n.green().bold());

			copy_files(p, &geode_library(install_path))?;

			println!("{}", "Sucessfully updated Geode".bold());
			Ok(())
		},

		None => Err(Error::new(ErrorKind::Other, "Geode has no pending updates"))
	}
	//unimplemented!("the");
}
