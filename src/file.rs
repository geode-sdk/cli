use std::path::PathBuf;
use std::{fs, io};

/// Return all files in directory and subdirectories
pub fn read_dir_recursive(src: &PathBuf) -> Result<Vec<PathBuf>, io::Error> {
	let mut res = Vec::new();
	for item in fs::read_dir(src)? {
		let path = item?.path();
		if path.is_dir() {
			res.extend(read_dir_recursive(&path)?);
			res.push(path);
		} else {
			res.push(path);
		}
	}
	Ok(res)
}

pub fn copy_dir_recursive(src: &PathBuf, dest: &PathBuf) -> Result<(), io::Error> {
	fs::create_dir_all(dest)?;
	for item in fs::read_dir(src)? {
		let item_path = item?.path();
		let dest_path = dest.join(item_path.file_name().unwrap());
		if item_path.is_dir() {
			copy_dir_recursive(&item_path, &dest_path)?;
		} else {
			fs::copy(&item_path, &dest_path)?;
		}
	}
	Ok(())
}
