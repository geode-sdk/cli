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
		color[0] = (color[0] / 15) * 15;
		color[1] = (color[1] / 15) * 15;
		color[2] = (color[2] / 15) * 15;
		color[3] = (color[3] / 15) * 15;
	}
}
