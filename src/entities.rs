//! The entities and a part of their handling by the chunks.

use std::{
	io::{Read, Write},
	sync::Arc,
};

use cgmath::EuclideanSpace;
use serde::{Deserialize, Serialize};

use crate::{
	block_types::BlockTypeTable,
	chunk_blocks::Block,
	chunks::ChunkGrid,
	coords::{AlignedBox, ChunkCoords, ChunkCoordsSpan, ChunkDimensions},
	entity_parts::{
		colored_icosahedron::PartColoredIcosahedronInstanceData,
		textured_cubes::PartTexturedCubeInstanceData, PartHandler, PartInstance, PartTables,
		TextureMappingTable,
	},
	physics::AlignedPhysBox,
	rendering_init::BindingThingy,
	saves::{Save, WhichChunkFile},
	shaders::{part_colored::PartColoredInstancePod, part_textured::PartTexturedInstancePod},
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
		part_handler: PartHandler<PartTexturedInstancePod>,
	},
	TestIcosahedron {
		phys: AlignedPhysBox,
		#[serde(skip)]
		part_handler: PartHandler<PartColoredInstancePod>,
	},
	/// Turns off warnings about irrefutability of patterns.
	/// Can be removed when an other type is added.
	_DummyOtherType,
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
				part_handler: PartHandler::default(),
			},
		}
	}

	pub(crate) fn new_test_icosahedron(
		pos: cgmath::Point3<f32>,
		motion: cgmath::Vector3<f32>,
	) -> Entity {
		Entity {
			to_delete: false,
			typed: EntityTyped::TestIcosahedron {
				phys: AlignedPhysBox::new(
					AlignedBox { pos, dims: cgmath::vec3(0.99, 0.99, 0.99) },
					motion,
				),
				part_handler: PartHandler::default(),
			},
		}
	}

	pub(crate) fn pos(&self) -> cgmath::Point3<f32> {
		match &self.typed {
			EntityTyped::Block { phys, .. } => phys.aligned_box().pos,
			EntityTyped::TestIcosahedron { phys, .. } => phys.aligned_box().pos,
			EntityTyped::_DummyOtherType => panic!(),
		}
	}

	pub(crate) fn chunk_coords(&self, cd: ChunkDimensions) -> ChunkCoords {
		let coords = self.pos().map(|x| x.round() as i32);
		cd.world_coords_to_containing_chunk_coords(coords)
	}

	pub(crate) fn aligned_box(&self) -> Option<AlignedBox> {
		match &self.typed {
			EntityTyped::Block { phys, .. } => Some(phys.aligned_box().clone()),
			EntityTyped::TestIcosahedron { phys, .. } => Some(phys.aligned_box().clone()),
			EntityTyped::_DummyOtherType => panic!(),
		}
	}

	fn _collides_with_blocks(
		&self,
		chunk_grid: &ChunkGrid,
		block_type_table: &Arc<BlockTypeTable>,
	) -> bool {
		if let Some(aligned_box) = self.aligned_box() {
			for coords in aligned_box.overlapping_block_coords_span().iter() {
				if chunk_grid
					.get_block(coords)
					.is_some_and(|block| block_type_table.get(block.type_id).unwrap().is_opaque())
				{
					return true;
				}
			}
			false
		} else {
			let coords = self.pos().map(|x| x.round() as i32);
			chunk_grid
				.get_block(coords)
				.is_some_and(|block| block_type_table.get(block.type_id).unwrap().is_opaque())
		}
	}

	/// If an entity "does stuff", then it probably happens here.
	fn apply_one_physics_step(
		&mut self,
		chunk_grid: &mut ChunkGrid,
		block_type_table: &Arc<BlockTypeTable>,
		dt: std::time::Duration,
		part_manipulation: &mut ForPartManipulation,
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
				if let EntityTyped::Block { block, part_handler, .. } = &mut self.typed {
					part_handler.ensure_is_allocated(
						&mut part_manipulation.part_tables.textured_cubes,
						|| {
							let texture_mapping_point_offset = part_manipulation
								.texture_mapping_table
								.get_offset_of_block(
									block.type_id,
									block_type_table,
									part_manipulation.coords_in_atlas_array_thingy,
									part_manipulation.queue,
								)
								.unwrap();
							PartTexturedCubeInstanceData::new(pos, texture_mapping_point_offset).into_pod()
						},
					);

					part_handler.modify_instance(
						&mut part_manipulation.part_tables.textured_cubes,
						|instance| {
							instance
								.set_model_matrix(&cgmath::Matrix4::<f32>::from_translation(pos.to_vec()));
						},
					);
				}
			},

			EntityTyped::TestIcosahedron { .. } => {
				if let EntityTyped::TestIcosahedron { phys, .. } = &mut self.typed {
					phys.apply_one_physics_step(
						cgmath::vec3(0.0, 0.0, 0.0),
						chunk_grid,
						block_type_table,
						dt,
						true,
					);
				} else {
					unreachable!()
				};

				// Manage the part.
				let pos = self.pos();
				if let EntityTyped::TestIcosahedron { part_handler, .. } = &mut self.typed {
					part_handler.ensure_is_allocated(
						&mut part_manipulation.part_tables.colored_icosahedron,
						|| PartColoredIcosahedronInstanceData::new(pos).into_pod(),
					);

					part_handler.modify_instance(
						&mut part_manipulation.part_tables.colored_icosahedron,
						|instance| {
							instance
								.set_model_matrix(&cgmath::Matrix4::<f32>::from_translation(pos.to_vec()));
						},
					);
				}
			},

			EntityTyped::_DummyOtherType => panic!(),
		}
	}

	/// Called when an entity is not loaded anymore (be it deleted or simply unloaded (and saved)).
	/// This is not always deletion, on-death effects should not be triggered here.
	///
	/// The entity parts should be deleted here (or they will "leak" and remain
	/// visible and unmoving where they are until the game is closed).
	fn handle_unloading_or_deletion(self, part_tables: &mut PartTables) {
		match self.typed {
			EntityTyped::Block { part_handler, .. } => {
				part_handler.delete(&mut part_tables.textured_cubes);
			},
			EntityTyped::TestIcosahedron { part_handler, .. } => {
				part_handler.delete(&mut part_tables.colored_icosahedron);
			},
			EntityTyped::_DummyOtherType => panic!(),
		}
	}
}

/// All that is needed for entities to be able to manipulate their parts.
pub(crate) struct ForPartManipulation<'a> {
	pub(crate) part_tables: &'a mut PartTables,
	pub(crate) texture_mapping_table: &'a mut TextureMappingTable,
	pub(crate) coords_in_atlas_array_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) queue: &'a wgpu::Queue,
}

/// The entities of a chunk.
pub(crate) struct ChunkEntities {
	pub(crate) coords_span: ChunkCoordsSpan,
	savable: ChunkEntitiesSavable,
}
#[derive(Clone, Serialize, Deserialize)]
struct ChunkEntitiesSavable {
	/// The `Option` is always `Some` and is there to ease the moving of entities out of the vec.
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
	) {
		for entity in self.savable.entities.iter_mut() {
			entity.as_mut().unwrap().apply_one_physics_step(
				chunk_grid,
				block_type_table,
				dt,
				part_manipulation,
			);
		}
		self.savable.entities.retain_mut(|entity| {
			if entity.as_ref().unwrap().to_delete {
				// The entity was flagged for deletion and is now deletd.
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
