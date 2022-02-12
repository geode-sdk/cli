#[cfg(windows)] extern crate winres;

fn main() {
  if cfg!(target_os = "windows") {
    let mut res = winres::WindowsResource::new();
    res.set_icon("geode.ico");
    res.compile().unwrap();
  }
}
