use std::sync::Arc;

use cgmath::MetricSpace;
use rand::Rng;

use crate::{
	block_types::BlockTypeTable,
	chunk_blocks::{ChunkBlocks, ChunkCullingInfo, FaceCullingInfo},
	chunks::ChunkGrid,
	coords::{iter_3d_cube_center_radius, ChunkCoords, ChunkDimensions, OrientedAxis},
	entities::{ChunkEntities, IdGenerator},
	saves::Save,
	tasks::CurrentWorkerTasks,
	threadpool::ThreadPool,
	world_gen::WorldGenerator,
};

/// Manages the loading of chunks, loading well-chosen ones in a well-chosen order.
pub(crate) struct LoadingManager {
	pub(crate) loading_enabled: bool,
	/// Radius (in blocks) of the spherical area around the player
	/// inside of which the world is to be loaded.
	pub(crate) loading_distance: f32,
	/// When added to `loading_distance` it gives the radius of the spherical area around the player
	/// outside of which the world is to not be loaded.
	pub(crate) margin_before_unloading: f32,
	/// Chunks to consider for loading.
	pub(crate) front_high_priority: Vec<ChunkCoords>,
	/// Chunks that have to be loaded but are probably not intresting so thay are made to wait for now.
	pub(crate) front_low_priority: Vec<ChunkCoords>,
	/// Chunks that will have to be loaded if/when the loading area moves over them.
	pub(crate) front_too_far: Vec<ChunkCoords>,
}

impl LoadingManager {
	pub(crate) fn new(loading_distance: f32, margin_before_unloading: f32) -> LoadingManager {
		LoadingManager {
			loading_enabled: true,
			loading_distance,
			margin_before_unloading,
			front_high_priority: vec![],
			front_low_priority: vec![],
			front_too_far: vec![],
		}
	}

	#[allow(clippy::too_many_arguments)]
	pub(crate) fn handle_loading(
		&mut self,
		chunk_grid: &mut ChunkGrid,
		worker_tasks: &mut CurrentWorkerTasks,
		pool: &mut ThreadPool,
		player_chunk_coords: ChunkCoords,
		world_generator: &Arc<dyn WorldGenerator + Sync + Send>,
		block_type_table: &Arc<BlockTypeTable>,
		save: Option<&Arc<Save>>,
		id_generator: &Arc<IdGenerator>,
	) {
		if !self.loading_enabled {
			return;
		}

		let workers_dedicated_to_meshing = 1;
		let available_workers_to_load = (pool.number_of_workers() - workers_dedicated_to_meshing)
			.saturating_sub(worker_tasks.tasks.len());
		if available_workers_to_load == 0 {
			return;
		}

		let loading_distance_in_chunks = self.loading_distance / chunk_grid.cd().edge as f32;
		let unloading_distance_in_chunks = {
			let unloading_distance = self.loading_distance + self.margin_before_unloading;
			unloading_distance / chunk_grid.cd().edge as f32
		};

		if self.front_high_priority.is_empty() {
			self.front_high_priority.append(&mut self.front_low_priority);
		} else if let Some(front_chunk_coords) = self.front_low_priority.pop() {
			self.front_high_priority.push(front_chunk_coords);
		}

		self.front_high_priority.retain(|front_chunk_coords| {
			let too_far =
				front_chunk_coords.map(|x| x as f32).distance(player_chunk_coords.map(|x| x as f32))
					> loading_distance_in_chunks;
			if too_far {
				self.front_too_far.push(*front_chunk_coords);
			}
			!too_far
		});

		self.front_too_far.retain(|front_chunk_coords| {
			let way_too_far =
				front_chunk_coords.map(|x| x as f32).distance(player_chunk_coords.map(|x| x as f32))
					> unloading_distance_in_chunks;
			!way_too_far
		});

		if !self.front_too_far.is_empty() {
			for _ in 0..10 {
				// Just checking a few per frame at random should be enough.
				if self.front_too_far.is_empty() {
					break;
				}
				let index = rand::thread_rng().gen_range(0..self.front_too_far.len());
				let front_chunk_coords = self.front_too_far[index];
				let still_too_far =
					front_chunk_coords.map(|x| x as f32).distance(player_chunk_coords.map(|x| x as f32))
						> loading_distance_in_chunks;
				if !still_too_far {
					self.front_too_far.remove(index);
					self.front_high_priority.push(front_chunk_coords);
				}
			}
		}

		self.front_high_priority.push(player_chunk_coords);
		for direction in OrientedAxis::all_the_six_possible_directions() {
			self.front_high_priority.push(player_chunk_coords + direction.delta());
		}
		self.front_high_priority.extend(chunk_grid.iter_chunk_with_entities_coords());

		self.front_high_priority.retain(|&chunk_coords| {
			let blocks_was_loaded = chunk_grid.is_loaded(chunk_coords);
			let blocks_is_being_loaded = worker_tasks.is_being_loaded(chunk_coords);
			(!blocks_was_loaded) && (!blocks_is_being_loaded)
		});

		// Sort to put closer chunks at the end.
		self.front_high_priority.sort_unstable_by_key(|chunk_coords| {
			-(chunk_coords.map(|x| x as f32).distance2(player_chunk_coords.map(|x| x as f32)) * 10.0)
				as i64
		});

		let mut slot_count = available_workers_to_load;
		while slot_count >= 1 {
			let chunk_coords = self.front_high_priority.pop();
			let chunk_coords = match chunk_coords {
				Some(chunk_coords) => chunk_coords,
				None => break,
			};

			let blocks_was_loaded = chunk_grid.is_loaded(chunk_coords);
			let blocks_is_being_loaded = worker_tasks.is_being_loaded(chunk_coords);

			if (!blocks_was_loaded) && (!blocks_is_being_loaded) {
				// Asking a worker for the generation of chunk blocks.
				slot_count -= 1;
				let data_for_chunk_loading = DataForChunkLoading {
					was_already_generated_before: chunk_grid.was_already_generated_before(chunk_coords),
					world_generator: world_generator.clone(),
					block_type_table: block_type_table.clone(),
					save: save.cloned(),
					cd: chunk_grid.cd(),
				};
				worker_tasks.run_chunk_loading_task(
					pool,
					chunk_coords,
					data_for_chunk_loading,
					Arc::clone(id_generator),
				);
			}
		}
	}

	pub(crate) fn handle_chunk_loading_results(
		&mut self,
		chunk_coords: ChunkCoords,
		chunk_blocks: ChunkBlocks,
		chunk_culling_info: ChunkCullingInfo,
		chunk_entities: Option<ChunkEntities>,
		chunk_grid: &mut ChunkGrid,
	) {
		chunk_grid.add_chunk_loading_results(
			chunk_coords,
			chunk_blocks,
			chunk_culling_info.clone(),
			chunk_entities,
		);

		for neighbor_chunk_coords in iter_3d_cube_center_radius(chunk_coords, 2) {
			chunk_grid.require_remeshing(neighbor_chunk_coords);
		}

		// Propagate the front.
		// The whole point of having a front is that it does not propagate through fully opaque
		// chunk faces and it propagates with lower priority through fully empty chunk faces.
		// - Not propagating though fully opaque chunk faces makes it so that inaccessible chunks
		// do not even get loaded (saves loading time and there are less loaded chunks to manage).
		// - Propagating with lower priority through air-only chunk faces makes it so that chunks
		// that are likely nothing but air get loaded later, so that the loading can focus on more
		// instresting chunks (that are likely to require a mesh).
		for (face_index, straight_direction) in
			OrientedAxis::all_the_six_possible_directions().enumerate()
		{
			let delta = straight_direction.delta();
			let adjacent_chunk_coords = chunk_coords + delta;
			match chunk_culling_info.faces[face_index] {
				FaceCullingInfo::AllOpaque => {},
				FaceCullingInfo::AllAir => {
					self.front_low_priority.push(adjacent_chunk_coords);
				},
				FaceCullingInfo::SomeAirSomeOpaque => {
					self.front_high_priority.push(adjacent_chunk_coords);
				},
			}
		}
	}
}

/// Data that is needed to load one chunk (its block data and its entities),
/// be it generated or loaded from a save.
pub(crate) struct DataForChunkLoading {
	pub(crate) was_already_generated_before: bool,
	pub(crate) world_generator: Arc<dyn WorldGenerator + Sync + Send>,
	pub(crate) block_type_table: Arc<BlockTypeTable>,
	pub(crate) save: Option<Arc<Save>>,
	pub(crate) cd: ChunkDimensions,
}
