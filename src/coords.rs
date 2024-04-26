//! Coordinate systems for Qwy3.
//!
//! Note that the up direction is Z+, unlike Minecraft's Y+ up direction.
//!
//! The chunks are cubic and organized on a 3D grid, unlike Minecraft's 2D grid of chunks.
//! This means that the world is as infinite along the Z axis (in both directions) as it is
//! horizontally.

use std::f32::consts::TAU;

use cgmath::EuclideanSpace;
use serde::{Deserialize, Serialize};

/// Coordinates of a block in the world.
pub(crate) type BlockCoords = cgmath::Point3<i32>;

/// Chunks are cubic parts of the world, all of the same size and arranged in a 3D grid.
/// The length (in blocks) of the edges of the chunks is not hardcoded. It can be
/// modified (to some extent) and passed around in a `ChunkDimensions`.
#[derive(Clone, Copy)]
pub(crate) struct ChunkDimensions {
	/// Length (in blocks) of the edge of each (cubic) chunk.
	pub(crate) edge: i32,
}

impl From<i32> for ChunkDimensions {
	fn from(chunk_side_length: i32) -> ChunkDimensions {
		ChunkDimensions { edge: chunk_side_length }
	}
}

impl ChunkDimensions {
	pub(crate) fn number_of_blocks_in_a_chunk(self) -> usize {
		self.edge.pow(3) as usize
	}

	pub(crate) fn _dimensions(self) -> cgmath::Vector3<i32> {
		(self.edge, self.edge, self.edge).into()
	}
}

/// Represents the cubic area of the voxel grid that is
/// in the chunk at the specified chunk coords.
#[derive(Clone, Copy)]
pub(crate) struct ChunkCoordsSpan {
	pub(crate) cd: ChunkDimensions,
	pub(crate) chunk_coords: ChunkCoords,
}

impl ChunkCoordsSpan {
	pub(crate) fn block_coords_inf(self) -> BlockCoords {
		self.chunk_coords * self.cd.edge
	}

	pub(crate) fn block_coords_sup_excluded(self) -> BlockCoords {
		self.chunk_coords.map(|x| x + 1) * self.cd.edge
	}

	pub(crate) fn iter_coords(self) -> impl Iterator<Item = cgmath::Point3<i32>> {
		iter_3d_rect_inf_sup_excluded(self.block_coords_inf(), self.block_coords_sup_excluded())
	}

	pub(crate) fn contains(self, coords: BlockCoords) -> bool {
		let inf = self.block_coords_inf();
		let sup_excluded = self.block_coords_sup_excluded();
		(inf.x <= coords.x && coords.x < sup_excluded.x)
			&& (inf.y <= coords.y && coords.y < sup_excluded.y)
			&& (inf.z <= coords.z && coords.z < sup_excluded.z)
	}

	/// Iterate over all the blocks (contained in the chunk) that touch the chunk face
	/// that faces to the given direction.
	pub(crate) fn iter_block_coords_on_chunk_face(
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

	pub(crate) fn internal_index(self, coords: BlockCoords) -> Option<usize> {
		self.contains(coords).then(|| {
			let internal_coords = coords - self.block_coords_inf();
			let (x, y, z) = internal_coords.into();
			(z * self.cd.edge.pow(2) + y * self.cd.edge + x) as usize
		})
	}
}

/// Represents a cubic area of the voxel grid.
#[derive(Clone, Copy)]
pub(crate) struct CubicCoordsSpan {
	pub(crate) inf: cgmath::Point3<i32>,
	pub(crate) sup_excluded: cgmath::Point3<i32>,
}

impl CubicCoordsSpan {
	pub(crate) fn with_inf_sup_but_sup_is_excluded(
		inf: cgmath::Point3<i32>,
		sup_excluded: cgmath::Point3<i32>,
	) -> CubicCoordsSpan {
		assert!(inf.x <= sup_excluded.x);
		assert!(inf.y <= sup_excluded.y);
		assert!(inf.z <= sup_excluded.z);
		CubicCoordsSpan { inf, sup_excluded }
	}

	pub(crate) fn with_inf_sup_but_sup_is_included(
		inf: cgmath::Point3<i32>,
		sup_included: cgmath::Point3<i32>,
	) -> CubicCoordsSpan {
		let sup_excluded = sup_included + cgmath::vec3(1, 1, 1);
		CubicCoordsSpan::with_inf_sup_but_sup_is_excluded(inf, sup_excluded)
	}

	pub(crate) fn _with_inf_and_dims(
		inf: cgmath::Point3<i32>,
		dims: cgmath::Vector3<i32>,
	) -> CubicCoordsSpan {
		let sup_excluded = inf + dims;
		CubicCoordsSpan::with_inf_sup_but_sup_is_excluded(inf, sup_excluded)
	}

	pub(crate) fn _with_inf_and_edge(inf: cgmath::Point3<i32>, edge: i32) -> CubicCoordsSpan {
		CubicCoordsSpan::_with_inf_and_dims(inf, cgmath::vec3(1, 1, 1) * edge)
	}

	/// A radius of 0 gives an empty area and a radius of 1 gives just the center.
	pub(crate) fn with_center_and_radius(
		center: cgmath::Point3<i32>,
		radius: i32,
	) -> CubicCoordsSpan {
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

	pub(crate) fn from_chunk_span(chunk_span: ChunkCoordsSpan) -> CubicCoordsSpan {
		CubicCoordsSpan::with_inf_sup_but_sup_is_excluded(
			chunk_span.block_coords_inf(),
			chunk_span.block_coords_sup_excluded(),
		)
	}

	pub(crate) fn to_aligned_box(self) -> AlignedBox {
		let pos = self.inf.map(|x| x as f32).midpoint(self.sup_included().map(|x| x as f32));
		let dims = self.sup_excluded.map(|x| x as f32) - self.inf.map(|x| x as f32);
		AlignedBox { pos, dims }
	}

	pub(crate) fn sup_included(&self) -> BlockCoords {
		self.sup_excluded - cgmath::vec3(1, 1, 1)
	}

	pub(crate) fn add_margins(&mut self, margin_to_add: i32) {
		self.inf -= cgmath::vec3(1, 1, 1) * margin_to_add;
		self.sup_excluded += cgmath::vec3(1, 1, 1) * margin_to_add;
	}

	pub(crate) fn overlaps(&self, other: &CubicCoordsSpan) -> bool {
		let no_overlap_x = other.sup_excluded.x <= self.inf.x || self.sup_excluded.x <= other.inf.x;
		let no_overlap_y = other.sup_excluded.y <= self.inf.y || self.sup_excluded.y <= other.inf.y;
		let no_overlap_z = other.sup_excluded.z <= self.inf.z || self.sup_excluded.z <= other.inf.z;
		let no_overlap = no_overlap_x || no_overlap_y || no_overlap_z;
		!no_overlap
	}

	pub(crate) fn contains(&self, coords: cgmath::Point3<i32>) -> bool {
		(self.inf.x <= coords.x && coords.x < self.sup_excluded.x)
			&& (self.inf.y <= coords.y && coords.y < self.sup_excluded.y)
			&& (self.inf.z <= coords.z && coords.z < self.sup_excluded.z)
	}

	pub(crate) fn iter(self) -> impl Iterator<Item = cgmath::Point3<i32>> {
		iter_3d_rect_inf_sup_excluded(self.inf, self.sup_excluded)
	}

	pub(crate) fn intersection(&self, other: &CubicCoordsSpan) -> Option<CubicCoordsSpan> {
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

	pub(crate) fn side(mut self, oriented_axis: OrientedAxis) -> CubicCoordsSpan {
		let axis = oriented_axis.axis.index();
		if oriented_axis.orientation == AxisOrientation::Positivewards {
			self.inf[axis] = self.sup_excluded[axis] - 1;
		} else {
			self.sup_excluded[axis] = self.inf[axis] + 1;
		}
		self
	}
}

/// Iterates over the 3D rectangle area `inf..sup_excluded` (`sup_excluded` not included).
#[inline]
pub(crate) fn iter_3d_rect_inf_sup_excluded(
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

/// Iterates over the 3D rectangle area `inf..=sup_included` (`sup_included` is included).
#[inline]
pub(crate) fn iter_3d_rect_inf_sup_included(
	inf: cgmath::Point3<i32>,
	sup_included: cgmath::Point3<i32>,
) -> impl Iterator<Item = cgmath::Point3<i32>> {
	iter_3d_rect_inf_sup_excluded(inf, sup_included + cgmath::vec3(1, 1, 1))
}

/// Iterates over the 3D rectangle area `inf..(inf+dims)` (`inf+dims` not included).
#[inline]
pub(crate) fn iter_3d_rect_inf_dims(
	inf: cgmath::Point3<i32>,
	dims: cgmath::Vector3<i32>,
) -> impl Iterator<Item = cgmath::Point3<i32>> {
	let sup = inf + dims;
	iter_3d_rect_inf_sup_excluded(inf, sup)
}

/// Iterates over a 3D cubic area of negativewards corner at `inf` and edges of length `edge`.
#[inline]
pub(crate) fn iter_3d_cube_inf_edge(
	inf: cgmath::Point3<i32>,
	edge: i32,
) -> impl Iterator<Item = cgmath::Point3<i32>> {
	assert!(0 <= edge);
	iter_3d_rect_inf_dims(inf, (edge, edge, edge).into())
}

/// Iterates over a 3D cubic area of the given center and given radius.
///
/// A radius of 0 gives an empty iterator and a radius of 1 gives just the center.
#[inline]
pub(crate) fn iter_3d_cube_center_radius(
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
pub(crate) type ChunkCoords = cgmath::Point3<i32>;

impl ChunkDimensions {
	pub(crate) fn world_coords_to_containing_chunk_coords(self, coords: BlockCoords) -> ChunkCoords {
		coords.map(|x| x.div_euclid(self.edge))
	}
}

/// Axis (without considering its orientation).
///
/// Note that the vertical axis is Z.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum NonOrientedAxis {
	X,
	Y,
	Z,
}

impl NonOrientedAxis {
	pub(crate) fn iter_over_the_three_possible_axes() -> impl Iterator<Item = NonOrientedAxis> {
		[NonOrientedAxis::X, NonOrientedAxis::Y, NonOrientedAxis::Z].into_iter()
	}

	pub(crate) fn index(self) -> usize {
		match self {
			NonOrientedAxis::X => 0,
			NonOrientedAxis::Y => 1,
			NonOrientedAxis::Z => 2,
		}
	}

	pub(crate) fn as_char(self) -> char {
		match self {
			NonOrientedAxis::X => 'x',
			NonOrientedAxis::Y => 'y',
			NonOrientedAxis::Z => 'z',
		}
	}

	pub(crate) fn the_other_two_axes(self) -> [NonOrientedAxis; 2] {
		match self {
			NonOrientedAxis::X => [NonOrientedAxis::Y, NonOrientedAxis::Z],
			NonOrientedAxis::Y => [NonOrientedAxis::X, NonOrientedAxis::Z],
			NonOrientedAxis::Z => [NonOrientedAxis::X, NonOrientedAxis::Y],
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum AxisOrientation {
	Positivewards,
	Negativewards,
}

impl AxisOrientation {
	pub(crate) fn iter_over_the_two_possible_orientations() -> impl Iterator<Item = AxisOrientation>
	{
		// Truly one of the functions of all times.
		[
			AxisOrientation::Positivewards,
			AxisOrientation::Negativewards,
		]
		.into_iter()
	}

	pub(crate) fn sign(self) -> i32 {
		match self {
			AxisOrientation::Positivewards => 1,
			AxisOrientation::Negativewards => -1,
		}
	}

	pub(crate) fn as_char(self) -> char {
		match self {
			AxisOrientation::Positivewards => '+',
			AxisOrientation::Negativewards => '-',
		}
	}

	fn from_sign(sign: i32) -> Option<AxisOrientation> {
		match sign {
			1 => Some(AxisOrientation::Positivewards),
			-1 => Some(AxisOrientation::Negativewards),
			_ => None,
		}
	}
}

/// Axis but oriented.
///
/// Note that upwards is Z+ and downwards is Z-.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct OrientedAxis {
	pub(crate) axis: NonOrientedAxis,
	pub(crate) orientation: AxisOrientation,
}

impl OrientedAxis {
	pub(crate) fn from_delta(delta: cgmath::Vector3<i32>) -> Option<OrientedAxis> {
		if delta.x.abs() + delta.y.abs() + delta.z.abs() != 1 {
			return None;
		}
		let sign = delta.x + delta.y + delta.z;
		let orientation = AxisOrientation::from_sign(sign).unwrap();
		let axis = if delta.x != 0 {
			NonOrientedAxis::X
		} else if delta.y != 0 {
			NonOrientedAxis::Y
		} else if delta.z != 0 {
			NonOrientedAxis::Z
		} else {
			unreachable!()
		};
		Some(OrientedAxis { axis, orientation })
	}

	pub(crate) fn all_the_six_possible_directions() -> impl Iterator<Item = OrientedAxis> {
		NonOrientedAxis::iter_over_the_three_possible_axes().flat_map(|axis| {
			AxisOrientation::iter_over_the_two_possible_orientations()
				.map(move |orientation| OrientedAxis { axis, orientation })
		})
	}

	pub(crate) fn delta(self) -> cgmath::Vector3<i32> {
		let mut delta: cgmath::Vector3<i32> = (0, 0, 0).into();
		delta[self.axis.index()] = self.orientation.sign();
		delta
	}
}

#[allow(dead_code)]
impl OrientedAxis {
	pub(crate) const X_PLUS: OrientedAxis = OrientedAxis {
		axis: NonOrientedAxis::X,
		orientation: AxisOrientation::Positivewards,
	};
	pub(crate) const X_MINUS: OrientedAxis = OrientedAxis {
		axis: NonOrientedAxis::X,
		orientation: AxisOrientation::Negativewards,
	};
	pub(crate) const Y_PLUS: OrientedAxis = OrientedAxis {
		axis: NonOrientedAxis::Y,
		orientation: AxisOrientation::Positivewards,
	};
	pub(crate) const Y_MINUS: OrientedAxis = OrientedAxis {
		axis: NonOrientedAxis::Y,
		orientation: AxisOrientation::Negativewards,
	};
	pub(crate) const Z_PLUS: OrientedAxis = OrientedAxis {
		axis: NonOrientedAxis::Z,
		orientation: AxisOrientation::Positivewards,
	};
	pub(crate) const Z_MINUS: OrientedAxis = OrientedAxis {
		axis: NonOrientedAxis::Z,
		orientation: AxisOrientation::Negativewards,
	};
}

/// Coordinates of a face.
///
/// `BlockCoords` refers to a block, and `OrientedFaceCoords` refers to one face of such block.
///
/// Also, the referred face is oriented; that means that, for two adjacent blocks A and B,
/// the face between A and B can have two orientations: from A to B and from B et A.
/// One of these two blocks is the "interior" block (field `interior_coords`)
/// and the other is the "exterior" block (method `exterior_coords`).
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct OrientedFaceCoords {
	pub(crate) interior_coords: BlockCoords,
	pub(crate) direction_to_exterior: OrientedAxis,
}

impl OrientedFaceCoords {
	pub(crate) fn exterior_coords(&self) -> BlockCoords {
		self.interior_coords + self.direction_to_exterior.delta()
	}
}

/// Spherical polar coordinates, represent a direction in 3D (a vector without a magnitude).
/// It makes wokring with some stuff eazier than via a normalized vector.
///
/// Angles are in radians.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub(crate) struct AngularDirection {
	/// Angle of the direction when projected on the horizontal plane.
	/// It should range from `0.0` to `TAU` radians.
	pub(crate) angle_horizontal: f32,
	/// Angle that gives the direction a height component (along the Z axis).
	/// * `0.0` radians means that the direction points straight upwards,
	/// * `TAU / 4.0` means that the direction is horizontal,
	/// * `TAU / 2.0` means that the direction points straight downwards.
	///
	/// It is not really an issue if this angle gets outside of its range
	/// (from `0.0` to `TAU / 2.0`) but uh.. idk, maybe it is in some cases, beware.
	pub(crate) angle_vertical: f32,
}

impl AngularDirection {
	/// When `AngularDirection::angle_vertical` is `ANGLE_VERTICAL_HORIZONTAL` then
	/// it means the angular direction is horizontal (no Z component).
	///
	/// See the documentation of `AngularDirection::angle_vertical`.
	const ANGLE_VERTICAL_HORIZONTAL: f32 = TAU / 4.0;

	pub(crate) fn from_angles(angle_horizontal: f32, angle_vertical: f32) -> AngularDirection {
		AngularDirection { angle_horizontal, angle_vertical }
	}

	pub(crate) fn from_angle_horizontal(angle_horizontal: f32) -> AngularDirection {
		AngularDirection::from_angles(
			angle_horizontal,
			AngularDirection::ANGLE_VERTICAL_HORIZONTAL,
		)
	}

	pub(crate) fn add_to_horizontal_angle(
		mut self,
		angle_horizontal_to_add: f32,
	) -> AngularDirection {
		self.angle_horizontal += angle_horizontal_to_add;
		self
	}

	pub(crate) fn add_to_vertical_angle(mut self, angle_vertical_to_add: f32) -> AngularDirection {
		self.angle_vertical += angle_vertical_to_add;
		self
	}

	pub(crate) fn to_horizontal(mut self) -> AngularDirection {
		self.angle_vertical = AngularDirection::ANGLE_VERTICAL_HORIZONTAL;
		self
	}

	/// Turn it into a good old vec3, normalized.
	pub(crate) fn to_vec3(self) -> cgmath::Vector3<f32> {
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

impl From<&[f32; 2]> for AngularDirection {
	fn from(angles: &[f32; 2]) -> AngularDirection {
		AngularDirection::from_angles(angles[0], angles[1])
	}
}
impl From<AngularDirection> for [f32; 2] {
	fn from(angular_direction: AngularDirection) -> [f32; 2] {
		[
			angular_direction.angle_horizontal,
			angular_direction.angle_vertical,
		]
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
pub(crate) struct BitCube3Coords {
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
	pub(crate) fn set(&mut self, axis: NonOrientedAxis, value: i32) {
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
pub(crate) struct BitCube3 {
	data: BitArray27,
}
impl BitCube3 {
	pub(crate) fn new_zero() -> BitCube3 {
		BitCube3 { data: BitArray27::new_zero() }
	}
	pub(crate) fn get(self, coords: BitCube3Coords) -> bool {
		self.data.get(coords.index())
	}
	pub(crate) fn set(&mut self, coords: BitCube3Coords, value: bool) {
		self.data.set(coords.index(), value);
	}
}

/// Just a 3D rectangular axis-aligned box.
/// It cannot rotate as it stays aligned on the axes.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct AlignedBox {
	/// Position of the center of the box.
	pub(crate) pos: cgmath::Point3<f32>,
	/// Width of the box along each axis.
	pub(crate) dims: cgmath::Vector3<f32>,
}

impl AlignedBox {
	pub(crate) fn overlapping_block_coords_span(&self) -> CubicCoordsSpan {
		let inf = (self.pos - self.dims / 2.0).map(|x| x.round() as i32);
		let sup_included = (self.pos + self.dims / 2.0).map(|x| x.round() as i32);
		CubicCoordsSpan::with_inf_sup_but_sup_is_included(inf, sup_included)
	}

	pub(crate) fn overlaps(&self, other: &AlignedBox) -> bool {
		let self_inf = self.pos - self.dims / 2.0;
		let self_sup = self.pos + self.dims / 2.0;
		let other_inf = other.pos - other.dims / 2.0;
		let other_sup = other.pos + other.dims / 2.0;
		let no_overlap_x = other_sup.x <= self_inf.x || self_sup.x <= other_inf.x;
		let no_overlap_y = other_sup.y <= self_inf.y || self_sup.y <= other_inf.y;
		let no_overlap_z = other_sup.z <= self_inf.z || self_sup.z <= other_inf.z;
		let no_overlap = no_overlap_x || no_overlap_y || no_overlap_z;
		!no_overlap
	}
}
