use std::sync::Arc;

use cgmath::{EuclideanSpace, MetricSpace};

use crate::{
	block_types::{BlockTypeId, BlockTypeTable},
	chunks::ChunkBlocksBeingGenerated,
	coords::{BlockCoords, CubicCoordsSpan},
	noise::OctavedNoise,
};

#[derive(Clone, Copy)]
pub(crate) struct StructureTypeId {
	pub(crate) index: usize,
}

/// A structure origin is a first step in the generation of a structure.
/// Before even generating the blocks of an instance of some type of structure,
/// we have to decide (in a deterministic way, etc.) where (i.e. from which block)
/// should structures of which type can generate. A structure origin describes
/// where and which type a structure should generate.
///
/// Generating structure origins and having some size limits on structure generations
/// (constraining how far from its origin a structure can place/remove blocks)
/// allows for a chunk to know which origins could place/remove blocks in the chunk
/// and thus should actually have their structure generated.
#[derive(Clone, Copy)]
pub(crate) struct StructureOrigin {
	pub(crate) coords: BlockCoords,
	pub(crate) type_id: StructureTypeId,
}

/// Handles generation of structure origins.
pub(crate) trait StructureOriginGenerator {
	fn get_origins_in_span(&self, span: CubicCoordsSpan) -> Vec<StructureOrigin>;
}

/// The idea of this structure origin generator is that it considers a grid of big cubic cells
/// and uses noise to deterministically place, in each cell, a noise-obtained number of origins
/// at noise-obtained coords in the cell.
pub(crate) struct TestStructureOriginGenerator {
	cell_size: i32,
	/// How many structure origins to generate per cell (min, max_included).
	/// The range can overlap with the negatives, getting a negative number of origins
	/// to generate in a cell will mean zero.
	how_many_min_max: (i32, i32),
	how_many_types: i32,
	noise: OctavedNoise,
}

impl StructureOriginGenerator for TestStructureOriginGenerator {
	fn get_origins_in_span(&self, span: CubicCoordsSpan) -> Vec<StructureOrigin> {
		let block_inf = span.inf;
		let block_sup_included = span.sup_excluded - cgmath::vec3(1, 1, 1);
		let cell_inf = self.block_coords_to_cell_coords(block_inf);
		let cell_sup_included = self.block_coords_to_cell_coords(block_sup_included);
		let cell_span =
			CubicCoordsSpan::with_inf_sup_but_sup_is_included(cell_inf, cell_sup_included);
		let mut origins = vec![];
		for cell_coords in cell_span.iter() {
			self.get_origins_in_cell_and_span(cell_coords, span, &mut origins);
		}
		origins
	}
}

impl TestStructureOriginGenerator {
	pub(crate) fn new(
		seed: i32,
		cell_size: i32,
		how_many_min_max: (i32, i32),
		how_many_types: i32,
	) -> TestStructureOriginGenerator {
		TestStructureOriginGenerator {
			cell_size,
			how_many_min_max,
			how_many_types,
			noise: OctavedNoise::new(1, vec![seed]),
		}
	}

	/// Given a cell, returns how many origins conatined in the cell.
	fn get_cell_origin_number(&self, cell_coords: cgmath::Point3<i32>) -> usize {
		let v = self.noise.sample_i3d_1d(cell_coords, &[1]);
		let (min, max) = self.how_many_min_max;
		let (min, max) = (min as f32, max as f32);
		(v * (max - min + 1.0) + min).max(0.0).round() as usize
	}

	/// Given a cell and an index of an origin in that cell,
	/// returns the coords (in the world) of that structure origin.
	fn get_origin_coords(
		&self,
		cell_coords: cgmath::Point3<i32>,
		origin_index: usize,
	) -> BlockCoords {
		let coords_in_unit_cube = self.noise.sample_i3d_3d(cell_coords, &[origin_index as i32]);
		let coords_in_cell =
			coords_in_unit_cube.map(|x| (x * (self.cell_size as f32 - 0.001)).floor() as i32);
		let cell_coords_in_world = cell_coords * self.cell_size;
		cell_coords_in_world + coords_in_cell.to_vec()
	}

	fn get_origin_type_id(
		&self,
		cell_coords: cgmath::Point3<i32>,
		origin_index: usize,
	) -> StructureTypeId {
		let value = self.noise.sample_i3d_1d(cell_coords, &[origin_index as i32]);
		let type_id_index = ((self.how_many_types as f32 - 0.0001) * value).floor() as usize;
		StructureTypeId { index: type_id_index }
	}

	/// Given a cell and a block span,
	/// pushes in the given vec the origins in the cell that are also in the span.
	fn get_origins_in_cell_and_span(
		&self,
		cell_coords: cgmath::Point3<i32>,
		span: CubicCoordsSpan,
		add_origins_in_there: &mut Vec<StructureOrigin>,
	) {
		let origin_number = self.get_cell_origin_number(cell_coords);
		for origin_index in 0..origin_number {
			let origin_coords = self.get_origin_coords(cell_coords, origin_index);
			let origin_type_id = self.get_origin_type_id(cell_coords, origin_index);
			if span.contains(origin_coords) {
				add_origins_in_there
					.push(StructureOrigin { coords: origin_coords, type_id: origin_type_id })
			}
		}
	}

	fn block_coords_to_cell_coords(&self, block_coords: BlockCoords) -> cgmath::Point3<i32> {
		block_coords.map(|x| x.div_euclid(self.cell_size))
	}
}

type TerrainGenerator<'a> = dyn Fn(BlockCoords) -> BlockTypeId + 'a;

/// All that is needed for the generation of a structure instance.
/// A structure instance is just one structure with an origin position
/// (and a type, though that is given in an other way).
pub(crate) struct StructureInstanceGenerationContext<'a> {
	pub(crate) origin: StructureOrigin,
	pub(crate) chunk_blocks: &'a mut ChunkBlocksBeingGenerated,
	pub(crate) _origin_generator: &'a dyn StructureOriginGenerator,
	pub(crate) block_type_table: &'a Arc<BlockTypeTable>,
	pub(crate) terrain_generator: &'a TerrainGenerator<'a>,
}

/// When a structure generation wants to place a block, it may want to do so in some way
/// that is specified by this type. For example, the structure generation might want to
/// place some block somewhere but only if it replaces air, well this would be specified
/// by this type.
pub(crate) struct BlockPlacing {
	pub(crate) block_type_to_place: BlockTypeId,
	pub(crate) only_place_on_air: bool,
}

impl<'a> StructureInstanceGenerationContext<'a> {
	pub(crate) fn place_block(&mut self, block_placing: &BlockPlacing, coords: BlockCoords) {
		let shall_place_block = !block_placing.only_place_on_air
			|| self
				.chunk_blocks
				.get(coords)
				.is_some_and(|block| self.block_type_table.get(block.type_id).unwrap().is_air());
		if shall_place_block {
			self.chunk_blocks.set_simple(coords, block_placing.block_type_to_place);
		}
	}

	pub(crate) fn place_ball(
		&mut self,
		block_placing: &BlockPlacing,
		center: cgmath::Point3<f32>,
		radius: f32,
	) {
		let ball_inf = (center - cgmath::vec3(1.0, 1.0, 1.0) * radius).map(|x| x.floor() as i32);
		let ball_sup = (center + cgmath::vec3(1.0, 1.0, 1.0) * radius).map(|x| x.ceil() as i32);
		let ball_span = CubicCoordsSpan::with_inf_sup_but_sup_is_included(ball_inf, ball_sup);
		let chunk_span = CubicCoordsSpan::from_chunk_span(self.chunk_blocks.coords_span());
		if let Some(span) = chunk_span.intersection(&ball_span) {
			for coords in span.iter() {
				if coords.map(|x| x as f32).distance(center) < radius {
					self.place_block(block_placing, coords);
				}
			}
		}
	}
}

/// Generates a structure instance of one specific type.
pub(crate) type StructureTypeInstanceGenerator<'a> =
	dyn Fn(StructureInstanceGenerationContext) + 'a;
