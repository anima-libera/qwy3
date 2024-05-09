//! The entities and a part of their handling by the chunks.

use std::{
	collections::hash_map::Entry,
	f32::consts::TAU,
	io::{Read, Write},
	sync::{Arc, Mutex},
};

use cgmath::{EuclideanSpace, InnerSpace, MetricSpace, Zero};
use fxhash::FxHashMap;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::{
	block_types::BlockTypeTable,
	chunk_blocks::Block,
	chunks::{ActionOnWorld, ChunkGrid},
	coords::{
		iter_3d_cube_center_radius, AlignedBox, AngularDirection, ChunkCoords, ChunkCoordsSpan,
		ChunkDimensions,
	},
	entity_parts::{
		colored_cube::{ColoredCubePartKind, PartColoredCubeInstanceData},
		colored_icosahedron::{ColoredIcosahedronPartKind, PartColoredIcosahedronInstanceData},
		textured_cube::{PartTexturedCubeInstanceData, TexturedCubePartKind},
		PartHandler, PartInstance, PartTables, TextureMappingAndColoringTableRwLock,
		WhichIcosahedronColoring,
	},
	physics::AlignedPhysBox,
	rendering_init::BindingThingy,
	saves::{Save, WhichChunkFile},
};

/// In the world there are two sorts of things: static blocks and entities.
/// Despite the constraint that an entity must have a position, it can be anything.
/// Entities can have parts (via `PartHandler`s) which are instances of models of simple shapes,
/// this is how they are rendered.
/// Entities are saved and loaded just like blocks, no loss, no random despawn.
///
/// Each entity must have a position so that it is in (exactly) one chunk (instead of in
/// multiple chunks at once, or everywhere, or nowhere at all). This makes some matters so much
/// simpler than if we allowed entities to not have a precise position. It allows the chunk to
/// be saved/loaded or generated with their entities, it allows entities to only be simulated if
/// they are in loaded chunks, etc. If something cannot be given a position, then it should not
/// be implemented as an entity and it should be something else.
///
/// An entity can move around and exit its chunk, it will be transfered to its new chunk
/// automatically, and will wait for the chunk loading (if it was not already loaded).
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Entity {
	id: Id,
	typed: EntityTyped,
}
#[derive(Clone, Serialize, Deserialize)]
enum EntityTyped {
	Block {
		block: Block,
		phys: AlignedPhysBox,
		#[serde(skip)]
		part: PartHandler<TexturedCubePartKind>,
	},
	TestBall {
		phys: AlignedPhysBox,
		rotation_matrix: cgmath::Matrix4<f32>,
		facing_direction: AngularDirection,
		rolling_speed: f32,
		#[serde(skip)]
		ball_part: PartHandler<ColoredIcosahedronPartKind>,
		#[serde(skip)]
		left_eye_part: PartHandler<ColoredCubePartKind>,
		#[serde(skip)]
		right_eye_part: PartHandler<ColoredCubePartKind>,
	},
}

/// Entity id generated by `IdGenerator`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct Id(u64);

/// Generator of unique `Id`s. Sharable, put it in an `Arc` and pass it around.
pub(crate) struct IdGenerator {
	next_id_value: Mutex<u64>,
}
impl IdGenerator {
	pub(crate) fn new() -> IdGenerator {
		IdGenerator { next_id_value: Mutex::new(0) }
	}

	pub(crate) fn from_state(state: IdGeneratorState) -> IdGenerator {
		IdGenerator { next_id_value: Mutex::new(state.0) }
	}
	pub(crate) fn state(&self) -> IdGeneratorState {
		IdGeneratorState(*self.next_id_value.lock().unwrap())
	}

	/// Generate an `Id` never generated before.
	fn generate_id(&self) -> Id {
		let mut locked = self.next_id_value.lock().unwrap();
		let id_value = *locked;
		*locked += 1;
		Id(id_value)
	}
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct IdGeneratorState(u64);

impl Entity {
	pub(crate) fn new_block(
		id_generator: &IdGenerator,
		block: Block,
		pos: cgmath::Point3<f32>,
		motion: cgmath::Vector3<f32>,
	) -> Entity {
		Entity {
			id: id_generator.generate_id(),
			typed: EntityTyped::Block {
				block,
				phys: AlignedPhysBox::new(
					AlignedBox { pos, dims: cgmath::vec3(0.99, 0.99, 0.99) },
					motion,
				),
				part: PartHandler::default(),
			},
		}
	}

	pub(crate) fn new_test_ball(
		id_generator: &IdGenerator,
		pos: cgmath::Point3<f32>,
		motion: cgmath::Vector3<f32>,
	) -> Entity {
		Entity {
			id: id_generator.generate_id(),
			typed: EntityTyped::TestBall {
				phys: AlignedPhysBox::new(
					AlignedBox { pos, dims: cgmath::vec3(0.99, 0.99, 0.99) },
					motion,
				),
				rotation_matrix: cgmath::Matrix4::from_angle_x(cgmath::Rad::zero()),
				facing_direction: AngularDirection::from_angle_horizontal(
					thread_rng().gen_range(0.0..TAU),
				),
				rolling_speed: thread_rng().gen_range(0.5..2.5),
				ball_part: PartHandler::default(),
				left_eye_part: PartHandler::default(),
				right_eye_part: PartHandler::default(),
			},
		}
	}

	pub(crate) fn pos(&self) -> cgmath::Point3<f32> {
		match &self.typed {
			EntityTyped::Block { phys, .. } => phys.aligned_box().pos,
			EntityTyped::TestBall { phys, .. } => phys.aligned_box().pos,
		}
	}

	pub(crate) fn chunk_coords(&self, cd: ChunkDimensions) -> ChunkCoords {
		let coords = self.pos().map(|x| x.round() as i32);
		cd.world_coords_to_containing_chunk_coords(coords)
	}

	pub(crate) fn aligned_box(&self) -> Option<AlignedBox> {
		match &self.typed {
			EntityTyped::Block { phys, .. } => Some(phys.aligned_box().clone()),
			EntityTyped::TestBall { phys, .. } => Some(phys.aligned_box().clone()),
		}
	}

	/// If an entity "does stuff", then it probably happens here.
	///
	/// The `chunk_entity_of_self` was taken out of the `chunk_grid`,
	/// and `self` was taken out of the `chunk_entity_of_self`, beware.
	#[allow(clippy::too_many_arguments)]
	fn apply_one_physics_step(
		&self,
		entities_for_next_step: &mut Vec<Entity>,
		chunk_grid: &ChunkGrid,
		actions_on_world: &mut Vec<ActionOnWorld>,
		block_type_table: &Arc<BlockTypeTable>,
		entity_physics_dt: std::time::Duration,
		part_manipulation: &ForPartManipulation,
		id_generator: &IdGenerator,
	) {
		match self.typed {
			EntityTyped::Block { .. } => {
				let mut next_block = self.clone();

				let try_to_place = if let EntityTyped::Block { phys, .. } = &mut next_block.typed {
					phys.apply_one_physics_step(
						cgmath::vec3(0.0, 0.0, 0.0),
						chunk_grid,
						block_type_table,
						entity_physics_dt,
						true,
					);

					phys.on_ground_and_not_overlapping()
				} else {
					unreachable!()
				};

				let mut delete_self = false;

				// Place itself on the block grid if on the ground and there is room.
				if try_to_place {
					let coords = next_block.pos().map(|x| x.round() as i32);
					let coords_are_empty = !chunk_grid
						.get_block(coords)
						.is_some_and(|block| block_type_table.get(block.type_id).unwrap().is_opaque());
					let coords_below_are_empty = !chunk_grid
						.get_block(coords - cgmath::vec3(0, 0, 1))
						.is_some_and(|block| block_type_table.get(block.type_id).unwrap().is_opaque());
					if coords_below_are_empty {
						if let EntityTyped::Block { phys, .. } = &mut next_block.typed {
							phys.impose_position(coords.map(|x| x as f32));
							phys.impose_null_horizontal_motion();
						};
					} else if coords_are_empty {
						let chunk_coords =
							chunk_grid.cd().world_coords_to_containing_chunk_coords(coords);
						let is_loaded = chunk_grid.is_loaded(chunk_coords);
						if is_loaded {
							if let EntityTyped::Block { block, .. } = &next_block.typed {
								let block = block.clone();
								actions_on_world
									.push(ActionOnWorld::PlaceBlockWithoutLoss { block, coords });
							}
							delete_self = true;
						}
					}
				}

				// Manage the part.
				let pos = next_block.pos();
				if let EntityTyped::Block { block, part, .. } = &mut next_block.typed {
					part.ensure_is_allocated(
						&mut part_manipulation.part_tables.textured_cubes.lock().unwrap(),
						|| {
							let texture_mapping_offset = part_manipulation
								.texture_mapping_and_coloring_table
								.get_offset_of_block(
									block.type_id,
									block_type_table,
									&part_manipulation.texturing_and_coloring_array_thingy,
									&part_manipulation.queue,
								)
								.unwrap();
							PartTexturedCubeInstanceData::new(pos, texture_mapping_offset).into_pod()
						},
					);

					part.modify_instance(
						&mut part_manipulation.part_tables.textured_cubes.lock().unwrap(),
						|instance| {
							instance
								.set_model_matrix(&cgmath::Matrix4::<f32>::from_translation(pos.to_vec()));
						},
					);
				}

				if delete_self {
					next_block.handle_unloading_or_deletion(&part_manipulation.part_tables);
				} else {
					entities_for_next_step.push(next_block);
				}
			},

			EntityTyped::TestBall { .. } => {
				let mut next_ball = self.clone();

				if let EntityTyped::TestBall {
					phys,
					rotation_matrix,
					facing_direction,
					rolling_speed,
					..
				} = &mut next_ball.typed
				{
					let mut walking = facing_direction.to_vec3() * *rolling_speed;

					// We do not just look at the current chunk for colliding entities,
					// we should look at all the neighboring chunks that contain
					// an entity that is suceptible to be colliding with us.
					// To do that, each chunk knows the maximum of the dimensions of
					// its entities, and here we ask neighboring chunks for that and do some
					// calculations to see for each neigboring chunk if its biggest entity might
					// be able to collide with us even from its chunk.
					let block_coords = phys.aligned_box().pos.map(|x| x.round() as i32);
					let chunk_coords =
						chunk_grid.cd().world_coords_to_containing_chunk_coords(block_coords);
					let mut chunk_to_iterate: SmallVec<[ChunkCoords; 4]> = SmallVec::new();
					for neigboring_chunk_coords in iter_3d_cube_center_radius(chunk_coords, 2) {
						if chunk_grid.can_entity_in_chunk_maybe_collide_with_box(
							neigboring_chunk_coords,
							phys.aligned_box(),
						) {
							chunk_to_iterate.push(neigboring_chunk_coords);
						}
					}
					let other_entities_iterator = chunk_to_iterate
						.into_iter()
						.filter_map(|chunk_coords| chunk_grid.iter_entities_in_chunk(chunk_coords))
						.flatten()
						.filter(|entity| entity.id != self.id);

					// Getting pushed out of other entities we overlap with.
					//
					// TODO: Make it so that one entity of the pair does not get priority.
					for entity in other_entities_iterator {
						if let Some(other_aligned_box) = entity.aligned_box() {
							if other_aligned_box.overlaps(phys.aligned_box()) {
								let mut displacement = phys.aligned_box().pos - other_aligned_box.pos;
								if displacement.is_zero() {
									displacement = cgmath::vec3(0.0, 0.0, 1.0);
								} else {
									displacement = displacement.normalize() / 2.0;
								}
								let distance = phys.aligned_box().pos.distance(other_aligned_box.pos);
								let overlap_factor = if distance.is_zero() {
									1.0
								} else {
									(1.0 / (distance * 0.1)).clamp(0.0, 1.0)
								};
								phys.add_motion(displacement * overlap_factor * 0.01);
								walking += displacement * 1.0;
							}
						}
					}

					let last_pos = phys.aligned_box().pos;
					phys.apply_one_physics_step(
						walking,
						chunk_grid,
						block_type_table,
						entity_physics_dt,
						true,
					);

					// Just to see if it worked, it sometimes throw a leaf block.
					let test_leaf_throwing_probability = 0.01 * entity_physics_dt.as_secs_f64();
					if test_leaf_throwing_probability <= 1.0
						&& rand::thread_rng().gen_bool(test_leaf_throwing_probability)
					{
						entities_for_next_step.push(Entity::new_block(
							id_generator,
							Block { type_id: block_type_table.kinda_leaf_id(), data: None },
							last_pos,
							cgmath::vec3(0.0, 0.0, 0.2),
						));
					}

					// Make motion on the ground roll the ball.
					let delta_pos = phys.aligned_box().pos - last_pos;
					if phys.on_ground_and_not_overlapping() {
						let radius = 0.5;
						let circumference = TAU * radius;
						let delta_angle_x = (delta_pos.x / circumference) * TAU;
						let delta_angle_y = (delta_pos.y / circumference) * TAU;
						*rotation_matrix =
							cgmath::Matrix4::<f32>::from_angle_y(cgmath::Rad(delta_angle_x))
								* *rotation_matrix;
						*rotation_matrix =
							cgmath::Matrix4::<f32>::from_angle_x(-cgmath::Rad(delta_angle_y))
								* *rotation_matrix;
					}
				} else {
					unreachable!()
				};

				// Manage the parts.
				let pos = next_ball.pos();
				if let EntityTyped::TestBall {
					ball_part,
					left_eye_part,
					right_eye_part,
					rotation_matrix,
					facing_direction,
					..
				} = &mut next_ball.typed
				{
					ball_part.ensure_is_allocated(
						&mut part_manipulation.part_tables.colored_icosahedron.lock().unwrap(),
						|| {
							let coloring_offset = part_manipulation
								.texture_mapping_and_coloring_table
								.get_offset_of_icosahedron_coloring(
									WhichIcosahedronColoring::Test,
									&part_manipulation.texturing_and_coloring_array_thingy,
									&part_manipulation.queue,
								);
							PartColoredIcosahedronInstanceData::new(pos, coloring_offset).into_pod()
						},
					);
					ball_part.modify_instance(
						&mut part_manipulation.part_tables.colored_icosahedron.lock().unwrap(),
						|instance| {
							instance.set_model_matrix(
								&(cgmath::Matrix4::<f32>::from_translation(pos.to_vec())
									* *rotation_matrix),
							);
						},
					);

					let angle_horizontal = facing_direction.angle_horizontal;
					let facing_direction = facing_direction.to_vec3() * 0.485;
					let leftward_direction =
						-facing_direction.cross(cgmath::vec3(0.0, 0.0, 1.0)).normalize();

					let mut eye_parts = [left_eye_part, right_eye_part];
					for left_or_right in [0, 1] {
						let part = &mut eye_parts[left_or_right];
						let left_or_right_offset =
							leftward_direction * 0.1 * (left_or_right as f32 * 2.0 - 1.0);
						part.ensure_is_allocated(
							&mut part_manipulation.part_tables.colored_cubes.lock().unwrap(),
							|| {
								let coloring_offset = part_manipulation
									.texture_mapping_and_coloring_table
									.get_offset_of_cube_coloring_uni(
										[20, 20, 50],
										&part_manipulation.texturing_and_coloring_array_thingy,
										&part_manipulation.queue,
									);
								PartColoredCubeInstanceData::new(pos, coloring_offset).into_pod()
							},
						);
						part.modify_instance(
							&mut part_manipulation.part_tables.colored_cubes.lock().unwrap(),
							|instance| {
								instance.set_model_matrix(
									&(cgmath::Matrix4::<f32>::from_translation(
										facing_direction + left_or_right_offset,
									) * cgmath::Matrix4::<f32>::from_translation(pos.to_vec())
										* cgmath::Matrix4::<f32>::from_angle_z(cgmath::Rad(
											angle_horizontal,
										)) * cgmath::Matrix4::<f32>::from_nonuniform_scale(0.02, 0.05, 0.11)),
								);
							},
						);
					}
				}

				entities_for_next_step.push(next_ball);
			},
		}
	}

	/// Called when an entity is not loaded anymore (be it deleted or simply unloaded (and saved)).
	/// This is not always deletion, on-death effects should not be triggered here.
	///
	/// The entity parts should be deleted here (or they will "leak" and remain
	/// visible and unmoving where they are until the game is closed).
	fn handle_unloading_or_deletion(&self, part_tables: &PartTables) {
		match &self.typed {
			EntityTyped::Block { part, .. } => {
				part.delete(&mut part_tables.textured_cubes.lock().unwrap());
			},
			EntityTyped::TestBall { ball_part, left_eye_part, right_eye_part, .. } => {
				ball_part.delete(&mut part_tables.colored_icosahedron.lock().unwrap());
				left_eye_part.delete(&mut part_tables.colored_cubes.lock().unwrap());
				right_eye_part.delete(&mut part_tables.colored_cubes.lock().unwrap());
			},
		}
	}
}

/// All that is needed for entities to be able to manipulate their parts.
#[derive(Clone)]
pub(crate) struct ForPartManipulation {
	pub(crate) part_tables: Arc<PartTables>,
	pub(crate) texture_mapping_and_coloring_table: Arc<TextureMappingAndColoringTableRwLock>,
	pub(crate) texturing_and_coloring_array_thingy: Arc<BindingThingy<wgpu::Buffer>>,
	pub(crate) queue: Arc<wgpu::Queue>,
}

/// The entities of a chunk.
#[derive(Clone)]
pub(crate) struct ChunkEntities {
	pub(crate) coords_span: ChunkCoordsSpan,
	savable: ChunkEntitiesSavable,
}
#[derive(Clone, Serialize, Deserialize)]
struct ChunkEntitiesSavable {
	entities: Vec<Entity>,
	/// Dimensions of a bounding box that could contain any entity in this chunk.
	/// This allows for entities E in neighboring chunks to know if this chunk contains
	/// an entity big enough to be able to collide with E.
	max_entity_dims: cgmath::Vector3<f32>,
}

impl ChunkEntities {
	pub(crate) fn new_empty(coords_span: ChunkCoordsSpan) -> ChunkEntities {
		ChunkEntities {
			coords_span,
			savable: ChunkEntitiesSavable {
				entities: vec![],
				max_entity_dims: cgmath::vec3(0.0, 0.0, 0.0),
			},
		}
	}

	pub(crate) fn merge_to(&mut self, mut other: ChunkEntities) {
		assert_eq!(
			self.coords_span.chunk_coords,
			other.coords_span.chunk_coords
		);
		if self.count_entities() < other.count_entities() {
			// We make sure that `other` has the smaller vec, that will require moving fewer entities.
			std::mem::swap(&mut self.savable.entities, &mut other.savable.entities);
		}
		self.savable.entities.append(&mut other.savable.entities);
	}
	pub(crate) fn merged(mut self, other: ChunkEntities) -> ChunkEntities {
		self.merge_to(other);
		self
	}

	pub(crate) fn iter_entities(&self) -> impl Iterator<Item = &Entity> {
		self.savable.entities.iter()
	}
	pub(crate) fn count_entities(&self) -> usize {
		self.savable.entities.len()
	}
	pub(crate) fn max_entity_dims(&self) -> cgmath::Vector3<f32> {
		self.savable.max_entity_dims
	}

	fn extended_max_entity_dims(max_entity_dims: &mut cgmath::Vector3<f32>, entity: &Entity) {
		if let Some(aligned_box) = entity.aligned_box() {
			let max_dims = *max_entity_dims;
			let dims = aligned_box.dims;
			*max_entity_dims = cgmath::vec3(
				max_dims.x.max(dims.x),
				max_dims.y.max(dims.y),
				max_dims.z.max(dims.z),
			);
		}
	}

	pub(crate) fn add_entity(&mut self, entity: Entity) {
		ChunkEntities::extended_max_entity_dims(&mut self.savable.max_entity_dims, &entity);
		self.savable.entities.push(entity);
	}

	#[allow(clippy::too_many_arguments)]
	pub(crate) fn apply_one_physics_step(
		chunk_coords: ChunkCoords,
		cd: ChunkDimensions,
		next_entities_map: &mut FxHashMap<ChunkCoords, ChunkEntities>,
		chunk_grid: &ChunkGrid,
		actions_on_world: &mut Vec<ActionOnWorld>,
		block_type_table: &Arc<BlockTypeTable>,
		entity_physics_dt: std::time::Duration,
		part_manipulation: &ForPartManipulation,
		id_generator: &IdGenerator,
	) {
		let mut entities_for_next_step = vec![];
		for entity in chunk_grid.get_chunk_entities(chunk_coords).unwrap().savable.entities.iter() {
			entity.apply_one_physics_step(
				&mut entities_for_next_step,
				chunk_grid,
				actions_on_world,
				block_type_table,
				entity_physics_dt,
				part_manipulation,
				id_generator,
			);
		}
		for entity in entities_for_next_step {
			let chunk_coords = entity.chunk_coords(cd);
			next_entities_map
				.entry(chunk_coords)
				.or_insert(ChunkEntities::new_empty(ChunkCoordsSpan {
					cd,
					chunk_coords,
				}))
				.add_entity(entity);
		}
	}

	/// Tells the entities that they are being unloaded.
	pub(crate) fn handle_unloading(self, part_tables: &PartTables) {
		for entity in self.savable.entities.into_iter() {
			entity.handle_unloading_or_deletion(part_tables);
		}
	}

	pub(crate) fn save(&self, save: &Arc<Save>) {
		// TODO: Use buffered streams instead of full vecs of data as intermediary steps.
		let chunk_file_path =
			save.chunk_file_path(self.coords_span.chunk_coords, WhichChunkFile::Entities);
		let uncompressed_data = rmp_serde::encode::to_vec(&self.savable).unwrap();
		let mut compressed_data = vec![];
		{
			let mut encoder = flate2::write::DeflateEncoder::new(
				&mut compressed_data,
				flate2::Compression::default(),
			);
			encoder.write_all(&uncompressed_data).unwrap();
		}
		let chunk_file = save.get_file_io(chunk_file_path);
		chunk_file.write(&compressed_data);
	}

	pub(crate) fn load_from_save_while_removing_the_save(
		coords_span: ChunkCoordsSpan,
		save: &Arc<Save>,
	) -> Option<ChunkEntities> {
		// TODO: Use buffered streams instead of full vecs of data as intermediary steps.
		let chunk_file_path =
			save.chunk_file_path(coords_span.chunk_coords, WhichChunkFile::Entities);
		let chunk_file = save.get_file_io(chunk_file_path);
		let compressed_data = chunk_file.read(true)?;
		let mut uncompressed_data = vec![];
		{
			let mut decoder = flate2::bufread::DeflateDecoder::new(compressed_data.as_slice());
			decoder.read_to_end(&mut uncompressed_data).unwrap();
		}
		let savable: ChunkEntitiesSavable =
			rmp_serde::decode::from_slice(&uncompressed_data).unwrap();
		Some(ChunkEntities { coords_span, savable })
	}
}

pub(crate) struct EntitiesPhysicsStepResult {
	pub(crate) next_entities_map: FxHashMap<ChunkCoords, ChunkEntities>,
	pub(crate) actions_on_world: Vec<ActionOnWorld>,
}

/// When entity physics tasks are done, they are not all done at the same time,
/// so here is a type that can collects the results as they arrive.
pub(crate) struct EntitiesPhysicsStepCollector {
	number_of_tasks_not_yet_completed: u32,
	/// The `ChunkEntities` that were not run and that are to be transfered from the old map
	/// to the next so that we do not lose the entities in them.
	chunk_entities_to_preserve: Vec<ChunkCoords>,
	next_entities_map: FxHashMap<ChunkCoords, ChunkEntities>,
	actions_on_world: Vec<ActionOnWorld>,
}

impl EntitiesPhysicsStepCollector {
	pub(crate) fn new(
		number_of_tasks_not_yet_completed: u32,
		chunk_entities_to_preserve: Vec<ChunkCoords>,
		next_entities_map: FxHashMap<ChunkCoords, ChunkEntities>,
		actions_on_world: Vec<ActionOnWorld>,
	) -> EntitiesPhysicsStepCollector {
		EntitiesPhysicsStepCollector {
			number_of_tasks_not_yet_completed,
			chunk_entities_to_preserve,
			next_entities_map,
			actions_on_world,
		}
	}

	/// When an entity physics task is done, its results are added to the collector in here.
	pub(crate) fn collect_a_task_result(&mut self, mut task_result: EntitiesPhysicsStepResult) {
		for (chunk_coords, chunk_entities) in task_result.next_entities_map.into_iter() {
			match self.next_entities_map.entry(chunk_coords) {
				Entry::Vacant(vacant) => {
					vacant.insert(chunk_entities);
				},
				Entry::Occupied(mut occupied) => {
					occupied.get_mut().merge_to(chunk_entities);
				},
			}
		}
		self.actions_on_world.append(&mut task_result.actions_on_world);
		assert!(self.number_of_tasks_not_yet_completed >= 1);
		self.number_of_tasks_not_yet_completed -= 1;
	}

	pub(crate) fn add_an_action_on_world(&mut self, action_on_world: ActionOnWorld) {
		self.actions_on_world.push(action_on_world);
	}

	/// Do we have collected all the results and can now apply them to the world?
	pub(crate) fn is_complete(&self) -> bool {
		self.number_of_tasks_not_yet_completed == 0
	}

	pub(crate) fn into_complete_result(
		self,
	) -> (
		FxHashMap<ChunkCoords, ChunkEntities>,
		Vec<ActionOnWorld>,
		Vec<ChunkCoords>,
	) {
		assert_eq!(self.number_of_tasks_not_yet_completed, 0);
		(
			self.next_entities_map,
			self.actions_on_world,
			self.chunk_entities_to_preserve,
		)
	}
}
