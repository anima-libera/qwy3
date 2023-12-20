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

	fn _set_pixel(&mut self, coords: cgmath::Point2<i32>, color: Color) {
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

	fn apply_texture_generator(
		&mut self,
		generator: &TextureGenerator,
		world_seed: i32,
		texture_seed: i32,
	) {
		self.apply_initializer(&generator.initializer, world_seed, texture_seed);
		for unary_step in generator.unary_steps.iter() {
			self.apply_unary_step(unary_step, world_seed, texture_seed);
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
				let index = (noise_value * (palette.len() as f32 - 0.0001)).floor() as usize;
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
	Smooth02 { kernel: [f32; 9] },
	SmoothColors01 { seed: i32 },
	SmoothColors02 { seed: i32 },
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
			UnaryStep::Smooth02 { kernel } => {
				let offsets: [cgmath::Vector2<i32>; 9] = [
					cgmath::vec2(-1, -1),
					cgmath::vec2(-1, 0),
					cgmath::vec2(-1, 1),
					cgmath::vec2(0, -1),
					cgmath::vec2(0, 0),
					cgmath::vec2(0, 1),
					cgmath::vec2(1, -1),
					cgmath::vec2(1, 0),
					cgmath::vec2(1, 1),
				];
				let mut weighted_values: [WeightedValue<Color>; 9] =
					[WeightedValue { weight: 1.0, value: Color::from([0, 0, 0, 255]) }; 9];
				for i in 0..9 {
					weighted_values[i] = WeightedValue {
						weight: kernel[i],
						value: previous_texture.get_pixel(coords + offsets[i]),
					};
				}
				color_weighted_mean(&weighted_values)
			},
			UnaryStep::SmoothColors01 { seed } => {
				let ax = noise.sample_i2d_1d(coords, &[*seed, 1]);
				let ay = noise.sample_i2d_1d(coords, &[*seed, 2]);
				let bx = noise.sample_i2d_1d(coords, &[*seed, 3]);
				let by = noise.sample_i2d_1d(coords, &[*seed, 4]);
				let noise_to_coord =
					|unit: f32, coord_max: u32| (unit * (coord_max as f32 - 0.0001)).floor() as i32;
				let (w, h) = previous_texture.view.dimensions();
				let coords_a = cgmath::point2(noise_to_coord(ax, w), noise_to_coord(ay, h));
				let coords_b = cgmath::point2(noise_to_coord(bx, w), noise_to_coord(by, h));
				let color = previous_texture.get_pixel(coords);
				let color_a = previous_texture.get_pixel(coords_a);
				let color_b = previous_texture.get_pixel(coords_b);
				let dist_to_a = color_dist(color, color_a);
				let dist_to_b = color_dist(color, color_b);
				let dist_between_a_b = color_dist(color_a, color_b);
				if dist_between_a_b < dist_to_a || dist_between_a_b < dist_to_b {
					if noise.sample_i2d_1d(coords, &[*seed, 5]) < 0.5 {
						color_a
					} else {
						color_b
					}
				} else {
					color
				}
			},
			UnaryStep::SmoothColors02 { seed } => {
				let ax = noise.sample_i2d_1d(coords, &[*seed, 1]);
				let ay = noise.sample_i2d_1d(coords, &[*seed, 2]);
				let bx = noise.sample_i2d_1d(coords, &[*seed, 3]);
				let by = noise.sample_i2d_1d(coords, &[*seed, 4]);
				let noise_to_coord =
					|unit: f32, coord_max: u32| (unit * (coord_max as f32 - 0.0001)).floor() as i32;
				let (w, h) = previous_texture.view.dimensions();
				let coords_a = cgmath::point2(noise_to_coord(ax, w), noise_to_coord(ay, h));
				let coords_b = cgmath::point2(noise_to_coord(bx, w), noise_to_coord(by, h));
				let color = previous_texture.get_pixel(coords);
				let color_a = previous_texture.get_pixel(coords_a);
				let color_b = previous_texture.get_pixel(coords_b);
				color_weighted_mean(&[
					WeightedValue { weight: 1.0, value: color },
					WeightedValue { weight: 1.0, value: color_a },
					WeightedValue { weight: 1.0, value: color_b },
				])
			},
		}
	}
}

fn color_dist(a: Color, b: Color) -> u32 {
	a.0[0].abs_diff(b.0[0]) as u32 + a.0[1].abs_diff(b.0[1]) as u32 + a.0[2].abs_diff(b.0[2]) as u32
}

/// Takes `value` in `0.0..=1.0` and maps it to the given range.
fn unit_to_range(value: f32, range_inf: f32, range_sup_included: f32) -> f32 {
	value * (range_sup_included - range_inf) + range_inf
}

#[derive(Clone, Copy)]
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
	let mut texture = TextureViewWrapping::from_view(view);
	let initializer = Initializer::GreyRandom { inf: 240, sup_included: 255, seed: 1 };
	texture.apply_initializer(&initializer, world_seed, texture_seed);
}

pub fn generate_texture(view: View, world_seed: i32, texture_seed: i32) {
	let mut texture = TextureViewWrapping::from_view(view);
	texture.apply_texture_generator(
		&generate_texture_generator_not_uniform(world_seed, texture_seed),
		world_seed,
		texture_seed,
	);
}

struct TextureGenerator {
	initializer: Initializer,
	unary_steps: Vec<UnaryStep>,
}

fn generate_color(world_seed: i32, generator_seed: i32, color_seed: i32) -> Color {
	let noise = OctavedNoise::new(1, vec![world_seed, generator_seed, color_seed]);
	Color::from([
		(noise.sample_i1d_1d(0, &[]) * 255.0) as u8,
		(noise.sample_i1d_1d(1, &[]) * 255.0) as u8,
		(noise.sample_i1d_1d(2, &[]) * 255.0) as u8,
		255,
	])
}

fn generate_initializer(world_seed: i32, generator_seed: i32) -> Initializer {
	let noise = OctavedNoise::new(1, vec![world_seed, generator_seed]);
	let mut noise_i = 0;
	let mut random_unit = || {
		noise_i += 1;
		noise.sample_i1d_1d(noise_i, &[])
	};
	let mut seed_i = 0;
	let mut new_seed = || {
		seed_i += 1;
		seed_i
	};

	if random_unit() < 0.1 {
		Initializer::Uniform { color: generate_color(world_seed, generator_seed, new_seed()) }
	} else if random_unit() < 0.2 {
		let a = (random_unit() * 255.0) as i32;
		let b = (random_unit() * 255.0) as i32;
		Initializer::GreyRandom { inf: a.min(b), sup_included: a.max(b), seed: new_seed() }
	} else {
		let mut palette = vec![];
		loop {
			palette.push(generate_color(world_seed, generator_seed, new_seed()));
			if palette.len() >= 2 && random_unit() < 0.65 {
				break;
			}
		}
		Initializer::PaletteRandom { palette, seed: new_seed() }
	}
}

fn generate_unary_step(
	world_seed: i32,
	generator_seed: i32,
	step_index: i32,
	can_be_messy: bool,
	must_be_color_smoothing: bool,
) -> UnaryStep {
	let noise = OctavedNoise::new(1, vec![world_seed, generator_seed]);
	let mut noise_i = 20 * step_index;
	let mut random_unit = || {
		noise_i += 1;
		noise.sample_i1d_1d(noise_i, &[])
	};
	let mut seed_i = 20 * step_index;
	let mut new_seed = || {
		seed_i += 1;
		seed_i
	};

	if can_be_messy && !must_be_color_smoothing && random_unit() < 0.05 {
		UnaryStep::Noise01 { how_much: random_unit() * 35.0, seed: new_seed() }
	} else if can_be_messy && !must_be_color_smoothing && random_unit() < 0.05 {
		UnaryStep::Noise02 {
			how_much: random_unit() * 50.0,
			color_channel: (random_unit() * 3.0 - 0.001).floor() as usize,
			seed: new_seed(),
		}
	} else if !must_be_color_smoothing && random_unit() < 0.3 {
		UnaryStep::Scramble01 { seed: new_seed() }
	} else if !must_be_color_smoothing && random_unit() < 0.3 {
		UnaryStep::Smooth01 { how_much_x: random_unit(), how_much_y: random_unit() }
	} else if !must_be_color_smoothing && random_unit() < 0.3 {
		UnaryStep::Smooth02 {
			kernel: [
				random_unit(),
				random_unit(),
				random_unit(),
				random_unit(),
				random_unit(),
				random_unit(),
				random_unit(),
				random_unit(),
				random_unit(),
			],
		}
	} else if random_unit() < 0.4 {
		UnaryStep::SmoothColors01 { seed: new_seed() }
	} else {
		UnaryStep::SmoothColors02 { seed: new_seed() }
	}
}

fn generate_texture_generator_maybe_uniform(
	world_seed: i32,
	generator_seed: i32,
) -> TextureGenerator {
	let _noise = OctavedNoise::new(1, vec![world_seed, generator_seed]);
	let initializer = generate_initializer(world_seed, generator_seed);
	let mut unary_steps = vec![];
	let smoothing_steps_min = 5;
	let first_phase_steps_min = 10;
	let steps_min = first_phase_steps_min + smoothing_steps_min;
	loop {
		let can_be_messy = unary_steps.len() < first_phase_steps_min;
		let must_be_color_smoothing = false;
		unary_steps.push(generate_unary_step(
			world_seed,
			generator_seed,
			unary_steps.len() as i32,
			can_be_messy,
			must_be_color_smoothing,
		));
		if unary_steps.len() >= steps_min {
			break;
		}
	}
	TextureGenerator { initializer, unary_steps }
}

fn generate_texture_generator_not_uniform(
	world_seed: i32,
	generator_seed: i32,
) -> TextureGenerator {
	let mut i = 0;
	loop {
		let generator =
			generate_texture_generator_maybe_uniform(world_seed, generator_seed ^ (i << 10));
		i += 1;

		// Test generator and see if it outputs uniform textures.
		let mut image_buffer: image::RgbaImage = image::ImageBuffer::new(16, 16);
		let sub_image = image_buffer.sub_image(0, 0, 16, 16);
		let mut view = TextureViewWrapping::from_view(sub_image);
		view.apply_texture_generator(&generator, world_seed, 1);
		let some_color = view.get_pixel(cgmath::point2(0, 0));
		let max_dist = view
			.view
			.to_image()
			.pixels()
			.map(|color| color_dist(*color, some_color))
			.max()
			.unwrap();
		if max_dist > 60 {
			// Not uniform ^^.
			break generator;
		}
	}
}
