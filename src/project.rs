
use std::{fs, path::{PathBuf, Path}};
use clap::Subcommand;
use crate::{util::{config::Config, mod_file::parse_mod_info}, package::get_working_dir, done, warn};

#[derive(Subcommand, Debug)]
#[clap(rename_all = "kebab-case")]
pub enum Project {
    /// Clear this project's cached resource files
    ClearCache,
}

fn find_build_directory(root: &Path, _config: &Config) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        // this works for 99% of users. 
        // if you want to parse the CMakeLists.txt file to find the true build 
        // directory 100% of the time, go ahead, but i'm not doing it
        for path in [
            root.join("build").join("RelWithDebInfo"),
            root.join("build").join("Release"),
            root.join("build").join("MinSizeRel"),
        ] {
            if path.exists() {
                return Some(path);
            }
        }
    }
    None
}

fn clear_cache(config: &Config) {
	// Parse mod.json
	let mod_info = parse_mod_info(&std::env::current_dir().unwrap());

    // Remove cache directory
	let workdir = get_working_dir(&mod_info.id);
	fs::remove_dir_all(workdir).expect("Unable to remove cache directory");

    // Remove cached .geode package
    let dir = find_build_directory(&std::env::current_dir().unwrap(), config);
    if let Some(dir) = dir {
        for file in fs::read_dir(&dir).expect("Unable to read build directory") {
            let path = file.unwrap().path();
            let Some(ext) = path.extension() else { continue };
            if ext == "geode" {
                fs::remove_file(path).expect("Unable to delete cached .geode package");
            }
        }
    }
    else {
        warn!(
            "Unable to find cached .geode package, can't clear it. It might be \
            that this is not supported on the current platform, or that your \
            build directory has a different name"
        );
    }

	done!("Cache for {} cleared", mod_info.id);
}

pub fn subcommand(config: &mut Config, cmd: Project) {
	match cmd {
		Project::ClearCache => clear_cache(config),
	}
}
