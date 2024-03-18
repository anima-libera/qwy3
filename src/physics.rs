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
}

impl AlignedPhysBox {
	pub(crate) fn new(aligned_box: AlignedBox) -> AlignedPhysBox {
		let new_box_pos = aligned_box.pos;
		let motion = cgmath::vec3(0.0, 0.0, 0.0);
		let gravity_factor = 1.0;
		AlignedPhysBox { aligned_box, new_box_pos, motion, gravity_factor }
	}

	pub(crate) fn aligned_box(&self) -> &AlignedBox {
		&self.aligned_box
	}

	pub(crate) fn jump(&mut self) {
		self.motion.z = 0.1;
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
			return;
		}

		self.new_box_pos += self.motion;
		self.motion.z -= self.gravity_factor * 0.3 * dt.as_secs_f32();
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
				let axis = axis.index();
				if (next_aligned_box.pos[axis] - self.aligned_box.pos[axis]) * sign <= 0.0 {
					continue;
				}
				let blocks_on_side = next_block_span.side(oriented_axis);
				for coords in blocks_on_side.iter() {
					if is_opaque(coords) {
						// There is a collision to be solved.
						if self.motion[axis] * sign > 0.0 {
							self.motion[axis] = 0.0;
						}
						let mut player_side = next_aligned_box.pos;
						player_side[axis] += (next_aligned_box.dims[axis] / 2.0 + 0.0001) * sign;
						player_side = player_side.map(|x| x.round() - 0.0001 * sign);
						let mut new_pos = next_aligned_box.pos;
						new_pos[axis] =
							player_side[axis] - (0.5 + next_aligned_box.dims[axis] / 2.0) * sign;
						next_aligned_box.pos = new_pos;
						break;
					}
				}
			}

			self.aligned_box = next_aligned_box;
		}
		self.new_box_pos = self.aligned_box.pos;
	}
}
