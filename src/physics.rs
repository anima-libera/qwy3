use crate::{
	block_types::BlockTypeTable,
	chunks::ChunkGrid,
	coords::{AlignedBox, AxisOrientation, BlockCoords, NonOrientedAxis, OrientedAxis},
};

use std::{sync::Arc, time::Duration};

/// Represents an `AlignedBox`-shaped object that has physics or something like that.
#[derive(Clone)]
pub(crate) struct AlignedPhysBox {
	aligned_box: AlignedBox,
	new_box_pos: cgmath::Point3<f32>,
	motion: cgmath::Vector3<f32>,
	/// Gravity's acceleration of this box is influenced by this parameter.
	/// It may not be exactly analog to weight but it's not too far.
	gravity_factor: f32,
	on_ground: bool,
	last_time_on_ground_if_not_jumped: Option<std::time::Instant>,
}

impl AlignedPhysBox {
	pub(crate) fn new(aligned_box: AlignedBox) -> AlignedPhysBox {
		let new_box_pos = aligned_box.pos;
		let motion = cgmath::vec3(0.0, 0.0, 0.0);
		let gravity_factor = 1.0;
		let on_ground = false;
		let last_time_on_ground_if_not_jumped = None;
		AlignedPhysBox {
			aligned_box,
			new_box_pos,
			motion,
			gravity_factor,
			on_ground,
			last_time_on_ground_if_not_jumped,
		}
	}

	pub(crate) fn aligned_box(&self) -> &AlignedBox {
		&self.aligned_box
	}

	pub(crate) fn jump(&mut self) {
		let can_still_jump = || {
			self
				.last_time_on_ground_if_not_jumped
				.is_some_and(|time| time.elapsed() < std::time::Duration::from_secs_f32(0.15))
		};
		if self.on_ground || can_still_jump() {
			self.motion.z = 0.1;
			self.last_time_on_ground_if_not_jumped = None;
		}
	}

	pub(crate) fn walk(&mut self, walking_vector: cgmath::Vector3<f32>, impose_new_pos: bool) {
		self.new_box_pos += walking_vector;
		if impose_new_pos {
			self.aligned_box.pos = self.new_box_pos;
		}
	}

	pub(crate) fn impose_new_pos(&mut self, new_pos: cgmath::Point3<f32>) {
		self.aligned_box.pos = new_pos;
		self.new_box_pos = new_pos;
		self.on_ground = false;
	}

	pub(crate) fn apply_on_physics_step(
		&mut self,
		chunk_grid: &ChunkGrid,
		block_type_table: &Arc<BlockTypeTable>,
		dt: Duration,
	) {
		let is_opaque = |coords: BlockCoords| -> bool {
			chunk_grid
				.get_block(coords)
				.is_some_and(|block| block_type_table.get(block.type_id).unwrap().is_opaque())
		};

		// Bubble up through solid matter if the hit box happens to already be inside matter.
		let bottom_block = (self.aligned_box.pos
			+ cgmath::vec3(0.0, 0.0, -self.aligned_box.dims.z / 2.0 + 0.3))
		.map(|x| x.round() as i32);
		if is_opaque(bottom_block) {
			self.aligned_box.pos.z += 100.0 * dt.as_secs_f32();
			self.new_box_pos = self.aligned_box.pos;
			self.motion = cgmath::vec3(0.0, 0.0, 0.0);
			self.on_ground = true;
			return;
		}

		self.new_box_pos += self.motion * 144.0 * dt.as_secs_f32();
		self.motion.z -= self.gravity_factor * 0.35 * dt.as_secs_f32();
		// TODO: There has to be a missing `dt` here, but what would be the correct expression?
		self.motion *= 0.998;

		// Inspired from Minecraft's algorithm described at https://www.mcpk.wiki/wiki/Collisions

		for axis in [NonOrientedAxis::Z, NonOrientedAxis::X, NonOrientedAxis::Y] {
			let mut next_aligned_box = self.aligned_box.clone();
			next_aligned_box.pos[axis.index()] = self.new_box_pos[axis.index()];
			let next_block_span = next_aligned_box.overlapping_block_coords_span();
			for orientation in AxisOrientation::iter_over_the_two_possible_orientations() {
				let oriented_axis = OrientedAxis { axis, orientation };
				let sign = orientation.sign() as f32;
				let axis_i = axis.index();
				if (next_aligned_box.pos[axis_i] - self.aligned_box.pos[axis_i]) * sign <= 0.0 {
					continue;
				}
				let blocks_on_side = next_block_span.side(oriented_axis);
				for coords in blocks_on_side.iter() {
					if is_opaque(coords) {
						// There is a collision to be solved.
						if self.motion[axis_i] * sign > 0.0 {
							self.motion[axis_i] = 0.0;
						}
						let mut player_side = next_aligned_box.pos;
						player_side[axis_i] += (next_aligned_box.dims[axis_i] / 2.0 + 0.001) * sign;
						player_side = player_side.map(|x| x.round() - 0.001 * sign);
						let mut new_pos = next_aligned_box.pos;
						new_pos[axis_i] =
							player_side[axis_i] - (0.5 + next_aligned_box.dims[axis_i] / 2.0) * sign;
						next_aligned_box.pos = new_pos;
						break;
					}
				}
			}

			self.aligned_box = next_aligned_box;
		}
		self.new_box_pos = self.aligned_box.pos;

		// Check for being on some ground or not.
		self.on_ground = false;
		let mut moved_aligned_box = self.aligned_box.clone();
		moved_aligned_box.pos.z -= 0.1;
		let block_span_below = moved_aligned_box.overlapping_block_coords_span().side(OrientedAxis {
			axis: NonOrientedAxis::Z,
			orientation: AxisOrientation::Negativewards,
		});
		self.on_ground = block_span_below.iter().any(is_opaque);
		if self.on_ground {
			self.last_time_on_ground_if_not_jumped = Some(std::time::Instant::now());
		}
	}
}
