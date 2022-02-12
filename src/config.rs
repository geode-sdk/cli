use std::process::exit;
use std::path::Path;
use std::path::PathBuf;
use std::fs;
use std::io;
use std::env::current_exe;
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
				    match crate::update::figure_out_gd_path() {
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

	pub fn install_file_associations() -> io::Result<()> {
		if cfg!(windows) {
			use winreg::{enums::*, RegKey};
			use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_DWORD, SHCNF_FLUSH};
	
			let exe_path = current_exe()?;
			// let exe_name = exe_path
			//     .file_name()
			//     .map(|s| s.to_str())
			//     .flatten()
			//     .unwrap_or_default()
			//     .to_owned();
			let exe_path = exe_path.to_str().unwrap_or_default().to_owned();
	
			let icon_path = format!("\"{}\",0", exe_path);
			let open_command = format!("\"{}\" install \"%1\"", exe_path);
	
			let hkcu = RegKey::predef(HKEY_CURRENT_USER);
	
			const PROGID_CLASS_PATH: &str = r"SOFTWARE\Classes\Geode.CLI";
			let (progid_class, _) = hkcu.create_subkey(PROGID_CLASS_PATH)?;
			progid_class.set_value("", &"Geode Mod")?;
	
			let (progid_class_defaulticon, _) = progid_class.create_subkey("DefaultIcon")?;
			progid_class_defaulticon.set_value("", &icon_path)?;
	
			let (progid_class_shell_open_command, _) = progid_class.create_subkey(r"shell\open\command")?;
			progid_class_shell_open_command.set_value("", &open_command)?;
	
			const EXTENSION_CLASS_PATH: &str = r"SOFTWARE\Classes\.geode";
	
			let (extension_class, _) = hkcu.create_subkey(EXTENSION_CLASS_PATH)?;
			extension_class.set_value("", &"Geode.CLI")?;
	
			unsafe {
				SHChangeNotify(
					SHCNE_ASSOCCHANGED,
					SHCNF_DWORD | SHCNF_FLUSH,
					std::ptr::null_mut(),
					std::ptr::null_mut(),
				);
			}
	
			Ok(())
		} else {
			Err(io::Error::new(io::ErrorKind::Other, "Unimplemented file association command for os"))
		}
	}
}

