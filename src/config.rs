use std::process::exit;
use std::path::Path;
use std::path::PathBuf;
use std::fs;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Configuration {
    pub install_path: Option<PathBuf>, // only option because i dont wanna deal with lazy_static
    pub current_version: Option<String>
}

static mut CONFIG: Configuration = Configuration {
	install_path: None,
	current_version: None
};
static mut CONFIG_DONE: bool = false;

impl Configuration {

	// its fineeeee
	pub fn get() -> &'static mut Configuration {
		unsafe {
			if !CONFIG_DONE {
				let exe_path = std::env::current_exe().unwrap();
				let save_dir = exe_path.parent().unwrap();
				let save_file = save_dir.join("config.json");

				if save_file.exists() {
				    let raw = fs::read_to_string(&save_file).unwrap();
				    CONFIG = match serde_json::from_str(&raw) {
				        Ok(p) => p,
				        Err(_) => CONFIG.clone()
				    }
				}

				if CONFIG.install_path.is_none() {
				    match crate::install::figure_out_gd_path() {
				        Ok(install_path) => {
				            CONFIG.install_path = Some(install_path);
				            println!("Loaded default GD path automatically: {:?}", CONFIG.install_path.as_ref().unwrap());
				        },
				        Err(err) => {
				            println!("Unable to figure out GD path: {}", err);
				            exit(1);
				        },
				    }
				}

				let raw = serde_json::to_string(&CONFIG).unwrap();
				fs::write(save_file, raw).unwrap();

				CONFIG_DONE = true;
			}
			return &mut CONFIG;
		}
	}

	pub fn save_config() {
		unsafe {
			let exe_path = std::env::current_exe().unwrap();
			let save_dir = exe_path.parent().unwrap();
			let save_file = save_dir.join("config.json");

			let raw = serde_json::to_string(&CONFIG).unwrap();
			fs::write(save_file, raw).unwrap();
		}
	}

	pub fn set_install_path(f: PathBuf) {
		Configuration::get().install_path = Some(f);
		Configuration::save_config();
	}

	pub fn install_path() -> &'static Path {
		&Configuration::get().install_path.as_ref().unwrap()
	}
}
