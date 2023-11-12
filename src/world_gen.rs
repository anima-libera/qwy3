use std::{f32::consts::TAU, sync::Arc};

use cgmath::{InnerSpace, MetricSpace};

use crate::coords::NonOrientedAxis;
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

pub enum WhichWorldGenerator {
	Default,
	Flat,
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
}
impl WhichWorldGenerator {
	pub fn from_name(name: &str) -> Option<WhichWorldGenerator> {
		match name {
			"default" => Some(WhichWorldGenerator::Default),
			"flat" => Some(WhichWorldGenerator::Flat),
			"test001" => Some(WhichWorldGenerator::Test001),
			"test002" => Some(WhichWorldGenerator::Test002),
			"test003" => Some(WhichWorldGenerator::Test003),
			"test004" => Some(WhichWorldGenerator::Test004),
			"test005" => Some(WhichWorldGenerator::Test005),
			"test006" => Some(WhichWorldGenerator::Test006),
			"test007" => Some(WhichWorldGenerator::Test007),
			"test008" => Some(WhichWorldGenerator::Test008),
			"test009" => Some(WhichWorldGenerator::Test009),
			"test010" => Some(WhichWorldGenerator::Test010),
			"test011" => Some(WhichWorldGenerator::Test011),
			"test012" => Some(WhichWorldGenerator::Test012),
			"test013" => Some(WhichWorldGenerator::Test013),
			"test014" => Some(WhichWorldGenerator::Test014),
			"test015" => Some(WhichWorldGenerator::Test015),
			"test016" => Some(WhichWorldGenerator::Test016),
			"test017" => Some(WhichWorldGenerator::Test017),
			"test018" => Some(WhichWorldGenerator::Test018),
			"test019" => Some(WhichWorldGenerator::Test019),
			"test020" => Some(WhichWorldGenerator::Test020),
			_ => None,
		}
	}

	pub fn get_the_actual_generator(self, seed: i32) -> Arc<dyn WorldGenerator + Sync + Send> {
		match self {
			WhichWorldGenerator::Default => Arc::new(DefaultWorldGenerator { seed }),
			WhichWorldGenerator::Flat => Arc::new(FlatWorldGenerator {}),
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
			// Test chunk generation.
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
