use std::{
	collections::{HashMap, HashSet},
	io::{Read, Write},
	sync::Arc,
};

use cgmath::MetricSpace;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

use crate::{
	block_types::{BlockTypeId, BlockTypeTable},
	chunk_meshing::ChunkMesh,
	coords::{
		iter_3d_rect_inf_sup_included, BlockCoords, ChunkCoords, ChunkCoordsSpan, ChunkDimensions,
		CubicCoordsSpan, OrientedAxis,
	},
	entities::{ChunkEntities, Entity},
	font::Font,
	saves::Save,
	threadpool::ThreadPool,
	unsorted::CurrentWorkerTasks,
};

#[derive(Clone, Serialize, Deserialize)]
pub(crate) enum BlockData {
	Text(String),
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Block {
	pub(crate) type_id: BlockTypeId,
	pub(crate) data: Option<BlockData>,
}

impl From<BlockTypeId> for Block {
	fn from(type_id: BlockTypeId) -> Block {
		Block { type_id, data: None }
	}
}

impl Block {
	fn as_view(&self) -> BlockView<'_> {
		BlockView { type_id: self.type_id, data: self.data.as_ref() }
	}
}

pub(crate) struct BlockView<'a> {
	pub(crate) type_id: BlockTypeId,
	pub(crate) data: Option<&'a BlockData>,
}

/// The blocks of a chunk.
///
/// If no non-air block is ever placed in a `ChunkBlocks` then it never allocates memory.
#[derive(Clone)]
pub(crate) struct ChunkBlocks {
	pub(crate) coords_span: ChunkCoordsSpan,
	savable: ChunkBlocksSavable,
}
/// Part of `ChunkBlocks` that can be saved/loaded to/from disk.
#[derive(Clone, Serialize, Deserialize)]
struct ChunkBlocksSavable {
	/// If the length is zero then it means the chunk is full of air.
	block_ids: Vec<BlockTypeId>,
	/// Negative block ids are keys to this table.
	blocks_with_data: FxHashMap<BlockTypeId, Block>,
	/// Next available id number for blocks with data.
	next_id_for_block_with_data: i16,
	/// If the blocks ever underwent a change since the chunk generation, then it is flagged
	/// as `modified`. If we want to reduce the size of the saved data then we can avoid saving
	/// non-modified chunks as we could always re-generate them, but modified chunks must be saved.
	modified: bool,
}

impl ChunkBlocks {
	fn new_empty(coords_span: ChunkCoordsSpan) -> ChunkBlocks {
		ChunkBlocks {
			coords_span,
			savable: ChunkBlocksSavable {
				block_ids: vec![],
				blocks_with_data: HashMap::default(),
				next_id_for_block_with_data: -1,
				modified: false,
			},
		}
	}

	/// Get a view on a block, returns `None` if the given coords land outside the chunk's span.
	pub(crate) fn get(&self, coords: BlockCoords) -> Option<BlockView> {
		let internal_index = self.coords_span.internal_index(coords)?;
		Some(if self.savable.block_ids.is_empty() {
			BlockView { type_id: BlockTypeId { value: 0 }, data: None }
		} else {
			let block_id = self.savable.block_ids[internal_index];
			if block_id.value >= 0 {
				BlockView { type_id: block_id, data: None }
			} else {
				self.savable.blocks_with_data.get(&block_id).unwrap().as_view()
			}
		})
	}

	pub(crate) fn set_simple(&mut self, coords: BlockCoords, block_id: BlockTypeId) {
		if let Some(internal_index) = self.coords_span.internal_index(coords) {
			if self.savable.block_ids.is_empty() && block_id.value == 0 {
				// Setting a block to air, but we are already empty, there is no need to allocate.
			} else {
				if self.savable.block_ids.is_empty() && block_id.value != 0 {
					self.savable.block_ids = Vec::from_iter(
						std::iter::repeat(BlockTypeId { value: 0 })
							.take(self.coords_span.cd.number_of_blocks()),
					);
				}
				self.savable.block_ids[internal_index] = block_id;
			}
			self.savable.modified = true;
		}
	}

	pub(crate) fn set(&mut self, coords: BlockCoords, block: Block) {
		if self.coords_span.contains(coords) {
			if block.data.is_some() {
				let new_id = BlockTypeId { value: self.savable.next_id_for_block_with_data };
				self.savable.next_id_for_block_with_data -= 1;
				self.savable.blocks_with_data.insert(new_id, block);
				self.set_simple(coords, new_id);
			} else {
				self.set_simple(coords, block.type_id);
			}
			self.savable.modified = true;
		}
	}

	fn may_contain_non_air(&self) -> bool {
		!self.savable.block_ids.is_empty()
	}

	fn save(&self, save: &Arc<Save>) {
		// TODO: Use buffered streams instead of full vecs of data as intermediary steps.
		let chunk_file_path = save.chunk_file_path(self.coords_span.chunk_coords);
		let mut chunk_file = std::fs::File::create(chunk_file_path).unwrap();
		let uncompressed_data = rmp_serde::encode::to_vec(&self.savable).unwrap();
		let mut compressed_data = vec![];
		{
			let mut encoder = flate2::write::DeflateEncoder::new(
				&mut compressed_data,
				flate2::Compression::default(),
			);
			encoder.write_all(&uncompressed_data).unwrap();
		}
		chunk_file.write_all(&compressed_data).unwrap();
	}

	pub(crate) fn load_from_save(
		coords_span: ChunkCoordsSpan,
		save: &Arc<Save>,
	) -> Option<ChunkBlocks> {
		// TODO: Use buffered streams instead of full vecs of data as intermediary steps.
		let chunk_file_path = save.chunk_file_path(coords_span.chunk_coords);
		let mut chunk_file = std::fs::File::open(chunk_file_path).ok()?;
		let mut compressed_data = vec![];
		chunk_file.read_to_end(&mut compressed_data).unwrap();
		let mut uncompressed_data = vec![];
		{
			let mut decoder = flate2::bufread::DeflateDecoder::new(compressed_data.as_slice());
			decoder.read_to_end(&mut uncompressed_data).unwrap();
		}
		let savable: ChunkBlocksSavable = rmp_serde::decode::from_slice(&uncompressed_data).unwrap();
		Some(ChunkBlocks { coords_span, savable })
	}
}

/// Wrapper around `ChunkBlocks` to be used for generating chunk blocks.
/// It ensures that even after modifying the chunk blocks (in the process of generating it)
/// the resulting `ChunkBlocks` will not be flagged as `modified`.
pub(crate) struct ChunkBlocksBeingGenerated(ChunkBlocks);

impl ChunkBlocksBeingGenerated {
	pub(crate) fn new_empty(coords_span: ChunkCoordsSpan) -> ChunkBlocksBeingGenerated {
		ChunkBlocksBeingGenerated(ChunkBlocks::new_empty(coords_span))
	}

	pub(crate) fn coords_span(&self) -> ChunkCoordsSpan {
		self.0.coords_span
	}
	pub(crate) fn get(&self, coords: BlockCoords) -> Option<BlockView> {
		self.0.get(coords)
	}
	pub(crate) fn set_simple(&mut self, coords: BlockCoords, block_id: BlockTypeId) {
		self.0.set_simple(coords, block_id);
	}
	pub(crate) fn _set(&mut self, coords: BlockCoords, block: Block) {
		self.0.set(coords, block);
	}

	pub(crate) fn finish_generation(mut self) -> ChunkBlocks {
		self.0.savable.modified = false;
		self.0
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
		block_type_table: &Arc<BlockTypeTable>,
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
		for coords in blocks.coords_span.iter_coords() {
			let block = blocks.get(coords).unwrap();
			let block_type = block_type_table.get(block.type_id).unwrap();
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
			if ChunkCullingInfo::face_is_all_opaque(face, blocks, block_type_table) {
				all_opaque_faces.push(face);
			}
		}

		let mut all_air_faces = vec![];
		for face in OrientedAxis::all_the_six_possible_directions() {
			if ChunkCullingInfo::face_is_all_air(face, blocks, block_type_table) {
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
			let block_type_id = blocks.get(block_coords).unwrap().type_id;
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
			let block_type_id = blocks.get(block_coords).unwrap().type_id;
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
	cd: ChunkDimensions,
	blocks_map: FxHashMap<ChunkCoords, Arc<ChunkBlocks>>,
	culling_info_map: FxHashMap<ChunkCoords, ChunkCullingInfo>,
	mesh_map: FxHashMap<ChunkCoords, ChunkMesh>,
	remeshing_required_set: FxHashSet<ChunkCoords>,
	entities_map: FxHashMap<ChunkCoords, ChunkEntities>,
}

impl ChunkGrid {
	pub(crate) fn new(cd: ChunkDimensions) -> ChunkGrid {
		ChunkGrid {
			cd,
			blocks_map: HashMap::default(),
			culling_info_map: HashMap::default(),
			mesh_map: HashMap::default(),
			remeshing_required_set: HashSet::default(),
			entities_map: HashMap::default(),
		}
	}

	pub(crate) fn cd(&self) -> ChunkDimensions {
		self.cd
	}

	pub(crate) fn is_loaded(&self, chunk_coords: ChunkCoords) -> bool {
		self.blocks_map.contains_key(&chunk_coords)
	}

	pub(crate) fn iter_loaded_chunk_coords(&self) -> impl Iterator<Item = ChunkCoords> + '_ {
		self.blocks_map.keys().copied()
	}

	pub(crate) fn get_chunk_blocks(&self, chunk_coords: ChunkCoords) -> Option<&Arc<ChunkBlocks>> {
		self.blocks_map.get(&chunk_coords)
	}

	pub(crate) fn require_remeshing(&mut self, chunk_coords: ChunkCoords) {
		if self.is_loaded(chunk_coords) {
			self.remeshing_required_set.insert(chunk_coords);
		}
	}

	pub(crate) fn run_some_required_remeshing_tasks(
		&mut self,
		worker_tasks: &mut CurrentWorkerTasks,
		pool: &mut ThreadPool,
		block_type_table: &Arc<BlockTypeTable>,
		font: &Arc<Font>,
		device: &Arc<wgpu::Device>,
	) {
		let mut remeshing_tasked = vec![];
		for chunk_coords in self.remeshing_required_set.iter().copied() {
			if worker_tasks.tasks.len() >= pool.number_of_workers() {
				break;
			}

			let is_being_meshed = worker_tasks.is_being_meshed(chunk_coords);
			let doesnt_need_mesh = self
				.culling_info_map
				.get(&chunk_coords)
				.is_some_and(|culling_info| culling_info.all_air)
				|| !self.is_loaded(chunk_coords);

			if doesnt_need_mesh {
				remeshing_tasked.push(chunk_coords);
			} else if is_being_meshed {
				// Let the request be, it will be remeshed later.
				// We wait for this chunk because it is already being meshed (from some past state)
				// and we may not want to clog up the task queue with remeshing tasks for one chunk.
			} else {
				// Asking a worker for the meshing or remeshing of the chunk.
				remeshing_tasked.push(chunk_coords);
				let data_for_chunk_meshing = self
					.get_data_for_chunk_meshing(
						chunk_coords,
						Arc::clone(block_type_table),
						Arc::clone(font),
					)
					.unwrap();
				worker_tasks.run_chunk_meshing_task(
					pool,
					chunk_coords,
					data_for_chunk_meshing,
					Arc::clone(device),
				);
			}
		}
		for chunk_coords in remeshing_tasked {
			self.remeshing_required_set.remove(&chunk_coords);
		}
	}

	pub(crate) fn count_chunks_that_have_meshes(&self) -> usize {
		self.mesh_map.len()
	}

	pub(crate) fn iter_chunk_meshes(&self) -> impl Iterator<Item = &ChunkMesh> + '_ {
		self.mesh_map.values()
	}

	pub(crate) fn add_chunk_meshing_results(
		&mut self,
		chunk_coords: ChunkCoords,
		chunk_mesh: ChunkMesh,
	) {
		if self.is_loaded(chunk_coords) {
			self.mesh_map.insert(chunk_coords, chunk_mesh);
		} else {
			// The chunk have been unloaded since the meshing was ordered.
			// It really can happen, for example when the player travels very fast.
		}
	}

	fn set_block_but_do_not_update_meshes(&mut self, coords: BlockCoords, block: Block) {
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
		block: Block,
	) {
		self.set_block_but_do_not_update_meshes(coords, block);

		// Request a mesh update in all the chunks that the block touches (even with vertices),
		// so all the chunks that contain any of the blocks in the 3x3x3 blocks cube around.
		let block_span = CubicCoordsSpan::with_center_and_radius(coords, 2);
		let chunk_inf = self.cd.world_coords_to_containing_chunk_coords(block_span.inf);
		let chunk_sup_included =
			self.cd.world_coords_to_containing_chunk_coords(block_span.sup_included());
		for chunk_coords in iter_3d_rect_inf_sup_included(chunk_inf, chunk_sup_included) {
			self.require_remeshing(chunk_coords);
		}
	}

	pub(crate) fn get_block(&self, coords: BlockCoords) -> Option<BlockView> {
		let chunk_coords = self.cd.world_coords_to_containing_chunk_coords(coords);
		let chunk_blocks = self.blocks_map.get(&chunk_coords)?;
		Some(chunk_blocks.get(coords).unwrap())
	}

	pub(crate) fn count_chunks_that_have_blocks(&self) -> usize {
		self.blocks_map.len()
	}

	pub(crate) fn apply_one_physics_step(
		&mut self,
		block_type_table: &Arc<BlockTypeTable>,
		dt: std::time::Duration,
	) {
		let chunk_coords_list: Vec<_> = self.entities_map.keys().copied().collect();
		for chunk_coords in chunk_coords_list.into_iter() {
			let mut chunk_entities = self.entities_map.remove(&chunk_coords).unwrap();
			chunk_entities.apply_one_physics_step(self, block_type_table, dt);
			if chunk_entities.count_entities() > 0 {
				let chunk_entities_that_took_the_place =
					self.entities_map.insert(chunk_coords, chunk_entities);
				assert!(
					chunk_entities_that_took_the_place.is_none(),
					"What to do in this situation? Merge them?"
				);
			} else {
				// The chunk is now devoid of entities, it doesn't need a `ChunkEntities` anymore.
			}
		}
	}

	pub(crate) fn spawn_entity(&mut self, entity: Entity) {
		let coords = entity.pos().map(|x| x.round() as i32);
		let chunk_coords = self.cd.world_coords_to_containing_chunk_coords(coords);
		let coords_span = ChunkCoordsSpan { cd: self.cd, chunk_coords };
		self
			.entities_map
			.entry(chunk_coords)
			.or_insert(ChunkEntities::new_empty(coords_span))
			.spawn_entity(entity);
	}

	pub(crate) fn iter_entities(&self) -> impl Iterator<Item = &Entity> {
		self.entities_map.values().flat_map(|chunk_entities| chunk_entities.iter_entities())
	}

	pub(crate) fn count_entities_and_chunks_that_have_entities(&self) -> (usize, usize) {
		let chunks_that_have_entities_count = self.entities_map.len();
		let mut entities_count = 0;
		for chunk_entities in self.entities_map.values() {
			entities_count += chunk_entities.count_entities();
		}
		(entities_count, chunks_that_have_entities_count)
	}

	pub(crate) fn add_chunk_loading_results(
		&mut self,
		chunk_coords: ChunkCoords,
		chunk_blocks: ChunkBlocks,
		chunk_culling_info: ChunkCullingInfo,
	) {
		self.blocks_map.insert(chunk_coords, Arc::new(chunk_blocks));
		self.culling_info_map.insert(chunk_coords, chunk_culling_info);
	}

	fn unload_chunk(
		&mut self,
		chunk_coords: ChunkCoords,
		save: Option<&Arc<Save>>,
		only_save_modified_chunks: bool,
	) {
		let chunk_blocks = self.blocks_map.remove(&chunk_coords);
		if let Some(chunk_blocks) = chunk_blocks {
			if let Some(save) = save {
				if !only_save_modified_chunks || chunk_blocks.savable.modified {
					chunk_blocks.save(save);
				}
			}
		}
		self.culling_info_map.remove(&chunk_coords);
		self.mesh_map.remove(&chunk_coords);
		self.remeshing_required_set.remove(&chunk_coords);
	}

	pub(crate) fn unload_chunks_too_far(
		&mut self,
		player_chunk_coords: ChunkCoords,
		unloading_distance_in_blocks: f32,
		save: Option<&Arc<Save>>,
		only_save_modified_chunks: bool,
	) {
		let unloading_distance_in_chunks = unloading_distance_in_blocks / self.cd.edge as f32;
		// TODO: Avoid copying all the keys in a vector and iterating over all the chunks every frame.
		let chunk_coords_list: Vec<_> = self.blocks_map.keys().copied().collect();
		for chunk_coords in chunk_coords_list.into_iter() {
			let dist_in_chunks =
				chunk_coords.map(|x| x as f32).distance(player_chunk_coords.map(|x| x as f32));
			if dist_in_chunks > unloading_distance_in_chunks {
				self.unload_chunk(chunk_coords, save, only_save_modified_chunks);
			}
		}
	}

	pub(crate) fn unload_all_chunks(
		&mut self,
		save: Option<&Arc<Save>>,
		only_save_modified_chunks: bool,
	) {
		let chunk_coords_list: Vec<_> = self.blocks_map.keys().copied().collect();
		for chunk_coords in chunk_coords_list.into_iter() {
			self.unload_chunk(chunk_coords, save, only_save_modified_chunks);
		}
	}
}
