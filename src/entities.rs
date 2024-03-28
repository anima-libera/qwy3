use serde::{Deserialize, Serialize};

use crate::{chunks::Block, coords::ChunkCoordsSpan};

#[derive(Clone, Serialize, Deserialize)]
struct Entity {
	pos: cgmath::Point3<f32>,
	typed: EntityTyped,
}
#[derive(Clone, Serialize, Deserialize)]
enum EntityTyped {
	Block { block: Block, motion: cgmath::Vector3<f32> },
}

pub(crate) struct ChunkEntities {
	pub(crate) coords_span: ChunkCoordsSpan,
}
#[derive(Clone, Serialize, Deserialize)]
struct ChunkEntitiesSavable {
	entities: Vec<Entity>,
}

impl ChunkEntities {
	fn new_empty(coords_span: ChunkCoordsSpan) -> ChunkEntities {
		ChunkEntities { coords_span }
	}
}

// TODO:
// Have a few meshes of a few shapes like a cube and all (start with the cube).
// Have one table of data per such mesh, data would be like textures, transformation matrix, etc.
// Each entity can have a list of parts, each part having its own entry in a table of data
//   so that like an entity can be made of shapes and texture and move each shape separately.
// Render these with instanced rendering.
// Get falling blocks working.
