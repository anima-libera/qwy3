//! The entities and a part of their handling by the chunks.

use std::{
	f32::consts::TAU,
	io::{Read, Write},
	sync::Arc,
};

use cgmath::{EuclideanSpace, InnerSpace, MetricSpace, Zero};
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};

use crate::{
	block_types::BlockTypeTable,
	chunk_blocks::Block,
	chunks::ChunkGrid,
	coords::{AlignedBox, AngularDirection, ChunkCoords, ChunkCoordsSpan, ChunkDimensions},
	entity_parts::{
		colored_cube::{ColoredCubePartKind, PartColoredCubeInstanceData},
		colored_icosahedron::{ColoredIcosahedronPartKind, PartColoredIcosahedronInstanceData},
		textured_cube::{PartTexturedCubeInstanceData, TexturedCubePartKind},
		PartHandler, PartInstance, PartTables, TextureMappingAndColoringTable,
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
	to_delete: bool,
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

impl Entity {
	pub(crate) fn new_block(
		block: Block,
		pos: cgmath::Point3<f32>,
		motion: cgmath::Vector3<f32>,
	) -> Entity {
		Entity {
			to_delete: false,
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

	pub(crate) fn new_test_ball(pos: cgmath::Point3<f32>, motion: cgmath::Vector3<f32>) -> Entity {
		Entity {
			to_delete: false,
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
	fn apply_one_physics_step(
		&mut self,
		chunk_grid: &mut ChunkGrid,
		chunk_entity_of_self: &mut ChunkEntities,
		block_type_table: &Arc<BlockTypeTable>,
		dt: std::time::Duration,
		part_manipulation: &mut ForPartManipulation,
		save: Option<&Arc<Save>>,
	) {
		match self.typed {
			EntityTyped::Block { .. } => {
				let try_to_place = if let EntityTyped::Block { phys, .. } = &mut self.typed {
					phys.apply_one_physics_step(
						cgmath::vec3(0.0, 0.0, 0.0),
						chunk_grid,
						block_type_table,
						dt,
						true,
					);

					phys.on_ground_and_not_overlapping()
				} else {
					unreachable!()
				};

				// Place itself on the block grid if on the ground and there is room.
				if try_to_place {
					let coords = self.pos().map(|x| x.round() as i32);
					let coords_are_empty = !chunk_grid
						.get_block(coords)
						.is_some_and(|block| block_type_table.get(block.type_id).unwrap().is_opaque());
					let coords_below_are_empty = !chunk_grid
						.get_block(coords - cgmath::vec3(0, 0, 1))
						.is_some_and(|block| block_type_table.get(block.type_id).unwrap().is_opaque());
					if coords_below_are_empty {
						if let EntityTyped::Block { phys, .. } = &mut self.typed {
							phys.impose_position(coords.map(|x| x as f32));
						} else {
							unreachable!();
						};
					} else if coords_are_empty {
						let chunk_coords =
							chunk_grid.cd().world_coords_to_containing_chunk_coords(coords);
						let is_loaded = chunk_grid.is_loaded(chunk_coords);
						if is_loaded {
							let block = if let EntityTyped::Block { block, .. } = &self.typed {
								block.clone()
							} else {
								unreachable!()
							};
							chunk_grid.set_block_and_request_updates_to_meshes(coords, block);
							self.to_delete = true;
						}
					}
				}

				// Manage the part.
				let pos = self.pos();
				if let EntityTyped::Block { block, part, .. } = &mut self.typed {
					part.ensure_is_allocated(&mut part_manipulation.part_tables.textured_cubes, || {
						let texture_mapping_offset = part_manipulation
							.texture_mapping_and_coloring_table
							.get_offset_of_block(
								block.type_id,
								block_type_table,
								part_manipulation.texturing_and_coloring_array_thingy,
								part_manipulation.queue,
							)
							.unwrap();
						PartTexturedCubeInstanceData::new(pos, texture_mapping_offset).into_pod()
					});

					part.modify_instance(
						&mut part_manipulation.part_tables.textured_cubes,
						|instance| {
							instance
								.set_model_matrix(&cgmath::Matrix4::<f32>::from_translation(pos.to_vec()));
						},
					);
				}
			},

			EntityTyped::TestBall { .. } => {
				if let EntityTyped::TestBall {
					phys,
					rotation_matrix,
					facing_direction,
					rolling_speed,
					..
				} = &mut self.typed
				{
					let mut walking = facing_direction.to_vec3() * *rolling_speed;

					// TODO: Do not just look at the current chunk for colliding entities,
					// we should look at all the chunks that are suceptible to contain an entity
					// that is suceptible to be colliding with us.
					// To do that, each chunk should be able to give a maximum of the dimensions of
					// its entities, and here we should ask neighboring chunks and do some calculations
					// to see for each neigboring chunk if its bigget entity migh be able to collide
					// with us even from its chunk.

					// Getting pushed out of other entities we overlap with.
					for entity in chunk_entity_of_self.savable.entities.iter() {
						if let Some(other_aligned_box) =
							entity.as_ref().and_then(|entity| entity.aligned_box())
						{
							if other_aligned_box.overlaps(phys.aligned_box()) {
								let mut displacement = phys.aligned_box().pos - other_aligned_box.pos;
								if displacement.is_zero() {
									displacement = cgmath::vec3(0.0, 0.0, 1.0);
								} else {
									displacement = displacement.normalize();
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
					phys.apply_one_physics_step(walking, chunk_grid, block_type_table, dt, true);

					// Just to see if it worked, it sometimes throw a leaf block.
					let test_leaf_throwing_probability = 0.01 * dt.as_secs_f64();
					if test_leaf_throwing_probability <= 1.0
						&& rand::thread_rng().gen_bool(test_leaf_throwing_probability)
					{
						chunk_grid.add_entity(
							Entity::new_block(
								Block { type_id: block_type_table.kinda_leaf_id(), data: None },
								last_pos,
								cgmath::vec3(0.0, 0.0, 0.2),
							),
							save,
						);
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
				let pos = self.pos();
				if let EntityTyped::TestBall {
					ball_part,
					left_eye_part,
					right_eye_part,
					rotation_matrix,
					facing_direction,
					..
				} = &mut self.typed
				{
					ball_part.ensure_is_allocated(
						&mut part_manipulation.part_tables.colored_icosahedron,
						|| {
							let coloring_offset = part_manipulation
								.texture_mapping_and_coloring_table
								.get_offset_of_icosahedron_coloring(
									WhichIcosahedronColoring::Test,
									part_manipulation.texturing_and_coloring_array_thingy,
									part_manipulation.queue,
								);
							PartColoredIcosahedronInstanceData::new(pos, coloring_offset).into_pod()
						},
					);
					ball_part.modify_instance(
						&mut part_manipulation.part_tables.colored_icosahedron,
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
							&mut part_manipulation.part_tables.colored_cubes,
							|| {
								let coloring_offset = part_manipulation
									.texture_mapping_and_coloring_table
									.get_offset_of_cube_coloring_uni(
										[20, 20, 50],
										part_manipulation.texturing_and_coloring_array_thingy,
										part_manipulation.queue,
									);
								PartColoredCubeInstanceData::new(pos, coloring_offset).into_pod()
							},
						);
						part.modify_instance(
							&mut part_manipulation.part_tables.colored_cubes,
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
			},
		}
	}

	/// Called when an entity is not loaded anymore (be it deleted or simply unloaded (and saved)).
	/// This is not always deletion, on-death effects should not be triggered here.
	///
	/// The entity parts should be deleted here (or they will "leak" and remain
	/// visible and unmoving where they are until the game is closed).
	fn handle_unloading_or_deletion(self, part_tables: &mut PartTables) {
		match self.typed {
			EntityTyped::Block { part, .. } => {
				part.delete(&mut part_tables.textured_cubes);
			},
			EntityTyped::TestBall { ball_part, left_eye_part, right_eye_part, .. } => {
				ball_part.delete(&mut part_tables.colored_icosahedron);
				left_eye_part.delete(&mut part_tables.colored_cubes);
				right_eye_part.delete(&mut part_tables.colored_cubes);
			},
		}
	}
}

/// All that is needed for entities to be able to manipulate their parts.
pub(crate) struct ForPartManipulation<'a> {
	pub(crate) part_tables: &'a mut PartTables,
	pub(crate) texture_mapping_and_coloring_table: &'a mut TextureMappingAndColoringTable,
	pub(crate) texturing_and_coloring_array_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) queue: &'a wgpu::Queue,
}

/// The entities of a chunk.
pub(crate) struct ChunkEntities {
	pub(crate) coords_span: ChunkCoordsSpan,
	savable: ChunkEntitiesSavable,
}
#[derive(Clone, Serialize, Deserialize)]
struct ChunkEntitiesSavable {
	/// The `Option` is almost always `Some`, except when it is not (>w<) which can mean:
	/// - that we are in the process of migrating out and deleting entities from the chunk, or
	/// - that the entity was temporarily taken out of the vec for it to borrow the rest of the vec.
	entities: Vec<Option<Entity>>,
}

impl ChunkEntities {
	pub(crate) fn new_empty(coords_span: ChunkCoordsSpan) -> ChunkEntities {
		ChunkEntities { coords_span, savable: ChunkEntitiesSavable { entities: vec![] } }
	}

	pub(crate) fn merge_to(&mut self, mut other: ChunkEntities) {
		assert_eq!(
			self.coords_span.chunk_coords,
			other.coords_span.chunk_coords
		);
		self.savable.entities.append(&mut other.savable.entities);
	}
	pub(crate) fn merged(mut self, other: ChunkEntities) -> ChunkEntities {
		self.merge_to(other);
		self
	}

	pub(crate) fn iter_entities(&self) -> impl Iterator<Item = &Entity> {
		self.savable.entities.iter().map(|entity| entity.as_ref().unwrap())
	}
	pub(crate) fn count_entities(&self) -> usize {
		self.savable.entities.len()
	}

	pub(crate) fn add_entity(&mut self, entity: Entity) {
		self.savable.entities.push(Some(entity));
	}

	pub(crate) fn apply_one_physics_step(
		&mut self,
		chunk_grid: &mut ChunkGrid,
		block_type_table: &Arc<BlockTypeTable>,
		dt: std::time::Duration,
		changes_of_chunk: &mut Vec<ChunkEntitiesPhysicsStepChangeOfChunk>,
		part_manipulation: &mut ForPartManipulation,
		save: Option<&Arc<Save>>,
	) {
		let entity_indices = 0..self.savable.entities.len();
		for entity_index in entity_indices {
			let mut entity = self.savable.entities[entity_index].take().unwrap();
			entity.apply_one_physics_step(
				chunk_grid,
				self,
				block_type_table,
				dt,
				part_manipulation,
				save,
			);
			self.savable.entities[entity_index] = Some(entity);
		}
		self.savable.entities.retain_mut(|entity| {
			if entity.as_ref().unwrap().to_delete {
				// The entity was flagged for deletion and is now deleted.
				entity.take().unwrap().handle_unloading_or_deletion(part_manipulation.part_tables);
				false
			} else {
				let entity_chunk_coords = entity.as_ref().unwrap().chunk_coords(chunk_grid.cd());
				if entity_chunk_coords != self.coords_span.chunk_coords {
					// The entity moved out of this chunk and is sent away into transit
					// in order to be transfered to its new chunk.
					changes_of_chunk.push(ChunkEntitiesPhysicsStepChangeOfChunk {
						new_chunk: entity_chunk_coords,
						entity: entity.take().unwrap(),
					});
					false
				} else {
					true
				}
			}
		});
	}

	/// Tells the entities that they are being unloaded.
	pub(crate) fn handle_unloading(self, part_tables: &mut PartTables) {
		for entity in self.savable.entities.into_iter() {
			entity.unwrap().handle_unloading_or_deletion(part_tables);
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

/// Entity in transit to a new chunk.
///
/// If an entity moves out of its chunk during its physics step, then it must be transfered
/// to its new chunk. This transfer is done in two steps: first the entity is taken out of the
/// chunk it exited and is put in a `ChunkEntitiesPhysicsStepChangeOfChunk`, alongside the coords
/// of its new chunk. Then, after all the entites have had their physics step, the entities in
/// transit are placed in their new chunk.
pub(crate) struct ChunkEntitiesPhysicsStepChangeOfChunk {
	pub(crate) entity: Entity,
	pub(crate) new_chunk: ChunkCoords,
}
