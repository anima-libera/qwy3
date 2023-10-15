use std::f32::consts::TAU;

fn positive_fract(x: f32) -> f32 {
	x - f32::floor(x)
}

fn _smoothstep(x: f32) -> f32 {
	if x < 0.0 {
		0.0
	} else if 1.0 < x {
		1.0
	} else {
		x * x * (3.0 - 2.0 * x)
	}
}

fn smoothcos(x: f32) -> f32 {
	if x < 0.0 {
		0.0
	} else if 1.0 < x {
		1.0
	} else {
		(f32::cos((1.0 - x) * TAU / 2.0) + 1.0) / 2.0
	}
}

fn interpolate(
	smoothing: &dyn Fn(f32) -> f32,
	x: f32,
	x_inf: f32,
	x_sup: f32,
	dst_inf: f32,
	dst_sup: f32,
) -> f32 {
	let ratio = (x - x_inf) / (x_sup - x_inf);
	let smooth_ratio = smoothing(ratio);
	dst_inf + smooth_ratio * (dst_sup - dst_inf)
}

fn raw_noise_a_node(xs: &[i32]) -> f32 {
	let mut a = 0;
	let mut b = 0;
	for (i, x) in xs.iter().copied().enumerate() {
		a ^= x;
		b ^= 17 * (i as i32 + 11) + x;
		std::mem::swap(&mut a, &mut b);
		a ^= a << ((i + 7) % (((b % 11) as usize).saturating_add(5)));
	}
	positive_fract(f32::cos(a as f32 + b as f32))
}

fn raw_noise_a(xs: &[f32], channels: &[i32]) -> f32 {
	if xs.is_empty() {
		raw_noise_a_node(channels)
	} else {
		// For every continuous coordinate, we interpolate between
		// the two closest discreet node values on that axis.
		// In one dimension (with N <= x < N+1), it looks like this:
		// ... --|------#----|--> ...
		//       N      x   N+1
		//      inf         sup
		// And we can do that by calling this recursively
		// with N and N+1 as additional channel parameters.
		let mut channels_inf = Vec::from(channels);
		let mut channels_sup = Vec::from(channels);
		channels_inf.push(f32::floor(xs[0]) as i32);
		channels_sup.push(f32::floor(xs[0]) as i32 + 1);
		let sub_noise_inf = raw_noise_a(&xs[1..], &channels_inf);
		let sub_noise_sup = raw_noise_a(&xs[1..], &channels_sup);
		let x_fract = positive_fract(xs[0]);
		interpolate(&smoothcos, x_fract, 0.0, 1.0, sub_noise_inf, sub_noise_sup)
	}
}

fn octaves_noise_a(number_of_octaves: u32, xs: &[f32], channels: &[i32]) -> f32 {
	let mut xs = Vec::from(xs);
	let mut value_sum = 0.0;
	let mut coef_sum = 0.0;
	let mut coef = 1.0;
	for _i in 0..number_of_octaves {
		value_sum += coef * raw_noise_a(&xs, channels);
		coef_sum += coef;
		coef /= 2.0;
		xs.iter_mut().for_each(|x| *x *= 2.0);
	}
	value_sum / coef_sum
}

pub struct OctavedNoise {
	number_of_octaves: u32,
	base_channels: Vec<i32>,
}

impl OctavedNoise {
	pub fn new(number_of_octaves: u32, base_channels: Vec<i32>) -> OctavedNoise {
		OctavedNoise { number_of_octaves, base_channels }
	}

	pub fn sample(&self, xs: &[f32], additional_channels: &[i32]) -> f32 {
		let mut channels = self.base_channels.clone();
		channels.extend(additional_channels);
		octaves_noise_a(self.number_of_octaves, xs, &channels)
	}

	pub fn _sample_2d(&self, coords: cgmath::Point2<f32>, additional_channels: &[i32]) -> f32 {
		let xs: [f32; 2] = coords.into();
		self.sample(&xs, additional_channels)
	}
	pub fn sample_3d(&self, coords: cgmath::Point3<f32>, additional_channels: &[i32]) -> f32 {
		let xs: [f32; 3] = coords.into();
		self.sample(&xs, additional_channels)
	}
}
