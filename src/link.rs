use std::ffi::CStr;
use std::os::raw::c_char;

#[repr(C)]
pub struct CPackInfo {
	pub suffix_removals: u32,
	pub created_files: *mut *const c_char
}

impl CPackInfo {
	pub fn get_files(&self) -> Vec<String> {
		unsafe {
			let mut out: Vec<String> = vec![];

			let sl = std::slice::from_raw_parts_mut(self.created_files, self.suffix_removals as usize);

			for file_idx in 0..self.suffix_removals {
				out.push(CStr::from_ptr(sl[file_idx as usize]).to_str().unwrap().to_string());
			}

			out
		}
	}
}

// there is a solution with cfg_if crate but imlazy

#[cfg(windows)]
#[link(name = "geodeutils.dll")]
extern "C" {
	pub fn geode_update(
		location: *const c_char,
		version: *const c_char
	) -> *const c_char;

	pub fn geode_update_check(
		location: *const c_char,
		version: *const c_char,
		has_update: *mut bool
	) -> *const c_char;

	pub fn geode_version() -> i32;

	pub fn geode_create_template(
		project_location: *const c_char,
		name: *const c_char,
		version: *const c_char,
		id: *const c_char,
		developer: *const c_char,
		description: *const c_char
	) -> *const c_char;

	pub fn geode_package(
	    resource_dir: *const c_char,
	    exec_dir: *const c_char,
	    out_file: *const c_char,
	    log: bool,
	    use_cached_resources: bool,
	) -> *const c_char;

	pub fn geode_amend_package(
		geode_file: *const c_char,
		file_to_add: *const c_char,
		dir_in_zip: *const c_char
	) -> *const c_char;

	pub fn geode_install_package(
		out_file: *const c_char,
		install_path: *const c_char
	) -> *const c_char;

	pub fn geode_sprite_sheet(
		in_dir: *const c_char,
		out_dir: *const c_char,
		create_variants: bool,
		name: *const c_char, // can be null
		prefix: *const c_char, // can be null
		pack_info: *mut CPackInfo
	) -> *const c_char;


	pub fn geode_sprite_variants(
		file: *const c_char,
		out_dir: *const c_char,
		prefix: *const c_char // can be null
	) -> *const c_char;

	pub fn geode_create_bitmap_font_from_ttf(
		ttf_path: *const c_char,
		out_dir: *const c_char,
		name: *const c_char, // can be null
		fontsize: u32,
		prefix: *const c_char, // can be null
		create_variants: bool,
		charset: *const c_char, // can be null
		outline: u32,
	) -> *const c_char;
}

#[cfg(not(windows))]
#[link(name = "geodeutils")]
extern "C" {
	pub fn geode_update(
		location: *const c_char,
		version: *const c_char
	) -> *const c_char;

	pub fn geode_update_check(
		location: *const c_char,
		version: *const c_char,
		has_update: *mut bool
	) -> *const c_char;

	pub fn geode_version() -> i32;

	pub fn geode_create_template(
		project_location: *const c_char,
		name: *const c_char,
		version: *const c_char,
		id: *const c_char,
		developer: *const c_char,
		description: *const c_char
	) -> *const c_char;

	pub fn geode_package(
	    resource_dir: *const c_char,
	    exec_dir: *const c_char,
	    out_file: *const c_char,
	    log: bool,
	    use_cached_resources: bool,
	) -> *const c_char;

	pub fn geode_install_package(
		out_file: *const c_char,
		install_path: *const c_char
	) -> *const c_char;

	pub fn geode_sprite_sheet(
		in_dir: *const c_char,
		out_dir: *const c_char,
		create_variants: bool,
		name: *const c_char, // can be null
		prefix: *const c_char, // can be null
		pack_info: *mut CPackInfo
	) -> *const c_char;


	pub fn geode_sprite_variants(
		file: *const c_char,
		out_dir: *const c_char,
		prefix: *const c_char // can be null
	) -> *const c_char;

	pub fn geode_create_bitmap_font_from_ttf(
		ttf_path: *const c_char,
		out_dir: *const c_char,
		name: *const c_char, // can be null
		fontsize: u32,
		prefix: *const c_char, // can be null
		create_variants: bool,
		charset: *const c_char, // can be null
		outline: u32,
	) -> *const c_char;
}

#[macro_export]
macro_rules! call_extern {
	($x: expr) => {{
		unsafe {
			let y = $x;
			if !(y == std::ptr::null()) {
				println!("Extern function call failed: {}", std::ffi::CStr::from_ptr(y).to_str().unwrap().red());
			}
		}
	}}
}

pub unsafe fn string2c<E>(err: E) -> *mut c_char
where E: ToString {
    let mut bytes = err.to_string().into_bytes();
    bytes.push(0);
    let desc = bytes.iter().map(|b| *b as c_char).collect::<Vec<c_char>>().as_mut_ptr();
    let new = libc::malloc(bytes.len()) as *mut c_char;
    libc::strcpy(new, desc);

    return new;
}

pub unsafe fn opt2c<E>(err: Option<E>) -> *mut c_char
where E: ToString {
	match err {
		Some(x) => string2c(x),
		None => std::ptr::null_mut()
	}
}
