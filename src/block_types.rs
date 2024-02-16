use crate::atlas::ATLAS_DIMS;

pub(crate) enum BlockType {
	Air,
	Solid { texture_coords_on_atlas: cgmath::Point2<i32> },
	XShaped { texture_coords_on_atlas: cgmath::Point2<i32> },
}

impl BlockType {
	pub(crate) fn is_opaque(&self) -> bool {
		matches!(self, BlockType::Solid { .. })
	}

	pub(crate) fn is_air(&self) -> bool {
		matches!(self, BlockType::Air)
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
		if id.value < 0 {
			None
		} else {
			self.block_types.get(id.value as usize)
		}
	}

	pub(crate) fn air_id(&self) -> BlockTypeId {
		BlockTypeId::new(0)
	}

	pub(crate) fn ground_id(&self) -> BlockTypeId {
		BlockTypeId::new(1)
	}

	pub(crate) fn kinda_grass_id(&self) -> BlockTypeId {
		BlockTypeId::new(2)
	}

	pub(crate) fn kinda_grass_blades_id(&self) -> BlockTypeId {
		BlockTypeId::new(3)
	}

	pub(crate) fn kinda_wood_id(&self) -> BlockTypeId {
		BlockTypeId::new(4)
	}

	pub(crate) fn kinda_leaf_id(&self) -> BlockTypeId {
		BlockTypeId::new(5)
	}

	pub(crate) fn generated_test_id(&self, index: usize) -> BlockTypeId {
		let id: i16 = (index + 6).try_into().unwrap();
		BlockTypeId::new(id)
	}
}

#[derive(Clone, Copy)]
pub(crate) struct BlockTypeId {
	/// Positive values are indices in the table of block types.
	/// Negative values will be used as ids in a table of blocks that have data, maybe?
	pub(crate) value: i16,
}

impl BlockTypeId {
	fn new(value: i16) -> BlockTypeId {
		BlockTypeId { value }
	}
}
