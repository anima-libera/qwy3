use std::{
	collections::{hash_map::Entry, HashMap, HashSet},
	sync::Arc,
	time::Duration,
};

use cgmath::{EuclideanSpace, MetricSpace};
use fxhash::{FxHashMap, FxHashSet};

use crate::{
	block_types::BlockTypeTable,
	chunk_blocks::{Block, BlockView, ChunkBlocks, ChunkCullingInfo},
	chunk_meshing::ChunkMesh,
	coords::{
		iter_3d_cube_center_radius, iter_3d_rect_inf_sup_included, AlignedBox, BlockCoords,
		ChunkCoords, ChunkCoordsSpan, ChunkDimensions, CubicCoordsSpan,
	},
	entities::{
		ChunkEntities, EntitiesPhysicsStepCollector, EntitiesPhysicsStepResult, Entity,
		ForPartManipulation, IdGenerator,
	},
	entity_parts::PartTables,
	font::Font,
	saves::Save,
	tasks::WorkerTasksManager,
	threadpool::ThreadPool,
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
	/// (and thus that shall not have their entities generated again).
	already_generated_set: FxHashSet<ChunkCoords>,
}

impl ChunkGrid {
	pub(crate) fn new(
		cd: ChunkDimensions,
		already_generated_set: Option<FxHashSet<ChunkCoords>>,
	) -> ChunkGrid {
		ChunkGrid {
			cd,
			blocks_map: HashMap::default(),
			culling_info_map: HashMap::default(),
			mesh_map: HashMap::default(),
			remeshing_required_set: HashSet::default(),
			entities_map: HashMap::default(),
			already_generated_set: already_generated_set.unwrap_or_default(),
		}
	}

	pub(crate) fn cd(&self) -> ChunkDimensions {
		self.cd
	}

	pub(crate) fn is_loaded(&self, chunk_coords: ChunkCoords) -> bool {
		self.blocks_map.contains_key(&chunk_coords)
	}

	pub(crate) fn was_already_generated_before(&self, chunk_coords: ChunkCoords) -> bool {
		self.already_generated_set.contains(&chunk_coords)
	}
	pub(crate) fn set_of_already_generated_chunks(&self) -> &FxHashSet<ChunkCoords> {
		&self.already_generated_set
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
		worker_tasks: &mut WorkerTasksManager,
		pool: &mut ThreadPool,
		block_type_table: &Arc<BlockTypeTable>,
		font: &Arc<Font>,
		device: &Arc<wgpu::Device>,
	) {
		let mut remeshing_request_handled = vec![];
		for chunk_coords in self.remeshing_required_set.iter().copied() {
			let meshing_workers_available =
				worker_tasks.how_many_meshing_compatible_workers_available(pool);
			if meshing_workers_available == 0 {
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

	fn run_entities_tasks(
		self_arc: &Arc<ChunkGrid>,
		worker_tasks: &mut WorkerTasksManager,
		pool: &mut ThreadPool,
		block_type_table: &Arc<BlockTypeTable>,
		dt: std::time::Duration,
		part_manipulation: ForPartManipulation,
		id_generator: &Arc<IdGenerator>,
	) -> EntitiesPhysicsStepCollector {
		let mut chunk_entities_to_run = vec![];
		let mut chunk_entities_to_preserve = vec![];
		for chunk_coords in self_arc.entities_map.keys().copied() {
			if self_arc.is_loaded(chunk_coords) {
				chunk_entities_to_run.push(chunk_coords);
			} else {
				chunk_entities_to_preserve.push(chunk_coords);
			}
		}

		let number_of_tasks = 1;

		worker_tasks.run_physics_step_on_some_entities(
			pool,
			chunk_entities_to_run,
			self_arc.cd,
			self_arc,
			block_type_table,
			dt,
			part_manipulation,
			id_generator,
		);

		EntitiesPhysicsStepCollector::new(
			number_of_tasks,
			chunk_entities_to_preserve,
			HashMap::default(),
			vec![],
		)
	}

	pub(crate) fn apply_completed_entities_step(
		&mut self,
		completed_step: EntitiesPhysicsStepCollector,
		save: Option<&Arc<Save>>,
		id_generator: &IdGenerator,
	) {
		let (mut next_entities_map, actions_on_world, chunk_entities_to_preserve) =
			completed_step.into_complete_result();

		std::mem::swap(&mut self.entities_map, &mut next_entities_map);
		let mut old_entities_map = next_entities_map;

		for chunk_coords in chunk_entities_to_preserve {
			let chunk_entities = old_entities_map.remove(&chunk_coords).unwrap();
			self.add_chunk_entities(chunk_entities);
		}

		for action_on_world in actions_on_world.into_iter() {
			self.apply_actions_on_world(action_on_world, save, id_generator);
		}
	}

	pub(crate) fn can_entity_in_chunk_maybe_collide_with_box(
		&self,
		chunk_coords: ChunkCoords,
		aligned_box: &AlignedBox,
	) -> bool {
		self.entities_map.get(&chunk_coords).is_some_and(|entity_chunk| {
			let max_entity_dims = entity_chunk.max_entity_dims();
			let chunk_dims = self.cd.dimensions().map(|x| x as f32);
			let chunk_span = ChunkCoordsSpan { cd: self.cd, chunk_coords };
			let chunk_center_coords = chunk_span
				.block_coords_inf()
				.map(|x| x as f32 - 0.5)
				.midpoint(chunk_span.block_coords_sup_excluded().map(|x| x as f32 - 0.5));
			let chunk_aligned_box = AlignedBox { pos: chunk_center_coords, dims: chunk_dims };
			let chunk_max_entity_reach_box = {
				let mut reach_box = chunk_aligned_box;
				reach_box.dims += max_entity_dims;
				reach_box
			};
			aligned_box.overlaps(&chunk_max_entity_reach_box)
		})
	}

	fn apply_actions_on_world(
		&mut self,
		action_on_world: ActionOnWorld,
		save: Option<&Arc<Save>>,
		id_generator: &IdGenerator,
	) {
		match action_on_world {
			ActionOnWorld::PlaceBlockWithoutLoss { block, coords } => {
				if self
					.get_block(coords)
					.is_some_and(|replaced_block| replaced_block.type_id == BlockTypeTable::AIR_ID)
				{
					// The placed block replaces air,
					// it can be placed without any non-air block being lost.
					self.set_block_and_request_updates_to_meshes(coords, block);
				} else {
					// If we place the block to be placed, it would replace a non-air block that
					// would be lost, which we do not want to happen.
					// So instead the placed block will be placed in the form of a block entity
					// that will manage to place itself elsewhere on air.
					self.add_entity(
						Entity::new_block(
							id_generator,
							block,
							coords.map(|x| x as f32),
							cgmath::vec3(0.0, 0.0, 0.0),
						),
						save,
					)
				}
			},
			ActionOnWorld::PlaceBlockAndMaybeLoseWhatWasThereBefore { block, coords } => {
				// If there was a non-air block there before, then it is lost.
				self.set_block_and_request_updates_to_meshes(coords, block);
			},
			ActionOnWorld::AddEntity(entity) => self.add_entity(entity, save),
			ActionOnWorld::AddChunkLoadingResults {
				chunk_coords,
				chunk_blocks,
				chunk_culling_info,
				chunk_entities,
			} => {
				self.add_chunk_loading_results(
					chunk_coords,
					chunk_blocks,
					chunk_culling_info.clone(),
					chunk_entities,
				);
				for neighbor_chunk_coords in iter_3d_cube_center_radius(chunk_coords, 2) {
					self.require_remeshing(neighbor_chunk_coords);
				}
			},
			ActionOnWorld::AddChunkMeshingResults { chunk_coords, chunk_mesh } => {
				self.add_chunk_meshing_results(chunk_coords, chunk_mesh);
			},
		}
	}

	pub(crate) fn iter_entities_in_chunk(
		&self,
		chunk_coords: ChunkCoords,
	) -> Option<impl Iterator<Item = &Entity> + '_> {
		self.entities_map.get(&chunk_coords).map(|entity_chunk| entity_chunk.iter_entities())
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

	pub(crate) fn get_chunk_entities(&self, chunk_coords: ChunkCoords) -> Option<&ChunkEntities> {
		self.entities_map.get(&chunk_coords)
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
		self.already_generated_set.insert(chunk_coords);
	}

	fn unload_chunk(
		&mut self,
		chunk_coords: ChunkCoords,
		save: Option<&Arc<Save>>,
		only_save_modified_chunks: bool,
		part_tables: &PartTables,
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
		part_tables: &PartTables,
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
		part_tables: &PartTables,
	) {
		let chunk_coords_list: Vec<_> = self.blocks_map.keys().copied().collect();
		for chunk_coords in chunk_coords_list.into_iter() {
			self.unload_chunk(chunk_coords, save, only_save_modified_chunks, part_tables);
		}
	}
}

/// An action to be performed on the world can be reptresented as such when it must be pending
/// for some time before being applied.
pub(crate) enum ActionOnWorld {
	PlaceBlockWithoutLoss {
		block: Block,
		coords: BlockCoords,
	},
	PlaceBlockAndMaybeLoseWhatWasThereBefore {
		block: Block,
		coords: BlockCoords,
	},
	AddEntity(Entity),
	AddChunkLoadingResults {
		chunk_coords: ChunkCoords,
		chunk_blocks: ChunkBlocks,
		chunk_culling_info: ChunkCullingInfo,
		chunk_entities: Option<ChunkEntities>,
	},
	AddChunkMeshingResults {
		chunk_coords: ChunkCoords,
		chunk_mesh: Option<ChunkMesh>,
	},
}

/// The main thread holds the `ChunkGrid` but must be able to share it to threads sometimes.
/// So it has two states:
/// - Exclusively owned: grants write access.
/// - Shared: grants read access and allows to store modifications for later.
///
/// The modifications stored while in shared state can be applied as soon as the main thread
/// becomes the single owner of the ChunkGrid again.
pub(crate) struct ChunkGridShareable {
	chunk_grid: Arc<ChunkGrid>,
	entities_step_collector: Option<EntitiesPhysicsStepCollector>,
}

impl ChunkGridShareable {
	pub(crate) fn new(chunk_grid: ChunkGrid) -> ChunkGridShareable {
		ChunkGridShareable { chunk_grid: Arc::new(chunk_grid), entities_step_collector: None }
	}

	pub(crate) fn get(&self) -> &ChunkGrid {
		&self.chunk_grid
	}

	pub(crate) fn add_entities_step_result(
		&mut self,
		entities_step_result: EntitiesPhysicsStepResult,
	) {
		if let Some(entities_step_collector) = self.entities_step_collector.as_mut() {
			entities_step_collector.collect_a_task_result(entities_step_result);
		} else {
			panic!("Adding an entities step result when we are in shared state should not happen");
		}
	}

	fn is_exclusively_owned(&self) -> bool {
		if self.entities_step_collector.is_none() {
			assert_eq!(
				Arc::strong_count(&self.chunk_grid),
				1,
				"We shoud be the only owner here"
			);
			true
		} else {
			false
		}
	}

	fn get_mut_if_exclusively_owned(&mut self) -> Option<&mut ChunkGrid> {
		if self.is_exclusively_owned() {
			Some(Arc::get_mut(&mut self.chunk_grid).unwrap())
		} else {
			None
		}
	}

	pub(crate) fn is_exclusively_owned_or_can_be(&self) -> bool {
		self.entities_step_collector.is_none()
			|| self
				.entities_step_collector
				.as_ref()
				.is_some_and(|entities_step_collector| entities_step_collector.is_complete())
	}

	pub(crate) fn make_sure_is_owned_by_applying_pending(
		&mut self,
		save: Option<&Arc<Save>>,
		id_generator: &IdGenerator,
	) {
		if self.is_exclusively_owned() {
			// Already owned, nothing to do.
		} else if self
			.entities_step_collector
			.as_ref()
			.is_some_and(|entities_step_collector| entities_step_collector.is_complete())
		{
			if let Some(chunk_grid) = Arc::get_mut(&mut self.chunk_grid) {
				chunk_grid.apply_completed_entities_step(
					self.entities_step_collector.take().unwrap(),
					save,
					id_generator,
				);
			} else {
				panic!("We collected all the entities step results, there should be no more `Arc`s");
			}
		}
	}

	pub(crate) fn if_owned_then_share_to_run_entities_tasks(
		&mut self,
		worker_tasks: &mut WorkerTasksManager,
		pool: &mut ThreadPool,
		block_type_table: &Arc<BlockTypeTable>,
		dt: std::time::Duration,
		part_manipulation: ForPartManipulation,
		id_generator: &Arc<IdGenerator>,
	) {
		if self.is_exclusively_owned() {
			let entities_step_collector = ChunkGrid::run_entities_tasks(
				&self.chunk_grid,
				worker_tasks,
				pool,
				block_type_table,
				dt,
				part_manipulation,
				id_generator,
			);
			self.entities_step_collector = Some(entities_step_collector);
		}
	}

	pub(crate) fn perform_now_or_later(
		&mut self,
		action_on_world: ActionOnWorld,
		save: Option<&Arc<Save>>,
		id_generator: &IdGenerator,
	) {
		if let Some(chunk_grid) = self.get_mut_if_exclusively_owned() {
			// We exclusively own the `chunk_grid`, we can perform the action immediately.
			chunk_grid.apply_actions_on_world(action_on_world, save, id_generator);
		} else if let Some(entities_step_collector) = self.entities_step_collector.as_mut() {
			// The `chunk_grid` is shared and thus read-only for now,
			// so we store the `action_on_world` so that is can be applied later.
			entities_step_collector.add_an_action_on_world(action_on_world);
		}
	}

	pub(crate) fn perform_now_or_dont(&mut self, f: impl FnOnce(&mut ChunkGrid)) {
		if let Some(chunk_grid) = self.get_mut_if_exclusively_owned() {
			// We exclusively own the `chunk_grid`, let's perform.
			f(chunk_grid);
		}
	}

	pub(crate) fn perform_now_or_block_until_possible(&mut self, f: impl FnOnce(&mut ChunkGrid)) {
		loop {
			if let Some(chunk_grid) = self.get_mut_if_exclusively_owned() {
				// We exclusively own the `chunk_grid`, let's perform.
				f(chunk_grid);
				break;
			} else {
				// Wait a bit before trying again.
				// Not too much (hopefully) to not wait too much time or miss the chance,
				// not too little (hopefully) to spinlock.
				std::thread::sleep(Duration::from_secs_f32((1.0 / 144.0) / 40.0));
			}
		}
	}
}
