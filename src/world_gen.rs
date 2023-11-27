use std::{f32::consts::TAU, sync::Arc};

use cgmath::{EuclideanSpace, InnerSpace, MetricSpace};
use enum_iterator::Sequence;
use smallvec::SmallVec;

use crate::coords::{iter_3d_rect_inf_sup_excluded, NonOrientedAxis};
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

#[derive(Clone, Copy, Sequence)]
pub enum WhichWorldGenerator {
	Default,
	Flat,
	Empty,
	Test001,
	Test002,
	Test003,
	Test004,
	Test005,
	Test006,
	Test007,
	Test008,
	Test009,
	Test010,
	Test011,
	Test012,
	Test013,
	Test014,
	Test015,
	Test016,
	Test017,
	Test018,
	Test019,
	Test020,
	Test021,
	Test022,
	Test023,
	Test024,
	Test025,
	Test026,
}

impl WhichWorldGenerator {
	pub fn name(self) -> &'static str {
		match self {
			WhichWorldGenerator::Default => "default",
			WhichWorldGenerator::Flat => "flat",
			WhichWorldGenerator::Empty => "empty",
			WhichWorldGenerator::Test001 => "test001",
			WhichWorldGenerator::Test002 => "test002",
			WhichWorldGenerator::Test003 => "test003",
			WhichWorldGenerator::Test004 => "test004",
			WhichWorldGenerator::Test005 => "test005",
			WhichWorldGenerator::Test006 => "test006",
			WhichWorldGenerator::Test007 => "test007",
			WhichWorldGenerator::Test008 => "test008",
			WhichWorldGenerator::Test009 => "test009",
			WhichWorldGenerator::Test010 => "test010",
			WhichWorldGenerator::Test011 => "test011",
			WhichWorldGenerator::Test012 => "test012",
			WhichWorldGenerator::Test013 => "test013",
			WhichWorldGenerator::Test014 => "test014",
			WhichWorldGenerator::Test015 => "test015",
			WhichWorldGenerator::Test016 => "test016",
			WhichWorldGenerator::Test017 => "test017",
			WhichWorldGenerator::Test018 => "test018",
			WhichWorldGenerator::Test019 => "test019",
			WhichWorldGenerator::Test020 => "test020",
			WhichWorldGenerator::Test021 => "test021",
			WhichWorldGenerator::Test022 => "test022",
			WhichWorldGenerator::Test023 => "test023",
			WhichWorldGenerator::Test024 => "test024",
			WhichWorldGenerator::Test025 => "test025",
			WhichWorldGenerator::Test026 => "test026",
		}
	}

	pub fn get_the_actual_generator(self, seed: i32) -> Arc<dyn WorldGenerator + Sync + Send> {
		match self {
			WhichWorldGenerator::Default => Arc::new(DefaultWorldGenerator { seed }),
			WhichWorldGenerator::Flat => Arc::new(FlatWorldGenerator {}),
			WhichWorldGenerator::Empty => Arc::new(EmptyWorldGenerator {}),
			WhichWorldGenerator::Test001 => Arc::new(WorldGeneratorTest001 { seed }),
			WhichWorldGenerator::Test002 => Arc::new(WorldGeneratorTest002 { seed }),
			WhichWorldGenerator::Test003 => Arc::new(WorldGeneratorTest003 { seed }),
			WhichWorldGenerator::Test004 => Arc::new(WorldGeneratorTest004 { seed }),
			WhichWorldGenerator::Test005 => Arc::new(WorldGeneratorTest005 { seed }),
			WhichWorldGenerator::Test006 => Arc::new(WorldGeneratorTest006 { seed }),
			WhichWorldGenerator::Test007 => Arc::new(WorldGeneratorTest007 { seed }),
			WhichWorldGenerator::Test008 => Arc::new(WorldGeneratorTest008 { seed }),
			WhichWorldGenerator::Test009 => Arc::new(WorldGeneratorTest009 { seed }),
			WhichWorldGenerator::Test010 => Arc::new(WorldGeneratorTest010 { seed }),
			WhichWorldGenerator::Test011 => Arc::new(WorldGeneratorTest011 { seed }),
			WhichWorldGenerator::Test012 => Arc::new(WorldGeneratorTest012 { seed }),
			WhichWorldGenerator::Test013 => Arc::new(WorldGeneratorTest013 { seed }),
			WhichWorldGenerator::Test014 => Arc::new(WorldGeneratorTest014 { seed }),
			WhichWorldGenerator::Test015 => Arc::new(WorldGeneratorTest015 { seed }),
			WhichWorldGenerator::Test016 => Arc::new(WorldGeneratorTest016 { seed }),
			WhichWorldGenerator::Test017 => Arc::new(WorldGeneratorTest017 { seed }),
			WhichWorldGenerator::Test018 => Arc::new(WorldGeneratorTest018 { seed }),
			WhichWorldGenerator::Test019 => Arc::new(WorldGeneratorTest019 { seed }),
			WhichWorldGenerator::Test020 => Arc::new(WorldGeneratorTest020 { seed }),
			WhichWorldGenerator::Test021 => Arc::new(WorldGeneratorTest021 { seed }),
			WhichWorldGenerator::Test022 => Arc::new(WorldGeneratorTest022 { seed }),
			WhichWorldGenerator::Test023 => Arc::new(WorldGeneratorTest023 { seed }),
			WhichWorldGenerator::Test024 => Arc::new(WorldGeneratorTest024 { seed }),
			WhichWorldGenerator::Test025 => Arc::new(WorldGeneratorTest025 { seed }),
			WhichWorldGenerator::Test026 => Arc::new(WorldGeneratorTest026 { seed }),
		}
	}

	pub fn from_name(name: &str) -> Option<WhichWorldGenerator> {
		// This is actually not that worse from a match from names to variants,
		// and it allows for the "compile time table" to be in the other direction,
		// which makes errors less probable.
		enum_iterator::all::<WhichWorldGenerator>().find(|&variant| variant.name() == name)
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
			let a = noise_a.sample_3d(coordsf / scale, &[]);
			let b = noise_b.sample_3d(coordsf / scale, &[]);
			(coordsf.z < b * 5.0 && a < 0.7) || b < 0.3
		};
		let coords_to_grass = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let d = noise_grass_a.sample_3d(coordsf / scale, &[]);
			let density = if d < 0.1 {
				d * 0.9 + 0.1
			} else if d < 0.3 {
				0.1
			} else {
				0.01
			};
			noise_grass_b.sample_3d(coordsf, &[]) < density
		};
		let coords_to_no_grass = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 75.0;
			noise_no_grass.sample_3d(coordsf / scale, &[]) < 0.25
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
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
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			#[allow(clippy::comparison_chain)]
			{
				*chunk_blocks.get_mut(coords).unwrap() = if coords.z < 0 {
					block_type_table.ground_id()
				} else if coords.z == 0 {
					block_type_table.kinda_grass_id()
				} else {
					block_type_table.air_id()
				};
			}
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
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			*chunk_blocks.get_mut(coords).unwrap() =
				if coords.z == -1 && (-3..=3).contains(&coords.x) && (-3..=3).contains(&coords.y) {
					block_type_table.ground_id()
				} else {
					block_type_table.air_id()
				};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest001 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest001 {
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
			let a = noise_a.sample_3d(coordsf / scale, &[]);
			let b = noise_b.sample_3d(coordsf / scale, &[]);
			(a - 0.5).abs() < 0.03 && (b - 0.5).abs() < 0.03
		};
		let coords_to_grass = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 30.0;
			let d = noise_grass_a.sample_3d(coordsf / scale, &[]);
			let density = if d < 0.1 {
				d * 0.9 + 0.1
			} else if d < 0.3 {
				0.1
			} else {
				0.01
			};
			noise_grass_b.sample_3d(coordsf, &[]) < density
		};
		let coords_to_no_grass = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 75.0;
			noise_no_grass.sample_3d(coordsf / scale, &[]) < 0.25
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
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
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest002 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest002 {
	fn generate_chunk_blocks(
		&self,
		coords_span: ChunkCoordsSpan,
		block_type_table: Arc<BlockTypeTable>,
	) -> ChunkBlocks {
		let noise_a = noise::OctavedNoise::new(5, vec![self.seed, 1]);
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let coordsf = coords.map(|x| x as f32);
			let scale = 100.0;
			let a = noise_a.sample_3d(coordsf / scale, &[]);
			a < 0.35
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest003 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest003 {
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
			let a = noise_a.sample_3d(coordsf_i_scaled, &[]);
			let b = noise_b.sample_3d(coordsf_i_scaled, &[]);
			let c = noise_c.sample_3d(coordsf_i_scaled, &[]);
			let coordsf_min = coords.map(|x| (x as f32 / scale).floor() * scale);
			let _coordsf_max = coords.map(|x| (x as f32 / scale).ceil() * scale);
			let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
			(coordsf - coordsf_min).distance(the) < radius
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest004 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest004 {
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
			let e = noise_e.sample_3d(coordsf_i_scaled, &[]);
			if e < 0.2 {
				return false;
			}
			let a = noise_a.sample_3d(coordsf_i_scaled, &[]);
			let b = noise_b.sample_3d(coordsf_i_scaled, &[]);
			let c = noise_c.sample_3d(coordsf_i_scaled, &[]);
			let d = noise_d.sample_3d(coordsf_i_scaled, &[]);
			let radius = d * (max_radius - min_radius) + min_radius;
			let coordsf_min = coords.map(|x| (x as f32 / scale).floor() * scale);
			let _coordsf_max = coords.map(|x| (x as f32 / scale).ceil() * scale);
			let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
			(coordsf - coordsf_min).distance(the) < radius
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
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

struct WorldGeneratorTest005 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest005 {
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
				let a = noise_a.sample_3d(coordsf_i_scaled, &[]);
				let b = noise_b.sample_3d(coordsf_i_scaled, &[]);
				let c = noise_c.sample_3d(coordsf_i_scaled, &[]);
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
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest006 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest006 {
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
				let a = noise_a.sample_3d(coordsf_i_scaled, &[]);
				let b = noise_b.sample_3d(coordsf_i_scaled, &[]);
				let c = noise_c.sample_3d(coordsf_i_scaled, &[]);
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
			let d = noise_d.sample_3d(coordsf / scale, &[]);
			let e = noise_e.sample_3d(coordsf / scale, &[]);
			let f = noise_f.sample_3d(coordsf / scale, &[]);
			use crate::coords::AngularDirection;
			let deformation = AngularDirection::from_angles(d * TAU, e * (TAU / 2.0)).to_vec3()
				* f * deformation_max_length;
			let deformed_coordsf = coordsf + deformation;
			coords_to_ground_uwu(deformed_coordsf)
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest007 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest007 {
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
				let a = noise_a.sample_3d(coordsf_i_scaled, &[]);
				let b = noise_b.sample_3d(coordsf_i_scaled, &[]);
				let c = noise_c.sample_3d(coordsf_i_scaled, &[]);
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
			let d = noise_d.sample_3d(coordsf / scale, &[]);
			let e = noise_e.sample_3d(coordsf / scale, &[]);
			let f = noise_f.sample_3d(coordsf / scale, &[]);
			use crate::coords::AngularDirection;
			let deformation = AngularDirection::from_angles(d * TAU, e * (TAU / 2.0)).to_vec3()
				* f * deformation_max_length;
			let deformed_coordsf = coordsf + deformation;
			coords_to_ground_uwu(deformed_coordsf)
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest008 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest008 {
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
				let a = noise_a.sample_3d(coordsf_i_scaled, &[]);
				let b = noise_b.sample_3d(coordsf_i_scaled, &[]);
				let c = noise_c.sample_3d(coordsf_i_scaled, &[]);
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
			let d = noise_d.sample_3d(coordsf / scale, &[]);
			let e = noise_e.sample_3d(coordsf / scale, &[]);
			let f = noise_f.sample_3d(coordsf / scale, &[]);
			use crate::coords::AngularDirection;
			let deformation = AngularDirection::from_angles(d * TAU, e * (TAU / 2.0)).to_vec3()
				* f * deformation_max_length;
			let deformed_coordsf = coordsf + deformation;
			coords_to_ground_uwu(deformed_coordsf)
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest009 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest009 {
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
				let a = noise_a.sample_3d(coordsf_i_scaled, &[]);
				let b = noise_b.sample_3d(coordsf_i_scaled, &[]);
				let c = noise_c.sample_3d(coordsf_i_scaled, &[]);
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
			let d = noise_d.sample_3d(coordsf / scale, &[]);
			let e = noise_e.sample_3d(coordsf / scale, &[]);
			let f = noise_f.sample_3d(coordsf / scale, &[]);
			use crate::coords::AngularDirection;
			let deformation = AngularDirection::from_angles(d * TAU, e * (TAU / 2.0)).to_vec3()
				* f * deformation_max_length;
			let deformed_coordsf = coordsf + deformation;
			coords_to_ground_uwu(deformed_coordsf)
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest010 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest010 {
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
				let a = noise_a.sample_3d(coordsf_i_scaled, &[]);
				let b = noise_b.sample_3d(coordsf_i_scaled, &[]);
				let c = noise_c.sample_3d(coordsf_i_scaled, &[]);
				let coordsf_min = coordsf.map(|x| (x / scale).floor() * scale);
				let _coordsf_max = coordsf.map(|x| (x / scale).ceil() * scale);
				let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
				coordsf_min + the
			};
			let coordsf_to_link_negativewards =
				|coordsf: cgmath::Point3<f32>, axis: NonOrientedAxis| -> bool {
					let coordsf_i_scaled = coordsf.map(|x| (x / scale).floor());
					let axis_channel = axis.index() as i32;
					let g = noise_g.sample_3d(coordsf_i_scaled, &[axis_channel]);
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
			let d = noise_d.sample_3d(coordsf / scale, &[]);
			let e = noise_e.sample_3d(coordsf / scale, &[]);
			let f = noise_f.sample_3d(coordsf / scale, &[]);
			use crate::coords::AngularDirection;
			let deformation = AngularDirection::from_angles(d * TAU, e * (TAU / 2.0)).to_vec3()
				* f * deformation_max_length;
			let deformed_coordsf = coordsf + deformation;
			coords_to_ground_uwu(deformed_coordsf)
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest011 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest011 {
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
					let a = noise_a.sample_3d(coordsf_i_scaled, &[]);
					let b = noise_b.sample_3d(coordsf_i_scaled, &[]);
					let c = noise_c.sample_3d(coordsf_i_scaled, &[]);
					let coordsf_min = coordsf.map(|x| (x / scale).floor() * scale);
					let _coordsf_max = coordsf.map(|x| (x / scale).ceil() * scale);
					let the = cgmath::vec3(a, b, c).map(|x| radius + x * (scale - 2.0 * radius));
					coordsf_min + the
				};
				let coordsf_to_link_negativewards =
					|coordsf: cgmath::Point3<f32>, axis: NonOrientedAxis| -> bool {
						let coordsf_i_scaled = coordsf.map(|x| (x / scale).floor());
						let axis_channel = axis.index() as i32;
						let g = noise_g.sample_3d(coordsf_i_scaled, &[axis_channel]);
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
			let d = noise_d.sample_3d(coordsf / scale, &[]);
			let e = noise_e.sample_3d(coordsf / scale, &[]);
			let f = noise_f.sample_3d(coordsf / scale, &[]);
			use crate::coords::AngularDirection;
			let mut deformation =
				AngularDirection::from_angles(d * TAU, e * (TAU / 2.0)).to_vec3() * f;
			deformation.x *= horizontal_deformation_max_length;
			deformation.y *= horizontal_deformation_max_length;
			deformation.z *= vertical_deformation_max_length;
			let deformed_coordsf = coordsf + deformation;
			coords_to_ground_uwu(deformed_coordsf)
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest012 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest012 {
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
			let e = noise_e.sample_3d(coordsf_i_scaled, &[]);
			if e < 0.2 {
				return false;
			}
			let a = noise_a.sample_3d(coordsf_i_scaled, &[]);
			let b = noise_b.sample_3d(coordsf_i_scaled, &[]);
			let c = noise_c.sample_3d(coordsf_i_scaled, &[]);
			let d = noise_d.sample_3d(coordsf_i_scaled, &[]);
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
			let f = noise_f.sample_3d(coordsf / scale, &[]);
			let g = noise_g.sample_3d(coordsf / scale, &[]);
			let h = noise_h.sample_3d(coordsf / scale, &[]);
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
			let d = noise_grass_a.sample_3d(coordsf / scale, &[]);
			let density = if d < 0.1 {
				d * 0.9 + 0.1
			} else if d < 0.3 {
				0.1
			} else {
				0.01
			};
			noise_grass_b.sample_3d(coordsf, &[]) < density
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
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
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest013 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest013 {
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
			let a = noise_a.sample_3d(coordsf / scale, &[]);
			let b = noise_b.sample_3d(coordsf / scale, &[]);
			let c = noise_c.sample_3d(coordsf / scale, &[]);
			let abc = cgmath::vec3(a - 0.5, b - 0.5, c - 0.5).normalize();
			let detail_scale = 85.0;
			let d = noise_d.sample_3d(coordsf / detail_scale, &[]);
			let e = noise_e.sample_3d(coordsf / detail_scale, &[]);
			let f = noise_f.sample_3d(coordsf / detail_scale, &[]);
			let def = cgmath::vec3(d - 0.5, e - 0.5, f - 0.5).normalize();
			let uwu = abc.dot(def);
			uwu < -0.4 && def.z < 0.0
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest014 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest014 {
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
			let a = noise_a.sample_3d(coordsf / scale, &[]);
			let b = noise_b.sample_3d(coordsf / scale, &[]);
			let c = noise_c.sample_3d(coordsf / scale, &[]);
			a.max(b).max(c) + a - c
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			coords_to_ground_uwu(coords) < coords_to_ground_uwu(coords + cgmath::vec3(0, 0, 1))
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest015 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest015 {
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
			let nosie_value_a = noise_a.sample_2d(coordsf_yx / scale_a, &[]);
			let nosie_value_b = noise_b.sample_2d(coordsf_yx / scale_b, &[]);
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
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest016 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest016 {
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
			let nosie_value_a = noise_a.sample_2d(coordsf_yx / scale_a, &[]);
			let nosie_value_b = noise_b.sample_2d(coordsf_yx / scale_b, &[]);
			let angle = f32::atan2(nosie_value_a - 0.5, nosie_value_b - 0.5);
			let distance = f32::hypot(nosie_value_a - 0.5, nosie_value_b - 0.5);
			let value = (f32::cos(angle * 3.0) * 0.5 + 0.5) * distance.powi(4);
			value / 0.001
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let void = coords_to_void(coords);
			(coords.z as f32).abs() < 6.0 / (1.0 / (1.0 - void))
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest017 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest017 {
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
			let nosie_value_a = noise_a.sample_2d(coordsf_yx / scale_a, &[]);
			let nosie_value_b = noise_b.sample_2d(coordsf_yx / scale_b, &[]);
			let angle = f32::atan2(nosie_value_a - 0.5, nosie_value_b - 0.5);
			let distance = f32::hypot(nosie_value_a - 0.5, nosie_value_b - 0.5);
			let value = (f32::cos(angle * 3.0) * 0.5 + 0.5) * distance.powi(4);
			value / 0.001
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let void = coords_to_void(coords);
			(coords.z as f32).abs() < (1.0 / void).log2() / (1.0 / (1.0 - void))
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest018 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest018 {
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
			let nosie_value_a = noise_a.sample_3d(coordsf / scale_a, &[]);
			let nosie_value_b = noise_b.sample_3d(coordsf / scale_b, &[]);
			let angle = f32::atan2(nosie_value_a - 0.5, nosie_value_b - 0.5);
			let distance = f32::hypot(nosie_value_a - 0.5, nosie_value_b - 0.5);
			let value = (f32::cos(angle * 3.0) * 0.5 + 0.5) * distance.powi(4);
			value / 0.001
		};
		let coords_to_ground = |coords: BlockCoords| -> bool {
			let void = coords_to_void(coords);
			(coords.z as f32).abs() < 20.0 / (1.0 / (1.0 - void))
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest019 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest019 {
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
			let scale_a = 240.0 * noise_m.sample_3d(coordsf / scale_m, &[]);
			let nosie_value_a = noise_a.sample_3d(coordsf / scale_a, &[]);
			let angle = nosie_value_a * TAU;
			let scale_d = 100.0;
			let distance = 80.0 * noise_b.sample_3d(coordsf / scale_d, &[]);
			let v = coordsf.z + f32::cos(angle) * distance;
			//let ry = ry + f32::sin(angle) * distance;
			v < 0.5
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest020 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest020 {
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
				let noise_value_x = noise_a.sample_3d(coordsf / scale_a, &[]);
				let noise_value_y = noise_b.sample_3d(coordsf / scale_a, &[]);
				let noise_value_z = noise_c.sample_3d(coordsf / scale_a, &[]);
				let vv = cgmath::vec3(noise_value_x, noise_value_y, noise_value_z);
				let vv = (vv - cgmath::vec3(0.5, 0.5, 0.5)).normalize();
				let d = noise_d.sample_3d(coordsf / scale_a, &[]);
				let vv = vv * d * 20.0;
				coordsf += vv;
				if coordsf.z < 0.0 {
					return true;
				}
			}
			false
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest021 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest021 {
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
				.map(|i| (i as usize, noise_biomes.sample_2d(coordsf_xy / scale, &[i])))
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
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest022 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest022 {
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
				.map(|i| (i as usize, noise_biomes.sample_3d(coordsf / scale, &[i])))
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
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest023 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest023 {
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
			let height = 20.0 * noise_a.sample_2d(coordsf_xy / scale, &[]);
			coordsf.z < height
		};
		let mut chunk_blocks = ChunkBlocks::new(coords_span);
		for coords in chunk_blocks.coords_span.iter_coords() {
			let ground = coords_to_ground(coords);
			*chunk_blocks.get_mut(coords).unwrap() = if ground {
				let ground_above = coords_to_ground(coords + cgmath::vec3(0, 0, 1));
				if ground_above {
					block_type_table.ground_id()
				} else {
					block_type_table.kinda_grass_id()
				}
			} else {
				block_type_table.air_id()
			};
		}
		chunk_blocks
	}
}

struct WorldGeneratorTest024 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest024 {
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
			let height = 20.0 * noise_terrain.sample_2d(coordsf_xy / scale, &[]);
			coordsf.z < height
		};
		use crate::BlockTypeId;
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
			let v = noise_cell_data.sample(&[], &[cell_coords.x, cell_coords.y, cell_coords.z, 1]);
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
			if let Some(block_type) = chunk_blocks.get_mut(block_coords) {
				*block_type = block_type_to_place;
			}
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
		let mut chunk_blocks = ChunkBlocks::new(coords_span);

		// Generate terrain in the chunk.
		for coords in chunk_blocks.coords_span.iter_coords() {
			*chunk_blocks.get_mut(coords).unwrap() = coords_to_terrain(coords);
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

struct WorldGeneratorTest025 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest025 {
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
			let height = 20.0 * noise_terrain.sample_2d(coordsf_xy / scale, &[]);
			coordsf.z < height
		};
		use crate::BlockTypeId;
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
			let v = noise_cell_data.sample(&[], &[cell_coords.x, cell_coords.y, cell_coords.z, 1]);
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
			if let Some(block_type) = chunk_blocks.get_mut(block_coords) {
				*block_type = block_type_to_place;
			}
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
				if coords
					.map(|x| x as f32)
					.distance(origin_block_coords.map(|x| x as f32))
					< ball_radius as f32
				{
					structure_place_block(coords, block_type_table.ground_id(), chunk_blocks);
				}
			}
		};

		// Now we generate the block data in the chunk.
		let mut chunk_blocks = ChunkBlocks::new(coords_span);

		// Generate terrain in the chunk.
		for coords in chunk_blocks.coords_span.iter_coords() {
			*chunk_blocks.get_mut(coords).unwrap() = coords_to_terrain(coords);
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

struct WorldGeneratorTest026 {
	pub seed: i32,
}

impl WorldGenerator for WorldGeneratorTest026 {
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
			let height = 20.0 * noise_terrain.sample_2d(coordsf_xy / scale, &[]);
			coordsf.z < height
		};
		use crate::BlockTypeId;
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
			let v = noise_cell_data.sample(&[], &[cell_coords.x, cell_coords.y, cell_coords.z, 1]);
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
			if let Some(block_type) = chunk_blocks.get_mut(block_coords) {
				*block_type = block_type_to_place;
			}
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
				let ground_here = !block_type_table
					.get(structure_look_terrain_block(placing_head))
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
			let noise_value_a =
				noise_structure.sample(&[], &[placing_head.x, placing_head.y, placing_head.z, 1]);
			let height =
				((noise_value_a * 0.5 + 0.5) * structure_max_blocky_radius.min(11) as f32) as i32;
			placing_head.z += height;
			let noise_value_b =
				noise_structure.sample(&[], &[placing_head.x, placing_head.y, placing_head.z, 2]);
			let ball_radius = (noise_value_b * 0.2 + 0.8) * 3.5;
			for coords in
				crate::coords::iter_3d_cube_center_radius(placing_head, ball_radius.ceil() as i32)
			{
				if coords
					.map(|x| x as f32)
					.distance(placing_head.map(|x| x as f32))
					< ball_radius
				{
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
		let mut chunk_blocks = ChunkBlocks::new(coords_span);

		// Generate terrain in the chunk.
		for coords in chunk_blocks.coords_span.iter_coords() {
			*chunk_blocks.get_mut(coords).unwrap() = coords_to_terrain(coords);
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
