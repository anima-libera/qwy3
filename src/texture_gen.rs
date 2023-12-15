use image::{GenericImage, GenericImageView, ImageBuffer, Rgba, SubImage};

use crate::noise::OctavedNoise;

pub type View<'a> = SubImage<&'a mut ImageBuffer<Rgba<u8>, Vec<u8>>>;

/// Takes `value` in `0.0..=1.0` and maps it to the given range.
fn unit_to_range(value: f32, range_inf: f32, range_sup_included: f32) -> f32 {
	value * (range_sup_included - range_inf) + range_inf
}

pub fn default_ground(mut view: View, world_seed: i32, texture_seed: i32) {
	let noise = OctavedNoise::new(1, vec![world_seed, texture_seed]);
	let (w, h) = view.dimensions();
	for y in 0..h {
		for x in 0..w {
			let grey = unit_to_range(
				noise.sample_i2d_1d((x as i32, y as i32).into(), &[]),
				240.0,
				255.0,
			) as u8;
			view.put_pixel(x, y, image::Rgba::from([grey, grey, grey, 255]));
		}
	}
}
