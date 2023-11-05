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
}
impl WhichWorldGenerator {
	pub fn from_name(name: &str) -> Option<WhichWorldGenerator> {
		match name {
			"default" => Some(WhichWorldGenerator::Default),
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
			_ => None,
		}
	}

	pub fn get_the_actual_generator(self, seed: i32) -> Arc<dyn WorldGenerator + Sync + Send> {
		match self {
			WhichWorldGenerator::Default => Arc::new(DefaultWorldGenerator { seed }),
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

pub struct WorldGeneratorTest001 {
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

pub struct WorldGeneratorTest002 {
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

pub struct WorldGeneratorTest003 {
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

pub struct WorldGeneratorTest004 {
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

pub struct WorldGeneratorTest005 {
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

pub struct WorldGeneratorTest006 {
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

pub struct WorldGeneratorTest007 {
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

pub struct WorldGeneratorTest008 {
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

pub struct WorldGeneratorTest009 {
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

pub struct WorldGeneratorTest010 {
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

pub struct WorldGeneratorTest011 {
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

pub struct WorldGeneratorTest012 {
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
