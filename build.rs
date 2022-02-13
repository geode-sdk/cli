fn main() {
  if cfg!(target_os = "windows") {
    extern crate winres;
    let mut res = winres::WindowsResource::new();
    res.set_icon("geode.ico");
    res.compile().unwrap();
  }
}
