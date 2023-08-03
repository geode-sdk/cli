use image::imageops::ColorMap;
use image::Rgba;

#[derive(Clone, Copy)]
pub struct RGBA4444;

impl ColorMap for RGBA4444 {
	type Color = Rgba<u8>;

	#[inline(always)]
	fn index_of(&self, _: &Rgba<u8>) -> usize {
		0
	}

	#[inline(always)]
	fn map_color(&self, color: &mut Rgba<u8>) {
		let convert = |x: u8| (x as f32 / 255.0 * 15.0) as u8 * (255 / 15);
		color[0] = convert(color[0]);
		color[1] = convert(color[1]);
		color[2] = convert(color[2]);
		color[3] = convert(color[3]);
	}
}
