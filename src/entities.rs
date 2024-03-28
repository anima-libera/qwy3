use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
	block_types::BlockTypeTable,
	chunks::{Block, ChunkGrid},
	coords::{AlignedBox, ChunkCoordsSpan},
};

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Entity {
	pos: cgmath::Point3<f32>,
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
		Entity { pos, typed: EntityTyped::Block { block, motion } }
	}

	pub(crate) fn pos(&self) -> cgmath::Point3<f32> {
		self.pos
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
	) -> EntityPhysicsStepKeepOrDelete {
		match &mut self.typed {
			EntityTyped::Block { block, motion } => {
				self.pos += *motion * 144.0 * dt.as_secs_f32();
				motion.z -= 1.0 * 0.35 * dt.as_secs_f32();
				*motion /= 1.0 + 0.0015 * 144.0 * dt.as_secs_f32();

				let block = block.clone(); // TODO: Don't clone this here, this is a waste.
				let collides = self.collides_with_blocks(chunk_grid, block_type_table);
				if collides {
					let coords = self.pos.map(|x| x.round() as i32);
					chunk_grid.set_block_and_request_updates_to_meshes(coords, block);
					EntityPhysicsStepKeepOrDelete::Delete
				} else {
					EntityPhysicsStepKeepOrDelete::Keep
				}
			},
		}
	}
}

enum EntityPhysicsStepKeepOrDelete {
	Keep,
	/// The entity is to be deleted.
	Delete,
}

pub(crate) struct ChunkEntities {
	pub(crate) coords_span: ChunkCoordsSpan,
	savable: ChunkEntitiesSavable,
}
#[derive(Clone, Serialize, Deserialize)]
struct ChunkEntitiesSavable {
	entities: Vec<Entity>,
	entities_coming_from_other_chunks: Vec<Entity>,
}

impl ChunkEntities {
	pub(crate) fn new_empty(coords_span: ChunkCoordsSpan) -> ChunkEntities {
		ChunkEntities {
			coords_span,
			savable: ChunkEntitiesSavable {
				entities: vec![],
				entities_coming_from_other_chunks: vec![],
			},
		}
	}

	pub(crate) fn iter_entities(&self) -> impl Iterator<Item = &Entity> {
		self.savable.entities.iter()
	}
	pub(crate) fn count_entities(&self) -> usize {
		self.savable.entities.len()
	}

	pub(crate) fn spawn_entity(&mut self, entity: Entity) {
		self.savable.entities.push(entity);
	}

	pub(crate) fn apply_one_physics_step(
		&mut self,
		chunk_grid: &mut ChunkGrid,
		block_type_table: &Arc<BlockTypeTable>,
		dt: std::time::Duration,
	) {
		let mut entities_to_delete_indices = vec![];
		for (index, entity) in self.savable.entities.iter_mut().enumerate() {
			let keep_or_delete = entity.apply_one_physics_step(chunk_grid, block_type_table, dt);
			if matches!(keep_or_delete, EntityPhysicsStepKeepOrDelete::Delete) {
				entities_to_delete_indices.push(index);
			}
		}
		for index in entities_to_delete_indices.into_iter().rev() {
			self.savable.entities.remove(index);
			// TODO: Also handle moving entities to their chunk if it changed here.
		}
	}
}

// TODO:
// Have a few meshes of a few shapes like a cube and all (start with the cube).
// Have one table of data per such mesh, data would be like textures, transformation matrix, etc.
// Each entity can have a list of parts, each part having its own entry in a table of data
//   so that like an entity can be made of shapes and texture and move each shape separately.
// Render these with instanced rendering.
// Get falling blocks working.
// Handle the case where an entity spawns new entities in its own chunk.
