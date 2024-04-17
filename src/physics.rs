use serde::{Deserialize, Serialize};

use crate::{
	block_types::BlockTypeTable,
	chunks::ChunkGrid,
	coords::{
		AlignedBox, AxisOrientation, BlockCoords, NonOrientedAxis, OrientedAxis, OrientedFaceCoords,
	},
};

use std::{cmp::Ordering, sync::Arc, time::Duration};

/// Represents an `AlignedBox`-shaped object that has physics or something like that.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct AlignedPhysBox {
	aligned_box: AlignedBox,
	motion: cgmath::Vector3<f32>,
	on_faces: Vec<OrientedFaceCoords>,
	is_overlapping_blocks: bool,
}

impl AlignedPhysBox {
	pub(crate) fn new(aligned_box: AlignedBox, motion: cgmath::Vector3<f32>) -> AlignedPhysBox {
		AlignedPhysBox {
			aligned_box,
			motion,
			on_faces: vec![],
			is_overlapping_blocks: false,
		}
	}

	pub(crate) fn aligned_box(&self) -> &AlignedBox {
		&self.aligned_box
	}
	pub(crate) fn on_ground_and_not_overlapping(&self) -> bool {
		self.on_faces.iter().any(|face| face.direction_to_exterior == OrientedAxis::Z_PLUS)
			&& !self.is_overlapping_blocks
	}

	pub(crate) fn impose_position(&mut self, position: cgmath::Point3<f32>) {
		self.aligned_box.pos = position;
		self.on_faces.clear();
	}
	pub(crate) fn impose_displacement(&mut self, displacement: cgmath::Vector3<f32>) {
		self.aligned_box.pos += displacement;
		self.on_faces.clear();
	}

	pub(crate) fn apply_one_physics_step(
		&mut self,
		walking_vector: cgmath::Vector3<f32>,
		chunk_grid: &ChunkGrid,
		block_type_table: &Arc<BlockTypeTable>,
		dt: Duration,
		bubble_up: bool,
	) {
		let is_opaque = |coords: BlockCoords| -> bool {
			chunk_grid
				.get_block(coords)
				.is_some_and(|block| block_type_table.get(block.type_id).unwrap().is_opaque())
		};

		// Is the hitbox inside matter?
		let overlapping_blocks = self
			.aligned_box
			.overlapping_block_coords_span()
			.iter()
			.filter(|&coords| is_opaque(coords));
		let top_z_overlapping_blocks = overlapping_blocks.map(|coords| coords.z).max();
		self.is_overlapping_blocks = top_z_overlapping_blocks.is_some();

		// Bubble up through solid matter if the hitbox happens to already be inside matter.
		if bubble_up && self.is_overlapping_blocks {
			let target_z_for_bottom_side = top_z_overlapping_blocks.unwrap() as f32 + 0.5;
			let target_z = target_z_for_bottom_side + self.aligned_box.dims.z / 2.0;
			self.aligned_box.pos.z =
				(self.aligned_box.pos.z + 1000.0 * dt.as_secs_f32()).min(target_z);
			self.motion = cgmath::vec3(0.0, 0.0, 0.0);
			return;
		}

		let displacement = (self.motion * 144.0 + walking_vector) * dt.as_secs_f32();
		self.motion.z -= 0.35 * dt.as_secs_f32();
		self.motion /= 1.0 + 0.0015 * 144.0 * dt.as_secs_f32();

		// We handle the motion axis by axis.
		// For each axis, we apply the motion then deal with collisions if any.
		// The idea of proceeding that way was inspired from Minecraft's algorithm described at
		// https://www.mcpk.wiki/wiki/Collisions
		for axis in [NonOrientedAxis::Z, NonOrientedAxis::X, NonOrientedAxis::Y] {
			let axis_i = axis.index();

			// The motion along the considered axis goes in either of the two possible orientations
			// of the axis (positiveward or negativeward), here we get that orientation for the
			// currently considered axis.
			let position_comparison = displacement[axis_i].partial_cmp(&0.0).unwrap();
			let orientation = match position_comparison {
				Ordering::Equal => {
					// There is no motion along the considered axis,
					// so nothing to do for the current axis.
					continue;
				},
				Ordering::Greater => AxisOrientation::Positivewards,
				Ordering::Less => AxisOrientation::Negativewards,
			};
			let sign = orientation.sign() as f32;
			let oriented_axis = OrientedAxis { axis, orientation };

			// Apply the motion along the considered axis.
			self.aligned_box.pos[axis_i] += displacement[axis_i];

			// The hitbox overlaps with some blocks (a rectangukar 3D span of blocks) (solid or not).
			// We get that block span to have a list of block to check for collisions, as the hitbox
			// can only collide with blocks that overlap with it.
			let next_block_span = self.aligned_box.overlapping_block_coords_span();
			// We only look at the blocks at one side of that span, the side the hitbox is moving
			// towards.
			let blocks_on_side = next_block_span.side(oriented_axis);
			// If any of these blocks is solid, the it means that the hitbox is moving towards a
			// solid block that overlaps with it, thus there is a collision.
			let collision = blocks_on_side.iter().any(is_opaque);
			if collision {
				// There is a collision to be solved.

				// Stop the motion, at least the component of which resulted in the collision.
				if self.motion[axis_i] * sign > 0.0 {
					self.motion[axis_i] = 0.0;
				}

				// Also, move the hitbox out of the colliding block, the moving happens along
				// the currently considered axis only.

				// First we get the coordinate (along the considered axis) of the colliding side
				// of the hitbox.
				let hitbox_side_coord =
					self.aligned_box.pos[axis_i] + (self.aligned_box.dims[axis_i] / 2.0) * sign;
				// We apply rounding to move this side to the block center (for now) and also
				// include a very small margin to influence some roundings (hacky fix >.<).
				let hitbox_side_coord_rounded_with_margin =
					(hitbox_side_coord + 0.001 * sign).round() - 0.001 * sign;
				// Move the side to the colliding block side instead of its center.
				// Note: Block centers are at integer coordinates (thus the rounding above)
				// and moving 0.5 along an axis brings the point to a side of a block.
				let hitbox_side_coord_solved = hitbox_side_coord_rounded_with_margin - 0.5 * sign;
				// Move the hitbox's position to make its side be at the coordinate we just got.
				let pos_coord_solved =
					hitbox_side_coord_solved - (self.aligned_box.dims[axis_i] / 2.0) * sign;
				self.aligned_box.pos[axis_i] = pos_coord_solved;
			}
		}

		// Check for being on some block faces or not.
		self.on_faces.clear();
		for direction in OrientedAxis::all_the_six_possible_directions() {
			let mut moved_aligned_box = self.aligned_box.clone();
			moved_aligned_box.pos += direction.delta().map(|x| x as f32) * 0.005;
			let block_span_on_side = moved_aligned_box.overlapping_block_coords_span().side(direction);
			for interior_coords in block_span_on_side.iter().filter(|&coords| is_opaque(coords)) {
				let direction_to_exterior = OrientedAxis::from_delta(direction.delta() * -1).unwrap();
				let face = OrientedFaceCoords { interior_coords, direction_to_exterior };
				let face_is_exposed = !is_opaque(face.exterior_coords());
				if face_is_exposed {
					self.on_faces.push(OrientedFaceCoords { interior_coords, direction_to_exterior })
				}
			}
		}

		// If perssed on face, then apply more friction.
		for face in self.on_faces.iter() {
			if self.motion[face.direction_to_exterior.axis.index()]
				* (face.direction_to_exterior.orientation.sign() as f32)
				< 0.0
			{
				self.motion /= 1.0 + 0.05 * 144.0 * dt.as_secs_f32();
			}
		}
	}
}

/// Manages the paleyr's ability to jump.
/// This handles permissive jumping (allows jumping even when it is a little bit too late
/// and the player is already falling off an edge).
pub(crate) struct PlayerJumpManager {
	last_time_on_ground_if_not_jumped: Option<std::time::Instant>,
}

impl PlayerJumpManager {
	pub(crate) fn new() -> PlayerJumpManager {
		PlayerJumpManager { last_time_on_ground_if_not_jumped: None }
	}

	/// Must be called at every frame.
	pub(crate) fn manage(&mut self, phys_box: &AlignedPhysBox) {
		if phys_box.on_ground_and_not_overlapping() {
			self.last_time_on_ground_if_not_jumped = Some(std::time::Instant::now());
		}
	}

	pub(crate) fn jump(&mut self, phys_box: &mut AlignedPhysBox) {
		let can_still_jump = || {
			self
				.last_time_on_ground_if_not_jumped
				.is_some_and(|time| time.elapsed() < std::time::Duration::from_secs_f32(0.15))
		};
		if phys_box.on_ground_and_not_overlapping() || can_still_jump() {
			phys_box.motion.z = 0.1;
			self.last_time_on_ground_if_not_jumped = None;
		}
	}
}
