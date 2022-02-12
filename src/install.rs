use std::path::PathBuf;
use std::fs;

use crate::{Configuration};

pub fn install(path: &PathBuf) {
    let mut target_path = Configuration::install_path().join("geode").join("mods");
    target_path = target_path.join(path.file_name().unwrap());

    fs::rename(path, target_path).unwrap();
}