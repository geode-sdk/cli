#![allow(clippy::missing_safety_doc)]

//pub mod template;
//pub mod spritesheet;
//pub mod dither;
//pub mod font;
pub mod suite;
pub mod install;
use std::os::raw::c_char;

use std::path::Path;

pub type ProgressCallback = extern "stdcall" fn(*const c_char, i32) -> ();

#[repr(C)]
pub struct VersionInfo {
	major: i32,
	minor: i32,
	patch: i32,
}

macro_rules! scan {
    ( $string:expr, $sep:expr, $( $x:ty ),+ ) => {{
        let mut iter = $string.split($sep);
        ($(iter.next().and_then(|word| word.parse::<$x>().ok()),)*)
    }}
}

impl VersionInfo {
    pub fn to_string(&self) -> String {
        format!("v{}.{}.{}", self.major, self.minor, self.patch)
    }

	pub fn from_string(str: &String) -> VersionInfo {
		let r = scan!(str, ".", i32, i32, i32);
		VersionInfo {
			major: r.0.unwrap(),
			minor: r.1.unwrap(),
			patch: r.2.unwrap(),
		}
	}
}

#[repr(C)]
pub struct InstallInfo {
	loader_version: VersionInfo,
	api_version: VersionInfo,
}

pub const GEODE_TARGET_VERSION: VersionInfo = VersionInfo {
	major: 0,
	minor: 1,
	patch: 0
};

use std::ffi::CStr;

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

#[no_mangle]
pub unsafe extern "C" fn geode_target_version() -> VersionInfo {
	return GEODE_TARGET_VERSION;
}

#[no_mangle]
pub unsafe extern "C" fn geode_install_suite(
	location: *const c_char,
	nightly: bool,
	callback: ProgressCallback
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
pub unsafe extern "C" fn geode_install_geode(
	location: *const c_char,
	nightly: bool,
	api: bool,
	callback: ProgressCallback
) -> *const c_char {
	match crate::install::install_geode(
		Path::new(c2string(location)),
		nightly,
		api,
		callback
	) {
		Ok(_) => std::ptr::null(),
		Err(b) => string2c(b)
	}
}

#[no_mangle]
pub unsafe extern "C" fn geode_uninstall_geode(
	location: *const c_char
) -> *const c_char {
	match crate::install::uninstall_geode(
		Path::new(c2string(location))
	) {
		Ok(_) => std::ptr::null(),
		Err(b) => string2c(b)
	}
}
