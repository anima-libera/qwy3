use std::sync::Arc;

use cgmath::MetricSpace;
use image::{GenericImage, GenericImageView};
use rand::Rng;

use crate::{saves::Save, texture_gen};

pub(crate) const ATLAS_DIMS: (usize, usize) = (512, 512);

pub(crate) struct Atlas {
	pub(crate) image: image::RgbaImage,
}

impl Atlas {
	pub(crate) fn new_fast_incomplete() -> Atlas {
		let mut image: image::RgbaImage =
			image::ImageBuffer::new(ATLAS_DIMS.0 as u32, ATLAS_DIMS.1 as u32);

		let default_color = image::Rgba::from([255, 100, 100, 255]);
		image.pixels_mut().for_each(|pixel| *pixel = default_color);

		// Font
		let mut font_image =
			image::load_from_memory(include_bytes!("../assets/font-02.png")).unwrap();
		font_image.as_mut_rgba8().unwrap().pixels_mut().for_each(|pixel| {
			// We keep black as white (to multiply with colors) and discard everything else.
			*pixel = if pixel.0 == [0, 0, 0, 255] {
				image::Rgba::from([255, 255, 255, 255])
			} else {
				image::Rgba::from([0, 0, 0, 0])
			}
		});
		image.copy_from(&font_image, 0, 32).unwrap();

		// Spritesheet
		let spritesheet_image =
			image::load_from_memory(include_bytes!("../assets/spritesheet.png")).unwrap();
		image.copy_from(&spritesheet_image, 256, 32).unwrap();

		Atlas { image }
	}

	pub(crate) fn new_slow_complete(world_gen_seed: i32) -> Atlas {
		let mut atlas = Atlas::new_fast_incomplete();

		// Test blocks
		'texture_gen: for y in 4..(ATLAS_DIMS.1 / 16) {
			for x in 0..(ATLAS_DIMS.0 / 16) {
				let view = atlas.image.sub_image(x as u32 * 16, y as u32 * 16, 16, 16);
				let index = (y as i32 - 4) * (ATLAS_DIMS.0 / 16) as i32 + x as i32;
				texture_gen::generate_texture(view, world_gen_seed, index);
				if index > 100 {
					break 'texture_gen;
				}
			}
		}

		// Rock block
		{
			let view = atlas.image.sub_image(0, 0, 16, 16);
			texture_gen::default_ground(view, world_gen_seed, 1);
		}

		// Grass block
		{
			let mut view = atlas.image.sub_image(16, 0, 16, 16);
			for y in 0..16 {
				for x in 0..16 {
					let r = rand::thread_rng().gen_range(80..100);
					let g = rand::thread_rng().gen_range(230..=255);
					let b = rand::thread_rng().gen_range(10..30);
					view.put_pixel(x, y, image::Rgba::from([r, g, b, 255]));
				}
			}
		}

		// Grass bush-like thingy
		{
			let mut view = atlas.image.sub_image(32, 0, 16, 16);
			for y in 0..16 {
				for x in 0..16 {
					let tp = cgmath::vec2(x as f32, y as f32 / 2.0);
					let bottom_center = cgmath::vec2(8.0, 0.0);
					let (r, g, b, a) = if bottom_center.distance(tp) > 8.0 {
						(0, 0, 0, 0)
					} else {
						(
							rand::thread_rng().gen_range(80..100),
							rand::thread_rng().gen_range(230..=255),
							rand::thread_rng().gen_range(10..30),
							255,
						)
					};
					view.put_pixel(x, y, image::Rgba::from([r, g, b, a]));
				}
			}
		}

		// Wood block
		{
			let mut view = atlas.image.sub_image(48, 0, 16, 16);
			for y in 0..16 {
				for x in 0..16 {
					let brown = rand::thread_rng().gen_range(100..200);
					let r = brown;
					let g = brown / 2;
					let b = rand::thread_rng().gen_range(50..70);
					view.put_pixel(x, y, image::Rgba::from([r, g, b, 255]));
				}
			}
			for _i in 0..8 {
				for y in 0..16 {
					for x in 0..16 {
						if rand::thread_rng().gen_range(0..5) == 0 {
							let y_above = if y == 0 { 15 } else { y - 1 };
							let pixel = view.get_pixel(x, y_above);
							view.put_pixel(x, y, pixel);
						}
					}
				}
			}
		}

		// Leaf block
		{
			let mut view = atlas.image.sub_image(64, 0, 16, 16);
			for y in 0..16 {
				for x in 0..16 {
					let r = 0;
					let g = rand::thread_rng().gen_range(50..255);
					let b = 0;
					view.put_pixel(x, y, image::Rgba::from([r, g, b, 255]));
				}
			}
			for _i in 0..3 {
				for y in 0..16 {
					for x in 0..16 {
						if rand::thread_rng().gen_range(0..5) == 0 {
							let neighbor_x = x as i32 + rand::thread_rng().gen_range((-1i32)..=1);
							let neighbor_x = if neighbor_x < 0 {
								15
							} else if neighbor_x == 16 {
								0
							} else {
								neighbor_x as u32
							};
							let neighbor_y = y as i32 + rand::thread_rng().gen_range((-1i32)..=1);
							let neighbor_y = if neighbor_y < 0 {
								15
							} else if neighbor_y == 16 {
								0
							} else {
								neighbor_y as u32
							};
							let pixel = view.get_pixel(neighbor_x, neighbor_y);
							view.put_pixel(x, y, pixel);
						}
					}
				}
			}
		}

		atlas
	}

	pub(crate) fn load_from_save(save: &Arc<Save>) -> Option<Atlas> {
		let atlas_texture_file_path = &save.atlas_texture_file_path;
		let atlas_texture = image::open(atlas_texture_file_path).ok()?;
		let image = atlas_texture.to_rgba8();
		Some(Atlas { image })
	}

	pub(crate) fn save(&self, save: &Arc<Save>) {
		let atlas_texture_file_path = &save.atlas_texture_file_path;
		self.image.save_with_format(atlas_texture_file_path, image::ImageFormat::Png).unwrap();
	}
}
