use std::{
	collections::{hash_map::Entry, HashMap, HashSet},
	sync::Arc,
};

use cgmath::MetricSpace;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
	block_types::BlockTypeTable,
	chunk_blocks::{Block, BlockView, ChunkBlocks, ChunkCullingInfo},
	chunk_meshing::ChunkMesh,
	coords::{
		iter_3d_rect_inf_sup_included, BlockCoords, ChunkCoords, ChunkCoordsSpan, ChunkDimensions,
		CubicCoordsSpan,
	},
	entities::{ChunkEntities, Entity, ForPartManipulation},
	entity_parts::PartTables,
	font::Font,
	saves::Save,
	threadpool::ThreadPool,
	unsorted::CurrentWorkerTasks,
};

pub(crate) struct ChunkGrid {
	cd: ChunkDimensions,
	/// The block data for each loaded chunk.
	blocks_map: FxHashMap<ChunkCoords, Arc<ChunkBlocks>>,
	/// The culling data for each loaded chunk that hadn't underwent modification since loading.
	// TODO: Remove it? This map is never used.
	culling_info_map: FxHashMap<ChunkCoords, ChunkCullingInfo>,
	/// The mesh for each chunk that needs one.
	mesh_map: FxHashMap<ChunkCoords, ChunkMesh>,
	/// The chunks that should be checked for remeshing.
	remeshing_required_set: FxHashSet<ChunkCoords>,
	/// The entities in chunks, for each chunk that has some.
	entities_map: FxHashMap<ChunkCoords, ChunkEntities>,
	/// The chunks that were already generated once
	/// (and thus shall not have their entities generated again).
	generated_set: FxHashSet<ChunkCoords>,
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
			generated_set: HashSet::default(),
		}
	}

	pub(crate) fn cd(&self) -> ChunkDimensions {
		self.cd
	}

	pub(crate) fn is_loaded(&self, chunk_coords: ChunkCoords) -> bool {
		self.blocks_map.contains_key(&chunk_coords)
	}

	pub(crate) fn was_already_generated_before(&self, chunk_coords: ChunkCoords) -> bool {
		self.generated_set.contains(&chunk_coords)
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
		let mut remeshing_request_handled = vec![];
		for chunk_coords in self.remeshing_required_set.iter().copied() {
			if worker_tasks.tasks.len() >= pool.number_of_workers() {
				break;
			}

			let is_only_air =
				self.blocks_map.get(&chunk_coords).is_some_and(|blocks| blocks.contains_only_air());
			let has_mesh = self.mesh_map.contains_key(&chunk_coords);
			let doesnt_need_mesh = (is_only_air && !has_mesh) || !self.is_loaded(chunk_coords);
			let is_being_meshed = worker_tasks.is_being_meshed(chunk_coords);

			if doesnt_need_mesh {
				remeshing_request_handled.push(chunk_coords);
			} else if is_being_meshed {
				// Let the request be, it will be remeshed later.
				// We wait for this chunk because it is already being meshed (from some past state)
				// and we may not want to clog up the task queue with remeshing tasks for one chunk.
			} else {
				// Asking a worker for the meshing or remeshing of the chunk.
				remeshing_request_handled.push(chunk_coords);
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
		for chunk_coords in remeshing_request_handled {
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
		chunk_mesh: Option<ChunkMesh>,
	) {
		if self.is_loaded(chunk_coords) {
			if let Some(chunk_mesh) = chunk_mesh {
				self.mesh_map.insert(chunk_coords, chunk_mesh);
			} else {
				self.mesh_map.remove(&chunk_coords);
			}
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
		save: Option<&Arc<Save>>,
		part_manipulation: &mut ForPartManipulation,
	) {
		let chunk_coords_list: Vec<_> = self.entities_map.keys().copied().collect();
		let mut changes_of_chunk = vec![];
		for chunk_coords in chunk_coords_list.into_iter() {
			if !self.is_loaded(chunk_coords) {
				continue;
			}
			let mut chunk_entities = self.entities_map.remove(&chunk_coords).unwrap();
			chunk_entities.apply_one_physics_step(
				self,
				block_type_table,
				dt,
				&mut changes_of_chunk,
				part_manipulation,
			);
			if chunk_entities.count_entities() > 0 {
				self.add_chunk_entities(chunk_entities);
			} else {
				// The chunk is now devoid of entities, it doesn't need a `ChunkEntities` anymore.
			}
		}
		// The entities that got out of their chunks are now put in their new chunks.
		for change_of_chunk in changes_of_chunk.into_iter() {
			self.put_entity_in_chunk(change_of_chunk.new_chunk, change_of_chunk.entity, save);
		}
	}

	/// To insert or re-insert a `ChunkEntities` in the map, using this method ensures that
	/// if the chunk already had a `ChunkEntities` then it is merged with the one given here.
	fn add_chunk_entities(&mut self, chunk_entities: ChunkEntities) {
		let chunk_coords = chunk_entities.coords_span.chunk_coords;
		let entry = self.entities_map.entry(chunk_coords);
		match entry {
			Entry::Occupied(mut occupied) => {
				occupied.get_mut().merge_to(chunk_entities);
			},
			Entry::Vacant(vacant) => {
				vacant.insert(chunk_entities);
			},
		}
	}

	fn put_entity_in_chunk(
		&mut self,
		chunk_coords: ChunkCoords,
		entity: Entity,
		save: Option<&Arc<Save>>,
	) {
		let coords_span = ChunkCoordsSpan { cd: self.cd, chunk_coords };
		self
			.entities_map
			.entry(chunk_coords)
			.or_insert_with(|| {
				save
					.and_then(|save| {
						ChunkEntities::load_from_save_while_removing_the_save(coords_span, save)
					})
					.unwrap_or(ChunkEntities::new_empty(coords_span))
			})
			.add_entity(entity);
	}

	pub(crate) fn add_entity(&mut self, entity: Entity, save: Option<&Arc<Save>>) {
		let coords = entity.pos().map(|x| x.round() as i32);
		let chunk_coords = self.cd.world_coords_to_containing_chunk_coords(coords);
		self.put_entity_in_chunk(chunk_coords, entity, save);
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

	pub(crate) fn iter_chunk_with_entities_coords(&self) -> impl Iterator<Item = ChunkCoords> + '_ {
		self.entities_map.keys().copied()
	}

	pub(crate) fn add_chunk_loading_results(
		&mut self,
		chunk_coords: ChunkCoords,
		chunk_blocks: ChunkBlocks,
		chunk_culling_info: ChunkCullingInfo,
		chunk_entities: Option<ChunkEntities>,
	) {
		self.blocks_map.insert(chunk_coords, Arc::new(chunk_blocks));
		self.culling_info_map.insert(chunk_coords, chunk_culling_info);
		if let Some(chunk_entities) = chunk_entities {
			self.add_chunk_entities(chunk_entities);
		}
		self.generated_set.insert(chunk_coords);
	}

	fn unload_chunk(
		&mut self,
		chunk_coords: ChunkCoords,
		save: Option<&Arc<Save>>,
		only_save_modified_chunks: bool,
		part_tables: &mut PartTables,
	) {
		let chunk_blocks = self.blocks_map.remove(&chunk_coords);
		let chunk_entities = self.entities_map.remove(&chunk_coords);
		if let Some(save) = save {
			if let Some(chunk_blocks) = chunk_blocks {
				if !only_save_modified_chunks || chunk_blocks.was_modified_since_generation() {
					chunk_blocks.save(save);
				}
			}
			if let Some(chunk_entities) = &chunk_entities {
				chunk_entities.save(save);
			}
		}
		if let Some(chunk_entities) = chunk_entities {
			chunk_entities.handle_unloading(part_tables);
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
		part_tables: &mut PartTables,
	) {
		let unloading_distance_in_chunks = unloading_distance_in_blocks / self.cd.edge as f32;
		// TODO: Avoid copying all the keys in a vector and iterating over all the chunks every frame.
		let chunk_coords_list: Vec<_> =
			self.blocks_map.keys().copied().chain(self.entities_map.keys().copied()).collect();
		for chunk_coords in chunk_coords_list.into_iter() {
			let dist_in_chunks =
				chunk_coords.map(|x| x as f32).distance(player_chunk_coords.map(|x| x as f32));
			if dist_in_chunks > unloading_distance_in_chunks {
				self.unload_chunk(chunk_coords, save, only_save_modified_chunks, part_tables);
			}
		}
	}

	pub(crate) fn unload_all_chunks(
		&mut self,
		save: Option<&Arc<Save>>,
		only_save_modified_chunks: bool,
		part_tables: &mut PartTables,
	) {
		let chunk_coords_list: Vec<_> = self.blocks_map.keys().copied().collect();
		for chunk_coords in chunk_coords_list.into_iter() {
			self.unload_chunk(chunk_coords, save, only_save_modified_chunks, part_tables);
		}
	}
}
