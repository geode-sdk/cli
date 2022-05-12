#![allow(clippy::missing_safety_doc)]

pub mod util;
pub mod package;
pub mod template;
pub mod windows_ansi;
pub mod spritesheet;
pub mod dither;
pub mod font;
pub mod suite;

use std::path::Path;

pub const GEODE_VERSION: i32 = 1;

use std::ffi::CStr;
use std::os::raw::c_char;

unsafe fn string2c<E>(err: E) -> *mut c_char
where E: ToString {
    let mut bytes = err.to_string().into_bytes();
    bytes.push(0);
    let desc = bytes.iter().map(|b| *b as c_char).collect::<Vec<c_char>>().as_mut_ptr();

    let new = libc::malloc(bytes.len()) as *mut c_char;
    libc::strcpy(new, desc);

    new
}

unsafe fn c2string(a: *const c_char) -> &'static str {
	assert!(!a.is_null());
	CStr::from_ptr(a).to_str().unwrap()
}

unsafe fn c2option(a: *const c_char) -> Option<&'static str> {
	if a.is_null() {
		None
	} else {
		Some(CStr::from_ptr(a).to_str().unwrap())
	}
}

#[no_mangle]
pub unsafe extern "C" fn geode_version() -> i32 {
	GEODE_VERSION
}

#[no_mangle]
pub unsafe extern "C" fn geode_install_suite(
	location: *const c_char,
	nightly: bool,
	callback: suite::SuiteProgressCallback
) -> *const c_char {
	match crate::suite::install_suite(
		Path::new(c2string(location)),
		nightly,
		callback
	) {
		Ok(_) => std::ptr::null(),
		Err(b) => string2c(b)
	}
}

#[no_mangle]
pub unsafe extern "C" fn geode_create_template(
	project_location: *const c_char,
	name: *const c_char,
	version: *const c_char,
	id: *const c_char,
	developer: *const c_char,
	description: *const c_char) -> *const c_char {
	match crate::template::create_template(
		Path::new(c2string(project_location)),
		c2string(name),
		c2string(version),
		c2string(id),
		c2string(developer),
		c2string(description)
	) {
		Ok(_) => std::ptr::null(),
		Err(b) => {
			string2c(b)
		}
	}
}

#[no_mangle]
pub unsafe extern "C" fn geode_package(
    resource_dir: *const c_char,
    exec_dir: *const c_char,
    out_file: *const c_char,
    log: bool,
    use_cached_resources: bool,
) -> *const c_char {
	match crate::package::create_geode(
		Path::new(c2string(resource_dir)),
		Path::new(c2string(exec_dir)),
		Path::new(c2string(out_file)),
		log,
		use_cached_resources,
	) {
		Ok(_) => std::ptr::null(),
		Err(b) => {
			string2c(b)
		}
	}
}

#[no_mangle]
pub unsafe extern "C" fn geode_amend_package(
    geode_file: *const c_char,
    file_to_add: *const c_char,
    dir_in_zip: *const c_char
) -> *const c_char {
	match crate::package::amend_geode(
		Path::new(c2string(geode_file)),
		Path::new(c2string(file_to_add)),
		Path::new(c2string(dir_in_zip)),
	) {
		Ok(_) => std::ptr::null(),
		Err(b) => string2c(b)
	}
}

#[no_mangle]
pub unsafe extern "C" fn geode_install_package(
	out_file: *const c_char,
	install_path: *const c_char
) -> *const c_char {
	match crate::package::install_geode(
		Path::new(c2string(out_file)),
		Path::new(c2string(install_path))
	) {
		Ok(_) => std::ptr::null(),
		Err(b) => {
			string2c(b)
		}
	}
}

#[repr(C)]
pub struct CPackInfo {
	pub suffix_removals: u32,
	pub created_files: *mut *const c_char
}

#[no_mangle]
pub unsafe extern "C" fn geode_sprite_sheet(
	in_dir: *const c_char,
	out_dir: *const c_char,
	create_variants: bool,
	name: *const c_char, // can be null
	prefix: *const c_char, // can be null
	pack_info: *mut CPackInfo
) -> *const c_char {
	match crate::spritesheet::pack_sprites_in_dir(
		Path::new(c2string(in_dir)),
		Path::new(c2string(out_dir)),
		create_variants,
		c2option(name),
		c2option(prefix)
	) {
		Ok(res) => {
			(*pack_info).suffix_removals = res.suffix_removals;
			(*pack_info).created_files = libc::malloc(res.suffix_removals as usize * std::mem::size_of::<*const c_char>()) as *mut *const c_char;


			let sl = std::slice::from_raw_parts_mut((*pack_info).created_files, res.suffix_removals as usize);

			for (file_idx, file) in res.created_files.into_iter().enumerate() {
				sl[file_idx] = string2c(file);
			}
			std::ptr::null()
		},
		Err(b) => {
			string2c(b)
		}
	}
}

#[no_mangle]
pub unsafe extern "C" fn geode_sprite_variants(
	file: *const c_char,
	out_dir: *const c_char,
	prefix: *const c_char // can be null
) -> *const c_char {
	match crate::spritesheet::create_variants_of_sprite(
		Path::new(c2string(file)),
		Path::new(c2string(out_dir)),
		c2option(prefix)
	) {
		Ok(_) => std::ptr::null(),
		Err(b) => {
			string2c(b)
		}
	}
}

#[no_mangle]
pub unsafe extern "C" fn geode_create_bitmap_font_from_ttf(
	ttf_path: *const c_char,
	out_dir: *const c_char,
	name: *const c_char, // can be null
	fontsize: u32,
	prefix: *const c_char, // can be null
	create_variants: bool,
	charset: *const c_char, // can be null
	outline: u32,
) -> *const c_char {
	match crate::font::create_bitmap_font_from_ttf(
		Path::new(c2string(ttf_path)),
		Path::new(c2string(out_dir)),
		c2option(name),
		fontsize,
		c2option(prefix),
		create_variants,
		c2option(charset),
		outline,
	) {
		Ok(_) => std::ptr::null(),
		Err(b) => {
			string2c(b)
		}
	}
}

