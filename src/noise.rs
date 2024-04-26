//! N-dimensional noise!
//!
//! This is a very "we have noise at home" implementation, slow and all.
//! Though, maybe not as slow now as it once was.
//!
//! The idea is that we consider an N-dimensional grid of nodes
//! where nodes are at every interger coordinates
//! and each node is given a noise value via `raw_noise_node`.
//! Then, for points that don't fall on nodes, they fall in N-dimensional
//! (hyper)cubic cells of 2^N nodes as vertices,
//! we then interpolate the nodes' noise values with `raw_noise`.
//! Then we can do the usual stuff and add octaves with `octaves_noise`.

use std::hash::Hasher;

use fxhash::FxHasher64;

fn positive_fract(x: f32) -> f32 {
	x - f32::floor(x)
}

fn unit_to_i32(x: f32) -> i32 {
	(x * (i32::MAX as f32 - i32::MIN as f32) + i32::MIN as f32).round() as i32
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
#[inline]
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

#[inline]
fn raw_noise_node(hash: FxHasher64) -> f32 {
	f32::cos(hash.finish() as f32) * 0.5 + 0.5
}

#[inline]
fn raw_noise_rec(xs: &[f32], factor: f32, hash: FxHasher64) -> f32 {
	if !xs.is_empty() {
		let x = xs[0] * factor;
		// For every continuous coordinate, we interpolate between
		// the two closest discreet node values on that axis.
		// In one dimension (with N <= x < N+1), it looks like this:
		// ... --|------#----|--> ...
		//       N      x   N+1
		//      inf         sup
		// And we can do that by calling this recursively
		// with N and N+1 as additional channel parameters.
		let channel_inf = f32::floor(x) as i32;
		let sub_noise_inf = {
			let mut hash_inf = hash.clone();
			hash_inf.write_i32(channel_inf);
			raw_noise_rec(&xs[1..], factor, hash_inf)
		};
		let channel_sup = channel_inf + 1;
		let sub_noise_sup = {
			let mut hash_sup = hash.clone();
			hash_sup.write_i32(channel_sup);
			raw_noise_rec(&xs[1..], factor, hash_sup)
		};
		let x_fract = positive_fract(x);
		interpolate(&smoothcos, x_fract, 0.0, 1.0, sub_noise_inf, sub_noise_sup)
	} else {
		// No more continuous coordinates, we are on a node and can get its noise value.
		raw_noise_node(hash)
	}
}

#[inline]
fn raw_noise(xs: &[f32], factor: f32, hash: FxHasher64) -> f32 {
	raw_noise_rec(xs, factor, hash)
}

fn octaves_noise(number_of_octaves: u32, xs: &[f32], hash: FxHasher64) -> f32 {
	let mut value_sum = 0.0;
	let mut coef_sum = 0.0;
	let mut coef = 1.0;
	let mut factor = 1.0;
	for _i in 0..number_of_octaves {
		value_sum += coef * raw_noise(xs, factor, hash.clone());
		coef_sum += coef;
		coef /= 2.0;
		factor *= 2.0;
	}
	value_sum / coef_sum
}

pub(crate) struct OctavedNoise {
	number_of_octaves: u32,
	base_hash: FxHasher64,
}

impl OctavedNoise {
	pub(crate) fn new(number_of_octaves: u32, base_channels: Vec<i32>) -> OctavedNoise {
		let mut base_hash = FxHasher64::default();
		base_channels.into_iter().for_each(|channel| base_hash.write_i32(channel));
		OctavedNoise { number_of_octaves, base_hash }
	}

	pub(crate) fn sample(&self, xs: &[f32], additional_channels: &[&[i32]]) -> f32 {
		let mut hash = self.base_hash.clone();
		additional_channels
			.iter()
			.for_each(|channels| channels.iter().for_each(|channel| hash.write_i32(*channel)));
		octaves_noise(self.number_of_octaves, xs, hash)
	}

	pub(crate) fn sample_2d_1d(
		&self,
		coords: cgmath::Point2<f32>,
		additional_channels: &[i32],
	) -> f32 {
		let xs: [f32; 2] = coords.into();
		self.sample(&xs, &[additional_channels])
	}
	pub(crate) fn sample_3d_1d(
		&self,
		coords: cgmath::Point3<f32>,
		additional_channels: &[i32],
	) -> f32 {
		let xs: [f32; 3] = coords.into();
		self.sample(&xs, &[additional_channels])
	}
	pub(crate) fn _sample_3d_3d(
		&self,
		coords: cgmath::Point3<f32>,
		additional_channels: &[i32],
	) -> cgmath::Point3<f32> {
		let xs: [f32; 3] = coords.into();
		let x = self.sample(&xs, &[additional_channels, &[1]]);
		let y = self.sample(&xs, &[additional_channels, &[2]]);
		let z = self.sample(&xs, &[additional_channels, &[3]]);
		cgmath::point3(x, y, z)
	}
	pub(crate) fn sample_i1d_1d(&self, coord: i32, additional_channels: &[i32]) -> f32 {
		self.sample(&[], &[additional_channels, &[coord]])
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
		self.sample(&[], &[additional_channels, &xs])
	}
	pub(crate) fn sample_i3d_1d(
		&self,
		coords: cgmath::Point3<i32>,
		additional_channels: &[i32],
	) -> f32 {
		let xs: [i32; 3] = coords.into();
		self.sample(&[], &[additional_channels, &xs])
	}
	pub(crate) fn sample_i3d_i1d(
		&self,
		coords: cgmath::Point3<i32>,
		additional_channels: &[i32],
	) -> i32 {
		unit_to_i32(self.sample_i3d_1d(coords, additional_channels))
	}
	pub(crate) fn sample_i3d_3d(
		&self,
		coords: cgmath::Point3<i32>,
		additional_channels: &[i32],
	) -> cgmath::Point3<f32> {
		let xs: [i32; 3] = coords.into();
		let x = self.sample(&[], &[additional_channels, &xs, &[1]]);
		let y = self.sample(&[], &[additional_channels, &xs, &[2]]);
		let z = self.sample(&[], &[additional_channels, &xs, &[3]]);
		cgmath::point3(x, y, z)
	}
}
