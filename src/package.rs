use crate::config::Config;
use std::path::Path;
use std::fs;

use crate::{fatal, done};

pub fn install(config: &mut Config, pkg_path: &Path) {
    let mod_path = config.get_profile(&config.current_profile)
    	.unwrap_or_else(|| fatal!("No current profile to install to!"))
    	.borrow().gd_path
    	.join("geode")
    	.join("mods");

    if !mod_path.exists() {
        fs::create_dir_all(&mod_path)
        	.unwrap_or_else(|e| fatal!("Could not setup mod installation: {}", e));
    }

	fs::copy(pkg_path, mod_path.join(pkg_path.file_name().unwrap()))
		.unwrap_or_else(|e| fatal!("Could not install mod: {}", e));

    done!("Installed {}", pkg_path.file_name().unwrap().to_str().unwrap());
}
