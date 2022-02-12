use colored::Colorize;
use crate::print_error;
use git2::Repository;
use std::io::{Result, Error, ErrorKind};
use std::path::PathBuf;
use std::fs;
use sysinfo::{ProcessExt, System, SystemExt};
use crate::package;

use crate::config::Configuration;

pub fn figure_out_gd_path() -> Result<PathBuf> {
    let mut sys = System::new();
    sys.refresh_processes();

    let mut gd_procs;

    if cfg!(windows) {
    	gd_procs = sys.processes_by_exact_name("GeometryDash.exe");
    } else if cfg!(target_os = "ios") {
    	match std::env::var("HOME") {
    		Ok(val) => {
    			return Ok(PathBuf::from(val));
    		},
    		Err(_) => return Err(Error::new(ErrorKind::Other, "Could not fetch $HOME variable, please set it."))
    	}
    } else {
    	// TODO: Check if in other systems can detect it like this.
    	gd_procs = sys.processes_by_exact_name("Geometry Dash");
    }


    let gd_proc = match gd_procs.next() {
        Some(e) => e,
        None => return Err(Error::new(ErrorKind::Other, "Please re-run with Geometry Dash open")),
    };

    if gd_procs.next().is_some() { 
    	return Err(Error::new(
    		ErrorKind::Other,
    		"It seems there is more than one instance of Geometry Dash open. Please re-run with only one instance."
    	));
    }

    let mut p = PathBuf::from(gd_proc.exe()).parent().unwrap().to_path_buf();

    if cfg!(target_os = "macos") {
        p = p.parent().unwrap().to_path_buf();
    }
    Ok(p)
}

fn geode_library() -> PathBuf {
	if cfg!(target_os = "macos") {
		Configuration::install_path().join("Frameworks")
	} else {
		Configuration::install_path().to_path_buf()
	}
}

fn check_update_needed(specific_version: Option<String>) -> Option<(String, PathBuf)> {
	let tmp_update = std::env::temp_dir().join("geode_update");

	if tmp_update.exists() {
	    fs::remove_dir_all(&tmp_update).unwrap();
	}


	let bin_repo = match Repository::clone("https://github.com/geode-sdk/bin", &tmp_update) {
	    Ok(r) => r,
	    Err(e) => print_error!("failed to fetch update: {}", e),
	};
	
	let mut last_name = String::new();

	match specific_version {
		Some(s) => { last_name = s; },
		None => {
			bin_repo.tag_foreach(|_, name| {
				last_name = String::from_utf8(name.to_vec()).unwrap();
				true
			}).unwrap();
		}
	}

	let (object, _) = bin_repo.revparse_ext(&last_name).unwrap();
	bin_repo.checkout_tree(&object, None).unwrap();
	bin_repo.set_head(&last_name).unwrap();


	let new_library_path = tmp_update.join(package::platform_string().to_string()).join("geode".to_string() + package::platform_extension());
	let old_library_path = geode_library().join("geode".to_string() + package::platform_extension());

	if 
		!old_library_path.exists()
		|| (sha256::digest_file(&new_library_path).unwrap() != sha256::digest_file(&old_library_path).unwrap())
	{
		return Some((last_name, new_library_path.parent().unwrap().to_path_buf()));
	}
	None
}

pub fn check_update(version: Option<String>) {
	let b = check_update_needed(version);

	match b {
		Some((name, _)) => {
			println!("{} {}", "Update available: ".bright_magenta().bold(), name.blue().bold());
		}
		None => print_error!("No update found.")
	}
}

pub fn update_geode(version: Option<String>) {
	let b = check_update_needed(version);

	match b {
		Some((n, ref p)) => {
			println!("{} {}", "Downloaded update ".bright_cyan(), n.green().bold());
			for file in fs::read_dir(p).unwrap() {
				let fname = file.unwrap().file_name().clone().to_str().unwrap().to_string();
				fs::copy(p.join(&fname), geode_library().join(&fname)).expect("Unable to copy geode to correct directory");
			}
			println!("{}", "Sucessfully updated Geode".bold());
		},

		None => print_error!("Geode has no pending updates")
	}
	//unimplemented!("the");
}