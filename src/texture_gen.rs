use image::{GenericImage, GenericImageView, ImageBuffer, Rgba, SubImage};

use crate::noise::OctavedNoise;

pub type View<'a> = SubImage<&'a mut ImageBuffer<Rgba<u8>, Vec<u8>>>;

pub struct TextureViewWrapping<'a> {
	view: View<'a>,
}

type Color = Rgba<u8>;

impl<'a> TextureViewWrapping<'a> {
	fn from_view(view: View<'a>) -> TextureViewWrapping<'a> {
		TextureViewWrapping { view }
	}

	fn get_actual_coords(&self, coords: cgmath::Point2<i32>) -> cgmath::Point2<i32> {
		let (w, h) = self.view.dimensions();
		cgmath::point2(coords.x.rem_euclid(w as i32), coords.y.rem_euclid(h as i32))
	}

	fn get_pixel(&self, coords: cgmath::Point2<i32>) -> Color {
		let coords = self.get_actual_coords(coords);
		self.view.get_pixel(coords.x as u32, coords.y as u32)
	}

	fn set_pixel(&mut self, coords: cgmath::Point2<i32>, color: Color) {
		let coords = self.get_actual_coords(coords);
		self.view.put_pixel(coords.x as u32, coords.y as u32, color);
	}

	fn apply_initializer(&mut self, initializer: &Initializer, world_seed: i32, texture_seed: i32) {
		let (w, h) = self.view.dimensions();
		for y in 0..h {
			for x in 0..w {
				let coords = (x as i32, y as i32).into();
				let color = initializer.initialize_pixel(coords, world_seed, texture_seed);
				self.view.put_pixel(x, y, color);
			}
		}
	}

	fn apply_unary_step(&mut self, step: &UnaryStep, world_seed: i32, texture_seed: i32) {
		let (w, h) = self.view.dimensions();
		let mut actual_copy = self.view.to_image();
		let copy = TextureViewWrapping::from_view(actual_copy.sub_image(0, 0, w, h));
		for y in 0..h {
			for x in 0..w {
				let coords = (x as i32, y as i32).into();
				let color = step.modify_pixel(&copy, coords, world_seed, texture_seed);
				self.view.put_pixel(x, y, color);
			}
		}
	}
}

enum Initializer {
	Uniform { color: Color },
	GreyRandom { inf: i32, sup_included: i32, seed: i32 },
	PaletteRandom { palette: Vec<Color>, seed: i32 },
}

impl Initializer {
	fn initialize_pixel(
		&self,
		coords: cgmath::Point2<i32>,
		world_seed: i32,
		texture_seed: i32,
	) -> Color {
		let noise = OctavedNoise::new(1, vec![texture_seed]);
		match self {
			Initializer::Uniform { color } => *color,
			Initializer::GreyRandom { inf, sup_included, seed } => {
				let grey = unit_to_range(
					noise.sample_i2d_1d(coords, &[world_seed, texture_seed, *seed]),
					*inf as f32,
					*sup_included as f32,
				) as u8;
				image::Rgba::from([grey, grey, grey, 255])
			},
			Initializer::PaletteRandom { palette, seed } => {
				let noise_value = noise.sample_i2d_1d(coords, &[world_seed, texture_seed, *seed]);
				let index = (noise_value * palette.len() as f32).floor() as usize;
				palette[index]
			},
		}
	}
}

enum UnaryStep {
	Noise01 { how_much: f32, seed: i32 },
	Noise02 { how_much: f32, color_channel: usize, seed: i32 },
	Scramble01 { seed: i32 },
	Smooth01 { how_much_x: f32, how_much_y: f32 },
}

impl<'a> UnaryStep {
	fn modify_pixel(
		&self,
		previous_texture: &TextureViewWrapping<'a>,
		coords: cgmath::Point2<i32>,
		world_seed: i32,
		texture_seed: i32,
	) -> Color {
		let noise = OctavedNoise::new(1, vec![world_seed, texture_seed]);
		match self {
			UnaryStep::Noise01 { how_much, seed } => {
				let mut color = previous_texture.get_pixel(coords);
				for i in 0..3 {
					let noise_value = noise.sample_i2d_1d(coords, &[*seed]);
					color.0[i] = (color.0[i] as f32 + (noise_value * 2.0 - 1.0) * *how_much)
						.clamp(0.0, 255.0) as u8;
				}
				color
			},
			UnaryStep::Noise02 { how_much, color_channel, seed } => {
				let mut color = previous_texture.get_pixel(coords);
				let noise_value = noise.sample_i2d_1d(coords, &[*seed]);
				let i = *color_channel;
				color.0[i] =
					(color.0[i] as f32 + (noise_value * 2.0 - 1.0) * *how_much).clamp(0.0, 255.0) as u8;
				color
			},
			UnaryStep::Scramble01 { seed } => {
				let noise_value = noise.sample_i2d_1d(coords, &[*seed]);
				if noise_value < 1.0 / 3.0 {
					previous_texture.get_pixel(coords)
				} else if noise_value < 2.0 / 3.0 {
					previous_texture.get_pixel(coords + cgmath::vec2(1, 0))
				} else {
					previous_texture.get_pixel(coords + cgmath::vec2(0, 1))
				}
			},
			UnaryStep::Smooth01 { how_much_x, how_much_y } => {
				let a = previous_texture.get_pixel(coords);
				let b = previous_texture.get_pixel(coords + cgmath::vec2(1, 0));
				let c = previous_texture.get_pixel(coords + cgmath::vec2(0, 1));
				color_weighted_mean(&[
					WeightedValue { weight: 1.0, value: a },
					WeightedValue { weight: *how_much_x, value: b },
					WeightedValue { weight: *how_much_y, value: c },
				])
			},
		}
	}
}

/// Takes `value` in `0.0..=1.0` and maps it to the given range.
fn unit_to_range(value: f32, range_inf: f32, range_sup_included: f32) -> f32 {
	value * (range_sup_included - range_inf) + range_inf
}

struct WeightedValue<T> {
	weight: f32,
	value: T,
}

fn weighted_mean(weighted_values: &[WeightedValue<f32>]) -> f32 {
	let mut weight_sum = 0.0;
	let mut accumulator = 0.0;
	for WeightedValue { weight, value } in weighted_values.iter() {
		weight_sum += weight;
		accumulator += value * weight;
	}
	accumulator / weight_sum
}

fn color_weighted_mean(weighted_colors: &[WeightedValue<Color>]) -> Color {
	let mut rgba: [u8; 4] = [0, 0, 0, 0];
	for (i, channel_value) in rgba.iter_mut().enumerate() {
		let weighted_values_for_channel_i: Vec<_> = weighted_colors
			.iter()
			.map(|WeightedValue { weight, value }| WeightedValue {
				weight: *weight,
				value: value.0[i] as f32,
			})
			.collect();
		*channel_value = weighted_mean(&weighted_values_for_channel_i) as u8;
	}
	image::Rgba::from(rgba)
}

pub fn default_ground(view: View, world_seed: i32, texture_seed: i32) {
	//let mut texture = TextureViewWrapping::from_view(view);
	//let initializer = Initializer::GreyRandom { inf: 240, sup_included: 255, seed: 1 };
	//texture.apply_initializer(&initializer, world_seed, texture_seed);
	test(view, world_seed, texture_seed);
}

pub fn test(view: View, world_seed: i32, texture_seed: i32) {
	let mut texture = TextureViewWrapping::from_view(view);
	//let initializer = Initializer::GreyRandom { inf: 0, sup_included: 255, seed: 1 };
	//let initializer = Initializer::Uniform { color: Color::from([255, 255, 255, 255]) };
	let initializer = Initializer::PaletteRandom {
		palette: vec![
			Color::from([255, 255, 255, 255]),
			Color::from([200, 200, 200, 255]),
		],
		seed: -1,
	};
	texture.apply_initializer(&initializer, world_seed, texture_seed);
	/*
	for i in 0..5 {
		texture.apply_unary_step(&UnaryStep::Smooth01, world_seed, texture_seed);
		texture.apply_unary_step(
			&UnaryStep::Noise02 { how_much: 30.0, color_channel: 0, seed: i },
			world_seed,
			texture_seed,
		);
	}
	for _i in 0..1 {
		texture.apply_unary_step(&UnaryStep::Smooth01, world_seed, texture_seed);
	}
	*/
	for i in 0..5 {
		//texture.apply_unary_step(
		//	&UnaryStep::Noise02 { how_much: 100.0, color_channel: 1, seed: i },
		//	world_seed,
		//	texture_seed,
		//);
		texture.apply_unary_step(&UnaryStep::Scramble01 { seed: i }, world_seed, texture_seed);
		texture.apply_unary_step(
			&UnaryStep::Smooth01 { how_much_x: 0.1, how_much_y: 1.3 },
			world_seed,
			texture_seed,
		);
	}
	texture.apply_unary_step(
		&UnaryStep::Smooth01 { how_much_x: 0.7, how_much_y: 0.1 },
		world_seed,
		texture_seed,
	);
	texture.apply_unary_step(
		&UnaryStep::Scramble01 { seed: -2 },
		world_seed,
		texture_seed,
	);
}
