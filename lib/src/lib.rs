#![allow(clippy::missing_safety_doc)]

//pub mod template;
//pub mod spritesheet;
//pub mod dither;
//pub mod font;
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

