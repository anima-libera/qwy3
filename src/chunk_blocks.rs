use std::{
	collections::{hash_map::Entry, HashMap},
	io::{Read, Write},
	sync::Arc,
};

use bitvec::{field::BitField, vec::BitVec};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::{
	block_types::{BlockTypeId, BlockTypeTable},
	coords::{BlockCoords, ChunkCoordsSpan, OrientedAxis},
	saves::{Save, WhichChunkFile},
};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Block {
	pub(crate) type_id: BlockTypeId,
	pub(crate) data: Option<BlockData>,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) enum BlockData {
	Text(String),
}

impl From<BlockTypeId> for Block {
	fn from(type_id: BlockTypeId) -> Block {
		Block { type_id, data: None }
	}
}

impl Block {
	fn new_air() -> Block {
		Block { type_id: BlockTypeTable::AIR_ID, data: None }
	}

	fn as_view(&self) -> BlockView<'_> {
		BlockView { type_id: self.type_id, data: self.data.as_ref() }
	}
}

pub(crate) struct BlockView<'a> {
	pub(crate) type_id: BlockTypeId,
	pub(crate) data: Option<&'a BlockData>,
}

impl<'a> BlockView<'a> {
	fn new_air() -> BlockView<'a> {
		BlockView { type_id: BlockTypeTable::AIR_ID, data: None }
	}

	pub(crate) fn as_owned_block(&self) -> Block {
		Block { type_id: self.type_id, data: self.data.cloned() }
	}
}

#[derive(Clone, Serialize, Deserialize)]
struct BlockPaletteEntry {
	instance_count: u32,
	block: Block,
}
type PaletteKey = u32;

/// The blocks of a chunk, stored in a palette compressed way.
///
/// As long as no non-air block is ever placed in a `ChunkBlocks` then it does not allocate memory.
#[derive(Clone)]
pub(crate) struct ChunkBlocks {
	pub(crate) coords_span: ChunkCoordsSpan,
	savable: ChunkBlocksSavable,
}
/// Part of `ChunkBlocks` that can be saved/loaded to/from disk.
#[derive(Clone, Serialize, Deserialize)]
struct ChunkBlocksSavable {
	/// If the length is zero then it means the chunk is full of air.
	/// Else, these are keys in the palette.
	block_keys: BitVec,
	block_key_size_in_bits: usize,
	/// The palette of blocks.
	palette: FxHashMap<PaletteKey, BlockPaletteEntry>,
	/// Next available key for the palette that was never used before.
	next_never_used_palette_key: PaletteKey,
	/// Available palette keys that have been used before.
	available_palette_keys: Vec<PaletteKey>,
	/// If the blocks ever underwent a change since the chunk generation, then it is flagged
	/// as `modified`. If we want to reduce the size of the saved data then we can avoid saving
	/// non-modified chunks as we could always re-generate them, but modified chunks must be saved.
	modified_since_generation: bool,
}

impl ChunkBlocks {
	fn new_empty(coords_span: ChunkCoordsSpan) -> ChunkBlocks {
		ChunkBlocks {
			coords_span,
			savable: ChunkBlocksSavable {
				block_keys: BitVec::new(),
				block_key_size_in_bits: 0,
				palette: HashMap::default(),
				next_never_used_palette_key: 0,
				available_palette_keys: Vec::new(),
				modified_since_generation: false,
			},
		}
	}

	fn does_the_key_fit(&self, key: PaletteKey) -> bool {
		let key_can_fit_in_that_many_bits = (key.checked_ilog2().unwrap_or(0) + 1) as usize;
		key_can_fit_in_that_many_bits <= self.savable.block_key_size_in_bits
	}

	fn get_block_key(&self, internal_index: usize) -> PaletteKey {
		let index_inf = internal_index * self.savable.block_key_size_in_bits;
		let index_sup_excluded = index_inf + self.savable.block_key_size_in_bits;
		self.savable.block_keys[index_inf..index_sup_excluded].load()
	}

	fn set_block_key(&mut self, internal_index: usize, key: PaletteKey) {
		let index_inf = internal_index * self.savable.block_key_size_in_bits;
		let index_sup_excluded = index_inf + self.savable.block_key_size_in_bits;
		self.savable.block_keys[index_inf..index_sup_excluded].store(key);
	}

	fn allocate_for_the_first_time_and_fill_with_air(&mut self) {
		// We first put the entry for air in the palette.
		assert_eq!(self.savable.next_never_used_palette_key, 0);
		let key = 0;
		self.savable.next_never_used_palette_key += 1;
		assert!(self.savable.palette.is_empty());
		self.savable.palette.insert(
			key,
			BlockPaletteEntry {
				instance_count: self.coords_span.cd.number_of_blocks() as u32,
				block: Block::new_air(),
			},
		);
		// Then we allocate the bit vec and fill it with zeros (`key` is zero so it works).
		assert_eq!(self.savable.block_key_size_in_bits, 0);
		self.savable.block_key_size_in_bits = 1;
		self.savable.block_keys = BitVec::repeat(
			false,
			self.coords_span.cd.number_of_blocks() * self.savable.block_key_size_in_bits,
		);
	}

	fn add_a_bit_to_block_key_size(&mut self) {
		// First we resize the bitvec.
		let old_key_size = self.savable.block_key_size_in_bits;
		self.savable.block_key_size_in_bits += 1;
		let new_len = self.coords_span.cd.number_of_blocks() * self.savable.block_key_size_in_bits;
		self.savable.block_keys.resize(new_len, false);
		// Then we move the old bitvec content to its new position.
		// Now we have availble space at the end of the bitvec (after the old keys) and
		// we must move keys so that they take all the space and that each key must now have one
		// additional bit in its representation size.
		// We can do it from the end, moving the last old key from its old position to its new
		// position (which is further on the right, so we do not overwrite unmoved keys), etc.
		for i in (0..self.coords_span.cd.number_of_blocks()).rev() {
			// Get the last not-yet moved key from its old position.
			let key: PaletteKey = {
				let index_inf = i * old_key_size;
				let index_sup_excluded = index_inf + old_key_size;
				self.savable.block_keys[index_inf..index_sup_excluded].load()
			};
			// Move it to its new position, its size now takes one more bit form its old size.
			{
				let index_inf = i * self.savable.block_key_size_in_bits;
				let index_sup_excluded = index_inf + self.savable.block_key_size_in_bits;
				self.savable.block_keys[index_inf..index_sup_excluded].store(key);
			}
		}
	}

	fn get_new_key(&mut self) -> PaletteKey {
		if let Some(new_key) = self.savable.available_palette_keys.pop() {
			// There is a previously-used key available. This does not risk to
			// `add_a_bit_to_block_key_size` so we prefer resuing old keys.
			new_key
		} else {
			// There is no old key that are available for reuse, so we have to get new keys
			// that were never used before on this chunk, at the risk of having to use more bits
			// on each key if the new key does not fit in the current number of bits per key.
			let new_key = self.savable.next_never_used_palette_key;
			self.savable.next_never_used_palette_key += 1;
			while !self.does_the_key_fit(new_key) {
				self.add_a_bit_to_block_key_size();
			}
			new_key
		}
	}

	fn give_back_key_no_longer_in_use(&mut self, key: PaletteKey) {
		self.savable.available_palette_keys.push(key);
	}

	fn add_one_block_instance_to_palette(&mut self, block: Block) -> PaletteKey {
		let already_in_palette =
			self.savable.palette.iter_mut().find(|(_key, palette_entry)| palette_entry.block == block);
		if let Some((&key, entry)) = already_in_palette {
			entry.instance_count += 1;
			key
		} else {
			let key = self.get_new_key();
			self.savable.palette.insert(key, BlockPaletteEntry { instance_count: 1, block });
			key
		}
	}

	fn remove_one_block_instance_from_palette(&mut self, key: PaletteKey) {
		match self.savable.palette.entry(key) {
			Entry::Vacant(_) => panic!(),
			Entry::Occupied(mut occupied) => {
				let entry = occupied.get_mut();
				entry.instance_count = entry.instance_count.saturating_sub(1);
				if entry.instance_count == 0 {
					occupied.remove();
					self.give_back_key_no_longer_in_use(key);
				}
			},
		}
	}

	/// Get a view on a block, returns `None` if the given coords land outside the chunk's span.
	pub(crate) fn get(&self, coords: BlockCoords) -> Option<BlockView> {
		let internal_index = self.coords_span.internal_index(coords)?;
		Some(if self.savable.block_keys.is_empty() {
			BlockView::new_air()
		} else {
			let key = self.get_block_key(internal_index);
			self.savable.palette[&key].block.as_view()
		})
	}

	pub(crate) fn set(&mut self, coords: BlockCoords, block: Block) {
		if self.coords_span.contains(coords) {
			// Make sure that we have allocated the block keys if that is needed.
			if self.savable.block_keys.is_empty() {
				if block.type_id == BlockTypeTable::AIR_ID {
					// Setting a block to air, but we are already empty, we have nothing to do.
					return;
				} else {
					// Setting a block to non-air, but we were empty (all air, no setup for other blocks),
					// so we have to actually allocate the blocks (all set to air).
					self.allocate_for_the_first_time_and_fill_with_air();
				}
			}

			// All is good, we just have to get the block's palette key and put it in the grid.
			let index = self.coords_span.internal_index(coords).unwrap();
			let key_of_block_to_remove = self.get_block_key(index);
			self.remove_one_block_instance_from_palette(key_of_block_to_remove);
			let key_of_block_being_added = self.add_one_block_instance_to_palette(block);
			self.set_block_key(index, key_of_block_being_added);
			self.savable.modified_since_generation = true;
		}
	}

	fn may_contain_non_air(&self) -> bool {
		!self.savable.block_keys.is_empty()
	}

	pub(crate) fn was_modified_since_generation(&self) -> bool {
		self.savable.modified_since_generation
	}

	pub(crate) fn save(&self, save: &Arc<Save>) {
		// TODO: Use buffered streams instead of full vecs of data as intermediary steps.
		let chunk_file_path =
			save.chunk_file_path(self.coords_span.chunk_coords, WhichChunkFile::Blocks);
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

	pub(crate) fn load_from_save(
		coords_span: ChunkCoordsSpan,
		save: &Arc<Save>,
	) -> Option<ChunkBlocks> {
		// TODO: Use buffered streams instead of full vecs of data as intermediary steps.
		let chunk_file_path = save.chunk_file_path(coords_span.chunk_coords, WhichChunkFile::Blocks);
		let chunk_file = save.get_file_io(chunk_file_path);
		let compressed_data = chunk_file.read(false)?;
		let mut uncompressed_data = vec![];
		{
			let mut decoder = flate2::bufread::DeflateDecoder::new(compressed_data.as_slice());
			decoder.read_to_end(&mut uncompressed_data).unwrap();
		}
		let savable: ChunkBlocksSavable = rmp_serde::decode::from_slice(&uncompressed_data).unwrap();
		Some(ChunkBlocks { coords_span, savable })
	}
}

/// Wrapper around `ChunkBlocks` to be used for generating chunk blocks.
/// It ensures that even after modifying the chunk blocks (in the process of generating it)
/// the resulting `ChunkBlocks` will not be flagged as `modified`.
pub(crate) struct ChunkBlocksBeingGenerated(ChunkBlocks);

impl ChunkBlocksBeingGenerated {
	pub(crate) fn new_empty(coords_span: ChunkCoordsSpan) -> ChunkBlocksBeingGenerated {
		ChunkBlocksBeingGenerated(ChunkBlocks::new_empty(coords_span))
	}

	pub(crate) fn coords_span(&self) -> ChunkCoordsSpan {
		self.0.coords_span
	}
	pub(crate) fn get(&self, coords: BlockCoords) -> Option<BlockView> {
		self.0.get(coords)
	}
	pub(crate) fn set(&mut self, coords: BlockCoords, block: Block) {
		self.0.set(coords, block);
	}
	pub(crate) fn set_id(&mut self, coords: BlockCoords, block_id: BlockTypeId) {
		self.set(coords, Block::from(block_id));
	}

	pub(crate) fn finish_generation(mut self) -> ChunkBlocks {
		self.0.savable.modified_since_generation = false;
		self.0
	}
}

/// Information that can be used to decide if some chunks should not be loaded or be unloaded.
#[derive(Clone)]
pub(crate) struct ChunkCullingInfo {
	pub(crate) all_air: bool,
	pub(crate) _all_opaque: bool,
	pub(crate) all_opaque_faces: Vec<OrientedAxis>,
	pub(crate) all_air_faces: Vec<OrientedAxis>,
}

impl ChunkCullingInfo {
	pub(crate) fn compute_from_blocks(
		blocks: &ChunkBlocks,
		block_type_table: &Arc<BlockTypeTable>,
	) -> ChunkCullingInfo {
		if !blocks.may_contain_non_air() {
			return ChunkCullingInfo {
				all_air: true,
				_all_opaque: false,
				all_opaque_faces: vec![],
				all_air_faces: OrientedAxis::all_the_six_possible_directions().collect(),
			};
		}

		let mut all_air = true;
		let mut all_opaque = true;
		for coords in blocks.coords_span.iter_coords() {
			let block = blocks.get(coords).unwrap();
			let block_type = block_type_table.get(block.type_id).unwrap();
			if !block_type.is_air() {
				all_air = false;
			}
			if !block_type.is_opaque() {
				all_opaque = false;
			}
			if (!all_air) && (!all_opaque) {
				break;
			}
		}

		let mut all_opaque_faces = vec![];
		for face in OrientedAxis::all_the_six_possible_directions() {
			if ChunkCullingInfo::face_is_all_opaque(face, blocks, block_type_table) {
				all_opaque_faces.push(face);
			}
		}

		let mut all_air_faces = vec![];
		for face in OrientedAxis::all_the_six_possible_directions() {
			if ChunkCullingInfo::face_is_all_air(face, blocks, block_type_table) {
				all_air_faces.push(face);
			}
		}

		ChunkCullingInfo { all_air, _all_opaque: all_opaque, all_opaque_faces, all_air_faces }
	}

	fn face_is_all_opaque(
		face: OrientedAxis,
		blocks: &ChunkBlocks,
		block_type_table: &Arc<BlockTypeTable>,
	) -> bool {
		let mut all_opaque = true;
		for block_coords in blocks.coords_span.iter_block_coords_on_chunk_face(face) {
			let block_type_id = blocks.get(block_coords).unwrap().type_id;
			let block_type = block_type_table.get(block_type_id).unwrap();
			if !block_type.is_opaque() {
				all_opaque = false;
				break;
			}
		}
		all_opaque
	}

	fn face_is_all_air(
		face: OrientedAxis,
		blocks: &ChunkBlocks,
		block_type_table: &Arc<BlockTypeTable>,
	) -> bool {
		let mut all_air = true;
		for block_coords in blocks.coords_span.iter_block_coords_on_chunk_face(face) {
			let block_type_id = blocks.get(block_coords).unwrap().type_id;
			let block_type = block_type_table.get(block_type_id).unwrap();
			if !block_type.is_air() {
				all_air = false;
				break;
			}
		}
		all_air
	}
}
