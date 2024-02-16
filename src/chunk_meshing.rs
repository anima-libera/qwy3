use std::sync::Arc;

use cgmath::{EuclideanSpace, InnerSpace};
use wgpu::util::DeviceExt;

use crate::{
	iter_3d_cube_center_radius, AxisOrientation, BitCube3, BitCube3Coords, BlockCoords, BlockType,
	BlockTypeTable, BlockVertexPod, ChunkBlocks, ChunkCoords, ChunkCoordsSpan, ChunkDimensions,
	ChunkGrid, NonOrientedAxis, OrientedAxis,
};

/// All the data that is needed to generate the mesh of a chunk.
pub(crate) struct DataForChunkMeshing {
	chunk_blocks: Arc<ChunkBlocks>,
	opaqueness_layer_for_face_culling: OpaquenessLayerAroundChunk,
	opaqueness_layer_for_ambiant_occlusion: OpaquenessLayerAroundChunk,
	block_type_table: Arc<BlockTypeTable>,
}

impl DataForChunkMeshing {
	pub(crate) fn generate_mesh(self) -> ChunkMesh {
		let is_opaque = |coords: BlockCoords, for_ambiant_occlusion: bool| {
			if let Some(block_id) = self.chunk_blocks.get(coords) {
				self.block_type_table.get(block_id).unwrap().is_opaque()
			} else if for_ambiant_occlusion {
				self.opaqueness_layer_for_ambiant_occlusion.get(coords).unwrap()
			} else {
				self.opaqueness_layer_for_face_culling.get(coords).unwrap()
			}
		};

		let mut block_vertices = Vec::new();
		for coords in self.chunk_blocks.coords_span.iter_coords() {
			let block_id = self.chunk_blocks.get(coords).unwrap();
			match self.block_type_table.get(block_id).unwrap() {
				BlockType::Air => {},
				BlockType::Solid { texture_coords_on_atlas } => {
					let opacity_bit_cube_3_for_ambiant_occlusion = {
						let mut cube = BitCube3::new_zero();
						for delta in iter_3d_cube_center_radius((0, 0, 0).into(), 2) {
							let neighbor_coords = coords + delta.to_vec();
							cube.set(delta.into(), is_opaque(neighbor_coords, true));
						}
						cube
					};
					for direction in OrientedAxis::all_the_six_possible_directions() {
						let is_covered_by_neighbor = {
							let neighbor_coords = coords + direction.delta();
							is_opaque(neighbor_coords, false)
						};
						if !is_covered_by_neighbor {
							generate_block_face_mesh(
								&mut block_vertices,
								direction,
								coords.map(|x| x as f32),
								opacity_bit_cube_3_for_ambiant_occlusion,
								*texture_coords_on_atlas,
							);
						}
					}
				},
				BlockType::XShaped { texture_coords_on_atlas } => {
					let opacity_bit_cube_3_for_ambiant_occlusion = {
						let mut cube = BitCube3::new_zero();
						for delta in iter_3d_cube_center_radius((0, 0, 0).into(), 2) {
							let neighbor_coords = coords + delta.to_vec();
							cube.set(delta.into(), is_opaque(neighbor_coords, true));
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
							opacity_bit_cube_3_for_ambiant_occlusion,
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

pub(crate) struct ChunkMesh {
	pub(crate) block_vertices: Vec<BlockVertexPod>,
	pub(crate) block_vertex_buffer: Option<wgpu::Buffer>,
	/// When `block_vertices` is modified, `block_vertex_buffer` becomes out of sync
	/// and must be updated. This is what this field keeps track of.
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

	pub(crate) fn update_gpu_data(&mut self, device: &wgpu::Device) {
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
	neighborhood_opaqueness_for_ambiant_occlusion: BitCube3,
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
	// We flip horizontally the texture for some face orientations so that
	// we don't observe a "mirror" effect on some vertical block edges.
	let order = if face_orientation
		== (OrientedAxis {
			axis: NonOrientedAxis::X,
			orientation: AxisOrientation::Positivewards,
		}) || face_orientation
		== (OrientedAxis {
			axis: NonOrientedAxis::Y,
			orientation: AxisOrientation::Negativewards,
		}) {
		[2, 3, 0, 1]
	} else {
		[0, 1, 2, 3]
	};
	coords_in_atlas_array[order[0]].x += texture_rect_in_atlas_wh.x * 0.0;
	coords_in_atlas_array[order[0]].y += texture_rect_in_atlas_wh.y * 0.0;
	coords_in_atlas_array[order[1]].x += texture_rect_in_atlas_wh.x * 0.0;
	coords_in_atlas_array[order[1]].y += texture_rect_in_atlas_wh.y * 1.0;
	coords_in_atlas_array[order[2]].x += texture_rect_in_atlas_wh.x * 1.0;
	coords_in_atlas_array[order[2]].y += texture_rect_in_atlas_wh.y * 0.0;
	coords_in_atlas_array[order[3]].x += texture_rect_in_atlas_wh.x * 1.0;
	coords_in_atlas_array[order[3]].y += texture_rect_in_atlas_wh.y * 1.0;

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
		let neighborhood_opaqueness = neighborhood_opaqueness_for_ambiant_occlusion;

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

	let normal = (offset_b - offset_a).extend(0.0).cross(cgmath::vec3(0.0, 0.0, -1.0)).normalize();

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

/// Information about the opaqueness of each block
/// contained in a 1-block-thick cubic layer around a chunk.
///
/// It is used for meshing of the chunk inside
/// (to get face culling and ambiant occlusion right on the edges)e
pub(crate) struct OpaquenessLayerAroundChunk {
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
		let sup: BlockCoords = self.surrounded_chunk_coords_span.block_coords_sup_excluded();
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

	fn get(&self, coords: BlockCoords) -> Option<bool> {
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

impl ChunkGrid {
	fn get_opaqueness_layer_around_chunk(
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

	pub(crate) fn get_data_for_chunk_meshing(
		&self,
		chunk_coords: ChunkCoords,
		block_type_table: Arc<BlockTypeTable>,
	) -> Option<DataForChunkMeshing> {
		let chunk_blocks = Arc::clone(self.blocks_map.get(&chunk_coords)?);
		let opaqueness_layer_for_face_culling =
			self.get_opaqueness_layer_around_chunk(chunk_coords, true, Arc::clone(&block_type_table));
		let opaqueness_layer_for_ambiant_occlusion =
			self.get_opaqueness_layer_around_chunk(chunk_coords, false, Arc::clone(&block_type_table));
		Some(DataForChunkMeshing {
			chunk_blocks,
			opaqueness_layer_for_face_culling,
			opaqueness_layer_for_ambiant_occlusion,
			block_type_table,
		})
	}
}
