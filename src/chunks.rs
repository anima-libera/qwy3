use std::{
	collections::{HashMap, HashSet},
	sync::Arc,
};

use cgmath::EuclideanSpace;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk_meshing::ChunkMesh;
pub(crate) use crate::{
	block_types::{BlockType, BlockTypeId, BlockTypeTable},
	coords::{
		iter_3d_cube_center_radius, BlockCoords, ChunkCoords, ChunkCoordsSpan, ChunkDimensions,
		OrientedAxis,
	},
	shaders::block::BlockVertexPod,
};

/// The blocks of a chunk.
///
/// If no non-air block is ever placed in a `ChunkBlocks` then it never allocates memory.
#[derive(Clone)]
pub(crate) struct ChunkBlocks {
	pub(crate) coords_span: ChunkCoordsSpan,
	/// If the length is zero then it means the chunk is full of air.
	blocks: Vec<BlockTypeId>,
}

impl ChunkBlocks {
	pub(crate) fn new_empty(coords_span: ChunkCoordsSpan) -> ChunkBlocks {
		ChunkBlocks { coords_span, blocks: vec![] }
	}

	pub(crate) fn get(&self, coords: BlockCoords) -> Option<BlockTypeId> {
		let internal_index = self.coords_span.internal_index(coords)?;
		Some(if self.blocks.is_empty() {
			BlockTypeId { value: 0 }
		} else {
			self.blocks[internal_index]
		})
	}

	pub(crate) fn set(&mut self, coords: BlockCoords, block: BlockTypeId) {
		if let Some(internal_index) = self.coords_span.internal_index(coords) {
			if self.blocks.is_empty() && block.value == 0 {
				// Setting a block to air, but we are already empty, there is no need to allocate.
			} else {
				if self.blocks.is_empty() && block.value != 0 {
					self.blocks = Vec::from_iter(
						std::iter::repeat(BlockTypeId { value: 0 })
							.take(self.coords_span.cd.number_of_blocks()),
					);
				}
				self.blocks[internal_index] = block;
			}
		}
	}

	fn may_contain_non_air(&self) -> bool {
		!self.blocks.is_empty()
	}
}

/// Information that can be used to decide if some chunks should not be loaded or be unloaded.
#[derive(Clone)]
pub(crate) struct ChunkCullingInfo {
	pub(crate) all_air: bool,
	pub(crate) _all_opaque: bool,
	pub(crate) all_opaque_faces: Vec<OrientedAxis>,
	pub(crate) all_air_faces: Vec<OrientedAxis>,
}

impl ChunkCullingInfo {
	pub(crate) fn compute_from_blocks(
		blocks: &ChunkBlocks,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkCullingInfo {
		if !blocks.may_contain_non_air() {
			return ChunkCullingInfo {
				all_air: true,
				_all_opaque: false,
				all_opaque_faces: vec![],
				all_air_faces: OrientedAxis::all_the_six_possible_directions().collect(),
			};
		}

		let mut all_air = true;
		let mut all_opaque = true;
		for block_type_id in blocks.blocks.iter().copied() {
			let block_type = block_type_table.get(block_type_id).unwrap();
			if !block_type.is_air() {
				all_air = false;
			}
			if !block_type.is_opaque() {
				all_opaque = false;
			}
			if (!all_air) && (!all_opaque) {
				break;
			}
		}

		let mut all_opaque_faces = vec![];
		for face in OrientedAxis::all_the_six_possible_directions() {
			if ChunkCullingInfo::face_is_all_opaque(face, blocks, &block_type_table) {
				all_opaque_faces.push(face);
			}
		}

		let mut all_air_faces = vec![];
		for face in OrientedAxis::all_the_six_possible_directions() {
			if ChunkCullingInfo::face_is_all_air(face, blocks, &block_type_table) {
				all_air_faces.push(face);
			}
		}

		ChunkCullingInfo { all_air, _all_opaque: all_opaque, all_opaque_faces, all_air_faces }
	}

	fn face_is_all_opaque(
		face: OrientedAxis,
		blocks: &ChunkBlocks,
		block_type_table: &Arc<BlockTypeTable>,
	) -> bool {
		let mut all_opaque = true;
		for block_coords in blocks.coords_span.iter_block_coords_on_chunk_face(face) {
			let block_type_id = blocks.get(block_coords).unwrap();
			let block_type = block_type_table.get(block_type_id).unwrap();
			if !block_type.is_opaque() {
				all_opaque = false;
				break;
			}
		}
		all_opaque
	}

	fn face_is_all_air(
		face: OrientedAxis,
		blocks: &ChunkBlocks,
		block_type_table: &Arc<BlockTypeTable>,
	) -> bool {
		let mut all_air = true;
		for block_coords in blocks.coords_span.iter_block_coords_on_chunk_face(face) {
			let block_type_id = blocks.get(block_coords).unwrap();
			let block_type = block_type_table.get(block_type_id).unwrap();
			if !block_type.is_air() {
				all_air = false;
				break;
			}
		}
		all_air
	}
}

pub(crate) struct ChunkGrid {
	pub(crate) cd: ChunkDimensions,
	pub(crate) blocks_map: FxHashMap<ChunkCoords, Arc<ChunkBlocks>>,
	pub(crate) culling_info_map: FxHashMap<ChunkCoords, ChunkCullingInfo>,
	pub(crate) mesh_map: FxHashMap<ChunkCoords, ChunkMesh>,
	pub(crate) remeshing_required_set: FxHashSet<ChunkCoords>,
}

impl ChunkGrid {
	pub(crate) fn new(cd: ChunkDimensions) -> ChunkGrid {
		ChunkGrid {
			cd,
			blocks_map: HashMap::default(),
			culling_info_map: HashMap::default(),
			mesh_map: HashMap::default(),
			remeshing_required_set: HashSet::default(),
		}
	}

	pub(crate) fn is_loaded(&self, chunk_coords: ChunkCoords) -> bool {
		self.blocks_map.contains_key(&chunk_coords)
	}

	fn set_block_but_do_not_update_meshes(&mut self, coords: BlockCoords, block: BlockTypeId) {
		let chunk_coords = self.cd.world_coords_to_containing_chunk_coords(coords);
		if !self.is_loaded(chunk_coords) {
			// TODO: Handle this case by storing the fact that a block
			// has to be set when loding the chunk.
			unimplemented!();
		} else {
			let chunk_blocks_arc = self.blocks_map.remove(&chunk_coords).unwrap();
			let mut chunk_blocks = Arc::unwrap_or_clone(chunk_blocks_arc);
			chunk_blocks.set(coords, block);
			self.blocks_map.insert(chunk_coords, Arc::new(chunk_blocks));

			// "Clear out" now maybe-invalidated culling info.
			self.culling_info_map.remove(&chunk_coords);
		}
	}

	pub(crate) fn set_block_and_request_updates_to_meshes(
		&mut self,
		coords: BlockCoords,
		block: BlockTypeId,
	) {
		self.set_block_but_do_not_update_meshes(coords, block);

		// Request a mesh update in all the chunks that the block touches.
		let mut chunk_coords_to_update = vec![];
		for delta in iter_3d_cube_center_radius((0, 0, 0).into(), 2) {
			let neighbor_coords = coords + delta.to_vec();
			let chunk_coords = self.cd.world_coords_to_containing_chunk_coords(neighbor_coords);
			chunk_coords_to_update.push(chunk_coords);
		}
		for chunk_coords in chunk_coords_to_update {
			if self.is_loaded(chunk_coords) {
				self.remeshing_required_set.insert(chunk_coords);
			}
		}
	}

	pub(crate) fn get_block(&self, coords: BlockCoords) -> Option<BlockTypeId> {
		let chunk_coords = self.cd.world_coords_to_containing_chunk_coords(coords);
		let chunk_blocks = self.blocks_map.get(&chunk_coords)?;
		Some(chunk_blocks.get(coords).unwrap())
	}
}
