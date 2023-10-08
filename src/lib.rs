#![allow(clippy::items_after_test_module)]

mod camera;
mod coords;
mod shaders;

use std::{
	collections::{hash_map::Entry, HashMap},
	f32::consts::TAU,
	io::Write,
};

use bytemuck::Zeroable;
use cgmath::{EuclideanSpace, InnerSpace, MetricSpace};
use rand::Rng;
use wgpu::util::DeviceExt;
use winit::{
	event_loop::{ControlFlow, EventLoop},
	window::WindowBuilder,
};

use camera::{
	aspect_ratio, CameraOrthographicSettings, CameraPerspectiveSettings, CameraSettings,
	Matrix4x4Pod,
};
use coords::*;
use shaders::{block::BlockVertexPod, simple_line::SimpleLineVertexPod};

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
struct BitCube3Coords {
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
	fn set(&mut self, axis: NonOrientedAxis, value: i32) {
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
struct BitCube3 {
	data: BitArray27,
}
impl BitCube3 {
	fn new_zero() -> BitCube3 {
		BitCube3 { data: BitArray27::new_zero() }
	}
	fn get(self, coords: BitCube3Coords) -> bool {
		self.data.get(coords.index())
	}
	fn set(&mut self, coords: BitCube3Coords, value: bool) {
		self.data.set(coords.index(), value);
	}
}

#[derive(Clone, Copy)]
struct BlockTypeId {
	is_not_air: bool,
}

/// The blocks of a chunk.
struct ChunkBlocks {
	coords_span: ChunkCoordsSpan,
	blocks: Vec<BlockTypeId>,
}

impl ChunkBlocks {
	fn new(coords_span: ChunkCoordsSpan) -> ChunkBlocks {
		ChunkBlocks {
			coords_span,
			blocks: Vec::from_iter(
				std::iter::repeat(BlockTypeId { is_not_air: false })
					.take(coords_span.cd.number_of_blocks()),
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
struct OpaquenessLayerAroundChunk {
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
	fn generate_mesh_given_surrounding_opaqueness(
		&self,
		mut opaqueness_layer: OpaquenessLayerAroundChunk,
	) -> ChunkMesh {
		let mut is_opaque = |coords: BlockCoords| {
			if let Some(block) = self.get(coords) {
				block.is_not_air
			} else if let Some(opaque) = opaqueness_layer.get(coords) {
				opaque
			} else {
				unreachable!()
			}
		};

		let mut block_vertices = Vec::new();
		for coords in self.coords_span.iter_coords() {
			if self.get(coords).unwrap().is_not_air {
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
						);
					}
				}
			}
		}
		ChunkMesh::from_vertices(block_vertices)
	}
}

struct ChunkMesh {
	block_vertices: Vec<BlockVertexPod>,
	block_vertex_buffer: Option<wgpu::Buffer>,
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

	fn update_gpu_data(&mut self, device: &wgpu::Device) {
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
	let texture_rect_in_atlas_xy: cgmath::Point2<f32> = (0.0, 0.0).into();
	let texture_rect_in_atlas_wh: cgmath::Vector2<f32> = (16.0, 16.0).into();
	let texture_rect_in_atlas_wh = texture_rect_in_atlas_wh * (1.0 / 512.0);
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

struct Chunk {
	coords_span: ChunkCoordsSpan,
	blocks: Option<ChunkBlocks>,
	remeshing_required: bool,
	mesh: Option<ChunkMesh>,
}

impl Chunk {
	fn new_empty(coords_span: ChunkCoordsSpan) -> Chunk {
		Chunk { coords_span, blocks: None, remeshing_required: false, mesh: None }
	}

	fn generate_mesh_given_surrounding_opaqueness(
		&mut self,
		opaqueness_layer: OpaquenessLayerAroundChunk,
	) {
		let mesh = self
			.blocks
			.as_ref()
			.unwrap()
			.generate_mesh_given_surrounding_opaqueness(opaqueness_layer);
		self.mesh = Some(mesh);
	}
}

struct ChunkGrid {
	cd: ChunkDimensions,
	map: HashMap<ChunkCoords, Chunk>,
}

impl ChunkGrid {
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

	fn set_block_and_update_meshes(
		&mut self,
		coords: BlockCoords,
		block: BlockTypeId,
		device: &wgpu::Device,
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
			if self.map.contains_key(&chunk_coords) {
				let opaqueness_layer = self.get_opaqueness_layer_around_chunk(chunk_coords, false);
				let chunk = self.map.get_mut(&chunk_coords).unwrap();
				chunk.generate_mesh_given_surrounding_opaqueness(opaqueness_layer);
				chunk.mesh.as_mut().unwrap().update_gpu_data(device);
			}
		}
	}

	fn get_block(&self, coords: BlockCoords) -> Option<BlockTypeId> {
		let chunk_coords = self.cd.world_coords_to_containing_chunk_coords(coords);
		let chunk = self.map.get(&chunk_coords)?;
		Some(chunk.blocks.as_ref().unwrap().get(coords).unwrap())
	}

	fn get_opaqueness_layer_around_chunk(
		&self,
		chunk_coords: ChunkCoords,
		default_to_opaque: bool,
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
								.map(|block_type_id| block_type_id.is_not_air)
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

	fn make_sure_that_a_chunk_exists(&mut self, chunk_coords: ChunkCoords) {
		if let Entry::Vacant(entry) = self.map.entry(chunk_coords) {
			let chunk = Chunk::new_empty(ChunkCoordsSpan { cd: self.cd, chunk_coords });
			entry.insert(chunk);
		}
	}

	fn generate_blocks(&mut self, chunk_coords: ChunkCoords, chunk_generator: ChunkGenerator) {
		self.make_sure_that_a_chunk_exists(chunk_coords);
		let chunk = self.map.get_mut(&chunk_coords).unwrap();
		chunk.blocks = Some(chunk_generator.generate_chunk_blocks(chunk.coords_span));

		for neighbor_chunk_coords in iter_3d_cube_center_radius(chunk_coords, 3) {
			if let Some(neighbor_chunk) = self.map.get_mut(&neighbor_chunk_coords) {
				neighbor_chunk.remeshing_required = true;
			}
		}
	}

	fn make_sure_that_a_chunk_has_blocks(
		&mut self,
		chunk_coords: ChunkCoords,
		chunk_generator: ChunkGenerator,
	) {
		self.make_sure_that_a_chunk_exists(chunk_coords);
		let chunk = self.map.get(&chunk_coords).unwrap();
		if chunk.blocks.is_none() {
			self.generate_blocks(chunk_coords, chunk_generator);
		}
	}

	fn remesh_all_chunks_that_require_it(&mut self, device: &wgpu::Device) {
		let chunk_coords_list: Vec<_> = self.map.keys().copied().collect();
		for chunk_coords in chunk_coords_list {
			if self
				.map
				.get(&chunk_coords)
				.as_ref()
				.is_some_and(|chunk| chunk.remeshing_required)
			{
				let opaqueness_layer = self.get_opaqueness_layer_around_chunk(chunk_coords, false);
				let chunk = self.map.get_mut(&chunk_coords).unwrap();
				chunk.generate_mesh_given_surrounding_opaqueness(opaqueness_layer);
				chunk.mesh.as_mut().unwrap().update_gpu_data(device);
				chunk.remeshing_required = false;
			}
		}
	}
}

struct ChunkGenerator {}

impl ChunkGenerator {
	fn generate_chunk_blocks(self, coords_span: ChunkCoordsSpan) -> ChunkBlocks {
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			// Test chunk generation.
			let ground = coords.z as f32
				- f32::cos(coords.x as f32 * 0.3)
				- f32::cos(coords.y as f32 * 0.3)
				- 3.0 < 0.0;
			*chunk_blocks.get_mut(coords).unwrap() = BlockTypeId { is_not_air: ground };
		}
		chunk_blocks
	}
}

/// Just a 3D rectangular axis-aligned box.
/// It cannot rotate as it stays aligned on the axes.
struct AlignedBox {
	/// Position of the center of the box.
	pos: cgmath::Point3<f32>,
	/// Width of the box along each axis.
	dims: cgmath::Vector3<f32>,
}

/// Represents an `AlignedBox`-shaped object that has physics or something like that.
struct AlignedPhysBox {
	aligned_box: AlignedBox,
	motion: cgmath::Vector3<f32>,
	/// Gravity's acceleration of this box is influenced by this parameter.
	/// It may not be exactly analog to weight but it's not too far.
	gravity_factor: f32,
}

/// Mesh of simple lines.
///
/// Can be used (for example) to display hit boxes for debugging purposes.
struct SimpleLineMesh {
	vertices: Vec<SimpleLineVertexPod>,
	vertex_buffer: wgpu::Buffer,
}

impl SimpleLineMesh {
	fn from_vertices(device: &wgpu::Device, vertices: Vec<SimpleLineVertexPod>) -> SimpleLineMesh {
		let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Simple Line Vertex Buffer"),
			contents: bytemuck::cast_slice(&vertices),
			usage: wgpu::BufferUsages::VERTEX,
		});
		SimpleLineMesh { vertices, vertex_buffer }
	}

	fn from_aligned_box(device: &wgpu::Device, aligned_box: &AlignedBox) -> SimpleLineMesh {
		// NO EARLY OPTIMIZATION
		// This shall remain in an unoptimized, unfactorized and flexible state for now!

		let color = [1.0, 1.0, 1.0];
		let mut vertices = Vec::new();
		// A---B  +--->   The L square and the H square are horizontal.
		// |   |  |   X+  L has lower value of Z coord.
		// C---D  v Y+    H has heigher value of Z coord.
		let al = aligned_box.pos - aligned_box.dims / 2.0;
		let bl = al + cgmath::Vector3::<f32>::unit_x() * aligned_box.dims.x;
		let cl = al + cgmath::Vector3::<f32>::unit_y() * aligned_box.dims.y;
		let dl = bl + cgmath::Vector3::<f32>::unit_y() * aligned_box.dims.y;
		let ah = al + cgmath::Vector3::<f32>::unit_z() * aligned_box.dims.z;
		let bh = bl + cgmath::Vector3::<f32>::unit_z() * aligned_box.dims.z;
		let ch = cl + cgmath::Vector3::<f32>::unit_z() * aligned_box.dims.z;
		let dh = dl + cgmath::Vector3::<f32>::unit_z() * aligned_box.dims.z;
		// L square
		vertices.push(SimpleLineVertexPod { position: al.into(), color });
		vertices.push(SimpleLineVertexPod { position: bl.into(), color });
		vertices.push(SimpleLineVertexPod { position: bl.into(), color });
		vertices.push(SimpleLineVertexPod { position: dl.into(), color });
		vertices.push(SimpleLineVertexPod { position: dl.into(), color });
		vertices.push(SimpleLineVertexPod { position: cl.into(), color });
		vertices.push(SimpleLineVertexPod { position: cl.into(), color });
		vertices.push(SimpleLineVertexPod { position: al.into(), color });
		// H square
		vertices.push(SimpleLineVertexPod { position: ah.into(), color });
		vertices.push(SimpleLineVertexPod { position: bh.into(), color });
		vertices.push(SimpleLineVertexPod { position: bh.into(), color });
		vertices.push(SimpleLineVertexPod { position: dh.into(), color });
		vertices.push(SimpleLineVertexPod { position: dh.into(), color });
		vertices.push(SimpleLineVertexPod { position: ch.into(), color });
		vertices.push(SimpleLineVertexPod { position: ch.into(), color });
		vertices.push(SimpleLineVertexPod { position: ah.into(), color });
		// Edges between L square and H square corresponding vertices.
		vertices.push(SimpleLineVertexPod { position: al.into(), color });
		vertices.push(SimpleLineVertexPod { position: ah.into(), color });
		vertices.push(SimpleLineVertexPod { position: bl.into(), color });
		vertices.push(SimpleLineVertexPod { position: bh.into(), color });
		vertices.push(SimpleLineVertexPod { position: cl.into(), color });
		vertices.push(SimpleLineVertexPod { position: ch.into(), color });
		vertices.push(SimpleLineVertexPod { position: dl.into(), color });
		vertices.push(SimpleLineVertexPod { position: dh.into(), color });
		SimpleLineMesh::from_vertices(device, vertices)
	}

	fn interface_2d_cursor(device: &wgpu::Device, window_size: (u32, u32)) -> SimpleLineMesh {
		let color = [1.0, 1.0, 1.0];
		let w = 20.0 / window_size.0 as f32;
		let h = 20.0 / window_size.1 as f32;
		let vertices = vec![
			SimpleLineVertexPod { position: [-w, 0.0, 0.5], color },
			SimpleLineVertexPod { position: [w, 0.0, 0.5], color },
			SimpleLineVertexPod { position: [0.0, -h, 0.5], color },
			SimpleLineVertexPod { position: [0.0, h, 0.5], color },
		];
		SimpleLineMesh::from_vertices(device, vertices)
	}
}

/// Vector in 3D.
#[derive(Copy, Clone, Debug)]
/// Certified Plain Old Data (so it can be sent to the GPU as a uniform).
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vector3Pod {
	values: [f32; 3],
}

fn letter_to_keycode(letter: char) -> winit::event::VirtualKeyCode {
	use winit::event::VirtualKeyCode as K;
	#[rustfmt::skip]
	let keycode = match letter.to_ascii_uppercase() {
		'A' => K::A, 'B' => K::B, 'C' => K::C, 'D' => K::D, 'E' => K::E, 'F' => K::F, 'G' => K::G,
		'H' => K::H, 'I' => K::I, 'J' => K::J, 'K' => K::K, 'L' => K::L, 'M' => K::M, 'N' => K::N,
		'O' => K::O, 'P' => K::P, 'Q' => K::Q, 'R' => K::R, 'S' => K::S, 'T' => K::T, 'U' => K::U,
		'V' => K::V, 'W' => K::W, 'X' => K::X, 'Y' => K::Y, 'Z' => K::Z,
		not_a_letter => panic!("can't convert \"{not_a_letter}\" to an ascii letter keycode"),
	};
	keycode
}

fn digit_to_keycode(digit: char) -> winit::event::VirtualKeyCode {
	use winit::event::VirtualKeyCode as K;
	#[rustfmt::skip]
	let keycode = match digit {
		'0' => K::Key0, '1' => K::Key1, '2' => K::Key2, '3' => K::Key3, '4' => K::Key4,
		'5' => K::Key5, '6' => K::Key6, '7' => K::Key7, '8' => K::Key8, '9' => K::Key9,
		not_a_digit => panic!("can't convert \"{not_a_digit}\" to an digit keycode"),
	};
	keycode
}

/// Type representation for the `ty` and `count` fields of a `wgpu::BindGroupLayoutEntry`.
struct BindingType {
	ty: wgpu::BindingType,
	count: Option<std::num::NonZeroU32>,
}

impl BindingType {
	fn layout_entry(
		&self,
		binding: u32,
		visibility: wgpu::ShaderStages,
	) -> wgpu::BindGroupLayoutEntry {
		wgpu::BindGroupLayoutEntry { binding, visibility, ty: self.ty, count: self.count }
	}
}

trait BindingResourceable {
	fn as_binding_resource(&self) -> wgpu::BindingResource;
}
impl BindingResourceable for wgpu::Buffer {
	fn as_binding_resource(&self) -> wgpu::BindingResource {
		self.as_entire_binding()
	}
}
impl BindingResourceable for wgpu::TextureView {
	fn as_binding_resource(&self) -> wgpu::BindingResource {
		wgpu::BindingResource::TextureView(self)
	}
}
impl BindingResourceable for wgpu::Sampler {
	fn as_binding_resource(&self) -> wgpu::BindingResource {
		wgpu::BindingResource::Sampler(self)
	}
}

/// Resource and associated information required for creations of both
/// a `wgpu::BindGroupLayoutEntry` and a `wgpu::BindGroupEntry`.
struct BindingThingy<T: BindingResourceable> {
	binding_type: BindingType,
	resource: T,
}

pub fn run() {
	// Wgpu uses the `log`/`env_logger` crates to log errors and stuff,
	// and we do want to see the errors very much.
	env_logger::init();

	let event_loop = EventLoop::new();
	let window = WindowBuilder::new()
		.with_title("Qwy3")
		.with_maximized(true)
		.with_resizable(true)
		.build(&event_loop)
		.unwrap();
	let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
		backends: wgpu::Backends::all(),
		dx12_shader_compiler: Default::default(),
	});
	let window_surface = unsafe { instance.create_surface(&window) }.unwrap();

	// Try to get a cool adapter first.
	let adapter = instance
		.enumerate_adapters(wgpu::Backends::all())
		.find(|adapter| {
			let info = adapter.get_info();
			info.device_type == wgpu::DeviceType::DiscreteGpu
				&& adapter.is_surface_supported(&window_surface)
		});
	// In case we didn't find any cool adapter, at least we can try to get a bad adapter.
	let adapter = adapter.or_else(|| {
		futures::executor::block_on(async {
			instance
				.request_adapter(&wgpu::RequestAdapterOptions {
					power_preference: wgpu::PowerPreference::HighPerformance,
					compatible_surface: Some(&window_surface),
					force_fallback_adapter: false,
				})
				.await
		})
	});
	let adapter = adapter.unwrap();

	// At some point it could be nice to allow the user to choose their preferred adapter.
	// No one should have to struggle to make some game use the big GPU instead of the tiny one.
	println!("AVAILABLE ADAPTERS:");
	for adapter in instance.enumerate_adapters(wgpu::Backends::all()) {
		dbg!(adapter.get_info());
		dbg!(adapter.limits().max_bind_groups);
	}
	println!("SELECTED ADAPTER:");
	dbg!(adapter.get_info());

	let (device, queue) = futures::executor::block_on(async {
		adapter
			.request_device(
				&wgpu::DeviceDescriptor {
					features: wgpu::Features::empty(),
					limits: wgpu::Limits { max_bind_groups: 8, ..wgpu::Limits::default() },
					label: None,
				},
				None,
			)
			.await
	})
	.unwrap();

	let surface_capabilities = window_surface.get_capabilities(&adapter);
	let surface_format = surface_capabilities
		.formats
		.iter()
		.copied()
		.find(|f| f.is_srgb())
		.unwrap_or(surface_capabilities.formats[0]);
	assert!(surface_capabilities
		.present_modes
		.contains(&wgpu::PresentMode::Fifo));
	let size = window.inner_size();
	let mut config = wgpu::SurfaceConfiguration {
		usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
		format: surface_format,
		width: size.width,
		height: size.height,
		present_mode: wgpu::PresentMode::Fifo,
		alpha_mode: surface_capabilities.alpha_modes[0],
		view_formats: vec![],
	};
	window_surface.configure(&device, &config);

	const ATLAS_DIMS: (usize, usize) = (512, 512);
	let mut atlas_data: [u8; 4 * ATLAS_DIMS.0 * ATLAS_DIMS.1] = [0; 4 * ATLAS_DIMS.0 * ATLAS_DIMS.1];
	for y in 0..16 {
		for x in 0..16 {
			let index = 4 * (y * ATLAS_DIMS.0 + x);
			let grey = rand::thread_rng().gen_range(240..=255);
			atlas_data[index..(index + 4)].clone_from_slice(&[grey, grey, grey, 255]);
			// Kinda grass:
			// atlas_data[index..(index + 4)].clone_from_slice(&[
			// 	rand::thread_rng().gen_range(80..100),
			// 	rand::thread_rng().gen_range(230..=255),
			// 	rand::thread_rng().gen_range(10..30),
			// 	255,
			// ]);
		}
	}

	let atlas_texture_size = wgpu::Extent3d {
		width: ATLAS_DIMS.0 as u32,
		height: ATLAS_DIMS.1 as u32,
		depth_or_array_layers: 1,
	};
	let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
		label: Some("Atlas Texture"),
		size: atlas_texture_size,
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: wgpu::TextureFormat::Rgba8UnormSrgb,
		usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
		view_formats: &[],
	});
	queue.write_texture(
		wgpu::ImageCopyTexture {
			texture: &atlas_texture,
			mip_level: 0,
			origin: wgpu::Origin3d::ZERO,
			aspect: wgpu::TextureAspect::All,
		},
		&atlas_data,
		wgpu::ImageDataLayout {
			offset: 0,
			bytes_per_row: Some(4 * ATLAS_DIMS.0 as u32),
			rows_per_image: Some(ATLAS_DIMS.1 as u32),
		},
		atlas_texture_size,
	);
	let atlas_texture_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
	let atlas_texture_view_binding_type = BindingType {
		ty: wgpu::BindingType::Texture {
			multisampled: false,
			view_dimension: wgpu::TextureViewDimension::D2,
			sample_type: wgpu::TextureSampleType::Float { filterable: true },
		},
		count: None,
	};
	let atlas_texture_view_thingy = BindingThingy {
		binding_type: atlas_texture_view_binding_type,
		resource: atlas_texture_view,
	};
	let atlas_texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
		address_mode_u: wgpu::AddressMode::ClampToEdge,
		address_mode_v: wgpu::AddressMode::ClampToEdge,
		address_mode_w: wgpu::AddressMode::ClampToEdge,
		mag_filter: wgpu::FilterMode::Nearest,
		min_filter: wgpu::FilterMode::Nearest,
		mipmap_filter: wgpu::FilterMode::Nearest,
		..Default::default()
	});
	let atlas_texture_sampler_binding_type = BindingType {
		ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
		count: None,
	};
	let atlas_texture_sampler_thingy = BindingThingy {
		binding_type: atlas_texture_sampler_binding_type,
		resource: atlas_texture_sampler,
	};

	let mut camera = CameraPerspectiveSettings {
		up_direction: (0.0, 0.0, 1.0).into(),
		aspect_ratio: config.width as f32 / config.height as f32,
		field_of_view_y: TAU / 4.0,
		near_plane: 0.001,
		far_plane: 400.0,
	};
	let camera_matrix_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Camera Buffer"),
		contents: bytemuck::cast_slice(&[Matrix4x4Pod::zeroed()]),
		usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
	});
	let camera_matrix_binding_type = BindingType {
		ty: wgpu::BindingType::Buffer {
			ty: wgpu::BufferBindingType::Uniform,
			has_dynamic_offset: false,
			min_binding_size: None,
		},
		count: None,
	};
	let camera_matrix_thingy = BindingThingy {
		binding_type: camera_matrix_binding_type,
		resource: camera_matrix_buffer,
	};

	let mut camera_direction = AngularDirection::from_angle_horizontal(0.0);
	let mut enable_camera_third_person = false;

	let mut cursor_is_captured = true;
	window
		.set_cursor_grab(winit::window::CursorGrabMode::Confined)
		.unwrap();
	window.set_cursor_visible(false);

	// First is the block of matter that is targeted,
	// second is the empty block near it that would be filled if a block was placed now.
	let mut targeted_block_coords: Option<(BlockCoords, BlockCoords)> = None;

	let mut walking_forward = false;
	let mut walking_backward = false;
	let mut walking_leftward = false;
	let mut walking_rightward = false;

	let mut player_phys = AlignedPhysBox {
		aligned_box: AlignedBox { pos: (5.5, 5.5, 5.5).into(), dims: (0.8, 0.8, 1.8).into() },
		motion: (0.0, 0.0, 0.0).into(),
		gravity_factor: 1.0,
	};
	let mut enable_physics = true;
	let mut enable_display_phys_box = false;

	let mut sun_position_in_sky = AngularDirection::from_angles(TAU / 16.0, TAU / 8.0);
	let sun_light_direction_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Sun Light Direction Buffer"),
		contents: bytemuck::cast_slice(&[Vector3Pod::zeroed()]),
		usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
	});
	let sun_light_direction_binding_type = BindingType {
		ty: wgpu::BindingType::Buffer {
			ty: wgpu::BufferBindingType::Uniform,
			has_dynamic_offset: false,
			min_binding_size: None,
		},
		count: None,
	};
	let sun_light_direction_thingy = BindingThingy {
		binding_type: sun_light_direction_binding_type,
		resource: sun_light_direction_buffer,
	};

	let sun_camera = CameraOrthographicSettings {
		up_direction: (0.0, 0.0, 1.0).into(),
		width: 85.0,
		height: 85.0,
		depth: 200.0,
	};
	let sun_camera_matrix_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Sun Camera Buffer"),
		contents: bytemuck::cast_slice(&[Matrix4x4Pod::zeroed()]),
		usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
	});
	let sun_camera_matrix_binding_type = BindingType {
		ty: wgpu::BindingType::Buffer {
			ty: wgpu::BufferBindingType::Uniform,
			has_dynamic_offset: false,
			min_binding_size: None,
		},
		count: None,
	};
	let sun_camera_matrix_thingy = BindingThingy {
		binding_type: sun_camera_matrix_binding_type,
		resource: sun_camera_matrix_buffer,
	};

	let mut use_sun_camera_to_render = false;

	let shadow_map_format = wgpu::TextureFormat::Depth32Float;
	let shadow_map_texture = device.create_texture(&wgpu::TextureDescriptor {
		label: Some("Shadow Map Texture"),
		size: wgpu::Extent3d { width: 8192, height: 8192, depth_or_array_layers: 1 },
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: shadow_map_format,
		usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
		view_formats: &[],
	});
	let shadow_map_view = shadow_map_texture.create_view(&wgpu::TextureViewDescriptor::default());
	let shadow_map_view_binding_type = BindingType {
		ty: wgpu::BindingType::Texture {
			sample_type: wgpu::TextureSampleType::Depth,
			view_dimension: wgpu::TextureViewDimension::D2,
			multisampled: false,
		},
		count: None,
	};
	let shadow_map_view_thingy = BindingThingy {
		binding_type: shadow_map_view_binding_type,
		resource: shadow_map_view,
	};
	let shadow_map_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
		label: Some("Shadow Map Sampler"),
		address_mode_u: wgpu::AddressMode::ClampToEdge,
		address_mode_v: wgpu::AddressMode::ClampToEdge,
		address_mode_w: wgpu::AddressMode::ClampToEdge,
		mag_filter: wgpu::FilterMode::Linear,
		min_filter: wgpu::FilterMode::Linear,
		mipmap_filter: wgpu::FilterMode::Nearest,
		compare: Some(wgpu::CompareFunction::LessEqual),
		..Default::default()
	});
	let shadow_map_sampler_binding_type = BindingType {
		ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
		count: None,
	};
	let shadow_map_sampler_thingy = BindingThingy {
		binding_type: shadow_map_sampler_binding_type,
		resource: shadow_map_sampler,
	};

	fn make_z_buffer_texture_view(
		device: &wgpu::Device,
		format: wgpu::TextureFormat,
		width: u32,
		height: u32,
	) -> wgpu::TextureView {
		let z_buffer_texture_description = wgpu::TextureDescriptor {
			label: Some("Z Buffer"),
			size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
			mip_level_count: 1,
			sample_count: 1,
			dimension: wgpu::TextureDimension::D2,
			format,
			view_formats: &[],
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
		};
		let z_buffer_texture = device.create_texture(&z_buffer_texture_description);
		z_buffer_texture.create_view(&wgpu::TextureViewDescriptor::default())
	}
	let z_buffer_format = wgpu::TextureFormat::Depth32Float;
	let mut z_buffer_view =
		make_z_buffer_texture_view(&device, z_buffer_format, config.width, config.height);

	let (block_shadow_render_pipeline, block_shadow_bind_group) =
		shaders::block_shadow::render_pipeline_and_bind_group(
			&device,
			shaders::block_shadow::BindingThingies {
				sun_camera_matrix_thingy: &sun_camera_matrix_thingy,
			},
			shadow_map_format,
		);

	let (block_render_pipeline, block_bind_group) = shaders::block::render_pipeline_and_bind_group(
		&device,
		shaders::block::BindingThingies {
			camera_matrix_thingy: &camera_matrix_thingy,
			sun_light_direction_thingy: &sun_light_direction_thingy,
			sun_camera_matrix_thingy: &sun_camera_matrix_thingy,
			shadow_map_view_thingy: &shadow_map_view_thingy,
			shadow_map_sampler_thingy: &shadow_map_sampler_thingy,
			atlas_texture_view_thingy: &atlas_texture_view_thingy,
			atlas_texture_sampler_thingy: &atlas_texture_sampler_thingy,
		},
		config.format,
		z_buffer_format,
	);

	let (simple_line_render_pipeline, simple_line_render_bind_group) =
		shaders::simple_line::render_pipeline_and_bind_group(
			&device,
			shaders::simple_line::BindingThingies { camera_matrix_thingy: &camera_matrix_thingy },
			config.format,
			z_buffer_format,
		);

	let simple_line_2d_render_pipeline = shaders::simple_line_2d::render_pipeline(
		&device,
		shaders::simple_line_2d::BindingThingies {},
		config.format,
		z_buffer_format,
	);

	let time_beginning = std::time::Instant::now();
	let mut time_from_last_iteration = std::time::Instant::now();

	#[derive(Clone, Copy, PartialEq, Eq, Hash)]
	enum Control {
		KeyboardKey(VirtualKeyCode),
		MouseButton(MouseButton),
	}
	enum Action {
		WalkForward,
		WalkBackward,
		WalkLeftward,
		WalkRightward,
		Jump,
		TogglePhysics,
		ToggleWorldGeneration,
		ToggleThirdPersonView,
		ToggleDisplayPlayerBox,
		ToggleSunView,
		ToggleCursorCaptured,
		PrintCoords,
		PlaceOrRemoveBlockUnderPlayer,
		PlaceBlockAtTarget,
		RemoveBlockAtTarget,
	}

	let mut control_bindings: HashMap<Control, Action> = HashMap::new();

	let command_file_path = "controls.qwy3_controls";
	if !std::path::Path::new(command_file_path).is_file() {
		let mut file =
			std::fs::File::create(command_file_path).expect("count not create config file");
		file
			.write_all(include_str!("default_controls.qwy3_controls").as_bytes())
			.expect("could not fill the default config in the new config file");
	}

	if let Ok(controls_config_string) = std::fs::read_to_string(command_file_path) {
		for (line_number, line) in controls_config_string.lines().enumerate() {
			let mut words = line.split_whitespace();
			let command_name = words.next();
			if command_name == Some("bind_control") {
				let control_name = words.next().expect("expected control name");
				let action_name = words.next().expect("expected action name");

				let control = if let Some(key_name) = control_name.strip_prefix("key:") {
					if key_name.chars().count() == 1 {
						let signle_char_key_name = key_name.chars().next().unwrap();
						if signle_char_key_name.is_ascii_alphabetic() {
							Control::KeyboardKey(letter_to_keycode(signle_char_key_name))
						} else if signle_char_key_name.is_ascii_digit() {
							Control::KeyboardKey(digit_to_keycode(signle_char_key_name))
						} else {
							panic!("unknown signle character key name \"{signle_char_key_name}\"")
						}
					} else {
						match key_name {
							"up" => Control::KeyboardKey(VirtualKeyCode::Up),
							"down" => Control::KeyboardKey(VirtualKeyCode::Down),
							"left" => Control::KeyboardKey(VirtualKeyCode::Left),
							"right" => Control::KeyboardKey(VirtualKeyCode::Right),
							"space" => Control::KeyboardKey(VirtualKeyCode::Space),
							"left_shift" => Control::KeyboardKey(VirtualKeyCode::LShift),
							"right_shift" => Control::KeyboardKey(VirtualKeyCode::RShift),
							"tab" => Control::KeyboardKey(VirtualKeyCode::Tab),
							"return" | "enter" => Control::KeyboardKey(VirtualKeyCode::Return),
							unknown_key_name => panic!("unknown key name \"{unknown_key_name}\""),
						}
					}
				} else if let Some(button_name) = control_name.strip_prefix("mouse_button:") {
					if button_name == "left" {
						Control::MouseButton(MouseButton::Left)
					} else if button_name == "right" {
						Control::MouseButton(MouseButton::Right)
					} else if button_name == "middle" {
						Control::MouseButton(MouseButton::Middle)
					} else if let Ok(number) = button_name.parse() {
						Control::MouseButton(MouseButton::Other(number))
					} else {
						panic!("unknown mouse button name \"{button_name}\"")
					}
				} else {
					panic!(
						"unknown control \"{control_name}\" \
					(it must start with \"key:\" or \"mouse_button:\")"
					)
				};

				let action = match action_name {
					"walk_forward" => Action::WalkForward,
					"walk_backward" => Action::WalkBackward,
					"walk_leftward" => Action::WalkLeftward,
					"walk_rightward" => Action::WalkRightward,
					"jump" => Action::Jump,
					"toggle_physics" => Action::TogglePhysics,
					"toggle_world_generation" => Action::ToggleWorldGeneration,
					"toggle_third_person_view" => Action::ToggleThirdPersonView,
					"toggle_display_player_box" => Action::ToggleDisplayPlayerBox,
					"toggle_sun_view" => Action::ToggleSunView,
					"toggle_cursor_captured" => Action::ToggleCursorCaptured,
					"print_coords" => Action::PrintCoords,
					"place_or_remove_block_under_player" => Action::PlaceOrRemoveBlockUnderPlayer,
					"place_block_at_target" => Action::PlaceBlockAtTarget,
					"remove_block_at_target" => Action::RemoveBlockAtTarget,
					unknown_action_name => panic!("unknown action name \"{unknown_action_name}\""),
				};
				control_bindings.insert(control, action);
			} else if let Some(unknown_command_name) = command_name {
				println!(
					"Error in file \"{command_file_path}\" at line {line_number}: \
					Command name \"{unknown_command_name}\" is unknown"
				);
			}
		}
	} else {
		println!("Couldn't read file \"{command_file_path}\"");
	}

	struct ControlEvent {
		control: Control,
		pressed: bool,
	}
	let mut controls_to_trigger: Vec<ControlEvent> = vec![];

	let cd = ChunkDimensions::from(16);
	let mut chunk_grid = ChunkGrid { cd, map: HashMap::new() };
	for chunk_coords in iter_3d_cube_center_radius((0, 0, 0).into(), 3) {
		chunk_grid.make_sure_that_a_chunk_has_blocks(chunk_coords, ChunkGenerator {});
	}
	chunk_grid.remesh_all_chunks_that_require_it(&device);

	let mut enable_world_generation = true;

	use winit::event::*;
	event_loop.run(move |event, _, control_flow| match event {
		Event::WindowEvent { ref event, window_id } if window_id == window.id() => match event {
			WindowEvent::CloseRequested
			| WindowEvent::KeyboardInput {
				input:
					KeyboardInput {
						state: ElementState::Pressed,
						virtual_keycode: Some(VirtualKeyCode::Escape),
						..
					},
				..
			} => *control_flow = ControlFlow::Exit,

			WindowEvent::Resized(new_size) => {
				let winit::dpi::PhysicalSize { width, height } = *new_size;
				config.width = width;
				config.height = height;
				window_surface.configure(&device, &config);
				z_buffer_view = make_z_buffer_texture_view(&device, z_buffer_format, width, height);
				camera.aspect_ratio = aspect_ratio(width, height);
			},

			WindowEvent::MouseInput {
				state: winit::event::ElementState::Pressed,
				button: winit::event::MouseButton::Left,
				..
			} if !cursor_is_captured => {
				cursor_is_captured = true;
				window
					.set_cursor_grab(winit::window::CursorGrabMode::Confined)
					.unwrap();
				window.set_cursor_visible(false);
			},

			WindowEvent::KeyboardInput {
				input:
					KeyboardInput {
						state: ElementState::Pressed,
						virtual_keycode: Some(VirtualKeyCode::Return),
						..
					},
				..
			} => {
				let player_block_coords = (player_phys.aligned_box.pos
					- cgmath::Vector3::<f32>::unit_z() * (player_phys.aligned_box.dims.z / 2.0 + 0.1))
					.map(|x| x.round() as i32);
				let player_chunk_coords =
					cd.world_coords_to_containing_chunk_coords(player_block_coords);

				for chunk_coords in iter_3d_cube_center_radius(player_chunk_coords, 6) {
					chunk_grid.make_sure_that_a_chunk_has_blocks(chunk_coords, ChunkGenerator {});
				}
				chunk_grid.remesh_all_chunks_that_require_it(&device);
			},

			WindowEvent::KeyboardInput {
				input: KeyboardInput { state, virtual_keycode: Some(key), .. },
				..
			} => {
				controls_to_trigger.push(ControlEvent {
					control: Control::KeyboardKey(*key),
					pressed: *state == ElementState::Pressed,
				});
			},

			WindowEvent::MouseInput { state, button, .. } if cursor_is_captured => {
				controls_to_trigger.push(ControlEvent {
					control: Control::MouseButton(*button),
					pressed: *state == ElementState::Pressed,
				});
			},

			_ => {},
		},

		Event::DeviceEvent { event: winit::event::DeviceEvent::MouseMotion { delta }, .. }
			if cursor_is_captured =>
		{
			// Move camera.
			let sensitivity = 0.0025;
			camera_direction.angle_horizontal += -1.0 * delta.0 as f32 * sensitivity;
			camera_direction.angle_vertical += delta.1 as f32 * sensitivity;
			if camera_direction.angle_vertical < 0.0 {
				camera_direction.angle_vertical = 0.0;
			}
			if TAU / 2.0 < camera_direction.angle_vertical {
				camera_direction.angle_vertical = TAU / 2.0;
			}
		},

		Event::DeviceEvent { event: winit::event::DeviceEvent::MouseWheel { delta }, .. } => {
			// Wheel moves the player along the vertical axis.
			// Useful when physics are disabled.
			let (dx, dy) = match delta {
				MouseScrollDelta::LineDelta(horizontal, vertical) => (horizontal, vertical),
				MouseScrollDelta::PixelDelta(position) => (position.x as f32, position.y as f32),
			};
			let sensitivity = 0.01;
			let direction_left_or_right = camera_direction
				.to_horizontal()
				.add_to_horizontal_angle(TAU / 4.0 * dx.signum());
			player_phys.aligned_box.pos.z -= dy * sensitivity;
			player_phys.aligned_box.pos +=
				direction_left_or_right.to_vec3() * f32::abs(dx) * sensitivity;
		},

		Event::MainEventsCleared => {
			let _time_since_beginning = time_beginning.elapsed();
			let now = std::time::Instant::now();
			let dt = now - time_from_last_iteration;
			time_from_last_iteration = now;

			// Perform actions triggered by controls.
			for control_event in controls_to_trigger.iter() {
				let pressed = control_event.pressed;
				if let Some(action) = control_bindings.get(&control_event.control) {
					match (action, pressed) {
						(Action::WalkForward, pressed) => {
							walking_forward = pressed;
						},
						(Action::WalkBackward, pressed) => {
							walking_backward = pressed;
						},
						(Action::WalkLeftward, pressed) => {
							walking_leftward = pressed;
						},
						(Action::WalkRightward, pressed) => {
							walking_rightward = pressed;
						},
						(Action::Jump, true) => {
							player_phys.motion.z = 0.1;
						},
						(Action::TogglePhysics, true) => {
							enable_physics = !enable_physics;
						},
						(Action::ToggleWorldGeneration, true) => {
							enable_world_generation = !enable_world_generation;
						},
						(Action::ToggleThirdPersonView, true) => {
							enable_camera_third_person = !enable_camera_third_person;
						},
						(Action::ToggleDisplayPlayerBox, true) => {
							enable_display_phys_box = !enable_display_phys_box;
						},
						(Action::ToggleSunView, true) => {
							use_sun_camera_to_render = !use_sun_camera_to_render;
						},
						(Action::ToggleCursorCaptured, true) => {
							cursor_is_captured = !cursor_is_captured;
							if cursor_is_captured {
								window
									.set_cursor_grab(winit::window::CursorGrabMode::Confined)
									.unwrap();
								window.set_cursor_visible(false);
							} else {
								window
									.set_cursor_grab(winit::window::CursorGrabMode::None)
									.unwrap();
								window.set_cursor_visible(true);
							}
						},
						(Action::PrintCoords, true) => {
							dbg!(player_phys.aligned_box.pos);
							let player_bottom = player_phys.aligned_box.pos
								- cgmath::Vector3::<f32>::from((
									0.0,
									0.0,
									player_phys.aligned_box.dims.z / 2.0,
								));
							dbg!(player_bottom);
						},
						(Action::PlaceOrRemoveBlockUnderPlayer, true) => {
							let player_bottom = player_phys.aligned_box.pos
								- cgmath::Vector3::<f32>::unit_z()
									* (player_phys.aligned_box.dims.z / 2.0 + 0.1);
							let player_bottom_block_coords = player_bottom.map(|x| x.round() as i32);
							let player_bottom_block_opt = chunk_grid.get_block(player_bottom_block_coords);
							if let Some(block) = player_bottom_block_opt {
								chunk_grid.set_block_and_update_meshes(
									player_bottom_block_coords,
									BlockTypeId { is_not_air: !block.is_not_air },
									&device,
								);
							}
						},
						(Action::PlaceBlockAtTarget, true) => {
							if let Some((_, coords)) = targeted_block_coords {
								chunk_grid.set_block_and_update_meshes(
									coords,
									BlockTypeId { is_not_air: true },
									&device,
								);
							}
						},
						(Action::RemoveBlockAtTarget, true) => {
							if let Some((coords, _)) = targeted_block_coords {
								chunk_grid.set_block_and_update_meshes(
									coords,
									BlockTypeId { is_not_air: false },
									&device,
								);
							}
						},
						(_, false) => {},
					}
				}
			}
			controls_to_trigger.clear();

			if enable_world_generation {
				let player_block_coords = (player_phys.aligned_box.pos
					- cgmath::Vector3::<f32>::unit_z() * (player_phys.aligned_box.dims.z / 2.0 + 0.1))
					.map(|x| x.round() as i32);
				let player_chunk_coords =
					cd.world_coords_to_containing_chunk_coords(player_block_coords);
				for chunk_coords in iter_3d_cube_center_radius(player_chunk_coords, 3) {
					chunk_grid.make_sure_that_a_chunk_has_blocks(chunk_coords, ChunkGenerator {});
				}
				chunk_grid.remesh_all_chunks_that_require_it(&device);
			}

			let walking_vector = {
				let walking_factor = if enable_physics { 12.0 } else { 35.0 } * dt.as_secs_f32();
				let walking_forward_factor =
					if walking_forward { 1 } else { 0 } + if walking_backward { -1 } else { 0 };
				let walking_rightward_factor =
					if walking_rightward { 1 } else { 0 } + if walking_leftward { -1 } else { 0 };
				let walking_forward_direction =
					camera_direction.to_horizontal().to_vec3() * walking_forward_factor as f32;
				let walking_rightward_direction = camera_direction
					.to_horizontal()
					.add_to_horizontal_angle(-TAU / 4.0)
					.to_vec3() * walking_rightward_factor as f32;
				let walking_vector_direction = walking_forward_direction + walking_rightward_direction;
				(if walking_vector_direction.magnitude() == 0.0 {
					walking_vector_direction
				} else {
					walking_vector_direction.normalize()
				} * walking_factor)
			};
			player_phys.aligned_box.pos += walking_vector;

			if enable_physics {
				// TODO: Work out something better here,
				// although it is not very important at the moment.
				let player_bottom = player_phys.aligned_box.pos
					- cgmath::Vector3::<f32>::from((0.0, 0.0, player_phys.aligned_box.dims.z / 2.0));
				let player_bottom_below = player_phys.aligned_box.pos
					- cgmath::Vector3::<f32>::from((
						0.0,
						0.0,
						player_phys.aligned_box.dims.z / 2.0 + 0.01,
					));
				let player_bottom_block_coords = player_bottom.map(|x| x.round() as i32);
				let player_bottom_block_coords_below = player_bottom_below.map(|x| x.round() as i32);
				let player_bottom_block_opt = chunk_grid.get_block(player_bottom_block_coords);
				let player_bottom_block_opt_below =
					chunk_grid.get_block(player_bottom_block_coords_below);
				let is_on_ground = if player_phys.motion.z <= 0.0 {
					if let Some(block) = player_bottom_block_opt_below {
						if block.is_not_air {
							// The player is on the ground, so we make sure we are not overlapping it.
							player_phys.motion.z = 0.0;
							player_phys.aligned_box.pos.z = player_bottom_block_coords_below.z as f32
								+ 0.5 + player_phys.aligned_box.dims.z / 2.0;
							true
						} else {
							false
						}
					} else {
						false
					}
				} else {
					false
				};
				let is_in_ground = if player_phys.motion.z <= 0.0 {
					if let Some(block) = player_bottom_block_opt {
						if block.is_not_air {
							// The player is inside the ground, so we uuh.. do something?
							player_phys.motion.z = 0.0;
							player_phys.aligned_box.pos.z = player_bottom_block_coords.z as f32
								+ 0.5 + player_phys.aligned_box.dims.z / 2.0;
							true
						} else {
							false
						}
					} else {
						false
					}
				} else {
					false
				};
				player_phys.aligned_box.pos += player_phys.motion;
				if !is_on_ground {
					player_phys.motion.z -= player_phys.gravity_factor * 0.3 * dt.as_secs_f32();
				}
				if is_in_ground {
					player_phys.aligned_box.pos.z += 0.01;
				}
			}

			let player_box_mesh = SimpleLineMesh::from_aligned_box(&device, &player_phys.aligned_box);

			let first_person_camera_position = player_phys.aligned_box.pos
				+ cgmath::Vector3::<f32>::from((0.0, 0.0, player_phys.aligned_box.dims.z / 2.0)) * 0.7;

			// Targeted block coords update.
			let direction = camera_direction.to_vec3();
			let mut position = first_person_camera_position;
			let mut last_position_int: Option<BlockCoords> = None;
			targeted_block_coords = loop {
				if first_person_camera_position.distance(position) > 6.0 {
					break None;
				}
				let position_int = position.map(|x| x.round() as i32);
				if chunk_grid
					.get_block(position_int)
					.is_some_and(|block| block.is_not_air)
				{
					if let Some(last_position_int) = last_position_int {
						break Some((position_int, last_position_int));
					} else {
						break None;
					}
				}
				if last_position_int != Some(position_int) {
					last_position_int = Some(position_int);
				}
				// TODO: Advance directly to the next block with exactly the right step distance,
				// also do not skip blocks (even a small arbitrary step can be too big sometimes).
				position += direction * 0.01;
			};

			let targeted_block_box_mesh_opt = targeted_block_coords.map(|(coords, _)| {
				SimpleLineMesh::from_aligned_box(
					&device,
					&AlignedBox {
						pos: coords.map(|x| x as f32),
						dims: cgmath::vec3(1.01, 1.01, 1.01),
					},
				)
			});

			sun_position_in_sky.angle_horizontal += (TAU / 150.0) * dt.as_secs_f32();

			let sun_camera_view_projection_matrix = {
				let camera_position = first_person_camera_position;
				let camera_direction_vector = -sun_position_in_sky.to_vec3();
				let camera_up_vector = (0.0, 0.0, 1.0).into();
				sun_camera.view_projection_matrix(
					camera_position,
					camera_direction_vector,
					camera_up_vector,
				)
			};
			queue.write_buffer(
				&sun_camera_matrix_thingy.resource,
				0,
				bytemuck::cast_slice(&[sun_camera_view_projection_matrix]),
			);

			let camera_view_projection_matrix = {
				if use_sun_camera_to_render {
					sun_camera_view_projection_matrix
				} else {
					let mut camera_position = first_person_camera_position;
					let camera_direction_vector = camera_direction.to_vec3();
					if enable_camera_third_person {
						camera_position -= camera_direction_vector * 5.0;
					}
					let camera_up_vector = camera_direction.add_to_vertical_angle(-TAU / 4.0).to_vec3();
					camera.view_projection_matrix(
						camera_position,
						camera_direction_vector,
						camera_up_vector,
					)
				}
			};
			queue.write_buffer(
				&camera_matrix_thingy.resource,
				0,
				bytemuck::cast_slice(&[camera_view_projection_matrix]),
			);

			let sun_light_direction = Vector3Pod { values: (-sun_position_in_sky.to_vec3()).into() };
			queue.write_buffer(
				&sun_light_direction_thingy.resource,
				0,
				bytemuck::cast_slice(&[sun_light_direction]),
			);

			let cursor_mesh =
				SimpleLineMesh::interface_2d_cursor(&device, (config.width, config.height));

			let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
				label: Some("Render Encoder"),
			});

			// Render pass to generate the shadow map.
			{
				let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: Some("Render Pass for Shadow Map"),
					color_attachments: &[],
					depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
						view: &shadow_map_view_thingy.resource,
						depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: true }),
						stencil_ops: None,
					}),
				});

				render_pass.set_pipeline(&block_shadow_render_pipeline);
				render_pass.set_bind_group(0, &block_shadow_bind_group, &[]);
				for chunk in chunk_grid.map.values() {
					if let Some(ref mesh) = chunk.mesh {
						render_pass
							.set_vertex_buffer(0, mesh.block_vertex_buffer.as_ref().unwrap().slice(..));
						render_pass.draw(0..(mesh.block_vertices.len() as u32), 0..1);
					}
				}
			}

			// Render pass to render the world to the screen.
			let window_texture = window_surface.get_current_texture().unwrap();
			{
				let window_texture_view = window_texture
					.texture
					.create_view(&wgpu::TextureViewDescriptor::default());
				let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: Some("Render Pass to render the world"),
					color_attachments: &[Some(wgpu::RenderPassColorAttachment {
						view: &window_texture_view,
						resolve_target: None,
						ops: wgpu::Operations {
							load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.7, b: 1.0, a: 1.0 }),
							store: true,
						},
					})],
					depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
						view: &z_buffer_view,
						depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: true }),
						stencil_ops: None,
					}),
				});

				if use_sun_camera_to_render {
					let scale = config.height as f32 / sun_camera.height;
					let w = sun_camera.width * scale;
					let h = sun_camera.height * scale;
					let x = config.width as f32 / 2.0 - w / 2.0;
					let y = config.height as f32 / 2.0 - h / 2.0;
					render_pass.set_viewport(x, y, w, h, 0.0, 1.0);
				}

				render_pass.set_pipeline(&block_render_pipeline);
				render_pass.set_bind_group(0, &block_bind_group, &[]);
				for chunk in chunk_grid.map.values() {
					if let Some(ref mesh) = chunk.mesh {
						render_pass
							.set_vertex_buffer(0, mesh.block_vertex_buffer.as_ref().unwrap().slice(..));
						render_pass.draw(0..(mesh.block_vertices.len() as u32), 0..1);
					}
				}

				if enable_display_phys_box {
					render_pass.set_pipeline(&simple_line_render_pipeline);
					render_pass.set_bind_group(0, &simple_line_render_bind_group, &[]);
					render_pass.set_vertex_buffer(0, player_box_mesh.vertex_buffer.slice(..));
					render_pass.draw(0..(player_box_mesh.vertices.len() as u32), 0..1);
				}

				if let Some(targeted_block_box_mesh) = &targeted_block_box_mesh_opt {
					render_pass.set_pipeline(&simple_line_render_pipeline);
					render_pass.set_bind_group(0, &simple_line_render_bind_group, &[]);
					render_pass.set_vertex_buffer(0, targeted_block_box_mesh.vertex_buffer.slice(..));
					render_pass.draw(0..(targeted_block_box_mesh.vertices.len() as u32), 0..1);
				}
			}

			// Render pass to draw the interface.
			{
				let window_texture_view = window_texture
					.texture
					.create_view(&wgpu::TextureViewDescriptor::default());
				let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: Some("Render Pass to render "),
					color_attachments: &[Some(wgpu::RenderPassColorAttachment {
						view: &window_texture_view,
						resolve_target: None,
						ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: true },
					})],
					depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
						view: &z_buffer_view,
						depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: true }),
						stencil_ops: None,
					}),
				});

				render_pass.set_pipeline(&simple_line_2d_render_pipeline);
				if !use_sun_camera_to_render {
					render_pass.set_vertex_buffer(0, cursor_mesh.vertex_buffer.slice(..));
					render_pass.draw(0..(cursor_mesh.vertices.len() as u32), 0..1);
				}
			}

			queue.submit(std::iter::once(encoder.finish()));
			window_texture.present();
		},
		_ => {},
	});
}
