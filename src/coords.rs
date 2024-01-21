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

	pub fn _dimensions(self) -> cgmath::Vector3<i32> {
		(self.edge, self.edge, self.edge).into()
	}
}

/// Represents the cubic area of the voxel grid that is
/// in the chunk at the specified chunk coords.
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
		(inf.x <= coords.x && coords.x < sup_excluded.x)
			&& (inf.y <= coords.y && coords.y < sup_excluded.y)
			&& (inf.z <= coords.z && coords.z < sup_excluded.z)
	}

	/// Iterate over all the blocks (contained in the chunk) that touch the chunk face
	/// that faces to the given direction.
	pub fn _iter_block_coords_on_chunk_face(
		self,
		face_orientation: OrientedAxis,
	) -> impl Iterator<Item = BlockCoords> {
		let mut inf = self.block_coords_inf();
		let mut dims = self.cd._dimensions();
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

/// Represents a cubic area of the voxel grid.
#[derive(Clone, Copy)]
pub struct CubicCoordsSpan {
	pub inf: cgmath::Point3<i32>,
	pub sup_excluded: cgmath::Point3<i32>,
}

impl CubicCoordsSpan {
	pub fn with_inf_sup_but_sup_is_excluded(
		inf: cgmath::Point3<i32>,
		sup_excluded: cgmath::Point3<i32>,
	) -> CubicCoordsSpan {
		assert!(inf.x <= sup_excluded.x);
		assert!(inf.y <= sup_excluded.y);
		assert!(inf.z <= sup_excluded.z);
		CubicCoordsSpan { inf, sup_excluded }
	}

	pub fn with_inf_sup_but_sup_is_included(
		inf: cgmath::Point3<i32>,
		sup_included: cgmath::Point3<i32>,
	) -> CubicCoordsSpan {
		let sup_excluded = sup_included + cgmath::vec3(1, 1, 1);
		CubicCoordsSpan::with_inf_sup_but_sup_is_excluded(inf, sup_excluded)
	}

	pub fn _with_inf_and_dims(
		inf: cgmath::Point3<i32>,
		dims: cgmath::Vector3<i32>,
	) -> CubicCoordsSpan {
		let sup_excluded = inf + dims;
		CubicCoordsSpan::with_inf_sup_but_sup_is_excluded(inf, sup_excluded)
	}

	pub fn _with_inf_and_edge(inf: cgmath::Point3<i32>, edge: i32) -> CubicCoordsSpan {
		CubicCoordsSpan::_with_inf_and_dims(inf, cgmath::vec3(1, 1, 1) * edge)
	}

	/// A radius of 0 gives an empty area and a radius of 1 gives just the center.
	pub fn _with_center_and_radius(center: cgmath::Point3<i32>, radius: i32) -> CubicCoordsSpan {
		assert!(0 <= radius);
		if radius == 0 {
			// This is empty since `sup_excluded` is excluded (like a range i..i is empty).
			CubicCoordsSpan { inf: center, sup_excluded: center }
		} else {
			CubicCoordsSpan::_with_inf_and_edge(
				cgmath::point3(
					center.x - (radius - 1),
					center.y - (radius - 1),
					center.z - (radius - 1),
				),
				radius * 2 - 1,
			)
		}
	}

	pub fn from_chunk_span(chunk_span: ChunkCoordsSpan) -> CubicCoordsSpan {
		CubicCoordsSpan::with_inf_sup_but_sup_is_excluded(
			chunk_span.block_coords_inf(),
			chunk_span.block_coords_sup_excluded(),
		)
	}

	pub fn add_margins(&mut self, margin_to_add: i32) {
		self.inf -= cgmath::vec3(1, 1, 1) * margin_to_add;
		self.sup_excluded += cgmath::vec3(1, 1, 1) * margin_to_add;
	}

	pub fn overlaps(&self, other: &CubicCoordsSpan) -> bool {
		let no_overlap_x = other.sup_excluded.x <= self.inf.x || self.sup_excluded.x <= other.inf.x;
		let no_overlap_y = other.sup_excluded.y <= self.inf.y || self.sup_excluded.y <= other.inf.y;
		let no_overlap_z = other.sup_excluded.z <= self.inf.z || self.sup_excluded.z <= other.inf.z;
		let no_overlap = no_overlap_x || no_overlap_y || no_overlap_z;
		!no_overlap
	}

	pub fn contains(&self, coords: cgmath::Point3<i32>) -> bool {
		(self.inf.x <= coords.x && coords.x < self.sup_excluded.x)
			&& (self.inf.y <= coords.y && coords.y < self.sup_excluded.y)
			&& (self.inf.z <= coords.z && coords.z < self.sup_excluded.z)
	}

	pub fn iter(&self) -> impl Iterator<Item = cgmath::Point3<i32>> {
		iter_3d_rect_inf_sup_excluded(self.inf, self.sup_excluded)
	}

	pub fn intersection(&self, other: &CubicCoordsSpan) -> Option<CubicCoordsSpan> {
		self.overlaps(other).then(|| {
			CubicCoordsSpan::with_inf_sup_but_sup_is_excluded(
				cgmath::point3(
					self.inf.x.max(other.inf.x),
					self.inf.y.max(other.inf.y),
					self.inf.z.max(other.inf.z),
				),
				cgmath::point3(
					self.sup_excluded.x.min(other.sup_excluded.x),
					self.sup_excluded.y.min(other.sup_excluded.y),
					self.sup_excluded.z.min(other.sup_excluded.z),
				),
			)
		})
	}
}

/// Iterates over the 3D rectangle area `inf..sup_excluded` (`sup_excluded` not included).
pub fn iter_3d_rect_inf_sup_excluded(
	inf: cgmath::Point3<i32>,
	sup_excluded: cgmath::Point3<i32>,
) -> impl Iterator<Item = cgmath::Point3<i32>> {
	assert!(inf.x <= sup_excluded.x);
	assert!(inf.y <= sup_excluded.y);
	assert!(inf.z <= sup_excluded.z);
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
	assert!(0 <= edge);
	iter_3d_rect_inf_dims(inf, (edge, edge, edge).into())
}

/// Iterates over a 3D cubic area of the given center and given radius.
///
/// A radius of 0 gives an empty iterator and a radius of 1 gives just the center.
pub fn iter_3d_cube_center_radius(
	center: cgmath::Point3<i32>,
	radius: i32,
) -> impl Iterator<Item = cgmath::Point3<i32>> {
	assert!(0 <= radius);
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

impl ChunkDimensions {
	pub fn world_coords_to_containing_chunk_coords(self, coords: BlockCoords) -> ChunkCoords {
		coords.map(|x| x.div_euclid(self.edge))
	}
}

/// Axis (without considering its orientation).
///
/// Note that the vertical axis is Z.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// An array of 27 boolean values stored in a `u32`.
#[derive(Debug, Clone, Copy)]
struct BitArray27 {
	data: u32,
}
impl BitArray27 {
	fn new_zero() -> BitArray27 {
		BitArray27 { data: 0 }
	}
	fn get(self, index: usize) -> bool {
		(self.data >> index) & 1 != 0
	}
	fn set(&mut self, index: usize, value: bool) {
		self.data = (self.data & !(1 << index)) | ((value as u32) << index);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn bit_array() {
		let mut bit_array = BitArray27::new_zero();
		bit_array.set(3, true);
		assert!(!bit_array.get(2));
		assert!(bit_array.get(3));
	}
}

/// Coords of a cell in a `BitCube3` ^^.
#[derive(Debug, Clone, Copy)]
pub struct BitCube3Coords {
	x: i32,
	y: i32,
	z: i32,
}

impl From<cgmath::Point3<i32>> for BitCube3Coords {
	fn from(coords: cgmath::Point3<i32>) -> BitCube3Coords {
		assert!((-1..=1).contains(&coords.x));
		assert!((-1..=1).contains(&coords.y));
		assert!((-1..=1).contains(&coords.z));
		BitCube3Coords { x: coords.x, y: coords.y, z: coords.z }
	}
}

impl BitCube3Coords {
	fn index(self) -> usize {
		((self.x + 1) + (self.y + 1) * 3 + (self.z + 1) * 3 * 3) as usize
	}
	pub fn set(&mut self, axis: NonOrientedAxis, value: i32) {
		assert!((-1..=1).contains(&value));
		match axis {
			NonOrientedAxis::X => self.x = value,
			NonOrientedAxis::Y => self.y = value,
			NonOrientedAxis::Z => self.z = value,
		}
	}
}

/// A 3x3x3 cube of boolean values.
/// The (0, 0, 0) coords is the center of the cube (that spans from (-1, -1, -1) to (1, 1, 1)).
#[derive(Debug, Clone, Copy)]
pub struct BitCube3 {
	data: BitArray27,
}
impl BitCube3 {
	pub fn new_zero() -> BitCube3 {
		BitCube3 { data: BitArray27::new_zero() }
	}
	pub fn get(self, coords: BitCube3Coords) -> bool {
		self.data.get(coords.index())
	}
	pub fn set(&mut self, coords: BitCube3Coords, value: bool) {
		self.data.set(coords.index(), value);
	}
}

/// Just a 3D rectangular axis-aligned box.
/// It cannot rotate as it stays aligned on the axes.
pub struct AlignedBox {
	/// Position of the center of the box.
	pub pos: cgmath::Point3<f32>,
	/// Width of the box along each axis.
	pub dims: cgmath::Vector3<f32>,
}
