//! N-dimensional noise!
//!
//! This is a very "we have noise at home" implementation, slow and all.
//!
//! The idea is that we consider an N-dimensional grid of nodes
//! where nodes are at every interger coordinates
//! and each node is given a noise value via `raw_noise_node`.
//! Then, for points that don't fall on nodes, they fall in N-dimensional
//! (hyper)cubic cells of 2^N nodes as vertices,
//! we then interpolate the nodes' noise values with `raw_noise`.
//! Then we can do the usual stuff and add octaves with `octaves_noise`.

fn positive_fract(x: f32) -> f32 {
	x - f32::floor(x)
}

fn unit_to_i32(x: f32) -> i32 {
	(x * (i32::MAX as f32 - i32::MIN as f32) - i32::MIN as f32).round() as i32
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
		use std::f32::consts::TAU;
		(f32::cos((1.0 - x) * TAU / 2.0) + 1.0) / 2.0
	}
}

/// If `x == x_inf` then the interpolation result is `dst_inf`,
/// if `x == x_sup` then the interpolation result is `dst_sup`,
/// and any value in betwee will lead to some interpolation between `dst_inf` and `dst_sup`.
/// `x` is expected to be between `x_inf` (included) and `x_sup` (also included).
/// The given `smoothing` function is used to smooth out the curve when x is near its edges.
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

#[derive(Clone, Copy)]
enum CoordOrChannel {
	Coord(f32),
	Channel(i32),
}

fn raw_noise_node(xs: &[CoordOrChannel]) -> f32 {
	let mut a = 0;
	let mut b = 0;
	for (i, x) in xs.iter().copied().enumerate() {
		let x = match x {
			CoordOrChannel::Channel(x) => x,
			CoordOrChannel::Coord(_) => unreachable!(),
		};
		a ^= x;
		b ^= 17i32.wrapping_mul(((i as i32).wrapping_add(11)).wrapping_add(x));
		std::mem::swap(&mut a, &mut b);
		a ^= a << ((i + 7) % (((b % 11) as usize).saturating_add(5)));
	}
	if true {
		// Uh `cos` is actually slow.
		positive_fract(f32::cos(a as f32 + b as f32))
	} else {
		// Faster (? probably) than a `cos`.
		// TODO: Does it allows for values arbitrarly close to any arbitrary value
		// in the range `0.0..=1.0`? It is actually important, the `cos` allows that,
		// does this allow it to.
		let uwu = 2.84929;
		((a as f32 + b as f32) % uwu).abs() / uwu
	}
}

fn _worst_raw_noise_node(xs: &[i32]) -> f32 {
	// This raw noise is not used due to not covering the full [0.0, 1.0] interval
	// as much as possible.
	// For example, stuff like `if noise_value < 0.0001` will never trigger, which is bad.
	let mut v = 0.0;
	for (i, x) in xs.iter().copied().enumerate() {
		let pool = [
			0.31, 0.41, 0.71, 0.214354, 0.12, 0.82, 0.211, 0.218, 0.04, 0.94,
		];
		v += pool[(i as i32 * 3 + x).rem_euclid(pool.len() as i32) as usize];
		v += x as f32 / 11.0 + (x + 3 + i as i32) as f32 / 13.0 + x as f32 / 2.71;
	}
	positive_fract(v)
}

fn raw_noise(xs: &mut [CoordOrChannel]) -> f32 {
	let coord_index_and_value_opt = xs.iter().enumerate().find_map(|(i, x)| match x {
		CoordOrChannel::Coord(value) => Some((i, *value)),
		_ => None,
	});
	if let Some((coord_index, coord_value)) = coord_index_and_value_opt {
		// For every continuous coordinate, we interpolate between
		// the two closest discreet node values on that axis.
		// In one dimension (with N <= x < N+1), it looks like this:
		// ... --|------#----|--> ...
		//       N      x   N+1
		//      inf         sup
		// And we can do that by calling this recursively
		// with N and N+1 as additional channel parameters.
		let channel_inf = f32::floor(coord_value) as i32;
		let channel_sup = f32::floor(coord_value) as i32 + 1;
		xs[coord_index] = CoordOrChannel::Channel(channel_inf);
		let sub_noise_inf = raw_noise(xs);
		xs[coord_index] = CoordOrChannel::Channel(channel_sup);
		let sub_noise_sup = raw_noise(xs);
		xs[coord_index] = CoordOrChannel::Coord(coord_value);
		let x_fract = positive_fract(coord_value);
		interpolate(&smoothcos, x_fract, 0.0, 1.0, sub_noise_inf, sub_noise_sup)
	} else {
		raw_noise_node(xs)
	}
}

fn octaves_noise(number_of_octaves: u32, xs: &mut [CoordOrChannel]) -> f32 {
	let mut value_sum = 0.0;
	let mut coef_sum = 0.0;
	let mut coef = 1.0;
	for _i in 0..number_of_octaves {
		value_sum += coef * raw_noise(xs);
		coef_sum += coef;
		coef /= 2.0;
		xs.iter_mut().for_each(|x| {
			if let CoordOrChannel::Coord(x) = x {
				*x *= 2.0
			}
		});
	}
	value_sum / coef_sum
}

pub(crate) struct OctavedNoise {
	number_of_octaves: u32,
	base_channels: Vec<i32>,
}

impl OctavedNoise {
	pub(crate) fn new(number_of_octaves: u32, base_channels: Vec<i32>) -> OctavedNoise {
		OctavedNoise { number_of_octaves, base_channels }
	}

	pub(crate) fn sample(
		&self,
		xs: &[f32],
		additional_channels: &[i32],
		xs_that_are_channels: &[i32],
		one_more_channel: Option<i32>, // This is ridiculous >w<
	) -> f32 {
		let mut working_xs = smallvec::SmallVec::<[CoordOrChannel; 8]>::with_capacity(
			xs.len() + self.base_channels.len() + additional_channels.len(),
		);
		for x in xs {
			working_xs.push(CoordOrChannel::Coord(*x));
		}
		for channel in xs_that_are_channels {
			working_xs.push(CoordOrChannel::Channel(*channel));
		}
		for channel in &self.base_channels {
			working_xs.push(CoordOrChannel::Channel(*channel));
		}
		for channel in additional_channels {
			working_xs.push(CoordOrChannel::Channel(*channel));
		}
		if let Some(channel) = one_more_channel {
			working_xs.push(CoordOrChannel::Channel(channel));
		}
		octaves_noise(self.number_of_octaves, &mut working_xs)
	}

	pub(crate) fn sample_2d_1d(
		&self,
		coords: cgmath::Point2<f32>,
		additional_channels: &[i32],
	) -> f32 {
		let xs: [f32; 2] = coords.into();
		self.sample(&xs, additional_channels, &[], None)
	}
	pub(crate) fn sample_3d_1d(
		&self,
		coords: cgmath::Point3<f32>,
		additional_channels: &[i32],
	) -> f32 {
		let xs: [f32; 3] = coords.into();
		self.sample(&xs, additional_channels, &[], None)
	}
	pub(crate) fn _sample_3d_3d(
		&self,
		coords: cgmath::Point3<f32>,
		additional_channels: &[i32],
	) -> cgmath::Point3<f32> {
		let xs: [f32; 3] = coords.into();
		let x = self.sample(&xs, additional_channels, &[], Some(1));
		let y = self.sample(&xs, additional_channels, &[], Some(2));
		let z = self.sample(&xs, additional_channels, &[], Some(3));
		cgmath::point3(x, y, z)
	}
	pub(crate) fn sample_i1d_1d(&self, coord: i32, additional_channels: &[i32]) -> f32 {
		self.sample(&[], additional_channels, &[coord], None)
	}
	pub(crate) fn sample_i1d_i1d(&self, coord: i32, additional_channels: &[i32]) -> i32 {
		unit_to_i32(self.sample_i1d_1d(coord, additional_channels))
	}
	pub(crate) fn sample_i2d_1d(
		&self,
		coords: cgmath::Point2<i32>,
		additional_channels: &[i32],
	) -> f32 {
		let xs: [i32; 2] = coords.into();
		self.sample(&[], additional_channels, &xs, None)
	}
	pub(crate) fn sample_i3d_1d(
		&self,
		coords: cgmath::Point3<i32>,
		additional_channels: &[i32],
	) -> f32 {
		let xs: [i32; 3] = coords.into();
		self.sample(&[], additional_channels, &xs, None)
	}
	pub(crate) fn sample_i3d_3d(
		&self,
		coords: cgmath::Point3<i32>,
		additional_channels: &[i32],
	) -> cgmath::Point3<f32> {
		let xs: [i32; 3] = coords.into();
		let x = self.sample(&[], additional_channels, &xs, Some(1));
		let y = self.sample(&[], additional_channels, &xs, Some(2));
		let z = self.sample(&[], additional_channels, &xs, Some(3));
		cgmath::point3(x, y, z)
	}
}
