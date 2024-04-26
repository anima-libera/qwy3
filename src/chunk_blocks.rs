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

/// An entry in the palette of a chunk of a `ChunkBlocks`.
#[derive(Clone, Serialize, Deserialize)]
struct BlockPaletteEntry {
	/// The number of blocks in the chunk that refer to this entry.
	/// We keep track of that count so that when it reaches zero then the entry can be removed and
	/// its key is no longer used and can be given to a future new entry.
	instance_count: u32,
	block: Block,
}
type PaletteKey = u32;

/// The blocks of a chunk, stored in a palette compressed way.
///
/// As long as no non-air block is ever placed in a `ChunkBlocks` then it does not allocate memory.
///
/// The palette compression means that actual `Block`s are in a palette, with no duplicates,
/// and the grid of blocks that the chunk is made of is actually a grid of keys (`PaletteKey`)
/// that each refer to a `Block` in the palette.
/// There can be multiple blocks in the grid that use the same key to refer to the same palette
/// entry, this removes some redundancy.
/// Also, the biggest used key's number of bits required to represent its value sets the number of
/// bits used to represent all the keys, this makes the grid of keys be so much smaller and tighter.
#[derive(Clone)]
pub(crate) struct ChunkBlocks {
	pub(crate) coords_span: ChunkCoordsSpan,
	savable: ChunkBlocksSavable,
}
/// Part of `ChunkBlocks` that can be saved/loaded to/from disk.
#[derive(Clone, Serialize, Deserialize)]
struct ChunkBlocksSavable {
	/// The grid of blocks of the chunk is stored here.
	/// If the length is zero then it means the chunk is full of air.
	/// Else, these are keys in the palette, each key being stored on `block_key_size_in_bits` bits.
	block_keys_grid: BitVec,
	/// The number of bits that each key in `block_keys` uses.
	block_key_size_in_bits: usize,
	/// The palette of blocks. Every key in `block_keys` refers to an entry in this palette.
	/// There may be multiple keys that are the same and thus refer to the same entry.
	palette: FxHashMap<PaletteKey, BlockPaletteEntry>,
	/// Next available key for the palette that was never used before.
	next_never_used_palette_key: PaletteKey,
	/// Available palette keys that have been used before.
	available_palette_keys: Vec<PaletteKey>,
	/// If the blocks ever underwent a change since the chunk generation, then it is flagged
	/// as modified. If we want to reduce the size of the saved data then we can avoid saving
	/// non-modified chunks as we could always re-generate them, but modified chunks must be saved.
	modified_since_generation: bool,
	/// The key to the air block type, if it is in the palette.
	air_key: Option<PaletteKey>,
}

impl ChunkBlocks {
	/// Returns a new `ChunkBlocks` full of air that did not allocate anything yet.
	fn new_empty(coords_span: ChunkCoordsSpan) -> ChunkBlocks {
		ChunkBlocks {
			coords_span,
			savable: ChunkBlocksSavable {
				block_keys_grid: BitVec::new(),
				block_key_size_in_bits: 0,
				palette: HashMap::default(),
				next_never_used_palette_key: 0,
				available_palette_keys: Vec::new(),
				modified_since_generation: false,
				air_key: None,
			},
		}
	}

	/// Returns true iff the given key can be represented in the key representation size
	/// currently used. If returns false then calling `add_a_bit_to_block_key_size` will
	/// be required for that key to fit in the representation size of this chunk.
	fn does_the_key_fit(&self, key: PaletteKey) -> bool {
		let key_can_fit_in_that_many_bits = (key.checked_ilog2().unwrap_or(0) + 1) as usize;
		key_can_fit_in_that_many_bits <= self.savable.block_key_size_in_bits
	}

	/// Returns the key of the block at the given internal index.
	fn get_block_key_from_grid(&self, internal_index: usize) -> PaletteKey {
		let index_inf = internal_index * self.savable.block_key_size_in_bits;
		let index_sup_excluded = index_inf + self.savable.block_key_size_in_bits;
		self.savable.block_keys_grid[index_inf..index_sup_excluded].load()
	}

	/// Sets the key of the block at the given internal index to the given key,
	/// without checking if the key can fit the current key representation size.
	fn set_block_key_to_grid(&mut self, internal_index: usize, key: PaletteKey) {
		let index_inf = internal_index * self.savable.block_key_size_in_bits;
		let index_sup_excluded = index_inf + self.savable.block_key_size_in_bits;
		self.savable.block_keys_grid[index_inf..index_sup_excluded].store(key);
	}

	/// The `ChunkBlocks` returned by `new_empty` has no data in allocated vecs and maps
	/// (which means that it contains only air). It avoids using memory for
	/// generated chunks full of air, but it is not suited for actually being modified properly.
	///
	/// This method makes the allocations and fills the grid of blocks with air so that now the
	/// blocks can be modified properly. It is like a delayed initialization that is only called
	/// when necessary to save the memory and the time of the allocations if not needed.
	fn allocate_for_the_first_time_and_fill_with_air(&mut self) {
		// We first put the entry for air in the palette.
		assert_eq!(self.savable.next_never_used_palette_key, 0);
		let key = 0;
		self.savable.next_never_used_palette_key += 1;
		assert!(self.savable.palette.is_empty());
		self.savable.palette.insert(
			key,
			BlockPaletteEntry {
				instance_count: self.coords_span.cd.number_of_blocks_in_a_chunk() as u32,
				block: Block::new_air(),
			},
		);
		self.savable.air_key = Some(key);
		// Then we allocate the bit vec and fill it with zeros (`key` is zero so it works).
		assert_eq!(self.savable.block_key_size_in_bits, 0);
		self.savable.block_key_size_in_bits = 1;
		self.savable.block_keys_grid = BitVec::repeat(
			false,
			self.coords_span.cd.number_of_blocks_in_a_chunk() * self.savable.block_key_size_in_bits,
		);
	}

	/// Makes the key representation size one bit larger. This requires to make all the keys of
	/// `block_keys_grid` one bit larger.
	fn add_a_bit_to_block_key_size(&mut self) {
		// First we resize the bitvec.
		let old_key_size = self.savable.block_key_size_in_bits;
		self.savable.block_key_size_in_bits += 1;
		let new_len =
			self.coords_span.cd.number_of_blocks_in_a_chunk() * self.savable.block_key_size_in_bits;
		self.savable.block_keys_grid.resize(new_len, false);
		// Then we move the old bitvec content to its new position.
		// Now we have availble space at the end of the bitvec (after the old keys) and
		// we must move keys so that they take all the space and that each key must now have one
		// additional bit in its representation size.
		// We can do it from the end, moving the last old key from its old position to its new
		// position (which is further on the right, so we do not overwrite unmoved keys), etc.
		for i in (0..self.coords_span.cd.number_of_blocks_in_a_chunk()).rev() {
			// Get the last not-yet moved key from its old position.
			let key: PaletteKey = {
				let index_inf = i * old_key_size;
				let index_sup_excluded = index_inf + old_key_size;
				self.savable.block_keys_grid[index_inf..index_sup_excluded].load()
			};
			// Move it to its new position, its size now takes one more bit form its old size.
			{
				let index_inf = i * self.savable.block_key_size_in_bits;
				let index_sup_excluded = index_inf + self.savable.block_key_size_in_bits;
				self.savable.block_keys_grid[index_inf..index_sup_excluded].store(key);
			}
		}
	}

	/// Returns a key that is not is use (and that must be used now, or else it is leaked forever).
	/// The key returned always fit the key representation size of this chunk, at the cost of
	/// a call to `add_a_bit_to_block_key_size` if necessary.
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

	/// Avoids leaking the given key no longer in use by remembering it so that it can be associated
	/// to a future new palette entry.
	fn give_back_key_no_longer_in_use(&mut self, key: PaletteKey) {
		self.savable.available_palette_keys.push(key);
	}

	/// Tells the palette that one more instance of the given `block` is in the chunk, and returns
	/// the key corresponding to that block.
	fn add_one_block_instance_to_palette(&mut self, block: Block) -> PaletteKey {
		let already_in_palette =
			self.savable.palette.iter_mut().find(|(_key, palette_entry)| palette_entry.block == block);
		if let Some((&key, entry)) = already_in_palette {
			entry.instance_count += 1;
			key
		} else {
			let key = self.get_new_key();
			if block.type_id == BlockTypeTable::AIR_ID {
				self.savable.air_key = Some(key);
			}
			self.savable.palette.insert(key, BlockPaletteEntry { instance_count: 1, block });
			key
		}
	}

	/// Tells the palette that there is one fewer instance of the block
	/// reffered to by the given `key` in the grid.
	fn remove_one_block_instance_from_palette(&mut self, key: PaletteKey) {
		match self.savable.palette.entry(key) {
			Entry::Vacant(_) => {
				panic!("It makes no sense to remove an instance of which the key is not in use.");
			},
			Entry::Occupied(mut occupied) => {
				let entry = occupied.get_mut();
				assert_ne!(entry.instance_count, 0);
				entry.instance_count -= 1;
				if entry.instance_count == 0 {
					// The palette entry is no longer used, we don't need it anymore.
					let removed_block_entry = occupied.remove();
					if removed_block_entry.block.type_id == BlockTypeTable::AIR_ID {
						self.savable.air_key = None;
					}
					self.give_back_key_no_longer_in_use(key);
				}
			},
		}
	}

	/// Get a view on the block at the given `coords`,
	/// returns `None` if the given coords land outside the chunk's span.
	pub(crate) fn get(&self, coords: BlockCoords) -> Option<BlockView> {
		let internal_index = self.coords_span.internal_index(coords)?;
		Some(if self.savable.block_keys_grid.is_empty() {
			// The chunk is empty, which represents the fact that it is full of air.
			BlockView::new_air()
		} else {
			let key = self.get_block_key_from_grid(internal_index);
			self.savable.palette[&key].block.as_view()
		})
	}

	/// Sets the block at the given `coords` to the given `block`,
	/// does nothing if the given coords land outside the chunk's span.
	pub(crate) fn set(&mut self, coords: BlockCoords, block: Block) {
		if self.coords_span.contains(coords) {
			// Make sure that we have allocated the block keys if that is needed.
			if self.savable.block_keys_grid.is_empty() {
				if block.type_id == BlockTypeTable::AIR_ID {
					// Setting a block to air, but we are already empty (which means full of air)
					// so we have nothing to do.
					return;
				} else {
					// Setting a block to non-air, but we were empty (all air, no setup),
					// so we have to actually allocate the blocks (all set to air).
					self.allocate_for_the_first_time_and_fill_with_air();
				}
			}

			// All is good, we just have to get the block's palette key and put it in the grid.
			// The block that it replaces has to be removed.
			let index = self.coords_span.internal_index(coords).unwrap();
			let key_of_block_to_remove = self.get_block_key_from_grid(index);
			self.remove_one_block_instance_from_palette(key_of_block_to_remove);
			let key_of_block_being_added = self.add_one_block_instance_to_palette(block);
			self.set_block_key_to_grid(index, key_of_block_being_added);
			self.savable.modified_since_generation = true;
		}
	}

	/// Just a look-up, no expensive counting.
	pub(crate) fn contains_only_air(&self) -> bool {
		if self.savable.block_keys_grid.is_empty() {
			// Being empty represents being full of air.
			true
		} else if let Some(air_key) = self.savable.air_key {
			let air_count = self.savable.palette[&air_key].instance_count;
			let block_count = self.coords_span.cd.number_of_blocks_in_a_chunk() as u32;
			air_count == block_count
		} else {
			// Air is not even in the palette, there is no air in the chunk.
			false
		}
	}

	fn contains_non_air(&self) -> bool {
		!self.contains_only_air()
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

// TODO: Add a counter for air blocks and opaque blocks for each face,
// so that `finish_generation` can make a `ChunkCullingInfo` for free (without expensive counting).
//
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
	/// Faces are given in the order of `OrientedAxis::all_the_six_possible_directions`.
	pub(crate) faces: [FaceCullingInfo; 6],
}

#[derive(Clone, Copy)]
pub(crate) enum FaceCullingInfo {
	AllAir,
	AllOpaque,
	SomeAirSomeOpaque,
}

impl ChunkCullingInfo {
	fn new_all_air() -> ChunkCullingInfo {
		ChunkCullingInfo { faces: [FaceCullingInfo::AllAir; 6] }
	}

	pub(crate) fn compute_from_blocks(
		blocks: &ChunkBlocks,
		block_type_table: &Arc<BlockTypeTable>,
	) -> ChunkCullingInfo {
		if !blocks.contains_non_air() {
			return ChunkCullingInfo::new_all_air();
		}

		let mut culling_info = ChunkCullingInfo::new_all_air();

		for (face_index, face) in OrientedAxis::all_the_six_possible_directions().enumerate() {
			let face_culling_info =
				ChunkCullingInfo::get_face_culling_info(face, blocks, block_type_table);
			culling_info.faces[face_index] = face_culling_info;
		}

		culling_info
	}

	fn get_face_culling_info(
		face: OrientedAxis,
		blocks: &ChunkBlocks,
		block_type_table: &Arc<BlockTypeTable>,
	) -> FaceCullingInfo {
		let mut all_air = true;
		let mut all_opaque = true;
		for block_coords in blocks.coords_span.iter_block_coords_on_chunk_face(face) {
			let block_type_id = blocks.get(block_coords).unwrap().type_id;
			let block_type = block_type_table.get(block_type_id).unwrap();
			if !block_type.is_opaque() {
				all_opaque = false;
			}
			if !block_type.is_air() {
				all_air = false;
			}
			if !all_air && !all_opaque {
				break;
			}
		}

		if all_air {
			FaceCullingInfo::AllAir
		} else if all_opaque {
			FaceCullingInfo::AllOpaque
		} else {
			FaceCullingInfo::SomeAirSomeOpaque
		}
	}
}
