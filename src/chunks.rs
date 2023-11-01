use std::{collections::HashMap, f32::consts::TAU, sync::Arc};

use cgmath::{EuclideanSpace, InnerSpace, MetricSpace};
use wgpu::util::DeviceExt;

pub(crate) use crate::{
	coords::{
		iter_3d_cube_center_radius, AxisOrientation, BitCube3, BitCube3Coords, BlockCoords,
		ChunkCoords, ChunkCoordsSpan, ChunkDimensions, NonOrientedAxis, OrientedAxis,
	},
	noise,
	shaders::block::BlockVertexPod,
};

pub enum BlockType {
	Air,
	Solid { texture_coords_on_atlas: cgmath::Point2<i32> },
	XShaped { texture_coords_on_atlas: cgmath::Point2<i32> },
}

impl BlockType {
	pub fn is_opaque(&self) -> bool {
		matches!(self, BlockType::Solid { .. })
	}

	pub fn is_air(&self) -> bool {
		matches!(self, BlockType::Air)
	}
}

pub struct BlockTypeTable {
	block_types: Vec<BlockType>,
}

impl BlockTypeTable {
	pub fn new() -> BlockTypeTable {
		BlockTypeTable {
			block_types: vec![
				BlockType::Air,
				BlockType::Solid { texture_coords_on_atlas: (0, 0).into() },
				BlockType::Solid { texture_coords_on_atlas: (16, 0).into() },
				BlockType::XShaped { texture_coords_on_atlas: (32, 0).into() },
			],
		}
	}

	pub fn get(&self, id: BlockTypeId) -> Option<&BlockType> {
		if id.value < 0 {
			None
		} else {
			self.block_types.get(id.value as usize)
		}
	}

	pub fn air_id(&self) -> BlockTypeId {
		BlockTypeId::new(0)
	}

	pub fn ground_id(&self) -> BlockTypeId {
		BlockTypeId::new(1)
	}

	fn kinda_grass_id(&self) -> BlockTypeId {
		BlockTypeId::new(2)
	}

	fn kinda_grass_blades_id(&self) -> BlockTypeId {
		BlockTypeId::new(3)
	}
}

#[derive(Clone, Copy)]
pub struct BlockTypeId {
	/// Positive values are indices in the table of block types.
	/// Negative values will be used as ids in a table of blocks that have data, maybe?
	value: i16,
}

impl BlockTypeId {
	fn new(value: i16) -> BlockTypeId {
		BlockTypeId { value }
	}
}

/// The blocks of a chunk.
#[derive(Clone)]
pub struct ChunkBlocks {
	coords_span: ChunkCoordsSpan,
	blocks: Vec<BlockTypeId>,
}

impl ChunkBlocks {
	fn new(coords_span: ChunkCoordsSpan) -> ChunkBlocks {
		ChunkBlocks {
			coords_span,
			blocks: Vec::from_iter(
				std::iter::repeat(BlockTypeId { value: 0 }).take(coords_span.cd.number_of_blocks()),
			),
		}
	}

	fn get(&self, coords: BlockCoords) -> Option<BlockTypeId> {
		Some(self.blocks[self.coords_span.internal_index(coords)?])
	}

	fn get_mut(&mut self, coords: BlockCoords) -> Option<&mut BlockTypeId> {
		Some(&mut self.blocks[self.coords_span.internal_index(coords)?])
	}
}

/// Information about the opaqueness of each block
/// contained in a 1-block-thick cubic layer around a chunk.
pub struct OpaquenessLayerAroundChunk {
	/// The coords span of the chunk that is surrounded by the layer that this struct describes.
	/// This is NOT the coords span of the layer. The layer is 1-block thick and encloses that
	/// coords span.
	surrounded_chunk_coords_span: ChunkCoordsSpan,
	data: bitvec::vec::BitVec,
}

impl OpaquenessLayerAroundChunk {
	fn new(surrounded_chunk_coords_span: ChunkCoordsSpan) -> OpaquenessLayerAroundChunk {
		let data = bitvec::vec::BitVec::repeat(
			false,
			OpaquenessLayerAroundChunk::data_size(surrounded_chunk_coords_span.cd),
		);
		OpaquenessLayerAroundChunk { surrounded_chunk_coords_span, data }
	}

	fn data_size(cd: ChunkDimensions) -> usize {
		let face_size = cd.edge.pow(2);
		let edge_size = cd.edge;
		let corner_size = 1;
		(face_size * 6 + edge_size * 12 + corner_size * 8) as usize
	}

	/// One of the functions of all times, ever!
	fn coords_to_index_in_data_unchecked(&self, coords: BlockCoords) -> Option<usize> {
		// Ok so here the goal is to map the coords that are in the layer to a unique index.
		if self.surrounded_chunk_coords_span.contains(coords) {
			// If we fall in the chunk that the layer encloses, then we are not in the layer
			// but we are just in the hole in the middle of the layer.
			return None;
		}
		// Get `inf` and `sup` to represent the cube that is the layer (if we ignore the hole
		// in the middle that was already taken care of), `sup` is included.
		let inf: BlockCoords =
			self.surrounded_chunk_coords_span.block_coords_inf() - cgmath::vec3(1, 1, 1);
		let sup: BlockCoords = self
			.surrounded_chunk_coords_span
			.block_coords_sup_excluded();
		let contained_in_the_layer = inf.x <= coords.x
			&& coords.x <= sup.x
			&& inf.y <= coords.y
			&& coords.y <= sup.y
			&& inf.z <= coords.z
			&& coords.z <= sup.z;
		if !contained_in_the_layer {
			// We are outside of the layer.
			return None;
		}
		// If we get here, then it meas we are in the layer and a unique index has to be determined.
		// `layer_edge` is the length in blocks of an edge of the layer.
		let layer_edge = self.surrounded_chunk_coords_span.cd.edge + 2;
		let ix = coords.x - inf.x;
		let iy = coords.y - inf.y;
		let iz = coords.z - inf.z;
		if iz == 0 {
			// Bottom face (lowest Z value).
			Some((ix + iy * layer_edge) as usize)
		} else if iz == layer_edge - 1 {
			// Top face (higest Z value).
			Some((layer_edge.pow(2) + (ix + iy * layer_edge)) as usize)
		} else {
			// One of the side faces that are not in the top/bottom faces.
			// We consider horizontal slices of the layer which are just squares here,
			// `sub_index` is just a unique index in the square we are in, and
			// we have to add enough `square_size` to distinguish between the different squares
			// (for different Z values).
			// `square_size` is the number of blocks in the line of the square (no middle).
			let square_size = (layer_edge - 1) * 4;
			let sub_index = if ix == 0 {
				iy
			} else if ix == layer_edge - 1 {
				layer_edge + iy
			} else if iy == 0 {
				layer_edge * 2 + (ix - 1)
			} else if iy == layer_edge - 1 {
				layer_edge * 2 + (layer_edge - 2) + (ix - 1)
			} else {
				unreachable!()
			};
			Some((layer_edge.pow(2) * 2 + (iz - 1) * square_size + sub_index) as usize)
			// >w<
		}
	}

	fn coords_to_index_in_data(&self, coords: BlockCoords) -> Option<usize> {
		let index_opt = self.coords_to_index_in_data_unchecked(coords);
		if let Some(index) = index_opt {
			assert!(index < self.data.len());
		}
		index_opt
	}

	fn set(&mut self, coords: BlockCoords, value: bool) {
		let index = self.coords_to_index_in_data(coords).unwrap();
		self.data.set(index, value);
	}

	fn get(&mut self, coords: BlockCoords) -> Option<bool> {
		let index = self.coords_to_index_in_data(coords)?;
		Some(*self.data.get(index).unwrap())
	}
}

#[cfg(test)]
mod test {
	use crate::coords::iter_3d_rect_inf_sup_excluded;

	use super::*;
	#[test]
	fn indexing_of_the_funky_layer_data_structure() {
		let cd = ChunkDimensions::from(18);
		let chunk_coords_span = ChunkCoordsSpan { cd, chunk_coords: (2, -3, 0).into() };
		let layer = OpaquenessLayerAroundChunk::new(chunk_coords_span);

		let layer_size = OpaquenessLayerAroundChunk::data_size(cd);
		let mut indices = vec![];

		// We iterate over all the coords in the layer and in the layer hole.
		let inf: BlockCoords = chunk_coords_span.block_coords_inf() - cgmath::vec3(1, 1, 1);
		let sup_excluded: BlockCoords =
			chunk_coords_span.block_coords_sup_excluded() + cgmath::vec3(1, 1, 1);
		for coords in iter_3d_rect_inf_sup_excluded(inf, sup_excluded) {
			let index_opt = layer.coords_to_index_in_data(coords);
			if let Some(index) = index_opt {
				// The layer gave an index to the coords, which means we are supposed
				// to be on the layer.
				// Indices must be unique so we check for that.
				assert!(!indices.contains(&index));
				indices.push(index);
			} else {
				// The layer didn't give an index so it means we are not on the layer.
				// We don't test for coords outside of the layer and its hole so we
				// must be in the hole at the center of the layer.
				assert!(chunk_coords_span.contains(coords));
			}
		}

		// The indices must be unique, and we already checked for that.
		// The indices also must cover all possible indices from 0 to the max expected index,
		// we check for that here.
		for expected_index in 0..layer_size {
			assert!(indices.contains(&expected_index));
		}
	}
}

impl ChunkBlocks {
	pub(crate) fn generate_mesh_given_surrounding_opaqueness(
		&self,
		mut opaqueness_layer: OpaquenessLayerAroundChunk,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkMesh {
		let mut is_opaque = |coords: BlockCoords| {
			if let Some(block_id) = self.get(coords) {
				block_type_table.get(block_id).unwrap().is_opaque()
			} else if let Some(opaque) = opaqueness_layer.get(coords) {
				opaque
			} else {
				unreachable!()
			}
		};

		let mut block_vertices = Vec::new();
		for coords in self.coords_span.iter_coords() {
			let block_id = self.get(coords).unwrap();
			match block_type_table.get(block_id).unwrap() {
				BlockType::Air => {},
				BlockType::Solid { texture_coords_on_atlas } => {
					let opacity_bit_cube_3 = {
						let mut cube = BitCube3::new_zero();
						for delta in iter_3d_cube_center_radius((0, 0, 0).into(), 2) {
							let neighbor_coords = coords + delta.to_vec();
							cube.set(delta.into(), is_opaque(neighbor_coords));
						}
						cube
					};
					for direction in OrientedAxis::all_the_six_possible_directions() {
						let is_covered_by_neighbor = {
							let neighbor_coords = coords + direction.delta();
							is_opaque(neighbor_coords)
						};
						if !is_covered_by_neighbor {
							generate_block_face_mesh(
								&mut block_vertices,
								direction,
								coords.map(|x| x as f32),
								opacity_bit_cube_3,
								*texture_coords_on_atlas,
							);
						}
					}
				},
				BlockType::XShaped { texture_coords_on_atlas } => {
					let opacity_bit_cube_3 = {
						let mut cube = BitCube3::new_zero();
						for delta in iter_3d_cube_center_radius((0, 0, 0).into(), 2) {
							let neighbor_coords = coords + delta.to_vec();
							cube.set(delta.into(), is_opaque(neighbor_coords));
						}
						cube
					};
					for vertices_offets_xy in [
						[[false, false], [true, true]],
						[[true, true], [false, false]],
						[[true, false], [false, true]],
						[[false, true], [true, false]],
					] {
						generate_xshaped_block_face_mesh(
							&mut block_vertices,
							coords.map(|x| x as f32),
							opacity_bit_cube_3,
							vertices_offets_xy,
							*texture_coords_on_atlas,
						);
					}
				},
			}
		}
		ChunkMesh::from_vertices(block_vertices)
	}
}

pub struct ChunkMesh {
	pub block_vertices: Vec<BlockVertexPod>,
	pub block_vertex_buffer: Option<wgpu::Buffer>,
	// When `block_vertices` is modified, `block_vertex_buffer` becomes out of sync
	// and must be updated. This is what this field keeps track of.
	cpu_to_gpu_update_required: bool,
}

impl ChunkMesh {
	fn from_vertices(block_vertices: Vec<BlockVertexPod>) -> ChunkMesh {
		let cpu_to_gpu_update_required = !block_vertices.is_empty();
		ChunkMesh {
			block_vertices,
			block_vertex_buffer: None,
			cpu_to_gpu_update_required,
		}
	}

	pub fn update_gpu_data(&mut self, device: &wgpu::Device) {
		let block_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Block Vertex Buffer"),
			contents: bytemuck::cast_slice(&self.block_vertices),
			usage: wgpu::BufferUsages::VERTEX,
		});
		self.block_vertex_buffer = Some(block_vertex_buffer);
		self.cpu_to_gpu_update_required = false;
	}
}

/// Generate the mesh of a face of a block, adding it to `vertices`.
fn generate_block_face_mesh(
	vertices: &mut Vec<BlockVertexPod>,
	face_orientation: OrientedAxis,
	block_center: cgmath::Point3<f32>,
	neighborhood_opaqueness: BitCube3,
	texture_coords_on_atlas: cgmath::Point2<i32>,
) {
	// NO EARLY OPTIMIZATION
	// This shall remain in an unoptimized, unfactorized and flexible state for now!

	// We are just meshing a single face, thus a square.
	// We start by 4 points at the center of a block.
	let mut coords_array: [cgmath::Point3<f32>; 4] =
		[block_center, block_center, block_center, block_center];
	// We move the 4 points to the center of the face we are meshing.
	for coords in coords_array.iter_mut() {
		coords[face_orientation.axis.index()] += 0.5 * face_orientation.orientation.sign() as f32;
	}

	// In doing so we moved the points along some axis.
	// The two other axes are the ones that describe a plane in which the 4 points will be moved
	// to make a square, so we get these two other axes.
	let mut other_axes = [NonOrientedAxis::X, NonOrientedAxis::Y, NonOrientedAxis::Z]
		.into_iter()
		.filter(|&axis| axis != face_orientation.axis);
	let other_axis_a = other_axes.next().unwrap();
	let other_axis_b = other_axes.next().unwrap();
	assert!(other_axes.next().is_none());

	// Now we move each point from the center of the face square to one of the square vertex.
	coords_array[0][other_axis_a.index()] -= 0.5;
	coords_array[0][other_axis_b.index()] -= 0.5;
	coords_array[1][other_axis_a.index()] -= 0.5;
	coords_array[1][other_axis_b.index()] += 0.5;
	coords_array[2][other_axis_a.index()] += 0.5;
	coords_array[2][other_axis_b.index()] -= 0.5;
	coords_array[3][other_axis_a.index()] += 0.5;
	coords_array[3][other_axis_b.index()] += 0.5;

	let normal = {
		let mut normal = [0.0, 0.0, 0.0];
		normal[face_orientation.axis.index()] = face_orientation.orientation.sign() as f32;
		normal
	};

	// Texture moment ^^.
	let texture_rect_in_atlas_xy: cgmath::Point2<f32> =
		texture_coords_on_atlas.map(|x| x as f32) * (1.0 / 512.0);
	let texture_rect_in_atlas_wh: cgmath::Vector2<f32> = cgmath::vec2(16.0, 16.0) * (1.0 / 512.0);
	let mut coords_in_atlas_array: [cgmath::Point2<f32>; 4] = [
		texture_rect_in_atlas_xy,
		texture_rect_in_atlas_xy,
		texture_rect_in_atlas_xy,
		texture_rect_in_atlas_xy,
	];
	coords_in_atlas_array[0].x += texture_rect_in_atlas_wh.x * 0.0;
	coords_in_atlas_array[0].y += texture_rect_in_atlas_wh.y * 0.0;
	coords_in_atlas_array[1].x += texture_rect_in_atlas_wh.x * 0.0;
	coords_in_atlas_array[1].y += texture_rect_in_atlas_wh.y * 1.0;
	coords_in_atlas_array[2].x += texture_rect_in_atlas_wh.x * 1.0;
	coords_in_atlas_array[2].y += texture_rect_in_atlas_wh.y * 0.0;
	coords_in_atlas_array[3].x += texture_rect_in_atlas_wh.x * 1.0;
	coords_in_atlas_array[3].y += texture_rect_in_atlas_wh.y * 1.0;

	// The ambiant occlusion trick used here was taken from
	// https://0fps.net/2013/07/03/ambient-occlusion-for-minecraft-like-worlds/
	// this cool blog post seem to be famous in the voxel engine scene.
	let ambiant_occlusion_base_value = |side_a: bool, side_b: bool, corner_ab: bool| {
		if side_a && side_b {
			0
		} else {
			3 - (side_a as i32 + side_b as i32 + corner_ab as i32)
		}
	};
	let ambiant_occlusion_uwu = |along_a: i32, along_b: i32| {
		let mut coords: BitCube3Coords = BitCube3Coords::from(cgmath::point3(0, 0, 0));
		coords.set(face_orientation.axis, face_orientation.orientation.sign());
		coords.set(other_axis_a, along_a);
		let side_a = neighborhood_opaqueness.get(coords);

		coords = BitCube3Coords::from(cgmath::point3(0, 0, 0));
		coords.set(face_orientation.axis, face_orientation.orientation.sign());
		coords.set(other_axis_b, along_b);
		let side_b = neighborhood_opaqueness.get(coords);

		coords = BitCube3Coords::from(cgmath::point3(0, 0, 0));
		coords.set(face_orientation.axis, face_orientation.orientation.sign());
		coords.set(other_axis_a, along_a);
		coords.set(other_axis_b, along_b);
		let corner_ab = neighborhood_opaqueness.get(coords);

		ambiant_occlusion_base_value(side_a, side_b, corner_ab) as f32 / 3.0
	};
	let ambiant_occlusion_array = [
		ambiant_occlusion_uwu(-1, -1),
		ambiant_occlusion_uwu(-1, 1),
		ambiant_occlusion_uwu(1, -1),
		ambiant_occlusion_uwu(1, 1),
	];

	// The four vertices currently corming a square are now being given as two triangles.
	// The diagonal of the square (aa-bb or ab-ba) that will be the "cut" in the square
	// to form triangles is selected based on the ambiant occlusion values of the vertices.
	// Depending on the diagonal picked, the ambiant occlusion behaves differently on the face,
	// and we make sure that this behavior is consistent.
	let other_triangle_cut = ambiant_occlusion_array[0] + ambiant_occlusion_array[3]
		<= ambiant_occlusion_array[1] + ambiant_occlusion_array[2];
	let indices = if other_triangle_cut {
		[0, 2, 1, 1, 2, 3]
	} else {
		[1, 0, 3, 3, 0, 2]
	};

	// Face culling will discard triangles whose verices don't end up clipped to the screen in
	// a counter-clockwise order. This means that triangles must be counter-clockwise when
	// we look at their front and clockwise when we look at their back.
	// `reverse_order` makes sure that they have the right orientation.
	let reverse_order = match face_orientation.axis {
		NonOrientedAxis::X => face_orientation.orientation == AxisOrientation::Negativewards,
		NonOrientedAxis::Y => face_orientation.orientation == AxisOrientation::Positivewards,
		NonOrientedAxis::Z => face_orientation.orientation == AxisOrientation::Negativewards,
	};
	let indices_indices_normal = [0, 1, 2, 3, 4, 5];
	let indices_indices_reversed = [0, 2, 1, 3, 5, 4];
	let mut handle_index = |index: usize| {
		vertices.push(BlockVertexPod {
			position: coords_array[index].into(),
			coords_in_atlas: coords_in_atlas_array[index].into(),
			normal,
			ambiant_occlusion: ambiant_occlusion_array[index],
		});
	};
	if !reverse_order {
		for indices_index in indices_indices_normal {
			handle_index(indices[indices_index]);
		}
	} else {
		for indices_index in indices_indices_reversed {
			handle_index(indices[indices_index]);
		}
	}
}

/// Generate one of the two faces in the mesh of an X-shaped block, adding it to `vertices`.
fn generate_xshaped_block_face_mesh(
	vertices: &mut Vec<BlockVertexPod>,
	block_center: cgmath::Point3<f32>,
	neighborhood_opaqueness: BitCube3,
	vertices_offets_xy: [[bool; 2]; 2],
	texture_coords_on_atlas: cgmath::Point2<i32>,
) {
	// NO EARLY OPTIMIZATION
	// This shall remain in an unoptimized, unfactorized and flexible state for now!

	// We are just meshing a single face, thus a rectangle.
	// We start by 4 points at the center of a block.
	let mut coords_array: [cgmath::Point3<f32>; 4] =
		[block_center, block_center, block_center, block_center];

	let offset_a: cgmath::Vector2<f32> = (
		if vertices_offets_xy[0][0] { 0.5 } else { -0.5 },
		if vertices_offets_xy[0][1] { 0.5 } else { -0.5 },
	)
		.into();
	let offset_b: cgmath::Vector2<f32> = (
		if vertices_offets_xy[1][0] { 0.5 } else { -0.5 },
		if vertices_offets_xy[1][1] { 0.5 } else { -0.5 },
	)
		.into();

	coords_array[0] += offset_a.extend(-0.5);
	coords_array[1] += offset_b.extend(-0.5);
	coords_array[2] += offset_a.extend(0.5);
	coords_array[3] += offset_b.extend(0.5);

	let normal = (offset_b - offset_a)
		.extend(0.0)
		.cross(cgmath::vec3(0.0, 0.0, -1.0))
		.normalize();

	// Texture moment ^^.
	let texture_rect_in_atlas_xy: cgmath::Point2<f32> =
		texture_coords_on_atlas.map(|x| x as f32) * (1.0 / 512.0);
	let texture_rect_in_atlas_wh: cgmath::Vector2<f32> = cgmath::vec2(16.0, 16.0) * (1.0 / 512.0);
	let mut coords_in_atlas_array: [cgmath::Point2<f32>; 4] = [
		texture_rect_in_atlas_xy,
		texture_rect_in_atlas_xy,
		texture_rect_in_atlas_xy,
		texture_rect_in_atlas_xy,
	];
	coords_in_atlas_array[0].x += texture_rect_in_atlas_wh.x * 0.0;
	coords_in_atlas_array[0].y += texture_rect_in_atlas_wh.y * 0.0;
	coords_in_atlas_array[1].x += texture_rect_in_atlas_wh.x * 1.0;
	coords_in_atlas_array[1].y += texture_rect_in_atlas_wh.y * 0.0;
	coords_in_atlas_array[2].x += texture_rect_in_atlas_wh.x * 0.0;
	coords_in_atlas_array[2].y += texture_rect_in_atlas_wh.y * 1.0;
	coords_in_atlas_array[3].x += texture_rect_in_atlas_wh.x * 1.0;
	coords_in_atlas_array[3].y += texture_rect_in_atlas_wh.y * 1.0;

	let ambiant_occlusion_base_value = |side_a: bool, side_b: bool, corner_ab: bool| {
		if side_a && side_b {
			0
		} else {
			3 - (side_a as i32 + side_b as i32 + corner_ab as i32)
		}
	};
	let ambiant_occlusion_uwu = |along_x: i32, along_y: i32| {
		let mut coords: BitCube3Coords = BitCube3Coords::from(cgmath::point3(0, 0, 0));
		coords.set(NonOrientedAxis::Z, 0);
		coords.set(NonOrientedAxis::X, along_x);
		let side_a = neighborhood_opaqueness.get(coords);

		coords = BitCube3Coords::from(cgmath::point3(0, 0, 0));
		coords.set(NonOrientedAxis::Z, 0);
		coords.set(NonOrientedAxis::Y, along_y);
		let side_b = neighborhood_opaqueness.get(coords);

		coords = BitCube3Coords::from(cgmath::point3(0, 0, 0));
		coords.set(NonOrientedAxis::Z, 0);
		coords.set(NonOrientedAxis::X, along_x);
		coords.set(NonOrientedAxis::Y, along_y);
		let corner_ab = neighborhood_opaqueness.get(coords);

		ambiant_occlusion_base_value(side_a, side_b, corner_ab) as f32 / 3.0
	};
	let ambiant_occlusion_array = [
		ambiant_occlusion_uwu(
			if offset_a.x < 0.0 { -1 } else { 1 },
			if offset_a.y < 0.0 { -1 } else { 1 },
		),
		ambiant_occlusion_uwu(
			if offset_b.x < 0.0 { -1 } else { 1 },
			if offset_b.y < 0.0 { -1 } else { 1 },
		),
		ambiant_occlusion_uwu(
			if offset_a.x < 0.0 { -1 } else { 1 },
			if offset_a.y < 0.0 { -1 } else { 1 },
		),
		ambiant_occlusion_uwu(
			if offset_b.x < 0.0 { -1 } else { 1 },
			if offset_b.y < 0.0 { -1 } else { 1 },
		),
	];

	let indices = [1, 0, 3, 3, 0, 2];

	// Face culling will discard triangles whose verices don't end up clipped to the screen in
	// a counter-clockwise order. This means that triangles must be counter-clockwise when
	// we look at their front and clockwise when we look at their back.
	// `reverse_order` makes sure that they have the right orientation.
	let reverse_order = false;
	let indices_indices_normal = [0, 1, 2, 3, 4, 5];
	let indices_indices_reversed = [0, 2, 1, 3, 5, 4];
	let mut handle_index = |index: usize| {
		vertices.push(BlockVertexPod {
			position: (coords_array[index] + normal * 0.025).into(),
			coords_in_atlas: coords_in_atlas_array[index].into(),
			normal: normal.into(),
			ambiant_occlusion: ambiant_occlusion_array[index],
		});
	};
	if !reverse_order {
		for indices_index in indices_indices_normal {
			handle_index(indices[indices_index]);
		}
	} else {
		for indices_index in indices_indices_reversed {
			handle_index(indices[indices_index]);
		}
	}
}

pub struct Chunk {
	_coords_span: ChunkCoordsSpan,
	pub blocks: Option<ChunkBlocks>,
	pub remeshing_required: bool,
	pub mesh: Option<ChunkMesh>,
}

impl Chunk {
	pub fn new_empty(coords_span: ChunkCoordsSpan) -> Chunk {
		Chunk {
			_coords_span: coords_span,
			blocks: None,
			remeshing_required: false,
			mesh: None,
		}
	}
}

pub struct ChunkGrid {
	cd: ChunkDimensions,
	pub map: HashMap<ChunkCoords, Chunk>,
}

impl ChunkGrid {
	pub fn new(cd: ChunkDimensions) -> ChunkGrid {
		ChunkGrid { cd, map: HashMap::new() }
	}

	fn set_block_but_do_not_update_meshes(&mut self, coords: BlockCoords, block: BlockTypeId) {
		let chunk_coords = self.cd.world_coords_to_containing_chunk_coords(coords);
		match self.map.get_mut(&chunk_coords) {
			Some(chunk) => {
				let block_dst = chunk.blocks.as_mut().unwrap().get_mut(coords).unwrap();
				*block_dst = block;
			},
			None => {
				// TODO: Handle this case by storing the fact that a block
				// has to be set when loding the chunk.
				unimplemented!()
			},
		}
	}

	pub(crate) fn set_block_and_request_updates_to_meshes(
		&mut self,
		coords: BlockCoords,
		block: BlockTypeId,
	) {
		self.set_block_but_do_not_update_meshes(coords, block);

		let mut chunk_coords_to_update = vec![];
		for delta in iter_3d_cube_center_radius((0, 0, 0).into(), 2) {
			let neighbor_coords = coords + delta.to_vec();
			let chunk_coords = self
				.cd
				.world_coords_to_containing_chunk_coords(neighbor_coords);
			chunk_coords_to_update.push(chunk_coords);
		}

		for chunk_coords in chunk_coords_to_update {
			if let Some(chunk) = self.map.get_mut(&chunk_coords) {
				chunk.remeshing_required = true;
			}
		}
	}

	pub(crate) fn get_block(&self, coords: BlockCoords) -> Option<BlockTypeId> {
		let chunk_coords = self.cd.world_coords_to_containing_chunk_coords(coords);
		let chunk = self.map.get(&chunk_coords)?;
		Some(chunk.blocks.as_ref().unwrap().get(coords).unwrap())
	}

	pub(crate) fn get_opaqueness_layer_around_chunk(
		&self,
		chunk_coords: ChunkCoords,
		default_to_opaque: bool,
		block_type_table: Arc<BlockTypeTable>,
	) -> OpaquenessLayerAroundChunk {
		let surrounded_chunk_coords_span = ChunkCoordsSpan { cd: self.cd, chunk_coords };
		let mut layer = OpaquenessLayerAroundChunk::new(surrounded_chunk_coords_span);

		let inf = surrounded_chunk_coords_span.block_coords_inf() - cgmath::vec3(1, 1, 1);
		let sup_excluded =
			surrounded_chunk_coords_span.block_coords_sup_excluded() + cgmath::vec3(1, 1, 1);
		for z in inf.z..sup_excluded.z {
			for y in inf.y..sup_excluded.y {
				let mut x = inf.x;
				while x < sup_excluded.x {
					let coords: BlockCoords = (x, y, z).into();
					if surrounded_chunk_coords_span.contains(coords) {
						// We skip over the chunk hole in the middle of the layer.
						x = sup_excluded.x - 1;
					} else {
						{
							let opaque = self
								.get_block(coords)
								.map(|block_type_id| {
									block_type_table.get(block_type_id).unwrap().is_opaque()
								})
								.unwrap_or(default_to_opaque);
							layer.set(coords, opaque);
						}
						x += 1;
					}
				}
			}
		}

		layer
	}
}

pub trait WorldGenerator {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks;
}

pub struct DefaultWorldGenerator {
	pub seed: i32,
}

impl WorldGenerator for DefaultWorldGenerator {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(5, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(5, vec![self.seed, 2]);
		let noise_no_grass = noise::OctavedNoise::new(5, vec![self.seed, 3]);
		let noise_grass_a = noise::OctavedNoise::new(2, vec![self.seed, 1, 1]);
		let noise_grass_b = noise::OctavedNoise::new(2, vec![self.seed, 1, 2]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 100.0;
			let a = noise_a.sample_3d(coordsf / scale, &[]);
			let b = noise_b.sample_3d(coordsf / scale, &[]);
			(coordsf.z < b * 5.0 && a < 0.7) || b < 0.3
		};
		let coords_to_grass = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let d = noise_grass_a.sample_3d(coordsf / scale, &[]);
			let density = if d < 0.1 {
				d * 0.9 + 0.1
			} else if d < 0.3 {
				0.1
			} else {
				0.01
			};
			noise_grass_b.sample_3d(coordsf, &[]) < density
		};
		let coords_to_no_grass = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 75.0;
			noise_no_grass.sample_3d(coordsf / scale, &[]) < 0.25
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			// Test chunk generation.
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					let no_grass = coords_to_no_grass(coords);
					if no_grass {
						block_type_table.ground_id()
					} else {
						block_type_table.kinda_grass_id()
					}
				}
			} else {
				let ground_below = coords_to_ground(coords + cgmath::vec3(0, 0, -1));
				if ground_below {
					let no_grass_below = coords_to_no_grass(coords + cgmath::vec3(0, 0, -1));
					if no_grass_below {
						block_type_table.air_id()
					} else if coords_to_grass(coords) {
						block_type_table.kinda_grass_blades_id()
					} else {
						block_type_table.air_id()
					}
				} else {
					block_type_table.air_id()
				}
			};
		}
		chunk_blocks
	}
}

pub struct WorldGeneratorTest001 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest001 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(5, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(5, vec![self.seed, 2]);
		let noise_no_grass = noise::OctavedNoise::new(5, vec![self.seed, 3]);
		let noise_grass_a = noise::OctavedNoise::new(2, vec![self.seed, 1, 1]);
		let noise_grass_b = noise::OctavedNoise::new(2, vec![self.seed, 1, 2]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 100.0;
			let a = noise_a.sample_3d(coordsf / scale, &[]);
			let b = noise_b.sample_3d(coordsf / scale, &[]);
			(a - 0.5).abs() < 0.03 && (b - 0.5).abs() < 0.03
		};
		let coords_to_grass = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let d = noise_grass_a.sample_3d(coordsf / scale, &[]);
			let density = if d < 0.1 {
				d * 0.9 + 0.1
			} else if d < 0.3 {
				0.1
			} else {
				0.01
			};
			noise_grass_b.sample_3d(coordsf, &[]) < density
		};
		let coords_to_no_grass = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 75.0;
			noise_no_grass.sample_3d(coordsf / scale, &[]) < 0.25
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			// Test chunk generation.
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					let no_grass = coords_to_no_grass(coords);
					if no_grass {
						block_type_table.ground_id()
					} else {
						block_type_table.kinda_grass_id()
					}
				}
			} else {
				let ground_below = coords_to_ground(coords + cgmath::vec3(0, 0, -1));
				if ground_below {
					let no_grass_below = coords_to_no_grass(coords + cgmath::vec3(0, 0, -1));
					if no_grass_below {
						block_type_table.air_id()
					} else if coords_to_grass(coords) {
						block_type_table.kinda_grass_blades_id()
					} else {
						block_type_table.air_id()
					}
				} else {
					block_type_table.air_id()
				}
			};
		}
		chunk_blocks
	}
}

pub struct WorldGeneratorTest002 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest002 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(5, vec![self.seed, 1]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 100.0;
			let a = noise_a.sample_3d(coordsf / scale, &[]);
			a < 0.35
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			// Test chunk generation.
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

pub struct WorldGeneratorTest003 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest003 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let scale = 45.0;
			let radius = 11.0;
			let coordsf = coords.map(|x| x as f32);
			let coordsf_i_scaled = coords.map(|x| (x as f32 / scale).floor());
			let a = noise_a.sample_3d(coordsf_i_scaled, &[]);
			let b = noise_b.sample_3d(coordsf_i_scaled, &[]);
			let c = noise_c.sample_3d(coordsf_i_scaled, &[]);
			let coordsf_min = coords.map(|x| (x as f32 / scale).floor() * scale);
			let _coordsf_max = coords.map(|x| (x as f32 / scale).ceil() * scale);
			let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
			(coordsf - coordsf_min).distance(the) < radius
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			// Test chunk generation.
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

pub struct WorldGeneratorTest004 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest004 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let noise_d = noise::OctavedNoise::new(1, vec![self.seed, 4]);
		let noise_e = noise::OctavedNoise::new(1, vec![self.seed, 5]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let scale = 45.0;
			let min_radius = 4.0;
			let max_radius = 15.0;
			let coordsf = coords.map(|x| x as f32);
			let coordsf_i_scaled = coords.map(|x| (x as f32 / scale).floor());
			let e = noise_e.sample_3d(coordsf_i_scaled, &[]);
			if e < 0.2 {
				return false;
			}
			let a = noise_a.sample_3d(coordsf_i_scaled, &[]);
			let b = noise_b.sample_3d(coordsf_i_scaled, &[]);
			let c = noise_c.sample_3d(coordsf_i_scaled, &[]);
			let d = noise_d.sample_3d(coordsf_i_scaled, &[]);
			let radius = d * (max_radius - min_radius) + min_radius;
			let coordsf_min = coords.map(|x| (x as f32 / scale).floor() * scale);
			let _coordsf_max = coords.map(|x| (x as f32 / scale).ceil() * scale);
			let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
			(coordsf - coordsf_min).distance(the) < radius
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			// Test chunk generation.
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

// For future stuff:
// https://iquilezles.org/articles/distfunctions/

/// Distance from the given `point` to the segment between the two point `a` and `b`.
fn distance_to_segment(
	a: cgmath::Point3<f32>,
	b: cgmath::Point3<f32>,
	point: cgmath::Point3<f32>,
) -> f32 {
	let pa = point - a;
	let ba = b - a;
	let h = f32::clamp(pa.dot(ba) / ba.dot(ba), 0.0, 1.0);
	(pa - ba * h).magnitude()
}

pub struct WorldGeneratorTest005 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest005 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let scale = 45.0;
			let radius = 10.0;
			let coordsf = coords.map(|x| x as f32);
			let coordsf_to_the = |coordsf: cgmath::Point3<f32>| -> cgmath::Point3<f32> {
				let coordsf_i_scaled = coordsf.map(|x| (x / scale).floor());
				let a = noise_a.sample_3d(coordsf_i_scaled, &[]);
				let b = noise_b.sample_3d(coordsf_i_scaled, &[]);
				let c = noise_c.sample_3d(coordsf_i_scaled, &[]);
				let coordsf_min = coordsf.map(|x| (x / scale).floor() * scale);
				let _coordsf_max = coordsf.map(|x| (x / scale).ceil() * scale);
				let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
				coordsf_min + the
			};
			let the = coordsf_to_the(coordsf);
			let xp = coordsf_to_the(coordsf + cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let xm = coordsf_to_the(coordsf - cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let vp = distance_to_segment(the, xp, coordsf);
			let vm = distance_to_segment(the, xm, coordsf);
			vp < radius || vm < radius
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			// Test chunk generation.
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

pub struct WorldGeneratorTest006 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest006 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let noise_d = noise::OctavedNoise::new(4, vec![self.seed, 4]);
		let noise_e = noise::OctavedNoise::new(4, vec![self.seed, 5]);
		let noise_f = noise::OctavedNoise::new(4, vec![self.seed, 6]);
		let coords_to_ground_uwu = |coordsf: cgmath::Point3<f32>| -> bool {
			let scale = 85.0;
			let radius = 10.0;
			let coordsf_to_the = |coordsf: cgmath::Point3<f32>| -> cgmath::Point3<f32> {
				let coordsf_i_scaled = coordsf.map(|x| (x / scale).floor());
				let a = noise_a.sample_3d(coordsf_i_scaled, &[]);
				let b = noise_b.sample_3d(coordsf_i_scaled, &[]);
				let c = noise_c.sample_3d(coordsf_i_scaled, &[]);
				let coordsf_min = coordsf.map(|x| (x / scale).floor() * scale);
				let _coordsf_max = coordsf.map(|x| (x / scale).ceil() * scale);
				let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
				coordsf_min + the
			};
			let the = coordsf_to_the(coordsf);
			let xp = coordsf_to_the(coordsf + cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let xm = coordsf_to_the(coordsf - cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let vp = distance_to_segment(the, xp, coordsf);
			let vm = distance_to_segment(the, xm, coordsf);
			vp < radius || vm < radius
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let deformation_max_length = 20.0;
			let d = noise_d.sample_3d(coordsf / scale, &[]);
			let e = noise_e.sample_3d(coordsf / scale, &[]);
			let f = noise_f.sample_3d(coordsf / scale, &[]);
			use crate::coords::AngularDirection;
			let deformation = AngularDirection::from_angles(d * TAU, e * (TAU / 2.0)).to_vec3()
				* f * deformation_max_length;
			let deformed_coordsf = coordsf + deformation;
			coords_to_ground_uwu(deformed_coordsf)
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			// Test chunk generation.
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

pub struct WorldGeneratorTest007 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest007 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let noise_d = noise::OctavedNoise::new(4, vec![self.seed, 4]);
		let noise_e = noise::OctavedNoise::new(4, vec![self.seed, 5]);
		let noise_f = noise::OctavedNoise::new(4, vec![self.seed, 6]);
		let coords_to_ground_uwu = |coordsf: cgmath::Point3<f32>| -> bool {
			let scale = 65.0;
			let radius = 7.0;
			let coordsf_to_the = |coordsf: cgmath::Point3<f32>| -> cgmath::Point3<f32> {
				let coordsf_i_scaled = coordsf.map(|x| (x / scale).floor());
				let a = noise_a.sample_3d(coordsf_i_scaled, &[]);
				let b = noise_b.sample_3d(coordsf_i_scaled, &[]);
				let c = noise_c.sample_3d(coordsf_i_scaled, &[]);
				let coordsf_min = coordsf.map(|x| (x / scale).floor() * scale);
				let _coordsf_max = coordsf.map(|x| (x / scale).ceil() * scale);
				let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
				coordsf_min + the
			};
			let the = coordsf_to_the(coordsf);
			let xp = coordsf_to_the(coordsf + cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let xm = coordsf_to_the(coordsf - cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let yp = coordsf_to_the(coordsf + cgmath::vec3(0.0, 1.0, 0.0) * scale);
			let ym = coordsf_to_the(coordsf - cgmath::vec3(0.0, 1.0, 0.0) * scale);
			let zp = coordsf_to_the(coordsf + cgmath::vec3(0.0, 0.0, 1.0) * scale);
			let zm = coordsf_to_the(coordsf - cgmath::vec3(0.0, 0.0, 1.0) * scale);
			let vxp = distance_to_segment(the, xp, coordsf);
			let vxm = distance_to_segment(the, xm, coordsf);
			let vyp = distance_to_segment(the, yp, coordsf);
			let vym = distance_to_segment(the, ym, coordsf);
			let vzp = distance_to_segment(the, zp, coordsf);
			let vzm = distance_to_segment(the, zm, coordsf);
			(vxp < radius || vxm < radius)
				|| (vyp < radius || vym < radius)
				|| (vzp < radius || vzm < radius)
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let deformation_max_length = 13.0;
			let d = noise_d.sample_3d(coordsf / scale, &[]);
			let e = noise_e.sample_3d(coordsf / scale, &[]);
			let f = noise_f.sample_3d(coordsf / scale, &[]);
			use crate::coords::AngularDirection;
			let deformation = AngularDirection::from_angles(d * TAU, e * (TAU / 2.0)).to_vec3()
				* f * deformation_max_length;
			let deformed_coordsf = coordsf + deformation;
			coords_to_ground_uwu(deformed_coordsf)
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			// Test chunk generation.
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

pub struct WorldGeneratorTest008 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest008 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let noise_d = noise::OctavedNoise::new(4, vec![self.seed, 4]);
		let noise_e = noise::OctavedNoise::new(4, vec![self.seed, 5]);
		let noise_f = noise::OctavedNoise::new(4, vec![self.seed, 6]);
		let coords_to_ground_uwu = |coordsf: cgmath::Point3<f32>| -> bool {
			if coordsf.z < 0.0 {
				return true;
			}
			let scale = 65.0;
			let radius = (10.0f32).min(1.0f32.min(1.0 / (coordsf.z * 0.1 + 4.0)) * 30.0);
			if radius < 1.0 {
				return false;
			}
			let coordsf_to_the = |coordsf: cgmath::Point3<f32>| -> cgmath::Point3<f32> {
				let coordsf_i_scaled = coordsf.map(|x| (x / scale).floor());
				let a = noise_a.sample_3d(coordsf_i_scaled, &[]);
				let b = noise_b.sample_3d(coordsf_i_scaled, &[]);
				let c = noise_c.sample_3d(coordsf_i_scaled, &[]);
				let coordsf_min = coordsf.map(|x| (x / scale).floor() * scale);
				let _coordsf_max = coordsf.map(|x| (x / scale).ceil() * scale);
				let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
				coordsf_min + the
			};
			let the = coordsf_to_the(coordsf);
			let xp = coordsf_to_the(coordsf + cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let xm = coordsf_to_the(coordsf - cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let yp = coordsf_to_the(coordsf + cgmath::vec3(0.0, 1.0, 0.0) * scale);
			let ym = coordsf_to_the(coordsf - cgmath::vec3(0.0, 1.0, 0.0) * scale);
			let zp = coordsf_to_the(coordsf + cgmath::vec3(0.0, 0.0, 1.0) * scale);
			let zm = coordsf_to_the(coordsf - cgmath::vec3(0.0, 0.0, 1.0) * scale);
			let vxp = distance_to_segment(the, xp, coordsf);
			let vxm = distance_to_segment(the, xm, coordsf);
			let vyp = distance_to_segment(the, yp, coordsf);
			let vym = distance_to_segment(the, ym, coordsf);
			let vzp = distance_to_segment(the, zp, coordsf);
			let vzm = distance_to_segment(the, zm, coordsf);
			(vxp < radius || vxm < radius)
				|| (vyp < radius || vym < radius)
				|| (vzp < radius || vzm < radius)
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let deformation_max_length = 13.0;
			let d = noise_d.sample_3d(coordsf / scale, &[]);
			let e = noise_e.sample_3d(coordsf / scale, &[]);
			let f = noise_f.sample_3d(coordsf / scale, &[]);
			use crate::coords::AngularDirection;
			let deformation = AngularDirection::from_angles(d * TAU, e * (TAU / 2.0)).to_vec3()
				* f * deformation_max_length;
			let deformed_coordsf = coordsf + deformation;
			coords_to_ground_uwu(deformed_coordsf)
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			// Test chunk generation.
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

pub struct WorldGeneratorTest009 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest009 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let noise_d = noise::OctavedNoise::new(4, vec![self.seed, 4]);
		let noise_e = noise::OctavedNoise::new(4, vec![self.seed, 5]);
		let noise_f = noise::OctavedNoise::new(4, vec![self.seed, 6]);
		let coords_to_ground_uwu = |coordsf: cgmath::Point3<f32>| -> bool {
			if coordsf.z > 0.0 {
				return false;
			}
			let scale = 65.0;
			let radius = 5.0;
			let coordsf_to_the = |coordsf: cgmath::Point3<f32>| -> cgmath::Point3<f32> {
				let coordsf_i_scaled = coordsf.map(|x| (x / scale).floor());
				let a = noise_a.sample_3d(coordsf_i_scaled, &[]);
				let b = noise_b.sample_3d(coordsf_i_scaled, &[]);
				let c = noise_c.sample_3d(coordsf_i_scaled, &[]);
				let coordsf_min = coordsf.map(|x| (x / scale).floor() * scale);
				let _coordsf_max = coordsf.map(|x| (x / scale).ceil() * scale);
				let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
				coordsf_min + the
			};
			let the = coordsf_to_the(coordsf);
			let xp = coordsf_to_the(coordsf + cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let xm = coordsf_to_the(coordsf - cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let yp = coordsf_to_the(coordsf + cgmath::vec3(0.0, 1.0, 0.0) * scale);
			let ym = coordsf_to_the(coordsf - cgmath::vec3(0.0, 1.0, 0.0) * scale);
			let zp = coordsf_to_the(coordsf + cgmath::vec3(0.0, 0.0, 1.0) * scale);
			let zm = coordsf_to_the(coordsf - cgmath::vec3(0.0, 0.0, 1.0) * scale);
			let vxp = distance_to_segment(the, xp, coordsf);
			let vxm = distance_to_segment(the, xm, coordsf);
			let vyp = distance_to_segment(the, yp, coordsf);
			let vym = distance_to_segment(the, ym, coordsf);
			let vzp = distance_to_segment(the, zp, coordsf);
			let vzm = distance_to_segment(the, zm, coordsf);
			!((vxp < radius || vxm < radius)
				|| (vyp < radius || vym < radius)
				|| (vzp < radius || vzm < radius))
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let deformation_max_length = 13.0;
			let d = noise_d.sample_3d(coordsf / scale, &[]);
			let e = noise_e.sample_3d(coordsf / scale, &[]);
			let f = noise_f.sample_3d(coordsf / scale, &[]);
			use crate::coords::AngularDirection;
			let deformation = AngularDirection::from_angles(d * TAU, e * (TAU / 2.0)).to_vec3()
				* f * deformation_max_length;
			let deformed_coordsf = coordsf + deformation;
			coords_to_ground_uwu(deformed_coordsf)
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			// Test chunk generation.
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}
