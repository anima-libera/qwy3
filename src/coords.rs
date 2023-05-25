//! Coordinate systems for Qwy3.
//!
//! Note that the up direction is Z+, unlike Minecraft's Y+ up direction.
//!
//! The chunks are cubic and organized on a 3D grid, unlike Minecraft's 2D grid of chunks.
//! This means that the world is as infinite along the Z axis (in both directions) as it is
//! horizontally.

/// Coordinates of a block in the world.
#[derive(Clone, Copy)]
pub struct BlockCoords {
	pub x: i32,
	pub y: i32,
	pub z: i32,
}

/// Chunks are cubic parts of the world, all of the same size and arranged in a 3D grid.
/// The length (in blocks) of the edges of the chunks is not hardcoded. It can be
/// modified (to some extent) and passed around in a `ChunkDimensions`.
#[derive(Clone, Copy)]
pub struct ChunkDimensions {
	/// Length (in blocks) of the edge of each (cubic) chunk.
	pub edge: u32,
}

impl From<u32> for ChunkDimensions {
	fn from(chunk_side_length: u32) -> ChunkDimensions {
		ChunkDimensions { edge: chunk_side_length }
	}
}

impl ChunkDimensions {
	pub fn number_of_blocks(self) -> usize {
		self.edge.pow(3) as usize
	}
}

/// Coordinates of a block inside a chunk
/// (so relative to the negativeward corner of a chunk).
#[derive(Clone, Copy)]
pub struct ChunkInternalBlockCoords {
	pub x: u32,
	pub y: u32,
	pub z: u32,
}

impl ChunkInternalBlockCoords {
	pub fn coord(self, axis: NonOrientedAxis) -> u32 {
		match axis {
			NonOrientedAxis::X => self.x,
			NonOrientedAxis::Y => self.y,
			NonOrientedAxis::Z => self.z,
		}
	}

	pub fn coord_mut(&mut self, axis: NonOrientedAxis) -> &mut u32 {
		match axis {
			NonOrientedAxis::X => &mut self.x,
			NonOrientedAxis::Y => &mut self.y,
			NonOrientedAxis::Z => &mut self.z,
		}
	}

	pub fn internal_neighbor(
		mut self,
		cd: ChunkDimensions,
		direction: OrientedAxis,
	) -> Option<ChunkInternalBlockCoords> {
		let new_coord_value_opt = self
			.coord(direction.axis)
			.checked_add_signed(direction.orientation.sign());
		match new_coord_value_opt {
			None => None,
			Some(new_coord_value) if cd.edge <= new_coord_value => None,
			Some(new_coord_value) => {
				*self.coord_mut(direction.axis) = new_coord_value;
				Some(self)
			},
		}
	}
}

impl ChunkDimensions {
	pub fn iter_internal_block_coords(self) -> impl Iterator<Item = ChunkInternalBlockCoords> {
		iter_3d_cube_inf_edge((0, 0, 0), self.edge).map(|(x, y, z)| ChunkInternalBlockCoords {
			x: x as u32,
			y: y as u32,
			z: z as u32,
		})
	}

	pub fn internal_index(self, internal_coords: ChunkInternalBlockCoords) -> usize {
		let ChunkInternalBlockCoords { x, y, z } = internal_coords;
		(z * self.edge.pow(2) + y * self.edge + x) as usize
	}
}

/// Iterates over the 3D rectangle area `inf..sup` (`sup` not included).
pub fn iter_3d_rect_inf_sup(
	inf: (i32, i32, i32),
	sup: (i32, i32, i32),
) -> impl Iterator<Item = (i32, i32, i32)> {
	let (inf_x, inf_y, inf_z) = inf;
	let (sup_x, sup_y, sup_z) = sup;
	debug_assert!(inf_x <= sup_x);
	debug_assert!(inf_y <= sup_y);
	debug_assert!(inf_z <= sup_z);
	(inf_z..sup_z)
		.flat_map(move |z| (inf_y..sup_y).flat_map(move |y| (inf_x..sup_x).map(move |x| (x, y, z))))
}

/// Iterates over the 3D rectangle area `inf..(inf+dims)` (`inf+dims` not included).
pub fn iter_3d_rect_inf_dims(
	inf: (i32, i32, i32),
	dims: (u32, u32, u32),
) -> impl Iterator<Item = (i32, i32, i32)> {
	let sup = (
		inf.0 + dims.0 as i32,
		inf.1 + dims.1 as i32,
		inf.2 + dims.2 as i32,
	);
	iter_3d_rect_inf_sup(inf, sup)
}

/// Iterates over a 3D cubic area of negativewards corner at `inf` and edges of length `edge`.
pub fn iter_3d_cube_inf_edge(
	inf: (i32, i32, i32),
	edge: u32,
) -> impl Iterator<Item = (i32, i32, i32)> {
	iter_3d_rect_inf_dims(inf, (edge, edge, edge))
}

/// Coordinates of a chunk in the 3D grid of chunks
/// (which is not on the same scale as block coords, here we designate whole chunks).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkCoords {
	pub x: i32,
	pub y: i32,
	pub z: i32,
}

impl ChunkDimensions {
	pub fn chunk_internal_coords_to_world_coords(
		self,
		chunk_coords: ChunkCoords,
		internal_coords: ChunkInternalBlockCoords,
	) -> BlockCoords {
		BlockCoords {
			x: (internal_coords.x as i32) + chunk_coords.x * (self.edge as i32),
			y: (internal_coords.y as i32) + chunk_coords.y * (self.edge as i32),
			z: (internal_coords.z as i32) + chunk_coords.z * (self.edge as i32),
		}
	}

	pub fn world_coords_to_containing_chunk_coords(self, coords: BlockCoords) -> ChunkCoords {
		ChunkCoords {
			x: coords.x.div_euclid(self.edge as i32),
			y: coords.y.div_euclid(self.edge as i32),
			z: coords.z.div_euclid(self.edge as i32),
		}
	}

	pub fn world_coords_to_chunk_internal_coords(
		self,
		coords: BlockCoords,
	) -> (ChunkCoords, ChunkInternalBlockCoords) {
		let chunk_coords = self.world_coords_to_containing_chunk_coords(coords);
		let internal_coords = ChunkInternalBlockCoords {
			x: coords.x.rem_euclid(self.edge as i32) as u32,
			y: coords.y.rem_euclid(self.edge as i32) as u32,
			z: coords.z.rem_euclid(self.edge as i32) as u32,
		};
		(chunk_coords, internal_coords)
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
/// Note that upwards is Z+, downwards is Z-.
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
}