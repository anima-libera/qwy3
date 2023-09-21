//! Coordinate systems for Qwy3.
//!
//! Note that the up direction is Z+, unlike Minecraft's Y+ up direction.
//!
//! The chunks are cubic and organized on a 3D grid, unlike Minecraft's 2D grid of chunks.
//! This means that the world is as infinite along the Z axis (in both directions) as it is
//! horizontally.

use std::f32::consts::TAU;

/// Coordinates of a block in the world.
pub type BlockCoords = cgmath::Point3<i32>;

/// Chunks are cubic parts of the world, all of the same size and arranged in a 3D grid.
/// The length (in blocks) of the edges of the chunks is not hardcoded. It can be
/// modified (to some extent) and passed around in a `ChunkDimensions`.
#[derive(Clone, Copy)]
pub struct ChunkDimensions {
	/// Length (in blocks) of the edge of each (cubic) chunk.
	pub edge: i32,
}

impl From<i32> for ChunkDimensions {
	fn from(chunk_side_length: i32) -> ChunkDimensions {
		ChunkDimensions { edge: chunk_side_length }
	}
}

impl ChunkDimensions {
	pub fn number_of_blocks(self) -> usize {
		self.edge.pow(3) as usize
	}

	pub fn dimensions(self) -> cgmath::Vector3<i32> {
		(self.edge, self.edge, self.edge).into()
	}
}

#[derive(Clone, Copy)]
pub struct ChunkCoordsSpan {
	pub cd: ChunkDimensions,
	pub chunk_coords: ChunkCoords,
}

impl ChunkCoordsSpan {
	pub fn block_coords_inf(self) -> BlockCoords {
		self.chunk_coords * self.cd.edge
	}

	pub fn block_coords_sup_excluded(self) -> BlockCoords {
		self.chunk_coords.map(|x| x + 1) * self.cd.edge
	}

	pub fn iter_coords(self) -> impl Iterator<Item = cgmath::Point3<i32>> {
		iter_3d_rect_inf_sup_excluded(self.block_coords_inf(), self.block_coords_sup_excluded())
	}

	pub fn contains(self, coords: BlockCoords) -> bool {
		let inf = self.block_coords_inf();
		let sup_excluded = self.block_coords_sup_excluded();
		inf.x <= coords.x
			&& coords.x < sup_excluded.x
			&& inf.y <= coords.y
			&& coords.y < sup_excluded.y
			&& inf.z <= coords.z
			&& coords.z < sup_excluded.z
	}

	/// Iterate over all the blocks (contained in the chunk) that touch the chunk face
	/// that faces to the given direction.
	pub fn iter_block_coords_on_chunk_face(
		self,
		face_orientation: OrientedAxis,
	) -> impl Iterator<Item = BlockCoords> {
		let mut inf = self.block_coords_inf();
		let mut dims = self.cd.dimensions();
		// We just flatten the area along the right axis.
		dims[face_orientation.axis.index()] = 1;
		// We also make sure the flatten area touches the right face.
		if face_orientation.orientation == AxisOrientation::Positivewards {
			inf[face_orientation.axis.index()] += self.cd.edge - 1;
		}
		iter_3d_rect_inf_dims(inf, dims)
	}

	pub fn internal_index(self, coords: BlockCoords) -> Option<usize> {
		self.contains(coords).then(|| {
			let internal_coords = coords - self.block_coords_inf();
			let (x, y, z) = internal_coords.into();
			(z * self.cd.edge.pow(2) + y * self.cd.edge + x) as usize
		})
	}
}

/// Iterates over the 3D rectangle area `inf..sup_excluded` (`sup_excluded` not included).
pub fn iter_3d_rect_inf_sup_excluded(
	inf: cgmath::Point3<i32>,
	sup_excluded: cgmath::Point3<i32>,
) -> impl Iterator<Item = cgmath::Point3<i32>> {
	debug_assert!(inf.x <= sup_excluded.x);
	debug_assert!(inf.y <= sup_excluded.y);
	debug_assert!(inf.z <= sup_excluded.z);
	(inf.z..sup_excluded.z).flat_map(move |z| {
		(inf.y..sup_excluded.y)
			.flat_map(move |y| (inf.x..sup_excluded.x).map(move |x| (x, y, z).into()))
	})
}

/// Iterates over the 3D rectangle area `inf..(inf+dims)` (`inf+dims` not included).
pub fn iter_3d_rect_inf_dims(
	inf: cgmath::Point3<i32>,
	dims: cgmath::Vector3<i32>,
) -> impl Iterator<Item = cgmath::Point3<i32>> {
	let sup = inf + dims;
	iter_3d_rect_inf_sup_excluded(inf, sup)
}

/// Iterates over a 3D cubic area of negativewards corner at `inf` and edges of length `edge`.
pub fn iter_3d_cube_inf_edge(
	inf: cgmath::Point3<i32>,
	edge: i32,
) -> impl Iterator<Item = cgmath::Point3<i32>> {
	iter_3d_rect_inf_dims(inf, (edge, edge, edge).into())
}

/// Iterates over a 3D cubic area of the given center and given radius.
pub fn iter_3d_cube_center_radius(
	center: cgmath::Point3<i32>,
	radius: i32,
) -> impl Iterator<Item = cgmath::Point3<i32>> {
	if radius == 0 {
		iter_3d_cube_inf_edge(center, 0)
	} else {
		iter_3d_cube_inf_edge(
			(
				center.x - (radius - 1),
				center.y - (radius - 1),
				center.z - (radius - 1),
			)
				.into(),
			radius * 2 - 1,
		)
	}
}

/// Coordinates of a chunk in the 3D grid of chunks
/// (which is not on the same scale as block coords, here we designate whole chunks).
pub type ChunkCoords = cgmath::Point3<i32>;

pub fn is_neighbor_with(a: cgmath::Point3<i32>, b: cgmath::Point3<i32>) -> bool {
	a.x.abs_diff(b.x) + a.y.abs_diff(b.y) + a.z.abs_diff(b.z) == 1
}

/// This is supposed to return the direction to go from `a` to `b` if that takes
/// just one move of one chunk (so if they are neighbors). Returns `None` if not neighbors.
pub fn direction_to_neighbor(
	a: cgmath::Point3<i32>,
	b: cgmath::Point3<i32>,
) -> Option<OrientedAxis> {
	let dx = b.x - a.x;
	let dy = b.y - a.y;
	let dz = b.z - a.z;
	let (axis, d) = if dx != 0 && dy == 0 && dz == 0 {
		(NonOrientedAxis::X, dx)
	} else if dx == 0 && dy != 0 && dz == 0 {
		(NonOrientedAxis::Y, dy)
	} else if dx == 0 && dy == 0 && dz != 0 {
		(NonOrientedAxis::Z, dz)
	} else {
		return None;
	};
	let orientation = if d == -1 {
		AxisOrientation::Negativewards
	} else if d == 1 {
		AxisOrientation::Positivewards
	} else {
		return None;
	};
	Some(OrientedAxis { axis, orientation })
}

impl ChunkDimensions {
	pub fn world_coords_to_containing_chunk_coords(self, coords: BlockCoords) -> ChunkCoords {
		coords.map(|x| x.div_euclid(self.edge))
	}
}

/// Axis (without considering its orientation).
///
/// Note that the vertical axis is Z.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NonOrientedAxis {
	X,
	Y,
	Z,
}

impl NonOrientedAxis {
	pub fn iter_over_the_three_possible_axes() -> impl Iterator<Item = NonOrientedAxis> {
		[NonOrientedAxis::X, NonOrientedAxis::Y, NonOrientedAxis::Z].into_iter()
	}

	pub fn index(self) -> usize {
		match self {
			NonOrientedAxis::X => 0,
			NonOrientedAxis::Y => 1,
			NonOrientedAxis::Z => 2,
		}
	}
}

/// For a given `NonOrientedAxis`, this allows to represent
/// one of the two orientations of said axis. See `OrientedAxis` to do exactly that.
///
/// For example, `NonOrientedAxis::Z` simply represents the vertical axis,
/// without a notion of upwards or downwards. `AxisOrientation::Positivewards` allows
/// to represent the orientation of increasing coordinate values along the given axis.
/// Thus, `NonOrientedAxis::Z` and `AxisOrientation::Positivewards` represent the upwards
/// direction (Z+), as increasing the Z coordinate of a point makes it go upwards.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AxisOrientation {
	Positivewards,
	Negativewards,
}

impl AxisOrientation {
	pub fn iter_over_the_two_possible_orientations() -> impl Iterator<Item = AxisOrientation> {
		// Truly one of the functions of all times.
		[
			AxisOrientation::Positivewards,
			AxisOrientation::Negativewards,
		]
		.into_iter()
	}

	pub fn sign(self) -> i32 {
		match self {
			AxisOrientation::Positivewards => 1,
			AxisOrientation::Negativewards => -1,
		}
	}
}

/// Axis but oriented.
///
/// Note that upwards is Z+ and downwards is Z-.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct OrientedAxis {
	pub axis: NonOrientedAxis,
	pub orientation: AxisOrientation,
}

impl OrientedAxis {
	pub fn all_the_six_possible_directions() -> impl Iterator<Item = OrientedAxis> {
		NonOrientedAxis::iter_over_the_three_possible_axes().flat_map(|axis| {
			AxisOrientation::iter_over_the_two_possible_orientations()
				.map(move |orientation| OrientedAxis { axis, orientation })
		})
	}

	pub fn delta(self) -> cgmath::Vector3<i32> {
		let mut delta: cgmath::Vector3<i32> = (0, 0, 0).into();
		delta[self.axis.index()] = self.orientation.sign();
		delta
	}
}

/// Spherical polar coordinates, represent a direction in 3D (a vector without a magnitude).
/// It makes wokring with some stuff eazier than via a normalized vector.
///
/// Angles are in radians.
#[derive(Clone, Copy)]
pub struct AngularDirection {
	/// Angle of the direction when projected on the horizontal plane.
	/// It should range from `0.0` to `TAU` radians.
	pub angle_horizontal: f32,
	/// Angle that gives the direction a height component (along the Z axis).
	/// * `0.0` radians means that the direction points straight upwards,
	/// * `TAU / 4.0` means that the direction is horizontal,
	/// * `TAU / 2.0` means that the direction points straight downwards.
	///
	/// It is not really an issue if this angle gets outside of its range
	/// (from `0.0` to `TAU / 2.0`) but uh.. idk, maybe it is in some cases, beware.
	pub angle_vertical: f32,
}

impl AngularDirection {
	/// When `AngularDirection::angle_vertical` is `ANGLE_VERTICAL_HORIZONTAL` then
	/// it means the angular direction is horizontal (no Z component).
	///
	/// See the documentation of `AngularDirection::angle_vertical`.
	const ANGLE_VERTICAL_HORIZONTAL: f32 = TAU / 4.0;

	pub fn from_angles(angle_horizontal: f32, angle_vertical: f32) -> AngularDirection {
		AngularDirection { angle_horizontal, angle_vertical }
	}

	pub fn from_angle_horizontal(angle_horizontal: f32) -> AngularDirection {
		AngularDirection::from_angles(
			angle_horizontal,
			AngularDirection::ANGLE_VERTICAL_HORIZONTAL,
		)
	}

	pub fn add_to_horizontal_angle(mut self, angle_horizontal_to_add: f32) -> AngularDirection {
		self.angle_horizontal += angle_horizontal_to_add;
		self
	}

	pub fn add_to_vertical_angle(mut self, angle_vertical_to_add: f32) -> AngularDirection {
		self.angle_vertical += angle_vertical_to_add;
		self
	}

	pub fn to_horizontal(mut self) -> AngularDirection {
		self.angle_vertical = AngularDirection::ANGLE_VERTICAL_HORIZONTAL;
		self
	}

	/// Turn it into a good old vec3, normalized.
	pub fn to_vec3(self) -> cgmath::Vector3<f32> {
		let direction_vertical = f32::cos(self.angle_vertical);
		let mut direction_horizontal = cgmath::Vector2::<f32>::from((
			f32::cos(self.angle_horizontal),
			f32::sin(self.angle_horizontal),
		));
		// Kinda normalize the result.
		direction_horizontal *= f32::sqrt(1.0 - direction_vertical.powi(2));
		// Handle the fact that `angle_vertical` may be outside of the `0.0` to `TAU / 2.0` range.
		direction_horizontal *= if self.angle_vertical < 0.0 { -1.0 } else { 1.0 };
		cgmath::Vector3::<f32>::from((
			direction_horizontal.x,
			direction_horizontal.y,
			direction_vertical,
		))
	}
}
