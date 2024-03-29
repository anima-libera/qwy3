use std::{
	io::{Read, Write},
	sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::{
	block_types::BlockTypeTable,
	chunks::{Block, ChunkGrid},
	coords::{AlignedBox, ChunkCoords, ChunkCoordsSpan, ChunkDimensions},
	saves::{Save, WhichChunkFile},
};

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Entity {
	pos: cgmath::Point3<f32>,
	to_delete: bool,
	typed: EntityTyped,
}
#[derive(Clone, Serialize, Deserialize)]
enum EntityTyped {
	Block { block: Block, motion: cgmath::Vector3<f32> },
}

impl Entity {
	pub(crate) fn new_block(
		block: Block,
		pos: cgmath::Point3<f32>,
		motion: cgmath::Vector3<f32>,
	) -> Entity {
		Entity {
			pos,
			to_delete: false,
			typed: EntityTyped::Block { block, motion },
		}
	}

	pub(crate) fn pos(&self) -> cgmath::Point3<f32> {
		self.pos
	}

	pub(crate) fn chunk_coords(&self, cd: ChunkDimensions) -> ChunkCoords {
		let coords = self.pos().map(|x| x.round() as i32);
		cd.world_coords_to_containing_chunk_coords(coords)
	}

	pub(crate) fn aligned_box(&self) -> Option<AlignedBox> {
		match self.typed {
			EntityTyped::Block { .. } => {
				Some(AlignedBox { pos: self.pos, dims: cgmath::vec3(1.0, 1.0, 1.0) })
			},
		}
	}

	fn collides_with_blocks(
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
			let coords = self.pos.map(|x| x.round() as i32);
			chunk_grid
				.get_block(coords)
				.is_some_and(|block| block_type_table.get(block.type_id).unwrap().is_opaque())
		}
	}

	fn apply_one_physics_step(
		&mut self,
		chunk_grid: &mut ChunkGrid,
		block_type_table: &Arc<BlockTypeTable>,
		dt: std::time::Duration,
	) {
		match &mut self.typed {
			EntityTyped::Block { block, motion } => {
				self.pos += *motion * 144.0 * dt.as_secs_f32();
				motion.z -= 1.0 * 0.35 * dt.as_secs_f32();
				*motion /= 1.0 + 0.0015 * 144.0 * dt.as_secs_f32();

				let block = block.clone(); // TODO: Don't clone this here, this is a waste.
				let collides = self.collides_with_blocks(chunk_grid, block_type_table);
				if collides {
					let coords = self.pos.map(|x| x.round() as i32);

					let chunk_coords = chunk_grid.cd().world_coords_to_containing_chunk_coords(coords);
					let is_loaded = chunk_grid.is_loaded(chunk_coords);

					if is_loaded {
						chunk_grid.set_block_and_request_updates_to_meshes(coords, block);
						self.to_delete = true;
					}
				}
			},
		}
	}
}

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
	) {
		for entity in self.savable.entities.iter_mut() {
			entity.as_mut().unwrap().apply_one_physics_step(chunk_grid, block_type_table, dt);
		}
		self.savable.entities.retain_mut(|entity| {
			if entity.as_ref().unwrap().to_delete {
				false
			} else {
				let entity_chunk_coords = entity.as_ref().unwrap().chunk_coords(chunk_grid.cd());
				if entity_chunk_coords != self.coords_span.chunk_coords {
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

pub(crate) struct ChunkEntitiesPhysicsStepChangeOfChunk {
	pub(crate) new_chunk: ChunkCoords,
	pub(crate) entity: Entity,
}

// TODO:
// Have a few meshes of a few shapes like a cube and all (start with the cube).
// Have one table of data per such mesh, data would be like textures, transformation matrix, etc.
// Each entity can have a list of parts, each part having its own entry in a table of data
//   so that like an entity can be made of shapes and texture and move each shape separately.
// Render these with instanced rendering.
// Get falling blocks working.
// Handle the case where an entity spawns new entities in its own chunk.
