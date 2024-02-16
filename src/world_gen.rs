use std::{cmp::Ordering, f32::consts::TAU, sync::Arc};

use cgmath::{EuclideanSpace, InnerSpace, MetricSpace};
use clap::ValueEnum;
use smallvec::SmallVec;

use crate::{
	chunks::BlockTypeId,
	coords::{iter_3d_rect_inf_sup_excluded, CubicCoordsSpan, NonOrientedAxis},
};
pub(crate) use crate::{
	chunks::{BlockTypeTable, ChunkBlocks},
	coords::{BlockCoords, ChunkCoordsSpan},
	noise,
};

pub trait WorldGenerator {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks;
}

#[derive(Clone, Copy, ValueEnum)]
pub enum WhichWorldGenerator {
	Default,
	Flat,
	Empty,
	Lines01,
	Volumes01,
	BallsSameSize,
	BallsDifferentSizes,
	LinksXRaw,
	LinksX,
	Links01,
	LinksGround,
	LinksCaves,
	Links02,
	LinksFlat,
	SkyIslands,
	Volumes02,
	Volumes03,
	Height01,
	Plane01,
	WierdTerrain01,
	Plane02,
	WierdTerrain02,
	Height02,
	HeightBiomes,
	HeightBiomesVolume,
	Height03,
	StructuresPoc,
	StructuresLinksPoc,
	StructuresTrees,
	StructuresSpikes,
	Lines02,
	Lines03,
	StructuresLinksSmooth,
	StructuresEnginePoc,
	StructuresGeneratedBlocks,
}

impl WhichWorldGenerator {
	pub fn get_the_actual_generator(self, seed: i32) -> Arc<dyn WorldGenerator + Sync + Send> {
		match self {
			WhichWorldGenerator::Default => Arc::new(DefaultWorldGenerator { seed }),
			WhichWorldGenerator::Flat => Arc::new(FlatWorldGenerator {}),
			WhichWorldGenerator::Empty => Arc::new(EmptyWorldGenerator {}),
			WhichWorldGenerator::Lines01 => Arc::new(WorldGeneratorLines01 { seed }),
			WhichWorldGenerator::Volumes01 => Arc::new(WorldGeneratorVolumes01 { seed }),
			WhichWorldGenerator::BallsSameSize => Arc::new(WorldGeneratorBallsSameSize { seed }),
			WhichWorldGenerator::BallsDifferentSizes => {
				Arc::new(WorldGeneratorBallsDifferentSizes { seed })
			},
			WhichWorldGenerator::LinksXRaw => Arc::new(WorldGeneratorLinksXRaw { seed }),
			WhichWorldGenerator::LinksX => Arc::new(WorldGeneratorLinksX { seed }),
			WhichWorldGenerator::Links01 => Arc::new(WorldGeneratorLinks { seed }),
			WhichWorldGenerator::LinksGround => Arc::new(WorldGeneratorLinksGround { seed }),
			WhichWorldGenerator::LinksCaves => Arc::new(WorldGeneratorLinksCaves { seed }),
			WhichWorldGenerator::Links02 => Arc::new(WorldGeneratorLinks02 { seed }),
			WhichWorldGenerator::LinksFlat => Arc::new(WorldGeneratorLinksFlat { seed }),
			WhichWorldGenerator::SkyIslands => Arc::new(WorldGeneratorSkyIslands { seed }),
			WhichWorldGenerator::Volumes02 => Arc::new(WorldGeneratorVolumes02 { seed }),
			WhichWorldGenerator::Volumes03 => Arc::new(WorldGeneratorVolumes03 { seed }),
			WhichWorldGenerator::Height01 => Arc::new(WorldGeneratorHeight01 { seed }),
			WhichWorldGenerator::Plane01 => Arc::new(WorldGeneratorPlane01 { seed }),
			WhichWorldGenerator::WierdTerrain01 => Arc::new(WorldGeneratorWierdTerrain01 { seed }),
			WhichWorldGenerator::Plane02 => Arc::new(WorldGeneratorPlane02 { seed }),
			WhichWorldGenerator::WierdTerrain02 => Arc::new(WorldGeneratorWierdTerrain02 { seed }),
			WhichWorldGenerator::Height02 => Arc::new(WorldGeneratorHeight02 { seed }),
			WhichWorldGenerator::HeightBiomes => Arc::new(WorldGeneratorHeightBiomes { seed }),
			WhichWorldGenerator::HeightBiomesVolume => {
				Arc::new(WorldGeneratorHeightBiomesVolume { seed })
			},
			WhichWorldGenerator::Height03 => Arc::new(WorldGeneratorHeight03 { seed }),
			WhichWorldGenerator::StructuresPoc => Arc::new(WorldGeneratorStructuresPoc { seed }),
			WhichWorldGenerator::StructuresLinksPoc => {
				Arc::new(WorldGeneratorStructuresLinksPoc { seed })
			},
			WhichWorldGenerator::StructuresTrees => Arc::new(WorldGeneratorStructuresTrees { seed }),
			WhichWorldGenerator::StructuresSpikes => Arc::new(WorldGeneratorStructuresSpikes { seed }),
			WhichWorldGenerator::Lines02 => Arc::new(WorldGeneratorLines02 { seed }),
			WhichWorldGenerator::Lines03 => Arc::new(WorldGeneratorLines03 { seed }),
			WhichWorldGenerator::StructuresLinksSmooth => {
				Arc::new(WorldGeneratorStructuresLinksSmooth { seed })
			},
			WhichWorldGenerator::StructuresEnginePoc => {
				Arc::new(WorldGeneratorStructuresEnginePoc { seed })
			},
			WhichWorldGenerator::StructuresGeneratedBlocks => {
				Arc::new(WorldGeneratorStructuresGeneratedBlocks { seed })
			},
		}
	}
}

pub struct DefaultWorldGenerator {
	pub seed: i32,
}

impl WorldGenerator for DefaultWorldGenerator {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(5, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(5, vec![self.seed, 2]);
		let noise_no_grass = noise::OctavedNoise::new(5, vec![self.seed, 3]);
		let noise_grass_a = noise::OctavedNoise::new(2, vec![self.seed, 1, 1]);
		let noise_grass_b = noise::OctavedNoise::new(2, vec![self.seed, 1, 2]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 100.0;
			let a = noise_a.sample_3d_1d(coordsf / scale, &[]);
			let b = noise_b.sample_3d_1d(coordsf / scale, &[]);
			(coordsf.z < b * 5.0 && a < 0.7) || b < 0.3
		};
		let coords_to_grass = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let d = noise_grass_a.sample_3d_1d(coordsf / scale, &[]);
			let density = if d < 0.1 {
				d * 0.9 + 0.1
			} else if d < 0.3 {
				0.1
			} else {
				0.01
			};
			noise_grass_b.sample_3d_1d(coordsf, &[]) < density
		};
		let coords_to_no_grass = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 75.0;
			noise_no_grass.sample_3d_1d(coordsf / scale, &[]) < 0.25
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					let no_grass = coords_to_no_grass(coords);
					if no_grass {
						block_type_table.ground_id()
					} else {
						block_type_table.kinda_grass_id()
					}
				}
			} else {
				let ground_below = coords_to_ground(coords + cgmath::vec3(0, 0, -1));
				if ground_below {
					let no_grass_below = coords_to_no_grass(coords + cgmath::vec3(0, 0, -1));
					if no_grass_below {
						block_type_table.air_id()
					} else if coords_to_grass(coords) {
						block_type_table.kinda_grass_blades_id()
					} else {
						block_type_table.air_id()
					}
				} else {
					block_type_table.air_id()
				}
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct FlatWorldGenerator {}

impl WorldGenerator for FlatWorldGenerator {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let block = match coords.z.cmp(&0) {
				Ordering::Less => block_type_table.ground_id(),
				Ordering::Equal => block_type_table.kinda_grass_id(),
				Ordering::Greater => block_type_table.air_id(),
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct EmptyWorldGenerator {}

impl WorldGenerator for EmptyWorldGenerator {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let block =
				if coords.z == -1 && (-3..=3).contains(&coords.x) && (-3..=3).contains(&coords.y) {
					block_type_table.ground_id()
				} else {
					block_type_table.air_id()
				};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorLines01 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorLines01 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(5, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(5, vec![self.seed, 2]);
		let noise_no_grass = noise::OctavedNoise::new(5, vec![self.seed, 3]);
		let noise_grass_a = noise::OctavedNoise::new(2, vec![self.seed, 1, 1]);
		let noise_grass_b = noise::OctavedNoise::new(2, vec![self.seed, 1, 2]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 100.0;
			let a = noise_a.sample_3d_1d(coordsf / scale, &[]);
			let b = noise_b.sample_3d_1d(coordsf / scale, &[]);
			(a - 0.5).abs() < 0.03 && (b - 0.5).abs() < 0.03
		};
		let coords_to_grass = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let d = noise_grass_a.sample_3d_1d(coordsf / scale, &[]);
			let density = if d < 0.1 {
				d * 0.9 + 0.1
			} else if d < 0.3 {
				0.1
			} else {
				0.01
			};
			noise_grass_b.sample_3d_1d(coordsf, &[]) < density
		};
		let coords_to_no_grass = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 75.0;
			noise_no_grass.sample_3d_1d(coordsf / scale, &[]) < 0.25
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					let no_grass = coords_to_no_grass(coords);
					if no_grass {
						block_type_table.ground_id()
					} else {
						block_type_table.kinda_grass_id()
					}
				}
			} else {
				let ground_below = coords_to_ground(coords + cgmath::vec3(0, 0, -1));
				if ground_below {
					let no_grass_below = coords_to_no_grass(coords + cgmath::vec3(0, 0, -1));
					if no_grass_below {
						block_type_table.air_id()
					} else if coords_to_grass(coords) {
						block_type_table.kinda_grass_blades_id()
					} else {
						block_type_table.air_id()
					}
				} else {
					block_type_table.air_id()
				}
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorVolumes01 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorVolumes01 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(5, vec![self.seed, 1]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 100.0;
			let a = noise_a.sample_3d_1d(coordsf / scale, &[]);
			a < 0.35
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorBallsSameSize {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorBallsSameSize {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let scale = 45.0;
			let radius = 11.0;
			let coordsf = coords.map(|x| x as f32);
			let coordsf_i_scaled = coords.map(|x| (x as f32 / scale).floor());
			let a = noise_a.sample_3d_1d(coordsf_i_scaled, &[]);
			let b = noise_b.sample_3d_1d(coordsf_i_scaled, &[]);
			let c = noise_c.sample_3d_1d(coordsf_i_scaled, &[]);
			let coordsf_min = coords.map(|x| (x as f32 / scale).floor() * scale);
			let _coordsf_max = coords.map(|x| (x as f32 / scale).ceil() * scale);
			let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
			(coordsf - coordsf_min).distance(the) < radius
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorBallsDifferentSizes {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorBallsDifferentSizes {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let noise_d = noise::OctavedNoise::new(1, vec![self.seed, 4]);
		let noise_e = noise::OctavedNoise::new(1, vec![self.seed, 5]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let scale = 45.0;
			let min_radius = 4.0;
			let max_radius = 15.0;
			let coordsf = coords.map(|x| x as f32);
			let coordsf_i_scaled = coords.map(|x| (x as f32 / scale).floor());
			let e = noise_e.sample_3d_1d(coordsf_i_scaled, &[]);
			if e < 0.2 {
				return false;
			}
			let a = noise_a.sample_3d_1d(coordsf_i_scaled, &[]);
			let b = noise_b.sample_3d_1d(coordsf_i_scaled, &[]);
			let c = noise_c.sample_3d_1d(coordsf_i_scaled, &[]);
			let d = noise_d.sample_3d_1d(coordsf_i_scaled, &[]);
			let radius = d * (max_radius - min_radius) + min_radius;
			let coordsf_min = coords.map(|x| (x as f32 / scale).floor() * scale);
			let _coordsf_max = coords.map(|x| (x as f32 / scale).ceil() * scale);
			let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
			(coordsf - coordsf_min).distance(the) < radius
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

// For future stuff:
// https://iquilezles.org/articles/distfunctions/

/// Distance from the given `point` to the segment between the two point `a` and `b`.
fn distance_to_segment(
	a: cgmath::Point3<f32>,
	b: cgmath::Point3<f32>,
	point: cgmath::Point3<f32>,
) -> f32 {
	let pa = point - a;
	let ba = b - a;
	let h = f32::clamp(pa.dot(ba) / ba.dot(ba), 0.0, 1.0);
	(pa - ba * h).magnitude()
}

struct WorldGeneratorLinksXRaw {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorLinksXRaw {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let scale = 45.0;
			let radius = 10.0;
			let coordsf = coords.map(|x| x as f32);
			let coordsf_to_the = |coordsf: cgmath::Point3<f32>| -> cgmath::Point3<f32> {
				let coordsf_i_scaled = coordsf.map(|x| (x / scale).floor());
				let a = noise_a.sample_3d_1d(coordsf_i_scaled, &[]);
				let b = noise_b.sample_3d_1d(coordsf_i_scaled, &[]);
				let c = noise_c.sample_3d_1d(coordsf_i_scaled, &[]);
				let coordsf_min = coordsf.map(|x| (x / scale).floor() * scale);
				let _coordsf_max = coordsf.map(|x| (x / scale).ceil() * scale);
				let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
				coordsf_min + the
			};
			let the = coordsf_to_the(coordsf);
			let xp = coordsf_to_the(coordsf + cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let xm = coordsf_to_the(coordsf - cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let vp = distance_to_segment(the, xp, coordsf);
			let vm = distance_to_segment(the, xm, coordsf);
			vp < radius || vm < radius
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorLinksX {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorLinksX {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let noise_d = noise::OctavedNoise::new(4, vec![self.seed, 4]);
		let noise_e = noise::OctavedNoise::new(4, vec![self.seed, 5]);
		let noise_f = noise::OctavedNoise::new(4, vec![self.seed, 6]);
		let coords_to_ground_uwu = |coordsf: cgmath::Point3<f32>| -> bool {
			let scale = 85.0;
			let radius = 10.0;
			let coordsf_to_the = |coordsf: cgmath::Point3<f32>| -> cgmath::Point3<f32> {
				let coordsf_i_scaled = coordsf.map(|x| (x / scale).floor());
				let a = noise_a.sample_3d_1d(coordsf_i_scaled, &[]);
				let b = noise_b.sample_3d_1d(coordsf_i_scaled, &[]);
				let c = noise_c.sample_3d_1d(coordsf_i_scaled, &[]);
				let coordsf_min = coordsf.map(|x| (x / scale).floor() * scale);
				let _coordsf_max = coordsf.map(|x| (x / scale).ceil() * scale);
				let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
				coordsf_min + the
			};
			let the = coordsf_to_the(coordsf);
			let xp = coordsf_to_the(coordsf + cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let xm = coordsf_to_the(coordsf - cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let vp = distance_to_segment(the, xp, coordsf);
			let vm = distance_to_segment(the, xm, coordsf);
			vp < radius || vm < radius
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let deformation_max_length = 20.0;
			let d = noise_d.sample_3d_1d(coordsf / scale, &[]);
			let e = noise_e.sample_3d_1d(coordsf / scale, &[]);
			let f = noise_f.sample_3d_1d(coordsf / scale, &[]);
			use crate::coords::AngularDirection;
			let deformation = AngularDirection::from_angles(d * TAU, e * (TAU / 2.0)).to_vec3()
				* f * deformation_max_length;
			let deformed_coordsf = coordsf + deformation;
			coords_to_ground_uwu(deformed_coordsf)
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorLinks {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorLinks {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let noise_d = noise::OctavedNoise::new(4, vec![self.seed, 4]);
		let noise_e = noise::OctavedNoise::new(4, vec![self.seed, 5]);
		let noise_f = noise::OctavedNoise::new(4, vec![self.seed, 6]);
		let coords_to_ground_uwu = |coordsf: cgmath::Point3<f32>| -> bool {
			let scale = 65.0;
			let radius = 7.0;
			let coordsf_to_the = |coordsf: cgmath::Point3<f32>| -> cgmath::Point3<f32> {
				let coordsf_i_scaled = coordsf.map(|x| (x / scale).floor());
				let a = noise_a.sample_3d_1d(coordsf_i_scaled, &[]);
				let b = noise_b.sample_3d_1d(coordsf_i_scaled, &[]);
				let c = noise_c.sample_3d_1d(coordsf_i_scaled, &[]);
				let coordsf_min = coordsf.map(|x| (x / scale).floor() * scale);
				let _coordsf_max = coordsf.map(|x| (x / scale).ceil() * scale);
				let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
				coordsf_min + the
			};
			let the = coordsf_to_the(coordsf);
			let xp = coordsf_to_the(coordsf + cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let xm = coordsf_to_the(coordsf - cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let yp = coordsf_to_the(coordsf + cgmath::vec3(0.0, 1.0, 0.0) * scale);
			let ym = coordsf_to_the(coordsf - cgmath::vec3(0.0, 1.0, 0.0) * scale);
			let zp = coordsf_to_the(coordsf + cgmath::vec3(0.0, 0.0, 1.0) * scale);
			let zm = coordsf_to_the(coordsf - cgmath::vec3(0.0, 0.0, 1.0) * scale);
			let vxp = distance_to_segment(the, xp, coordsf);
			let vxm = distance_to_segment(the, xm, coordsf);
			let vyp = distance_to_segment(the, yp, coordsf);
			let vym = distance_to_segment(the, ym, coordsf);
			let vzp = distance_to_segment(the, zp, coordsf);
			let vzm = distance_to_segment(the, zm, coordsf);
			(vxp < radius || vxm < radius)
				|| (vyp < radius || vym < radius)
				|| (vzp < radius || vzm < radius)
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let deformation_max_length = 13.0;
			let d = noise_d.sample_3d_1d(coordsf / scale, &[]);
			let e = noise_e.sample_3d_1d(coordsf / scale, &[]);
			let f = noise_f.sample_3d_1d(coordsf / scale, &[]);
			use crate::coords::AngularDirection;
			let deformation = AngularDirection::from_angles(d * TAU, e * (TAU / 2.0)).to_vec3()
				* f * deformation_max_length;
			let deformed_coordsf = coordsf + deformation;
			coords_to_ground_uwu(deformed_coordsf)
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorLinksGround {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorLinksGround {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let noise_d = noise::OctavedNoise::new(4, vec![self.seed, 4]);
		let noise_e = noise::OctavedNoise::new(4, vec![self.seed, 5]);
		let noise_f = noise::OctavedNoise::new(4, vec![self.seed, 6]);
		let coords_to_ground_uwu = |coordsf: cgmath::Point3<f32>| -> bool {
			if coordsf.z < 0.0 {
				return true;
			}
			let scale = 65.0;
			let radius = (10.0f32).min(1.0f32.min(1.0 / (coordsf.z * 0.1 + 4.0)) * 30.0);
			if radius < 1.0 {
				return false;
			}
			let coordsf_to_the = |coordsf: cgmath::Point3<f32>| -> cgmath::Point3<f32> {
				let coordsf_i_scaled = coordsf.map(|x| (x / scale).floor());
				let a = noise_a.sample_3d_1d(coordsf_i_scaled, &[]);
				let b = noise_b.sample_3d_1d(coordsf_i_scaled, &[]);
				let c = noise_c.sample_3d_1d(coordsf_i_scaled, &[]);
				let coordsf_min = coordsf.map(|x| (x / scale).floor() * scale);
				let _coordsf_max = coordsf.map(|x| (x / scale).ceil() * scale);
				let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
				coordsf_min + the
			};
			let the = coordsf_to_the(coordsf);
			let xp = coordsf_to_the(coordsf + cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let xm = coordsf_to_the(coordsf - cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let yp = coordsf_to_the(coordsf + cgmath::vec3(0.0, 1.0, 0.0) * scale);
			let ym = coordsf_to_the(coordsf - cgmath::vec3(0.0, 1.0, 0.0) * scale);
			let zp = coordsf_to_the(coordsf + cgmath::vec3(0.0, 0.0, 1.0) * scale);
			let zm = coordsf_to_the(coordsf - cgmath::vec3(0.0, 0.0, 1.0) * scale);
			let vxp = distance_to_segment(the, xp, coordsf);
			let vxm = distance_to_segment(the, xm, coordsf);
			let vyp = distance_to_segment(the, yp, coordsf);
			let vym = distance_to_segment(the, ym, coordsf);
			let vzp = distance_to_segment(the, zp, coordsf);
			let vzm = distance_to_segment(the, zm, coordsf);
			(vxp < radius || vxm < radius)
				|| (vyp < radius || vym < radius)
				|| (vzp < radius || vzm < radius)
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let deformation_max_length = 13.0;
			let d = noise_d.sample_3d_1d(coordsf / scale, &[]);
			let e = noise_e.sample_3d_1d(coordsf / scale, &[]);
			let f = noise_f.sample_3d_1d(coordsf / scale, &[]);
			use crate::coords::AngularDirection;
			let deformation = AngularDirection::from_angles(d * TAU, e * (TAU / 2.0)).to_vec3()
				* f * deformation_max_length;
			let deformed_coordsf = coordsf + deformation;
			coords_to_ground_uwu(deformed_coordsf)
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorLinksCaves {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorLinksCaves {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let noise_d = noise::OctavedNoise::new(4, vec![self.seed, 4]);
		let noise_e = noise::OctavedNoise::new(4, vec![self.seed, 5]);
		let noise_f = noise::OctavedNoise::new(4, vec![self.seed, 6]);
		let coords_to_ground_uwu = |coordsf: cgmath::Point3<f32>| -> bool {
			if coordsf.z > 0.0 {
				return false;
			}
			let scale = 65.0;
			let radius = 5.0;
			let coordsf_to_the = |coordsf: cgmath::Point3<f32>| -> cgmath::Point3<f32> {
				let coordsf_i_scaled = coordsf.map(|x| (x / scale).floor());
				let a = noise_a.sample_3d_1d(coordsf_i_scaled, &[]);
				let b = noise_b.sample_3d_1d(coordsf_i_scaled, &[]);
				let c = noise_c.sample_3d_1d(coordsf_i_scaled, &[]);
				let coordsf_min = coordsf.map(|x| (x / scale).floor() * scale);
				let _coordsf_max = coordsf.map(|x| (x / scale).ceil() * scale);
				let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
				coordsf_min + the
			};
			let the = coordsf_to_the(coordsf);
			let xp = coordsf_to_the(coordsf + cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let xm = coordsf_to_the(coordsf - cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let yp = coordsf_to_the(coordsf + cgmath::vec3(0.0, 1.0, 0.0) * scale);
			let ym = coordsf_to_the(coordsf - cgmath::vec3(0.0, 1.0, 0.0) * scale);
			let zp = coordsf_to_the(coordsf + cgmath::vec3(0.0, 0.0, 1.0) * scale);
			let zm = coordsf_to_the(coordsf - cgmath::vec3(0.0, 0.0, 1.0) * scale);
			let vxp = distance_to_segment(the, xp, coordsf);
			let vxm = distance_to_segment(the, xm, coordsf);
			let vyp = distance_to_segment(the, yp, coordsf);
			let vym = distance_to_segment(the, ym, coordsf);
			let vzp = distance_to_segment(the, zp, coordsf);
			let vzm = distance_to_segment(the, zm, coordsf);
			!((vxp < radius || vxm < radius)
				|| (vyp < radius || vym < radius)
				|| (vzp < radius || vzm < radius))
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let deformation_max_length = 13.0;
			let d = noise_d.sample_3d_1d(coordsf / scale, &[]);
			let e = noise_e.sample_3d_1d(coordsf / scale, &[]);
			let f = noise_f.sample_3d_1d(coordsf / scale, &[]);
			use crate::coords::AngularDirection;
			let deformation = AngularDirection::from_angles(d * TAU, e * (TAU / 2.0)).to_vec3()
				* f * deformation_max_length;
			let deformed_coordsf = coordsf + deformation;
			coords_to_ground_uwu(deformed_coordsf)
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorLinks02 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorLinks02 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let noise_d = noise::OctavedNoise::new(4, vec![self.seed, 4]);
		let noise_e = noise::OctavedNoise::new(4, vec![self.seed, 5]);
		let noise_f = noise::OctavedNoise::new(4, vec![self.seed, 6]);
		let noise_g = noise::OctavedNoise::new(1, vec![self.seed, 7]);
		let coords_to_ground_uwu = |coordsf: cgmath::Point3<f32>| -> bool {
			let scale = 65.0;
			let radius = 7.0;
			let coordsf_to_the = |coordsf: cgmath::Point3<f32>| -> cgmath::Point3<f32> {
				let coordsf_i_scaled = coordsf.map(|x| (x / scale).floor());
				let a = noise_a.sample_3d_1d(coordsf_i_scaled, &[]);
				let b = noise_b.sample_3d_1d(coordsf_i_scaled, &[]);
				let c = noise_c.sample_3d_1d(coordsf_i_scaled, &[]);
				let coordsf_min = coordsf.map(|x| (x / scale).floor() * scale);
				let _coordsf_max = coordsf.map(|x| (x / scale).ceil() * scale);
				let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
				coordsf_min + the
			};
			let coordsf_to_link_negativewards =
				|coordsf: cgmath::Point3<f32>, axis: NonOrientedAxis| -> bool {
					let coordsf_i_scaled = coordsf.map(|x| (x / scale).floor());
					let axis_channel = axis.index() as i32;
					let g = noise_g.sample_3d_1d(coordsf_i_scaled, &[axis_channel]);
					g < 0.5
				};
			let the = coordsf_to_the(coordsf);
			let xp = coordsf_to_the(coordsf + cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let xm = coordsf_to_the(coordsf - cgmath::vec3(1.0, 0.0, 0.0) * scale);
			let yp = coordsf_to_the(coordsf + cgmath::vec3(0.0, 1.0, 0.0) * scale);
			let ym = coordsf_to_the(coordsf - cgmath::vec3(0.0, 1.0, 0.0) * scale);
			let zp = coordsf_to_the(coordsf + cgmath::vec3(0.0, 0.0, 1.0) * scale);
			let zm = coordsf_to_the(coordsf - cgmath::vec3(0.0, 0.0, 1.0) * scale);
			let vxp = distance_to_segment(the, xp, coordsf);
			let vxm = distance_to_segment(the, xm, coordsf);
			let vyp = distance_to_segment(the, yp, coordsf);
			let vym = distance_to_segment(the, ym, coordsf);
			let vzp = distance_to_segment(the, zp, coordsf);
			let vzm = distance_to_segment(the, zm, coordsf);
			let lxp = coordsf_to_link_negativewards(
				coordsf + cgmath::vec3(1.0, 0.0, 0.0) * scale,
				NonOrientedAxis::X,
			);
			let lxm = coordsf_to_link_negativewards(coordsf, NonOrientedAxis::X);
			let lyp = coordsf_to_link_negativewards(
				coordsf + cgmath::vec3(0.0, 1.0, 0.0) * scale,
				NonOrientedAxis::Y,
			);
			let lym = coordsf_to_link_negativewards(coordsf, NonOrientedAxis::Y);
			let lzp = coordsf_to_link_negativewards(
				coordsf + cgmath::vec3(0.0, 0.0, 1.0) * scale,
				NonOrientedAxis::Z,
			);
			let lzm = coordsf_to_link_negativewards(coordsf, NonOrientedAxis::Z);
			(lxp && vxp < radius)
				|| (lxm && vxm < radius)
				|| (lyp && vyp < radius)
				|| (lym && vym < radius)
				|| (lzp && vzp < radius)
				|| (lzm && vzm < radius)
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let deformation_max_length = 25.0;
			let d = noise_d.sample_3d_1d(coordsf / scale, &[]);
			let e = noise_e.sample_3d_1d(coordsf / scale, &[]);
			let f = noise_f.sample_3d_1d(coordsf / scale, &[]);
			use crate::coords::AngularDirection;
			let deformation = AngularDirection::from_angles(d * TAU, e * (TAU / 2.0)).to_vec3()
				* f * deformation_max_length;
			let deformed_coordsf = coordsf + deformation;
			coords_to_ground_uwu(deformed_coordsf)
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorLinksFlat {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorLinksFlat {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let noise_d = noise::OctavedNoise::new(4, vec![self.seed, 4]);
		let noise_e = noise::OctavedNoise::new(4, vec![self.seed, 5]);
		let noise_f = noise::OctavedNoise::new(4, vec![self.seed, 6]);
		let noise_g = noise::OctavedNoise::new(1, vec![self.seed, 7]);
		let coords_to_ground_uwu =
			|coordsf: cgmath::Point3<f32>| -> bool {
				let scale = 55.0;
				let radius = 7.0;
				let coordsf_to_the = |coordsf: cgmath::Point3<f32>| -> cgmath::Point3<f32> {
					let coordsf_i_scaled = coordsf.map(|x| (x / scale).floor());
					let a = noise_a.sample_3d_1d(coordsf_i_scaled, &[]);
					let b = noise_b.sample_3d_1d(coordsf_i_scaled, &[]);
					let c = noise_c.sample_3d_1d(coordsf_i_scaled, &[]);
					let coordsf_min = coordsf.map(|x| (x / scale).floor() * scale);
					let _coordsf_max = coordsf.map(|x| (x / scale).ceil() * scale);
					let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
					coordsf_min + the
				};
				let coordsf_to_link_negativewards =
					|coordsf: cgmath::Point3<f32>, axis: NonOrientedAxis| -> bool {
						let coordsf_i_scaled = coordsf.map(|x| (x / scale).floor());
						let axis_channel = axis.index() as i32;
						let g = noise_g.sample_3d_1d(coordsf_i_scaled, &[axis_channel]);
						g < 0.5
					};
				let in_link = |a: cgmath::Point3<f32>,
				               b: cgmath::Point3<f32>,
				               coordsf: cgmath::Point3<f32>,
				               radius: f32|
				 -> bool {
					let dist = distance_to_segment(a, b, coordsf);
					if dist < radius {
						let dist_above = distance_to_segment(a, b, coordsf + cgmath::vec3(0.0, 0.0, 1.0));
						dist_above < dist
					} else {
						false
					}
				};
				let the = coordsf_to_the(coordsf);
				let xp = coordsf_to_the(coordsf + cgmath::vec3(1.0, 0.0, 0.0) * scale);
				let xm = coordsf_to_the(coordsf - cgmath::vec3(1.0, 0.0, 0.0) * scale);
				let yp = coordsf_to_the(coordsf + cgmath::vec3(0.0, 1.0, 0.0) * scale);
				let ym = coordsf_to_the(coordsf - cgmath::vec3(0.0, 1.0, 0.0) * scale);
				let zp = coordsf_to_the(coordsf + cgmath::vec3(0.0, 0.0, 1.0) * scale);
				let zm = coordsf_to_the(coordsf - cgmath::vec3(0.0, 0.0, 1.0) * scale);
				let vxp = in_link(the, xp, coordsf, radius);
				let vxm = in_link(the, xm, coordsf, radius);
				let vyp = in_link(the, yp, coordsf, radius);
				let vym = in_link(the, ym, coordsf, radius);
				let vzp = in_link(the, zp, coordsf, radius);
				let vzm = in_link(the, zm, coordsf, radius);
				let lxp = coordsf_to_link_negativewards(
					coordsf + cgmath::vec3(1.0, 0.0, 0.0) * scale,
					NonOrientedAxis::X,
				);
				let lxm = coordsf_to_link_negativewards(coordsf, NonOrientedAxis::X);
				let lyp = coordsf_to_link_negativewards(
					coordsf + cgmath::vec3(0.0, 1.0, 0.0) * scale,
					NonOrientedAxis::Y,
				);
				let lym = coordsf_to_link_negativewards(coordsf, NonOrientedAxis::Y);
				let lzp = coordsf_to_link_negativewards(
					coordsf + cgmath::vec3(0.0, 0.0, 1.0) * scale,
					NonOrientedAxis::Z,
				);
				let lzm = coordsf_to_link_negativewards(coordsf, NonOrientedAxis::Z);
				(lxp && vxp)
					|| (lxm && vxm) || (lyp && vyp)
					|| (lym && vym) || (lzp && vzp)
					|| (lzm && vzm)
			};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let horizontal_deformation_max_length = 14.0;
			let vertical_deformation_max_length = 4.0;
			let d = noise_d.sample_3d_1d(coordsf / scale, &[]);
			let e = noise_e.sample_3d_1d(coordsf / scale, &[]);
			let f = noise_f.sample_3d_1d(coordsf / scale, &[]);
			use crate::coords::AngularDirection;
			let mut deformation =
				AngularDirection::from_angles(d * TAU, e * (TAU / 2.0)).to_vec3() * f;
			deformation.x *= horizontal_deformation_max_length;
			deformation.y *= horizontal_deformation_max_length;
			deformation.z *= vertical_deformation_max_length;
			let deformed_coordsf = coordsf + deformation;
			coords_to_ground_uwu(deformed_coordsf)
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorSkyIslands {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorSkyIslands {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let noise_d = noise::OctavedNoise::new(1, vec![self.seed, 4]);
		let noise_e = noise::OctavedNoise::new(1, vec![self.seed, 5]);
		let noise_f = noise::OctavedNoise::new(4, vec![self.seed, 6]);
		let noise_g = noise::OctavedNoise::new(4, vec![self.seed, 7]);
		let noise_h = noise::OctavedNoise::new(4, vec![self.seed, 8]);
		let noise_grass_a = noise::OctavedNoise::new(2, vec![self.seed, 1, 1]);
		let noise_grass_b = noise::OctavedNoise::new(2, vec![self.seed, 1, 2]);
		let coords_to_ground_uwu = |coordsf: cgmath::Point3<f32>| -> bool {
			let scale = 100.0;
			let min_radius = 4.0;
			let max_radius = 50.0;
			let coordsf_i_scaled = coordsf.map(|x| (x / scale).floor());
			let e = noise_e.sample_3d_1d(coordsf_i_scaled, &[]);
			if e < 0.2 {
				return false;
			}
			let a = noise_a.sample_3d_1d(coordsf_i_scaled, &[]);
			let b = noise_b.sample_3d_1d(coordsf_i_scaled, &[]);
			let c = noise_c.sample_3d_1d(coordsf_i_scaled, &[]);
			let d = noise_d.sample_3d_1d(coordsf_i_scaled, &[]);
			let radius = d * (max_radius - min_radius) + min_radius;
			let coordsf_min = coordsf.map(|x| (x / scale).floor() * scale);
			let _coordsf_max = coordsf.map(|x| (x / scale).ceil() * scale);
			let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
			let dist = (coordsf - coordsf_min).distance(the);
			let dist_above = ((coordsf + cgmath::vec3(0.0, 0.0, 1.0)) - coordsf_min).distance(the);
			dist < radius && dist > dist_above
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let horizontal_deformation_max_length = 14.0;
			let vertical_deformation_max_length = 4.0;
			let f = noise_f.sample_3d_1d(coordsf / scale, &[]);
			let g = noise_g.sample_3d_1d(coordsf / scale, &[]);
			let h = noise_h.sample_3d_1d(coordsf / scale, &[]);
			use crate::coords::AngularDirection;
			let mut deformation =
				AngularDirection::from_angles(f * TAU, g * (TAU / 2.0)).to_vec3() * h;
			deformation.x *= horizontal_deformation_max_length;
			deformation.y *= horizontal_deformation_max_length;
			deformation.z *= vertical_deformation_max_length;
			let deformed_coordsf = coordsf + deformation;
			coords_to_ground_uwu(deformed_coordsf)
		};
		let coords_to_grass = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let d = noise_grass_a.sample_3d_1d(coordsf / scale, &[]);
			let density = if d < 0.1 {
				d * 0.9 + 0.1
			} else if d < 0.3 {
				0.1
			} else {
				0.01
			};
			noise_grass_b.sample_3d_1d(coordsf, &[]) < density
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				let ground_below = coords_to_ground(coords + cgmath::vec3(0, 0, -1));
				if ground_below && coords_to_grass(coords) {
					block_type_table.kinda_grass_blades_id()
				} else {
					block_type_table.air_id()
				}
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorVolumes02 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorVolumes02 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(4, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(4, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(4, vec![self.seed, 3]);
		let noise_d = noise::OctavedNoise::new(5, vec![self.seed, 4]);
		let noise_e = noise::OctavedNoise::new(5, vec![self.seed, 5]);
		let noise_f = noise::OctavedNoise::new(5, vec![self.seed, 6]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 100.0;
			let a = noise_a.sample_3d_1d(coordsf / scale, &[]);
			let b = noise_b.sample_3d_1d(coordsf / scale, &[]);
			let c = noise_c.sample_3d_1d(coordsf / scale, &[]);
			let abc = cgmath::vec3(a - 0.5, b - 0.5, c - 0.5).normalize();
			let detail_scale = 85.0;
			let d = noise_d.sample_3d_1d(coordsf / detail_scale, &[]);
			let e = noise_e.sample_3d_1d(coordsf / detail_scale, &[]);
			let f = noise_f.sample_3d_1d(coordsf / detail_scale, &[]);
			let def = cgmath::vec3(d - 0.5, e - 0.5, f - 0.5).normalize();
			let uwu = abc.dot(def);
			uwu < -0.4 && def.z < 0.0
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorVolumes03 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorVolumes03 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(5, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(5, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(5, vec![self.seed, 3]);
		let coords_to_ground_uwu = |coords: BlockCoords| -> f32 {
			let coordsf = coords.map(|x| x as f32);
			let scale = 200.0;
			let a = noise_a.sample_3d_1d(coordsf / scale, &[]);
			let b = noise_b.sample_3d_1d(coordsf / scale, &[]);
			let c = noise_c.sample_3d_1d(coordsf / scale, &[]);
			a.max(b).max(c) + a - c
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			coords_to_ground_uwu(coords) < coords_to_ground_uwu(coords + cgmath::vec3(0, 0, 1))
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorHeight01 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorHeight01 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(5, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(5, vec![self.seed, 2]);
		let coords_to_height = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let coordsf_yx = cgmath::point2(coordsf.x, coordsf.y);
			let scale_a = 100.0;
			let scale_b = 80.0;
			let nosie_value_a = noise_a.sample_2d_1d(coordsf_yx / scale_a, &[]);
			let nosie_value_b = noise_b.sample_2d_1d(coordsf_yx / scale_b, &[]);
			let angle = f32::atan2(nosie_value_a - 0.5, nosie_value_b - 0.5);
			let distance = f32::hypot(nosie_value_a - 0.5, nosie_value_b - 0.5);
			let value = (f32::cos(angle * 3.0) * 0.5 + 0.5) * distance.powi(4);
			value < 0.001
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			if coords_to_height(coords) {
				coords.z < 0
			} else {
				coords.z < 10
			}
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorPlane01 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorPlane01 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(5, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(5, vec![self.seed, 2]);
		let coords_to_void = |coords: BlockCoords| -> f32 {
			let coordsf = coords.map(|x| x as f32);
			let coordsf_yx = cgmath::point2(coordsf.x, coordsf.y);
			let scale_a = 100.0;
			let scale_b = 80.0;
			let nosie_value_a = noise_a.sample_2d_1d(coordsf_yx / scale_a, &[]);
			let nosie_value_b = noise_b.sample_2d_1d(coordsf_yx / scale_b, &[]);
			let angle = f32::atan2(nosie_value_a - 0.5, nosie_value_b - 0.5);
			let distance = f32::hypot(nosie_value_a - 0.5, nosie_value_b - 0.5);
			let value = (f32::cos(angle * 3.0) * 0.5 + 0.5) * distance.powi(4);
			value / 0.001
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let void = coords_to_void(coords);
			(coords.z as f32).abs() < 6.0 / (1.0 / (1.0 - void))
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorWierdTerrain01 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorWierdTerrain01 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(5, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(5, vec![self.seed, 2]);
		let coords_to_void = |coords: BlockCoords| -> f32 {
			let coordsf = coords.map(|x| x as f32);
			let coordsf_yx = cgmath::point2(coordsf.x, coordsf.y);
			let scale_a = 100.0;
			let scale_b = 80.0;
			let nosie_value_a = noise_a.sample_2d_1d(coordsf_yx / scale_a, &[]);
			let nosie_value_b = noise_b.sample_2d_1d(coordsf_yx / scale_b, &[]);
			let angle = f32::atan2(nosie_value_a - 0.5, nosie_value_b - 0.5);
			let distance = f32::hypot(nosie_value_a - 0.5, nosie_value_b - 0.5);
			let value = (f32::cos(angle * 3.0) * 0.5 + 0.5) * distance.powi(4);
			value / 0.001
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let void = coords_to_void(coords);
			(coords.z as f32).abs() < (1.0 / void).log2() / (1.0 / (1.0 - void))
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorPlane02 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorPlane02 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(5, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(5, vec![self.seed, 2]);
		let coords_to_void = |coords: BlockCoords| -> f32 {
			let coordsf = coords.map(|x| x as f32);
			let scale_a = 100.0;
			let scale_b = 80.0;
			let nosie_value_a = noise_a.sample_3d_1d(coordsf / scale_a, &[]);
			let nosie_value_b = noise_b.sample_3d_1d(coordsf / scale_b, &[]);
			let angle = f32::atan2(nosie_value_a - 0.5, nosie_value_b - 0.5);
			let distance = f32::hypot(nosie_value_a - 0.5, nosie_value_b - 0.5);
			let value = (f32::cos(angle * 3.0) * 0.5 + 0.5) * distance.powi(4);
			value / 0.001
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let void = coords_to_void(coords);
			(coords.z as f32).abs() < 20.0 / (1.0 / (1.0 - void))
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorWierdTerrain02 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorWierdTerrain02 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_m = noise::OctavedNoise::new(4, vec![self.seed, 1]);
		let noise_a = noise::OctavedNoise::new(4, vec![self.seed, 2]);
		let noise_b = noise::OctavedNoise::new(4, vec![self.seed, 3]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale_m = 60.0;
			let scale_a = 240.0 * noise_m.sample_3d_1d(coordsf / scale_m, &[]);
			let nosie_value_a = noise_a.sample_3d_1d(coordsf / scale_a, &[]);
			let angle = nosie_value_a * TAU;
			let scale_d = 100.0;
			let distance = 80.0 * noise_b.sample_3d_1d(coordsf / scale_d, &[]);
			let v = coordsf.z + f32::cos(angle) * distance;
			//let ry = ry + f32::sin(angle) * distance;
			v < 0.5
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorHeight02 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorHeight02 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(5, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(5, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(5, vec![self.seed, 3]);
		let noise_d = noise::OctavedNoise::new(5, vec![self.seed, 4]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let mut coordsf = coords.map(|x| x as f32);
			let scale_a = 100.0;
			for _i in 0..3 {
				let noise_value_x = noise_a.sample_3d_1d(coordsf / scale_a, &[]);
				let noise_value_y = noise_b.sample_3d_1d(coordsf / scale_a, &[]);
				let noise_value_z = noise_c.sample_3d_1d(coordsf / scale_a, &[]);
				let vv = cgmath::vec3(noise_value_x, noise_value_y, noise_value_z);
				let vv = (vv - cgmath::vec3(0.5, 0.5, 0.5)).normalize();
				let d = noise_d.sample_3d_1d(coordsf / scale_a, &[]);
				let vv = vv * d * 20.0;
				coordsf += vv;
				if coordsf.z < 0.0 {
					return true;
				}
			}
			false
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorHeightBiomes {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorHeightBiomes {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		fn interpolate(
			smoothing: &dyn Fn(f32) -> f32,
			x: f32,
			x_inf: f32,
			x_sup: f32,
			dst_inf: f32,
			dst_sup: f32,
		) -> f32 {
			let ratio = (x - x_inf) / (x_sup - x_inf);
			let smooth_ratio = smoothing(ratio);
			dst_inf + smooth_ratio * (dst_sup - dst_inf)
		}
		fn smoothing(x: f32) -> f32 {
			x
		}

		let noise_biomes = noise::OctavedNoise::new(5, vec![self.seed, 1]);
		type Height = f32;
		let biome_heights: [Height; 5] = [0.0, -3.0, 3.0, -6.0, 6.0];

		let coords_to_biome_height = |coords: BlockCoords| -> f32 {
			let coordsf = coords.map(|x| x as f32);
			let coordsf_xy = cgmath::point2(coordsf.x, coordsf.y);
			let scale = 120.0;
			let n = biome_heights.len() as i32;
			let mut values: Vec<_> = (0..n)
				.map(|i| {
					(
						i as usize,
						noise_biomes.sample_2d_1d(coordsf_xy / scale, &[i]),
					)
				})
				.collect();
			values.sort_by(|value_a, value_b| value_a.1.partial_cmp(&value_b.1).unwrap());
			values.reverse();
			fn get_height(
				i: usize,
				values: &[(usize, f32)],
				biome_heights: &[Height; 5],
			) -> (Height, f32) {
				let get_diff = |i: usize| values[i].1 - values[i + 1].1;
				let max_diff = 0.06;
				let get_coef = |i: usize| get_diff(i).clamp(0.0, max_diff) / max_diff;
				let get_base_height = |i: usize| -> Height { biome_heights[i] };

				if i == values.len() - 1 {
					(get_base_height(values[i].0), 1.0)
				} else {
					let coef = get_coef(i);
					let base = get_base_height(values[i].0);
					if false {
						(base, 1.0)
					} else {
						let (after, after_part) = get_height(i + 1, values, biome_heights);
						let part = 2.0 - after_part;
						let height = interpolate(&smoothing, 1.0 - coef, 0.0, part, base, after);
						(height, (1.0 - coef) / part)
					}
				}
			}
			get_height(0, &values, &biome_heights).0
		};

		let coords_to_ground = |coords: BlockCoords| -> bool {
			let height = coords_to_biome_height(coords);
			let coordsf = coords.map(|x| x as f32);
			coordsf.z < height
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorHeightBiomesVolume {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorHeightBiomesVolume {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		fn interpolate(
			smoothing: &dyn Fn(f32) -> f32,
			x: f32,
			x_inf: f32,
			x_sup: f32,
			dst_inf: f32,
			dst_sup: f32,
		) -> f32 {
			let ratio = (x - x_inf) / (x_sup - x_inf);
			let smooth_ratio = smoothing(ratio);
			dst_inf + smooth_ratio * (dst_sup - dst_inf)
		}
		fn smoothing(x: f32) -> f32 {
			x
		}

		let noise_biomes = noise::OctavedNoise::new(5, vec![self.seed, 1]);
		type Height = f32;
		let biome_heights: [Height; 5] = [0.0, 5.0, 10.0, 30.0, 100.0];

		let coords_to_biome_height = |coords: BlockCoords| -> f32 {
			let coordsf = coords.map(|x| x as f32);
			let scale = 120.0;
			let n = biome_heights.len() as i32;
			let mut values: Vec<_> = (0..n)
				.map(|i| (i as usize, noise_biomes.sample_3d_1d(coordsf / scale, &[i])))
				.collect();
			values.sort_by(|value_a, value_b| value_a.1.partial_cmp(&value_b.1).unwrap());
			values.reverse();
			fn get_height(
				i: usize,
				values: &[(usize, f32)],
				biome_heights: &[Height; 5],
			) -> (Height, f32) {
				let get_diff = |i: usize| values[i].1 - values[i + 1].1;
				let max_diff = 0.06;
				let get_coef = |i: usize| get_diff(i).clamp(0.0, max_diff) / max_diff;
				let get_base_height = |i: usize| -> Height { biome_heights[i] };

				if i == values.len() - 1 {
					(get_base_height(values[i].0), 1.0)
				} else {
					let coef = get_coef(i);
					let base = get_base_height(values[i].0);
					if false {
						(base, 1.0)
					} else {
						let (after, after_part) = get_height(i + 1, values, biome_heights);
						let part = 2.0 - after_part;
						let height = interpolate(&smoothing, 1.0 - coef, 0.0, part, base, after);
						(height, (1.0 - coef) / part)
					}
				}
			}
			get_height(0, &values, &biome_heights).0
		};

		let coords_to_ground = |coords: BlockCoords| -> bool {
			let height = coords_to_biome_height(coords);
			let coordsf = coords.map(|x| x as f32);
			coordsf.z < height
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorHeight03 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorHeight03 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(3, vec![self.seed, 1]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let coordsf_xy = cgmath::point2(coordsf.x, coordsf.y);
			let scale = 60.0;
			let height = 20.0 * noise_a.sample_2d_1d(coordsf_xy / scale, &[]);
			coordsf.z < height
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorStructuresPoc {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorStructuresPoc {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		// Define the terrain generation as a deterministic coords->block function.
		let noise_terrain = noise::OctavedNoise::new(3, vec![self.seed, 1]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let coordsf_xy = cgmath::point2(coordsf.x, coordsf.y);
			let scale = 60.0;
			let height = 20.0 * noise_terrain.sample_2d_1d(coordsf_xy / scale, &[]);
			coordsf.z < height
		};
		let coords_to_terrain = |coords: BlockCoords| -> BlockTypeId {
			let ground = coords_to_ground(coords);
			if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			}
		};

		// Setup structure origins generation stuff.
		let noise_cell_data = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let cell_size = 37;
		let block_coords_to_cell_coords = |block_coords: BlockCoords| -> cgmath::Point3<i32> {
			block_coords.map(|x| x.div_euclid(cell_size))
		};
		let cell_coords_to_number_of_structure_origins = |cell_coords: cgmath::Point3<i32>| -> usize {
			let v = noise_cell_data.sample(
				&[],
				&[cell_coords.x, cell_coords.y, cell_coords.z, 1],
				&[],
				None,
			);
			(v * 6.0 - 2.0).max(0.0).floor() as usize
		};
		let cell_coords_and_structure_origin_index_to_origin_coords_in_world =
			|cell_coords: cgmath::Point3<i32>, origin_index: usize| -> BlockCoords {
				let xyz: SmallVec<[f32; 3]> = [0, 1, 2]
					.into_iter()
					.map(|axis| {
						noise_cell_data.sample(
							&[],
							&[
								cell_coords.x,
								cell_coords.y,
								cell_coords.z,
								1 + axis,
								origin_index as i32,
							],
							&[],
							None,
						)
					})
					.collect();
				let coords_in_unit_cube = cgmath::point3(xyz[0], xyz[1], xyz[2]);
				let coords_in_cell =
					coords_in_unit_cube.map(|x| (x * (cell_size as f32 - 0.001)).floor() as i32);
				let cell_coords_in_world = cell_coords * cell_size;
				cell_coords_in_world + coords_in_cell.to_vec()
			};

		// Define structure generation.
		let structure_place_block = |block_coords: BlockCoords,
		                             block_type_to_place: BlockTypeId,
		                             chunk_blocks: &mut ChunkBlocks| {
			chunk_blocks.set(block_coords, block_type_to_place);
		};
		let structure_look_terrain_block = |block_coords: BlockCoords| -> BlockTypeId {
			// We already generated the terrain for the whole chunk,
			// BUT some structures may have already modified it, so we should not use it.
			coords_to_terrain(block_coords)
		};
		// Radius of the cube around the structure origin block coords in which the structure
		// generation can place blocks. A radius of 1 means just the origin block, a
		// radius of 2 means a 3x3x3 blocks sized cube around the origin block, etc.
		let structure_max_blocky_radius = 42;
		let generate_structure = |origin_block_coords: BlockCoords,
		                          chunk_blocks: &mut ChunkBlocks| {
			for direction in crate::coords::OrientedAxis::all_the_six_possible_directions() {
				let mut placing_head = origin_block_coords;
				let delta = direction.delta();
				for _i in 0..structure_max_blocky_radius {
					if !block_type_table
						.get(structure_look_terrain_block(placing_head))
						.unwrap()
						.is_air()
					{
						break;
					}
					structure_place_block(placing_head, block_type_table.ground_id(), chunk_blocks);
					placing_head += delta;
				}
			}
			for coords in crate::coords::iter_3d_cube_center_radius(
				origin_block_coords,
				structure_max_blocky_radius.min(4),
			) {
				structure_place_block(coords, block_type_table.ground_id(), chunk_blocks);
			}
		};

		// Now we generate the block data in the chunk.
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);

		// Generate terrain in the chunk.
		for coords in chunk_blocks.coords_span.iter_coords() {
			chunk_blocks.set(coords, coords_to_terrain(coords));
		}

		// Generate the structures that can overlap with the chunk.
		let coords_span_in_which_structure_origins_can_overlap_with_chunk_inf_included =
			coords_span.block_coords_inf() - cgmath::vec3(1, 1, 1) * structure_max_blocky_radius;
		let coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_excluded = coords_span
			.block_coords_sup_excluded()
			+ cgmath::vec3(1, 1, 1) * structure_max_blocky_radius;
		let coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_included =
			coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_excluded
				- cgmath::vec3(1, 1, 1);
		let structure_origin_can_overlap_with_chunk = |origin_block_coords: BlockCoords| -> bool {
			let inf = coords_span_in_which_structure_origins_can_overlap_with_chunk_inf_included;
			let sup_excluded =
				coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_excluded;
			let c = origin_block_coords;
			(inf.x <= c.x && c.x < sup_excluded.x)
				&& (inf.y <= c.y && c.y < sup_excluded.y)
				&& (inf.z <= c.z && c.z < sup_excluded.z)
		};
		let cell_coords_inf_included_that_can_have_origins_of_structures_that_can_overlap =
			block_coords_to_cell_coords(
				coords_span_in_which_structure_origins_can_overlap_with_chunk_inf_included,
			);
		let cell_coords_sup_included_that_can_have_origins_of_structures_that_can_overlap =
			block_coords_to_cell_coords(
				coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_included,
			);
		let cell_coords_sup_excluded_that_can_have_origins_of_structures_that_can_overlap =
			cell_coords_sup_included_that_can_have_origins_of_structures_that_can_overlap
				+ cgmath::vec3(1, 1, 1);
		for cell_coords in iter_3d_rect_inf_sup_excluded(
			cell_coords_inf_included_that_can_have_origins_of_structures_that_can_overlap,
			cell_coords_sup_excluded_that_can_have_origins_of_structures_that_can_overlap,
		) {
			let number_of_origins = cell_coords_to_number_of_structure_origins(cell_coords);
			for origin_index in 0..number_of_origins {
				let origin_coords = cell_coords_and_structure_origin_index_to_origin_coords_in_world(
					cell_coords,
					origin_index,
				);
				if structure_origin_can_overlap_with_chunk(origin_coords) {
					generate_structure(origin_coords, &mut chunk_blocks);
				}
			}
		}

		chunk_blocks
	}
}

struct WorldGeneratorStructuresLinksPoc {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorStructuresLinksPoc {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		// Define the terrain generation as a deterministic coords->block function.
		let noise_terrain = noise::OctavedNoise::new(3, vec![self.seed, 1]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let coordsf_xy = cgmath::point2(coordsf.x, coordsf.y);
			let scale = 60.0;
			let height = 20.0 * noise_terrain.sample_2d_1d(coordsf_xy / scale, &[]);
			coordsf.z < height
		};
		let coords_to_terrain = |coords: BlockCoords| -> BlockTypeId {
			let ground = coords_to_ground(coords);
			if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			}
		};

		// Setup structure origins generation stuff.
		let noise_cell_data = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let cell_size = 37;
		let block_coords_to_cell_coords = |block_coords: BlockCoords| -> cgmath::Point3<i32> {
			block_coords.map(|x| x.div_euclid(cell_size))
		};
		let cell_coords_to_number_of_structure_origins = |cell_coords: cgmath::Point3<i32>| -> usize {
			let v = noise_cell_data.sample(
				&[],
				&[cell_coords.x, cell_coords.y, cell_coords.z, 1],
				&[],
				None,
			);
			(v * 6.0 - 2.0).max(0.0).floor() as usize
		};
		let cell_coords_and_structure_origin_index_to_origin_coords_in_world =
			|cell_coords: cgmath::Point3<i32>, origin_index: usize| -> BlockCoords {
				let xyz: SmallVec<[f32; 3]> = [0, 1, 2]
					.into_iter()
					.map(|axis| {
						noise_cell_data.sample(
							&[],
							&[
								cell_coords.x,
								cell_coords.y,
								cell_coords.z,
								1 + axis,
								origin_index as i32,
							],
							&[],
							None,
						)
					})
					.collect();
				let coords_in_unit_cube = cgmath::point3(xyz[0], xyz[1], xyz[2]);
				let coords_in_cell =
					coords_in_unit_cube.map(|x| (x * (cell_size as f32 - 0.001)).floor() as i32);
				let cell_coords_in_world = cell_coords * cell_size;
				cell_coords_in_world + coords_in_cell.to_vec()
			};

		// Define structure generation.
		let structure_place_block = |block_coords: BlockCoords,
		                             block_type_to_place: BlockTypeId,
		                             chunk_blocks: &mut ChunkBlocks| {
			chunk_blocks.set(block_coords, block_type_to_place);
		};
		let _structure_look_terrain_block = |block_coords: BlockCoords| -> BlockTypeId {
			// We already generated the terrain for the whole chunk,
			// BUT some structures may have already modified it, so we should not use it.
			coords_to_terrain(block_coords)
		};
		// Radius of the cube around the structure origin block coords in which the structure
		// generation can place blocks. A radius of 1 means just the origin block, a
		// radius of 2 means a 3x3x3 blocks sized cube around the origin block, etc.
		let structure_max_blocky_radius = 42;
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let generate_structure = |origin_block_coords: BlockCoords,
		                          chunk_blocks: &mut ChunkBlocks| {
			// Setup function thta says if we are in the cubic area that we can actually modify.
			let coords_span_writable_inf_included =
				origin_block_coords - cgmath::vec3(1, 1, 1) * (structure_max_blocky_radius * 2 - 2);
			let coords_span_writable_sup_excluded =
				origin_block_coords + cgmath::vec3(1, 1, 1) * (structure_max_blocky_radius * 2 - 2 + 1);
			let coords_are_writable = |block_coords: BlockCoords| -> bool {
				let inf = coords_span_writable_inf_included;
				let sup_excluded = coords_span_writable_sup_excluded;
				let c = block_coords;
				(inf.x <= c.x && c.x < sup_excluded.x)
					&& (inf.y <= c.y && c.y < sup_excluded.y)
					&& (inf.z <= c.z && c.z < sup_excluded.z)
			};

			// Find nearby structures that we can link to.
			let coords_span_in_which_structure_origins_can_link_inf_included =
				origin_block_coords - cgmath::vec3(1, 1, 1) * (structure_max_blocky_radius * 2 - 2);
			let coords_span_in_which_structure_origins_can_link_sup_excluded =
				origin_block_coords + cgmath::vec3(1, 1, 1) * (structure_max_blocky_radius * 2 - 2 + 1);
			let coords_span_in_which_structure_origins_can_link_sup_included =
				coords_span_in_which_structure_origins_can_link_sup_excluded - cgmath::vec3(1, 1, 1);
			let structure_origin_can_link = |other_origin_block_coords: BlockCoords| -> bool {
				let inf = coords_span_in_which_structure_origins_can_link_inf_included;
				let sup_excluded = coords_span_in_which_structure_origins_can_link_sup_excluded;
				let c = other_origin_block_coords;
				(inf.x <= c.x && c.x < sup_excluded.x)
					&& (inf.y <= c.y && c.y < sup_excluded.y)
					&& (inf.z <= c.z && c.z < sup_excluded.z)
			};
			let cell_coords_inf_included_that_can_have_origins_of_structures_that_can_link =
				block_coords_to_cell_coords(
					coords_span_in_which_structure_origins_can_link_inf_included,
				);
			let cell_coords_sup_included_that_can_have_origins_of_structures_that_can_link =
				block_coords_to_cell_coords(
					coords_span_in_which_structure_origins_can_link_sup_included,
				);
			let cell_coords_sup_excluded_that_can_have_origins_of_structures_that_can_link =
				cell_coords_sup_included_that_can_have_origins_of_structures_that_can_link
					+ cgmath::vec3(1, 1, 1);
			for cell_coords in iter_3d_rect_inf_sup_excluded(
				cell_coords_inf_included_that_can_have_origins_of_structures_that_can_link,
				cell_coords_sup_excluded_that_can_have_origins_of_structures_that_can_link,
			) {
				let number_of_origins = cell_coords_to_number_of_structure_origins(cell_coords);
				for origin_index in 0..number_of_origins {
					let other_origin_coords =
						cell_coords_and_structure_origin_index_to_origin_coords_in_world(
							cell_coords,
							origin_index,
						);
					if other_origin_coords == origin_block_coords {
						// We just found ourselves.
						continue;
					}
					if structure_origin_can_link(other_origin_coords) {
						// Hehe found one, let's decide if we link.
						// We get two noise values that we will also get (in the other order)
						// in the other structure, and when we add them we get the same value
						// that the other would get, so we can agree on something that way ^^.
						let value_us_to_other = noise_a.sample(
							&[],
							&[
								origin_block_coords.x,
								origin_block_coords.y,
								origin_block_coords.z,
								other_origin_coords.x,
								other_origin_coords.y,
								other_origin_coords.z,
							],
							&[],
							None,
						);
						let value_other_to_us = noise_a.sample(
							&[],
							&[
								other_origin_coords.x,
								other_origin_coords.y,
								other_origin_coords.z,
								origin_block_coords.x,
								origin_block_coords.y,
								origin_block_coords.z,
							],
							&[],
							None,
						);
						// We only link to a few other structures because if we linked
						// to everyone we could then it fills the world with links
						// and it becomes difficult to see and appreciate the generation.
						let link = (value_us_to_other + value_other_to_us) * 0.5 < 0.08;

						if link {
							// Let's link!
							let us = origin_block_coords.map(|x| x as f32);
							let other = other_origin_coords.map(|x| x as f32);
							let direction = (other - us).normalize();
							let mut placing_head = us;
							while coords_are_writable(placing_head.map(|x| x.round() as i32)) {
								structure_place_block(
									placing_head.map(|x| x.round() as i32),
									block_type_table.ground_id(),
									chunk_blocks,
								);
								let dist_to_other_before_step = other.distance(placing_head);
								placing_head += direction * 0.1;
								let dist_to_other_after_step = other.distance(placing_head);
								if dist_to_other_before_step < dist_to_other_after_step {
									// We are moving away from other, which means we already
									// reached it and if we continued we would gon on behind it,
									// which is not what we want to do (we just want to link to it).
									break;
								}
							}
						}
					}
				}
			}

			let ball_radius = structure_max_blocky_radius.min(4);
			for coords in crate::coords::iter_3d_cube_center_radius(origin_block_coords, ball_radius) {
				if coords.map(|x| x as f32).distance(origin_block_coords.map(|x| x as f32))
					< ball_radius as f32
				{
					structure_place_block(coords, block_type_table.ground_id(), chunk_blocks);
				}
			}
		};

		// Now we generate the block data in the chunk.
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);

		// Generate terrain in the chunk.
		for coords in chunk_blocks.coords_span.iter_coords() {
			chunk_blocks.set(coords, coords_to_terrain(coords));
		}

		// Generate the structures that can overlap with the chunk.
		let coords_span_in_which_structure_origins_can_overlap_with_chunk_inf_included =
			coords_span.block_coords_inf() - cgmath::vec3(1, 1, 1) * structure_max_blocky_radius;
		let coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_excluded = coords_span
			.block_coords_sup_excluded()
			+ cgmath::vec3(1, 1, 1) * structure_max_blocky_radius;
		let coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_included =
			coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_excluded
				- cgmath::vec3(1, 1, 1);
		let structure_origin_can_overlap_with_chunk = |origin_block_coords: BlockCoords| -> bool {
			let inf = coords_span_in_which_structure_origins_can_overlap_with_chunk_inf_included;
			let sup_excluded =
				coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_excluded;
			let c = origin_block_coords;
			(inf.x <= c.x && c.x < sup_excluded.x)
				&& (inf.y <= c.y && c.y < sup_excluded.y)
				&& (inf.z <= c.z && c.z < sup_excluded.z)
		};
		let cell_coords_inf_included_that_can_have_origins_of_structures_that_can_overlap =
			block_coords_to_cell_coords(
				coords_span_in_which_structure_origins_can_overlap_with_chunk_inf_included,
			);
		let cell_coords_sup_included_that_can_have_origins_of_structures_that_can_overlap =
			block_coords_to_cell_coords(
				coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_included,
			);
		let cell_coords_sup_excluded_that_can_have_origins_of_structures_that_can_overlap =
			cell_coords_sup_included_that_can_have_origins_of_structures_that_can_overlap
				+ cgmath::vec3(1, 1, 1);
		for cell_coords in iter_3d_rect_inf_sup_excluded(
			cell_coords_inf_included_that_can_have_origins_of_structures_that_can_overlap,
			cell_coords_sup_excluded_that_can_have_origins_of_structures_that_can_overlap,
		) {
			let number_of_origins = cell_coords_to_number_of_structure_origins(cell_coords);
			for origin_index in 0..number_of_origins {
				let origin_coords = cell_coords_and_structure_origin_index_to_origin_coords_in_world(
					cell_coords,
					origin_index,
				);
				if structure_origin_can_overlap_with_chunk(origin_coords) {
					generate_structure(origin_coords, &mut chunk_blocks);
				}
			}
		}

		chunk_blocks
	}
}

struct WorldGeneratorStructuresTrees {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorStructuresTrees {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		// Define the terrain generation as a deterministic coords->block function.
		let noise_terrain = noise::OctavedNoise::new(3, vec![self.seed, 1]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let coordsf_xy = cgmath::point2(coordsf.x, coordsf.y);
			let scale = 60.0;
			let height = 20.0 * noise_terrain.sample_2d_1d(coordsf_xy / scale, &[]);
			coordsf.z < height
		};
		let coords_to_terrain = |coords: BlockCoords| -> BlockTypeId {
			let ground = coords_to_ground(coords);
			if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			}
		};

		// Setup structure origins generation stuff.
		let noise_cell_data = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let cell_size = 37;
		let block_coords_to_cell_coords = |block_coords: BlockCoords| -> cgmath::Point3<i32> {
			block_coords.map(|x| x.div_euclid(cell_size))
		};
		let cell_coords_to_number_of_structure_origins = |cell_coords: cgmath::Point3<i32>| -> usize {
			let v = noise_cell_data.sample(
				&[],
				&[cell_coords.x, cell_coords.y, cell_coords.z, 1],
				&[],
				None,
			);
			((v * 6.0 - 2.0) * 3.0).max(0.0).floor() as usize
		};
		let cell_coords_and_structure_origin_index_to_origin_coords_in_world =
			|cell_coords: cgmath::Point3<i32>, origin_index: usize| -> BlockCoords {
				let xyz: SmallVec<[f32; 3]> = [0, 1, 2]
					.into_iter()
					.map(|axis| {
						noise_cell_data.sample(
							&[],
							&[
								cell_coords.x,
								cell_coords.y,
								cell_coords.z,
								1 + axis,
								origin_index as i32,
							],
							&[],
							None,
						)
					})
					.collect();
				let coords_in_unit_cube = cgmath::point3(xyz[0], xyz[1], xyz[2]);
				let coords_in_cell =
					coords_in_unit_cube.map(|x| (x * (cell_size as f32 - 0.001)).floor() as i32);
				let cell_coords_in_world = cell_coords * cell_size;
				cell_coords_in_world + coords_in_cell.to_vec()
			};

		// Define structure generation.
		let structure_place_block = |block_coords: BlockCoords,
		                             block_type_to_place: BlockTypeId,
		                             chunk_blocks: &mut ChunkBlocks| {
			chunk_blocks.set(block_coords, block_type_to_place);
		};
		let structure_look_terrain_block = |block_coords: BlockCoords| -> BlockTypeId {
			// We already generated the terrain for the whole chunk,
			// BUT some structures may have already modified it, so we should not use it.
			coords_to_terrain(block_coords)
		};
		// Radius of the cube around the structure origin block coords in which the structure
		// generation can place blocks. A radius of 1 means just the origin block, a
		// radius of 2 means a 3x3x3 blocks sized cube around the origin block, etc.
		let structure_max_blocky_radius = 42;
		let noise_structure = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let generate_structure = |origin_block_coords: BlockCoords,
		                          chunk_blocks: &mut ChunkBlocks| {
			let mut placing_head = origin_block_coords;
			let mut found_ground = false;
			for _i in 0..structure_max_blocky_radius {
				let no_ground_above = block_type_table
					.get(structure_look_terrain_block(
						placing_head + cgmath::vec3(0, 0, 1),
					))
					.unwrap()
					.is_air();
				let ground_here =
					!block_type_table.get(structure_look_terrain_block(placing_head)).unwrap().is_air();
				if no_ground_above && ground_here {
					found_ground = true;
					break;
				}
				placing_head.z -= 1;
			}
			if !found_ground {
				return;
			}
			let noise_value_a = noise_structure.sample(
				&[],
				&[placing_head.x, placing_head.y, placing_head.z, 1],
				&[],
				None,
			);
			let height =
				((noise_value_a * 0.5 + 0.5) * structure_max_blocky_radius.min(11) as f32) as i32;
			placing_head.z += height;
			let noise_value_b = noise_structure.sample(
				&[],
				&[placing_head.x, placing_head.y, placing_head.z, 2],
				&[],
				None,
			);
			let ball_radius = (noise_value_b * 0.2 + 0.8) * 3.5;
			for coords in
				crate::coords::iter_3d_cube_center_radius(placing_head, ball_radius.ceil() as i32)
			{
				if coords.map(|x| x as f32).distance(placing_head.map(|x| x as f32)) < ball_radius {
					structure_place_block(coords, block_type_table.kinda_leaf_id(), chunk_blocks);
				}
			}
			placing_head.z -= height;
			for _i in 0..height {
				structure_place_block(placing_head, block_type_table.kinda_wood_id(), chunk_blocks);
				placing_head.z += 1;
			}
		};

		// Now we generate the block data in the chunk.
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);

		// Generate terrain in the chunk.
		for coords in chunk_blocks.coords_span.iter_coords() {
			chunk_blocks.set(coords, coords_to_terrain(coords));
		}

		// Generate the structures that can overlap with the chunk.
		let coords_span_in_which_structure_origins_can_overlap_with_chunk_inf_included =
			coords_span.block_coords_inf() - cgmath::vec3(1, 1, 1) * structure_max_blocky_radius;
		let coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_excluded = coords_span
			.block_coords_sup_excluded()
			+ cgmath::vec3(1, 1, 1) * structure_max_blocky_radius;
		let coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_included =
			coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_excluded
				- cgmath::vec3(1, 1, 1);
		let structure_origin_can_overlap_with_chunk = |origin_block_coords: BlockCoords| -> bool {
			let inf = coords_span_in_which_structure_origins_can_overlap_with_chunk_inf_included;
			let sup_excluded =
				coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_excluded;
			let c = origin_block_coords;
			(inf.x <= c.x && c.x < sup_excluded.x)
				&& (inf.y <= c.y && c.y < sup_excluded.y)
				&& (inf.z <= c.z && c.z < sup_excluded.z)
		};
		let cell_coords_inf_included_that_can_have_origins_of_structures_that_can_overlap =
			block_coords_to_cell_coords(
				coords_span_in_which_structure_origins_can_overlap_with_chunk_inf_included,
			);
		let cell_coords_sup_included_that_can_have_origins_of_structures_that_can_overlap =
			block_coords_to_cell_coords(
				coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_included,
			);
		let cell_coords_sup_excluded_that_can_have_origins_of_structures_that_can_overlap =
			cell_coords_sup_included_that_can_have_origins_of_structures_that_can_overlap
				+ cgmath::vec3(1, 1, 1);
		for cell_coords in iter_3d_rect_inf_sup_excluded(
			cell_coords_inf_included_that_can_have_origins_of_structures_that_can_overlap,
			cell_coords_sup_excluded_that_can_have_origins_of_structures_that_can_overlap,
		) {
			let number_of_origins = cell_coords_to_number_of_structure_origins(cell_coords);
			for origin_index in 0..number_of_origins {
				let origin_coords = cell_coords_and_structure_origin_index_to_origin_coords_in_world(
					cell_coords,
					origin_index,
				);
				if structure_origin_can_overlap_with_chunk(origin_coords) {
					generate_structure(origin_coords, &mut chunk_blocks);
				}
			}
		}

		chunk_blocks
	}
}

struct WorldGeneratorStructuresSpikes {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorStructuresSpikes {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		// Define the terrain generation as a deterministic coords->block function.
		let noise_terrain = noise::OctavedNoise::new(3, vec![self.seed, 1]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let coordsf_xy = cgmath::point2(coordsf.x, coordsf.y);
			let scale = 60.0;
			let height = 20.0 * noise_terrain.sample_2d_1d(coordsf_xy / scale, &[]);
			coordsf.z < height
		};
		let coords_to_terrain = |coords: BlockCoords| -> BlockTypeId {
			let ground = coords_to_ground(coords);
			if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			}
		};

		// Setup structure origins generation stuff.
		let noise_cell_data = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let cell_size = 37;
		let block_coords_to_cell_coords = |block_coords: BlockCoords| -> cgmath::Point3<i32> {
			block_coords.map(|x| x.div_euclid(cell_size))
		};
		let cell_coords_to_number_of_structure_origins = |cell_coords: cgmath::Point3<i32>| -> usize {
			let v = noise_cell_data.sample(
				&[],
				&[cell_coords.x, cell_coords.y, cell_coords.z, 1],
				&[],
				None,
			);
			(v * 3.5 - 2.0).max(0.0).floor() as usize
		};
		let cell_coords_and_structure_origin_index_to_origin_coords_in_world =
			|cell_coords: cgmath::Point3<i32>, origin_index: usize| -> BlockCoords {
				let xyz: SmallVec<[f32; 3]> = [0, 1, 2]
					.into_iter()
					.map(|axis| {
						noise_cell_data.sample(
							&[],
							&[
								cell_coords.x,
								cell_coords.y,
								cell_coords.z,
								1 + axis,
								origin_index as i32,
							],
							&[],
							None,
						)
					})
					.collect();
				let coords_in_unit_cube = cgmath::point3(xyz[0], xyz[1], xyz[2]);
				let coords_in_cell =
					coords_in_unit_cube.map(|x| (x * (cell_size as f32 - 0.001)).floor() as i32);
				let cell_coords_in_world = cell_coords * cell_size;
				cell_coords_in_world + coords_in_cell.to_vec()
			};

		// Define structure generation.
		let structure_place_block = |block_coords: BlockCoords,
		                             block_type_to_place: BlockTypeId,
		                             chunk_blocks: &mut ChunkBlocks| {
			chunk_blocks.set(block_coords, block_type_to_place);
		};
		let structure_look_terrain_block = |block_coords: BlockCoords| -> BlockTypeId {
			// We already generated the terrain for the whole chunk,
			// BUT some structures may have already modified it, so we should not use it.
			coords_to_terrain(block_coords)
		};
		// Radius of the cube around the structure origin block coords in which the structure
		// generation can place blocks. A radius of 1 means just the origin block, a
		// radius of 2 means a 3x3x3 blocks sized cube around the origin block, etc.
		let structure_max_blocky_radius = 61;
		let noise_structure = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let generate_structure = |origin_block_coords: BlockCoords,
		                          chunk_blocks: &mut ChunkBlocks| {
			let mut placing_head = origin_block_coords;
			let mut found_ground = false;
			let is_exactly_n_blocks_deep = |n: i32, coords: BlockCoords| -> bool {
				for i in 0..n {
					if block_type_table
						.get(structure_look_terrain_block(coords + cgmath::vec3(0, 0, i)))
						.unwrap()
						.is_air()
					{
						return false;
					}
				}
				block_type_table
					.get(structure_look_terrain_block(coords + cgmath::vec3(0, 0, n)))
					.unwrap()
					.is_air()
			};
			for _i in 0..structure_max_blocky_radius {
				if is_exactly_n_blocks_deep(6, placing_head) {
					found_ground = true;
					break;
				}
				placing_head.z -= 1;
			}
			if !found_ground {
				return;
			}
			let noise_value_a = noise_structure.sample(
				&[],
				&[placing_head.x, placing_head.y, placing_head.z, 1],
				&[],
				None,
			);
			let noise_value_b = noise_structure.sample(
				&[],
				&[placing_head.x, placing_head.y, placing_head.z, 2],
				&[],
				None,
			);
			let us = placing_head.map(|x| x as f32);
			let spike_end = us
				+ cgmath::vec3(
					structure_max_blocky_radius as f32 * (noise_value_a * 2.0 - 1.0),
					structure_max_blocky_radius as f32 * (noise_value_b * 2.0 - 1.0),
					structure_max_blocky_radius as f32,
				);
			let direction = (spike_end - us).normalize();
			let mut placing_head = us;
			loop {
				let ball_radius = spike_end.distance(placing_head) * 0.1 + 0.5;
				for coords in crate::coords::iter_3d_cube_center_radius(
					placing_head.map(|x| x.round() as i32),
					ball_radius.ceil() as i32,
				) {
					if coords.map(|x| x as f32).distance(placing_head) < ball_radius {
						structure_place_block(coords, block_type_table.ground_id(), chunk_blocks);
					}
				}

				let dist_to_spike_end_before_step = spike_end.distance(placing_head);
				let step = if dist_to_spike_end_before_step < 6.0 {
					0.1
				} else {
					1.0
				};
				placing_head += direction * step;
				let dist_to_spike_end_after_step = spike_end.distance(placing_head);
				if dist_to_spike_end_before_step < dist_to_spike_end_after_step {
					// We are moving away from spike_end, which means we already
					// reached it and if we continued we would gon on behind it,
					// which is not what we want to do (we just want to link to it).
					break;
				}
			}
		};

		// Now we generate the block data in the chunk.
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);

		// Generate terrain in the chunk.
		for coords in chunk_blocks.coords_span.iter_coords() {
			chunk_blocks.set(coords, coords_to_terrain(coords));
		}

		// Generate the structures that can overlap with the chunk.
		let coords_span_in_which_structure_origins_can_overlap_with_chunk_inf_included =
			coords_span.block_coords_inf() - cgmath::vec3(1, 1, 1) * structure_max_blocky_radius;
		let coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_excluded = coords_span
			.block_coords_sup_excluded()
			+ cgmath::vec3(1, 1, 1) * structure_max_blocky_radius;
		let coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_included =
			coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_excluded
				- cgmath::vec3(1, 1, 1);
		let structure_origin_can_overlap_with_chunk = |origin_block_coords: BlockCoords| -> bool {
			let inf = coords_span_in_which_structure_origins_can_overlap_with_chunk_inf_included;
			let sup_excluded =
				coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_excluded;
			let c = origin_block_coords;
			(inf.x <= c.x && c.x < sup_excluded.x)
				&& (inf.y <= c.y && c.y < sup_excluded.y)
				&& (inf.z <= c.z && c.z < sup_excluded.z)
		};
		let cell_coords_inf_included_that_can_have_origins_of_structures_that_can_overlap =
			block_coords_to_cell_coords(
				coords_span_in_which_structure_origins_can_overlap_with_chunk_inf_included,
			);
		let cell_coords_sup_included_that_can_have_origins_of_structures_that_can_overlap =
			block_coords_to_cell_coords(
				coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_included,
			);
		let cell_coords_sup_excluded_that_can_have_origins_of_structures_that_can_overlap =
			cell_coords_sup_included_that_can_have_origins_of_structures_that_can_overlap
				+ cgmath::vec3(1, 1, 1);
		for cell_coords in iter_3d_rect_inf_sup_excluded(
			cell_coords_inf_included_that_can_have_origins_of_structures_that_can_overlap,
			cell_coords_sup_excluded_that_can_have_origins_of_structures_that_can_overlap,
		) {
			let number_of_origins = cell_coords_to_number_of_structure_origins(cell_coords);
			for origin_index in 0..number_of_origins {
				let origin_coords = cell_coords_and_structure_origin_index_to_origin_coords_in_world(
					cell_coords,
					origin_index,
				);
				if structure_origin_can_overlap_with_chunk(origin_coords) {
					generate_structure(origin_coords, &mut chunk_blocks);
				}
			}
		}

		chunk_blocks
	}
}

struct WorldGeneratorLines02 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorLines02 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(5, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(5, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(5, vec![self.seed, 3]);
		let noise_no_grass = noise::OctavedNoise::new(5, vec![self.seed, 3]);
		let noise_grass_a = noise::OctavedNoise::new(2, vec![self.seed, 1, 1]);
		let noise_grass_b = noise::OctavedNoise::new(2, vec![self.seed, 1, 2]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 100.0;
			let a = noise_a.sample_3d_1d(coordsf / scale, &[]);
			let b = noise_b.sample_3d_1d(coordsf / scale, &[]);
			let c = noise_c.sample_3d_1d(coordsf / scale, &[]);
			let v = 0.03;
			(a - c).abs() < v && (b - c).abs() < v && (a - b).abs() < v
		};
		let coords_to_grass = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let d = noise_grass_a.sample_3d_1d(coordsf / scale, &[]);
			let density = if d < 0.1 {
				d * 0.9 + 0.1
			} else if d < 0.3 {
				0.1
			} else {
				0.01
			};
			noise_grass_b.sample_3d_1d(coordsf, &[]) < density
		};
		let coords_to_no_grass = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 75.0;
			noise_no_grass.sample_3d_1d(coordsf / scale, &[]) < 0.25
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					let no_grass = coords_to_no_grass(coords);
					if no_grass {
						block_type_table.ground_id()
					} else {
						block_type_table.kinda_grass_id()
					}
				}
			} else {
				let ground_below = coords_to_ground(coords + cgmath::vec3(0, 0, -1));
				if ground_below {
					let no_grass_below = coords_to_no_grass(coords + cgmath::vec3(0, 0, -1));
					if no_grass_below {
						block_type_table.air_id()
					} else if coords_to_grass(coords) {
						block_type_table.kinda_grass_blades_id()
					} else {
						block_type_table.air_id()
					}
				} else {
					block_type_table.air_id()
				}
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorLines03 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorLines03 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(4, vec![self.seed, 1]);
		let noise_b = noise::OctavedNoise::new(4, vec![self.seed, 2]);
		let noise_c = noise::OctavedNoise::new(4, vec![self.seed, 3]);
		let noise_no_grass = noise::OctavedNoise::new(5, vec![self.seed, 3]);
		let noise_grass_a = noise::OctavedNoise::new(2, vec![self.seed, 1, 1]);
		let noise_grass_b = noise::OctavedNoise::new(2, vec![self.seed, 1, 2]);
		let sorted_noises = |coordsf: cgmath::Point3<f32>| -> [(usize, f32); 3] {
			let mut array = [
				(0, noise_a.sample_3d_1d(coordsf, &[])),
				(1, noise_b.sample_3d_1d(coordsf, &[])),
				(2, noise_c.sample_3d_1d(coordsf, &[])),
			];
			array.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap().reverse());
			array
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 100.0;
			let sn = sorted_noises(coordsf / scale);
			sn[0].1 - sn[1].1 < 0.02 && sn[0].1 - sn[2].1 < 0.02
		};
		let coords_to_grass = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let d = noise_grass_a.sample_3d_1d(coordsf / scale, &[]);
			let density = if d < 0.1 {
				d * 0.9 + 0.1
			} else if d < 0.3 {
				0.1
			} else {
				0.01
			};
			noise_grass_b.sample_3d_1d(coordsf, &[]) < density
		};
		let coords_to_no_grass = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 75.0;
			noise_no_grass.sample_3d_1d(coordsf / scale, &[]) < 0.25
		};
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			let block = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					let no_grass = coords_to_no_grass(coords);
					if no_grass {
						block_type_table.ground_id()
					} else {
						block_type_table.kinda_grass_id()
					}
				}
			} else {
				let ground_below = coords_to_ground(coords + cgmath::vec3(0, 0, -1));
				if ground_below {
					let no_grass_below = coords_to_no_grass(coords + cgmath::vec3(0, 0, -1));
					if no_grass_below {
						block_type_table.air_id()
					} else if coords_to_grass(coords) {
						block_type_table.kinda_grass_blades_id()
					} else {
						block_type_table.air_id()
					}
				} else {
					block_type_table.air_id()
				}
			};
			chunk_blocks.set(coords, block);
		}
		chunk_blocks
	}
}

struct WorldGeneratorStructuresLinksSmooth {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorStructuresLinksSmooth {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		// Define the terrain generation as a deterministic coords->block function.
		let noise_terrain = noise::OctavedNoise::new(3, vec![self.seed, 1]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let coordsf_xy = cgmath::point2(coordsf.x, coordsf.y);
			let scale = 60.0;
			let height = 20.0 * noise_terrain.sample_2d_1d(coordsf_xy / scale, &[]);
			coordsf.z < height
		};
		let coords_to_terrain = |coords: BlockCoords| -> BlockTypeId {
			let ground = coords_to_ground(coords);
			if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			}
		};

		// Setup structure origins generation stuff.
		let noise_cell_data = noise::OctavedNoise::new(1, vec![self.seed, 2]);
		let cell_size = 51;
		let block_coords_to_cell_coords = |block_coords: BlockCoords| -> cgmath::Point3<i32> {
			block_coords.map(|x| x.div_euclid(cell_size))
		};
		let cell_coords_to_number_of_structure_origins = |cell_coords: cgmath::Point3<i32>| -> usize {
			let v = noise_cell_data.sample(
				&[],
				&[cell_coords.x, cell_coords.y, cell_coords.z, 1],
				&[],
				None,
			);
			(v * 20.0 - 17.5).max(0.0).floor() as usize
		};
		let cell_coords_and_structure_origin_index_to_origin_coords_in_world =
			|cell_coords: cgmath::Point3<i32>, origin_index: usize| -> BlockCoords {
				let xyz: SmallVec<[f32; 3]> = [0, 1, 2]
					.into_iter()
					.map(|axis| {
						noise_cell_data.sample(
							&[],
							&[
								cell_coords.x,
								cell_coords.y,
								cell_coords.z,
								1 + axis,
								origin_index as i32,
							],
							&[],
							None,
						)
					})
					.collect();
				let coords_in_unit_cube = cgmath::point3(xyz[0], xyz[1], xyz[2]);
				let coords_in_cell =
					coords_in_unit_cube.map(|x| (x * (cell_size as f32 - 0.001)).floor() as i32);
				let cell_coords_in_world = cell_coords * cell_size;
				cell_coords_in_world + coords_in_cell.to_vec()
			};

		// Define structure generation.
		let structure_place_block = |block_coords: BlockCoords,
		                             block_type_to_place: BlockTypeId,
		                             chunk_blocks: &mut ChunkBlocks| {
			chunk_blocks.set(block_coords, block_type_to_place);
		};
		let _structure_look_terrain_block = |block_coords: BlockCoords| -> BlockTypeId {
			// We already generated the terrain for the whole chunk,
			// BUT some structures may have already modified it, so we should not use it.
			coords_to_terrain(block_coords)
		};
		// Radius of the cube around the structure origin block coords in which the structure
		// generation can place blocks. A radius of 1 means just the origin block, a
		// radius of 2 means a 3x3x3 blocks sized cube around the origin block, etc.
		let structure_max_blocky_radius = 61;
		let noise_a = noise::OctavedNoise::new(1, vec![self.seed, 1]);
		let structure_place_ball = |center: cgmath::Point3<f32>,
		                            radius: f32,
		                            chunk_blocks: &mut ChunkBlocks| {
			let coords_inf = (center - cgmath::vec3(1.0, 1.0, 1.0) * radius).map(|x| x.floor() as i32);
			let coords_sup = (center + cgmath::vec3(1.0, 1.0, 1.0) * radius).map(|x| x.ceil() as i32);
			let chunk_inf = chunk_blocks.coords_span.block_coords_inf();
			let chunk_sup =
				chunk_blocks.coords_span.block_coords_sup_excluded() - cgmath::vec3(1, 1, 1);
			let no_overlap_x = chunk_sup.x < coords_inf.x || chunk_inf.x > coords_sup.x;
			let no_overlap_y = chunk_sup.y < coords_inf.y || chunk_inf.y > coords_sup.y;
			let no_overlap_z = chunk_sup.z < coords_inf.z || chunk_inf.z > coords_sup.z;
			let no_overlap = no_overlap_x && no_overlap_y && no_overlap_z;
			if no_overlap {
				return;
			}
			for coords in crate::coords::iter_3d_cube_center_radius(
				center.map(|x| x.round() as i32),
				radius.ceil() as i32,
			) {
				if coords.map(|x| x as f32).distance(center) < radius {
					structure_place_block(coords, block_type_table.ground_id(), chunk_blocks);
				}
			}
		};
		let generate_structure = |origin_block_coords: BlockCoords,
		                          chunk_blocks: &mut ChunkBlocks| {
			// Setup function thta says if we are in the cubic area that we can actually modify.
			let coords_span_writable_inf_included =
				origin_block_coords - cgmath::vec3(1, 1, 1) * (structure_max_blocky_radius * 2 - 2);
			let coords_span_writable_sup_excluded =
				origin_block_coords + cgmath::vec3(1, 1, 1) * (structure_max_blocky_radius * 2 - 2 + 1);
			let coords_are_writable = |block_coords: BlockCoords| -> bool {
				let inf = coords_span_writable_inf_included;
				let sup_excluded = coords_span_writable_sup_excluded;
				let c = block_coords;
				(inf.x <= c.x && c.x < sup_excluded.x)
					&& (inf.y <= c.y && c.y < sup_excluded.y)
					&& (inf.z <= c.z && c.z < sup_excluded.z)
			};

			// Find nearby structures that we can link to.
			let coords_span_in_which_structure_origins_can_link_inf_included =
				origin_block_coords - cgmath::vec3(1, 1, 1) * (structure_max_blocky_radius * 2 - 2);
			let coords_span_in_which_structure_origins_can_link_sup_excluded =
				origin_block_coords + cgmath::vec3(1, 1, 1) * (structure_max_blocky_radius * 2 - 2 + 1);
			let coords_span_in_which_structure_origins_can_link_sup_included =
				coords_span_in_which_structure_origins_can_link_sup_excluded - cgmath::vec3(1, 1, 1);
			let structure_origin_can_link = |other_origin_block_coords: BlockCoords| -> bool {
				let inf = coords_span_in_which_structure_origins_can_link_inf_included;
				let sup_excluded = coords_span_in_which_structure_origins_can_link_sup_excluded;
				let c = other_origin_block_coords;
				(inf.x <= c.x && c.x < sup_excluded.x)
					&& (inf.y <= c.y && c.y < sup_excluded.y)
					&& (inf.z <= c.z && c.z < sup_excluded.z)
			};
			let cell_coords_inf_included_that_can_have_origins_of_structures_that_can_link =
				block_coords_to_cell_coords(
					coords_span_in_which_structure_origins_can_link_inf_included,
				);
			let cell_coords_sup_included_that_can_have_origins_of_structures_that_can_link =
				block_coords_to_cell_coords(
					coords_span_in_which_structure_origins_can_link_sup_included,
				);
			let cell_coords_sup_excluded_that_can_have_origins_of_structures_that_can_link =
				cell_coords_sup_included_that_can_have_origins_of_structures_that_can_link
					+ cgmath::vec3(1, 1, 1);
			for cell_coords in iter_3d_rect_inf_sup_excluded(
				cell_coords_inf_included_that_can_have_origins_of_structures_that_can_link,
				cell_coords_sup_excluded_that_can_have_origins_of_structures_that_can_link,
			) {
				let number_of_origins = cell_coords_to_number_of_structure_origins(cell_coords);
				for origin_index in 0..number_of_origins {
					let other_origin_coords =
						cell_coords_and_structure_origin_index_to_origin_coords_in_world(
							cell_coords,
							origin_index,
						);
					if other_origin_coords == origin_block_coords {
						// We just found ourselves.
						continue;
					}
					if structure_origin_can_link(other_origin_coords) {
						// Hehe found one, let's decide if we link.
						// We get two noise values that we will also get (in the other order)
						// in the other structure, and when we add them we get the same value
						// that the other would get, so we can agree on something that way ^^.
						let value_us_to_other = noise_a.sample(
							&[],
							&[
								origin_block_coords.x,
								origin_block_coords.y,
								origin_block_coords.z,
								other_origin_coords.x,
								other_origin_coords.y,
								other_origin_coords.z,
							],
							&[],
							None,
						);
						let value_other_to_us = noise_a.sample(
							&[],
							&[
								other_origin_coords.x,
								other_origin_coords.y,
								other_origin_coords.z,
								origin_block_coords.x,
								origin_block_coords.y,
								origin_block_coords.z,
							],
							&[],
							None,
						);
						// We only link to a few other structures because if we linked
						// to everyone we could then it fills the world with links
						// and it becomes difficult to see and appreciate the generation.
						let link = (value_us_to_other + value_other_to_us) * 0.5 < 0.25;

						if link {
							// Let's link!
							let us = origin_block_coords.map(|x| x as f32);
							let other = other_origin_coords.map(|x| x as f32);
							let distance_us_other = us.distance(other);
							let direction = (other - us).normalize();
							let mut placing_head = us;
							while coords_are_writable(placing_head.map(|x| x.round() as i32)) {
								let dist_to_us = us.distance(placing_head);
								let dist_to_other_before_step = other.distance(placing_head);
								let progression = 1.0 - dist_to_us / (distance_us_other / 2.0);
								let smooth_progression = progression.powi(2);
								let radius_min = 2.0;
								let radius_max = 7.0;
								let radius = radius_min + smooth_progression * (radius_max - radius_min);
								structure_place_ball(placing_head, radius, chunk_blocks);
								placing_head += direction * 0.8;
								let dist_to_other_after_step = other.distance(placing_head);
								if dist_to_other_before_step < dist_to_other_after_step {
									// We are moving away from other, which means we already
									// reached it and if we continued we would gon on behind it,
									// which is not what we want to do (we just want to link to it).
									break;
								}
							}
						}
					}
				}
			}
		};

		// Now we generate the block data in the chunk.
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);

		// Generate terrain in the chunk.
		for coords in chunk_blocks.coords_span.iter_coords() {
			chunk_blocks.set(coords, coords_to_terrain(coords));
		}

		// Generate the structures that can overlap with the chunk.
		let coords_span_in_which_structure_origins_can_overlap_with_chunk_inf_included =
			coords_span.block_coords_inf() - cgmath::vec3(1, 1, 1) * structure_max_blocky_radius;
		let coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_excluded = coords_span
			.block_coords_sup_excluded()
			+ cgmath::vec3(1, 1, 1) * structure_max_blocky_radius;
		let coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_included =
			coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_excluded
				- cgmath::vec3(1, 1, 1);
		let structure_origin_can_overlap_with_chunk = |origin_block_coords: BlockCoords| -> bool {
			let inf = coords_span_in_which_structure_origins_can_overlap_with_chunk_inf_included;
			let sup_excluded =
				coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_excluded;
			let c = origin_block_coords;
			(inf.x <= c.x && c.x < sup_excluded.x)
				&& (inf.y <= c.y && c.y < sup_excluded.y)
				&& (inf.z <= c.z && c.z < sup_excluded.z)
		};
		let cell_coords_inf_included_that_can_have_origins_of_structures_that_can_overlap =
			block_coords_to_cell_coords(
				coords_span_in_which_structure_origins_can_overlap_with_chunk_inf_included,
			);
		let cell_coords_sup_included_that_can_have_origins_of_structures_that_can_overlap =
			block_coords_to_cell_coords(
				coords_span_in_which_structure_origins_can_overlap_with_chunk_sup_included,
			);
		let cell_coords_sup_excluded_that_can_have_origins_of_structures_that_can_overlap =
			cell_coords_sup_included_that_can_have_origins_of_structures_that_can_overlap
				+ cgmath::vec3(1, 1, 1);
		for cell_coords in iter_3d_rect_inf_sup_excluded(
			cell_coords_inf_included_that_can_have_origins_of_structures_that_can_overlap,
			cell_coords_sup_excluded_that_can_have_origins_of_structures_that_can_overlap,
		) {
			let number_of_origins = cell_coords_to_number_of_structure_origins(cell_coords);
			for origin_index in 0..number_of_origins {
				let origin_coords = cell_coords_and_structure_origin_index_to_origin_coords_in_world(
					cell_coords,
					origin_index,
				);
				if structure_origin_can_overlap_with_chunk(origin_coords) {
					generate_structure(origin_coords, &mut chunk_blocks);
				}
			}
		}

		chunk_blocks
	}
}

mod structure_gen {
	use std::sync::Arc;

	use cgmath::{EuclideanSpace, MetricSpace};

	use crate::{
		chunks::{BlockTypeId, BlockTypeTable, ChunkBlocks},
		coords::{BlockCoords, CubicCoordsSpan},
		noise::OctavedNoise,
	};

	#[derive(Clone, Copy)]
	pub struct StructureTypeId {
		pub index: usize,
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
	pub struct StructureOrigin {
		pub coords: BlockCoords,
		pub type_id: StructureTypeId,
	}

	/// Handles generation of structure origins.
	pub trait StructureOriginGenerator {
		fn get_origins_in_span(&self, span: CubicCoordsSpan) -> Vec<StructureOrigin>;
	}

	/// The idea of this structure origin generator is that it considers a grid of big cubic cells
	/// and uses noise to deterministically place, in each cell, a noise-obtained number of origins
	/// at noise-obtained coords in the cell.
	pub struct TestStructureOriginGenerator {
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
		pub fn new(
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
	pub struct StructureInstanceGenerationContext<'a> {
		pub origin: StructureOrigin,
		pub chunk_blocks: &'a mut ChunkBlocks,
		pub origin_generator: &'a dyn StructureOriginGenerator,
		pub block_type_table: &'a Arc<BlockTypeTable>,
		pub terrain_generator: &'a TerrainGenerator<'a>,
	}

	/// When a structure generation wants to place a block, it may want to do so in some way
	/// that is specified by this type. For example, the structure generation might want to
	/// place some block somewhere but only if it replaces air, well this would be specified
	/// by this type.
	pub struct BlockPlacing {
		pub block_type_to_place: BlockTypeId,
		pub only_place_on_air: bool,
	}

	impl<'a> StructureInstanceGenerationContext<'a> {
		pub fn place_block(&mut self, block_placing: &BlockPlacing, coords: BlockCoords) {
			let shall_place_block = !block_placing.only_place_on_air
				|| self
					.chunk_blocks
					.get(coords)
					.is_some_and(|block_type| self.block_type_table.get(block_type).unwrap().is_air());
			if shall_place_block {
				self.chunk_blocks.set(coords, block_placing.block_type_to_place);
			}
		}

		pub fn place_ball(
			&mut self,
			block_placing: &BlockPlacing,
			center: cgmath::Point3<f32>,
			radius: f32,
		) {
			let ball_inf = (center - cgmath::vec3(1.0, 1.0, 1.0) * radius).map(|x| x.floor() as i32);
			let ball_sup = (center + cgmath::vec3(1.0, 1.0, 1.0) * radius).map(|x| x.ceil() as i32);
			let ball_span = CubicCoordsSpan::with_inf_sup_but_sup_is_included(ball_inf, ball_sup);
			let chunk_span = CubicCoordsSpan::from_chunk_span(self.chunk_blocks.coords_span);
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
	pub type StructureTypeInstanceGenerator<'a> = dyn Fn(StructureInstanceGenerationContext) + 'a;
}

use structure_gen::*;

struct WorldGeneratorStructuresEnginePoc {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorStructuresEnginePoc {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		// Define the terrain generation as a deterministic coords->block function.
		let noise_terrain = noise::OctavedNoise::new(3, vec![self.seed, 1]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let coordsf_xy = cgmath::point2(coordsf.x, coordsf.y);
			let scale = 60.0;
			let height = 20.0 * noise_terrain.sample_2d_1d(coordsf_xy / scale, &[]);
			coordsf.z < height
		};
		let block_type_table_for_terrain = Arc::clone(&block_type_table);
		let coords_to_terrain = |coords: BlockCoords| -> BlockTypeId {
			let ground = coords_to_ground(coords);
			if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table_for_terrain.ground_id()
				} else {
					block_type_table_for_terrain.kinda_grass_id()
				}
			} else {
				block_type_table_for_terrain.air_id()
			}
		};

		// Define structure generation.
		let structure_max_blocky_radius = 42;
		let noise_structure = noise::OctavedNoise::new(1, vec![self.seed, 3]);
		let generate_structure_tree = |mut context: StructureInstanceGenerationContext| {
			// Let's (try to) generate a tree.
			let mut placing_head = context.origin.coords;
			// We try to find the ground (we don't want to generate a tree floating in the air).
			// We go down and stop on ground, or abort (and fail to generate) if no ground is found.
			let mut found_ground = false;
			for _i in 0..structure_max_blocky_radius {
				let no_ground_above = context
					.block_type_table
					.get((context.terrain_generator)(
						placing_head + cgmath::vec3(0, 0, 1),
					))
					.unwrap()
					.is_air();
				let ground_here = !context
					.block_type_table
					.get((context.terrain_generator)(placing_head))
					.unwrap()
					.is_air();
				if no_ground_above && ground_here {
					found_ground = true;
					break;
				}
				placing_head.z -= 1;
			}
			if !found_ground {
				return;
			}
			// We are on the ground now, we can generate a tree.
			// We pick a height of the trunk and generate it.
			let noise_value_a = noise_structure.sample_i3d_1d(placing_head, &[1]);
			let height =
				((noise_value_a * 0.5 + 0.5) * structure_max_blocky_radius.min(21) as f32) as i32;
			for _i in 0..height {
				context.place_block(
					&BlockPlacing {
						block_type_to_place: context.block_type_table.kinda_wood_id(),
						only_place_on_air: false,
					},
					placing_head,
				);
				placing_head.z += 1;
			}
			// We pick a radius for the ball of leaves and generate it.
			let noise_value_b = noise_structure.sample_i3d_1d(placing_head, &[2]);
			let ball_radius = (noise_value_b * 0.2 + 0.8) * 3.5;
			context.place_ball(
				&BlockPlacing {
					block_type_to_place: context.block_type_table.kinda_leaf_id(),
					only_place_on_air: true,
				},
				placing_head.map(|x| x as f32),
				ball_radius,
			);
			// The tree is done now ^^.
		};
		let noise_structure = noise::OctavedNoise::new(1, vec![self.seed, 4]);
		let generate_structure_boulder = |mut context: StructureInstanceGenerationContext| {
			let mut placing_head = context.origin.coords;
			let mut found_ground = false;
			for _i in 0..structure_max_blocky_radius {
				let no_ground_above = context
					.block_type_table
					.get((context.terrain_generator)(
						placing_head + cgmath::vec3(0, 0, 1),
					))
					.unwrap()
					.is_air();
				let ground_here = !context
					.block_type_table
					.get((context.terrain_generator)(placing_head))
					.unwrap()
					.is_air();
				if no_ground_above && ground_here {
					found_ground = true;
					break;
				}
				placing_head.z -= 1;
			}
			if !found_ground {
				return;
			}
			let noise_value_b = noise_structure.sample_i3d_1d(placing_head, &[2]);
			let ball_radius = (noise_value_b * 0.2 + 0.8) * 2.5;
			context.place_ball(
				&BlockPlacing {
					block_type_to_place: context.block_type_table.ground_id(),
					only_place_on_air: true,
				},
				placing_head.map(|x| x as f32),
				ball_radius,
			);
		};

		let structure_types: [&StructureTypeInstanceGenerator; 2] =
			[&generate_structure_tree, &generate_structure_boulder];

		// Setup structure origins generation stuff.
		let structure_origin_generator =
			TestStructureOriginGenerator::new(self.seed, 31, (-2, 21), structure_types.len() as i32);

		// Now we generate the block data in the chunk.
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);

		// Generate terrain in the chunk.
		for coords in chunk_blocks.coords_span.iter_coords() {
			chunk_blocks.set(coords, coords_to_terrain(coords));
		}

		// Generate the structures that can overlap with the chunk.
		let mut span_to_check = CubicCoordsSpan::from_chunk_span(coords_span);
		span_to_check.add_margins(structure_max_blocky_radius);
		let origins = structure_origin_generator.get_origins_in_span(span_to_check);
		for origin in origins.into_iter() {
			let context = StructureInstanceGenerationContext {
				origin,
				chunk_blocks: &mut chunk_blocks,
				origin_generator: &structure_origin_generator,
				block_type_table: &block_type_table,
				terrain_generator: &coords_to_terrain,
			};
			structure_types[origin.type_id.index](context);
		}

		chunk_blocks
	}
}

struct WorldGeneratorStructuresGeneratedBlocks {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorStructuresGeneratedBlocks {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		// Define the terrain generation as a deterministic coords->block function.
		let noise_terrain = noise::OctavedNoise::new(3, vec![self.seed, 1]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let coordsf_xy = cgmath::point2(coordsf.x, coordsf.y);
			let scale = 60.0;
			let height = 20.0 * noise_terrain.sample_2d_1d(coordsf_xy / scale, &[]);
			coordsf.z < height
		};
		let block_type_table_for_terrain = Arc::clone(&block_type_table);
		let coords_to_terrain = |coords: BlockCoords| -> BlockTypeId {
			let ground = coords_to_ground(coords);
			if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table_for_terrain.ground_id()
				} else {
					block_type_table_for_terrain.kinda_grass_id()
				}
			} else {
				block_type_table_for_terrain.air_id()
			}
		};

		// Define structure generation.
		let structure_max_blocky_radius = 42;
		let mut structure_types = vec![];
		for i in 0..100 {
			let noise_structure = noise::OctavedNoise::new(1, vec![self.seed, 3 + i]);
			let generate_structure = move |mut context: StructureInstanceGenerationContext| {
				let mut placing_head = context.origin.coords;
				let mut found_ground = false;
				for _i in 0..structure_max_blocky_radius {
					let no_ground_above = context
						.block_type_table
						.get((context.terrain_generator)(
							placing_head + cgmath::vec3(0, 0, 1),
						))
						.unwrap()
						.is_air();
					let ground_here = !context
						.block_type_table
						.get((context.terrain_generator)(placing_head))
						.unwrap()
						.is_air();
					if no_ground_above && ground_here {
						found_ground = true;
						break;
					}
					placing_head.z -= 1;
				}
				if !found_ground {
					return;
				}
				let noise_value_b = noise_structure.sample_i3d_1d(placing_head, &[2]);
				let noise_value_c = noise_structure.sample_i3d_1d(placing_head, &[3]);
				let mut ball_radius = (noise_value_b * 2.3 + 1.2) * 1.5;
				while ball_radius > 0.5 {
					context.place_ball(
						&BlockPlacing {
							block_type_to_place: context.block_type_table.generated_test_id(i as usize),
							only_place_on_air: true,
						},
						placing_head.map(|x| x as f32),
						ball_radius,
					);
					ball_radius -= 0.3 + 0.3 * noise_value_c;
					placing_head.z += 1;
				}
			};
			structure_types.push(generate_structure);
		}

		// Setup structure origins generation stuff.
		let structure_origin_generator =
			TestStructureOriginGenerator::new(self.seed, 31, (-2, 21), structure_types.len() as i32);

		// Now we generate the block data in the chunk.
		let mut chunk_blocks = ChunkBlocks::new_empty(coords_span);

		// Generate terrain in the chunk.
		for coords in chunk_blocks.coords_span.iter_coords() {
			chunk_blocks.set(coords, coords_to_terrain(coords));
		}

		// Generate the structures that can overlap with the chunk.
		let mut span_to_check = CubicCoordsSpan::from_chunk_span(coords_span);
		span_to_check.add_margins(structure_max_blocky_radius);
		let origins = structure_origin_generator.get_origins_in_span(span_to_check);
		for origin in origins.into_iter() {
			let context = StructureInstanceGenerationContext {
				origin,
				chunk_blocks: &mut chunk_blocks,
				origin_generator: &structure_origin_generator,
				block_type_table: &block_type_table,
				terrain_generator: &coords_to_terrain,
			};
			structure_types[origin.type_id.index](context);
		}

		chunk_blocks
	}
}
