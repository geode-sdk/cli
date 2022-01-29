// use resize::{self};
use std::fs::{self, File};
use std::path::{PathBuf};
use colored::*;
use path_absolutize::Absolutize;
use resize::Pixel::*;
use resize::Type::*;
use rgb::FromSlice;

fn is_uhd_image(src: &PathBuf) -> bool {
    return src.file_stem().unwrap().to_str().unwrap().ends_with("-uhd");
}

fn resize_image(src: &Vec<u8>, info: &png::OutputInfo, downscale: usize) -> Result<Vec<u8>, resize::Error> {
    let (w1, h1) = (info.width as usize, info.height as usize);
    let mut dst = vec![0u8; w1 * h1 * info.color_type.samples() / downscale];
    println!("{} {} {} {}", w1, h1, w1 / downscale, h1 / downscale);
    let mut resizer = resize::new(w1, h1, w1 / downscale, h1 / downscale, RGB8, Triangle)?;
    resizer.resize(src.as_rgb(), dst.as_rgb_mut()).unwrap();
    Ok(dst)
}

fn create_variants_for_image(r_src: PathBuf, r_dest: PathBuf) {
    let src = PathBuf::from(r_src.absolutize().unwrap().to_str().unwrap());
    let dest = PathBuf::from(r_dest.absolutize().unwrap().to_str().unwrap());
    if !is_uhd_image(&src) {
        println!("{}{}{}", "Warning: Skipping ".yellow(), src.to_str().unwrap().yellow(), " due to missing -uhd suffix".yellow());
        return;
    }
    println!("{} {} {} {}", "Resizing".cyan(), src.to_str().unwrap().cyan(), "->".cyan(), dest.to_str().unwrap().cyan());
    let decoder = png::Decoder::new(File::open(&src).unwrap());
    let mut reader = decoder.read_info().unwrap();
    let mut data = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut data).unwrap();
    for (scale, name) in [(2, "-hd"), (4, "")] {
        match resize_image(&data, &info, scale) {
            Ok(v) => {
                let dest_dir = dest.parent().unwrap().to_path_buf();
                let mut dst_name = src.file_stem().unwrap().to_str().unwrap().to_string();
                dst_name.pop();
                dst_name.pop();
                dst_name.pop();
                dst_name.pop();
                dst_name.push_str(name);
                dst_name.push_str(".png");
                let mut true_dest = dest_dir;
                fs::create_dir_all(&true_dest).unwrap();
                println!(" * Resized {}x image -> {}", scale, true_dest.to_str().unwrap());
                true_dest.push(dst_name);
                let out = File::create(true_dest).unwrap();
                let mut encoder = png::Encoder::new(
                    out, info.width as u32 / scale as u32, info.height as u32 / scale as u32
                );
                encoder.set_color(info.color_type);
                encoder.set_depth(info.bit_depth);
                encoder.write_header().unwrap().write_image_data(&v).unwrap();
            },
            Err(_) => println!("{} {}", "Error: Unable to resize ".red(), src.to_str().unwrap().red())
        }
    }
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
                create_variants_for_image(path, path_dest);
            }
        }
    }
}

pub fn process_resources(src: PathBuf, dest: Option<PathBuf>) {
    let actual_dest = dest.unwrap_or(src.clone());
    process_resources_recurse(&src, &actual_dest);
}
