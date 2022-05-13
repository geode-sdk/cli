#![allow(clippy::missing_safety_doc)]

use std::ffi::CStr;
use std::os::raw::c_char;

#[repr(C)]
pub struct CPackInfo {
	pub suffix_removals: u32,
	pub created_files: *mut *const c_char
}

#[repr(C)]
pub struct VersionInfo {
	major: i32,
	minor: i32,
	patch: i32,
}

impl VersionInfo {
    pub fn to_string(&self) -> String {
        format!("v{}.{}.{}", self.major, self.minor, self.patch)
    }
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

#[cfg_attr(target_os = "windows", link(name = "geodeutils.dll"))]
#[cfg_attr(not(target_os = "windows"), link(name = "geodeutils"))]
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

	pub fn geode_version() -> VersionInfo;
}

#[macro_export]
macro_rules! call_extern {
	($x: expr) => {{
		unsafe {
			let y = $x;
			if !y.is_null() {
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

    new
}

pub unsafe fn opt2c<E>(err: Option<E>) -> *mut c_char
where E: ToString {
	match err {
		Some(x) => string2c(x),
		None => std::ptr::null_mut()
	}
}
