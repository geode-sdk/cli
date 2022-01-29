// use resize::{self};
use std::fs;
use std::path::{PathBuf};

fn resize_image(src: PathBuf, dest: PathBuf) {
    println!("Resizing {} -> {}", src.to_str().unwrap(), dest.to_str().unwrap());
    // let mut resizer = resize::new();
}

pub fn process_resources_recurse(src: &PathBuf, dest: &PathBuf) {
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let data = fs::metadata(&path).unwrap();
        
        if data.is_dir() {
            let path_dest = dest.join(path.file_stem().unwrap());
            process_resources_recurse(&path, &path_dest);
        } else if data.is_file() {
            if path.extension().unwrap() == "png" {
                let path_dest = dest.join(path.file_name().unwrap());
                resize_image(path, path_dest);
            }
        }
    }
}

pub fn process_resources(src: PathBuf, dest: Option<PathBuf>) {
    let actual_dest = dest.unwrap_or(src.clone());
    process_resources_recurse(&src, &actual_dest);
}
