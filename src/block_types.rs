use crate::atlas::ATLAS_DIMS;

pub(crate) enum BlockType {
	Air,
	Solid { texture_coords_on_atlas: cgmath::Point2<i32> },
	XShaped { texture_coords_on_atlas: cgmath::Point2<i32> },
	Text,
}

impl BlockType {
	pub(crate) fn is_opaque(&self) -> bool {
		matches!(self, BlockType::Solid { .. })
	}

	pub(crate) fn is_air(&self) -> bool {
		matches!(self, BlockType::Air)
	}

	pub(crate) fn texture_coords_on_atlas(&self) -> Option<cgmath::Point2<i32>> {
		match self {
			BlockType::Solid { texture_coords_on_atlas } => Some(*texture_coords_on_atlas),
			BlockType::XShaped { texture_coords_on_atlas } => Some(*texture_coords_on_atlas),
			BlockType::Air => None,
			BlockType::Text => None,
		}
	}
}

pub(crate) struct BlockTypeTable {
	block_types: Vec<BlockType>,
}

impl BlockTypeTable {
	pub(crate) fn new() -> BlockTypeTable {
		let mut block_types = vec![
			BlockType::Air,
			BlockType::Solid { texture_coords_on_atlas: (0, 0).into() },
			BlockType::Solid { texture_coords_on_atlas: (16, 0).into() },
			BlockType::XShaped { texture_coords_on_atlas: (32, 0).into() },
			BlockType::Solid { texture_coords_on_atlas: (48, 0).into() },
			BlockType::Solid { texture_coords_on_atlas: (64, 0).into() },
			BlockType::Text,
		];

		for y in 4..(ATLAS_DIMS.1 / 16) {
			for x in 0..(ATLAS_DIMS.0 / 16) {
				let coords = (x as i32 * 16, y as i32 * 16);
				block_types.push(BlockType::Solid { texture_coords_on_atlas: coords.into() });
			}
		}

		BlockTypeTable { block_types }
	}

	pub(crate) fn get(&self, id: BlockTypeId) -> Option<&BlockType> {
		self.block_types.get(id as usize)
	}

	pub(crate) const AIR_ID: BlockTypeId = 0;

	pub(crate) fn air_id(&self) -> BlockTypeId {
		BlockTypeTable::AIR_ID
	}

	pub(crate) fn ground_id(&self) -> BlockTypeId {
		1
	}

	pub(crate) fn kinda_grass_id(&self) -> BlockTypeId {
		2
	}

	pub(crate) fn kinda_grass_blades_id(&self) -> BlockTypeId {
		3
	}

	pub(crate) fn kinda_wood_id(&self) -> BlockTypeId {
		4
	}

	pub(crate) fn kinda_leaf_id(&self) -> BlockTypeId {
		5
	}

	pub(crate) fn text_id(&self) -> BlockTypeId {
		6
	}

	pub(crate) fn generated_test_id(&self, index: usize) -> BlockTypeId {
		let id: BlockTypeId = (index + 7).try_into().unwrap();
		id
	}
}

/// Index in the table of block types.
pub(crate) type BlockTypeId = u32;
