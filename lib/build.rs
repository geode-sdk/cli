fn main() {
	#[cfg(not(windows))]
	println!("cargo:rustc-link-arg=-Wl,-install_name,@loader_path/libgeodeutils.dylib");
}