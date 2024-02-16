use crate::{
	chunks::{BlockTypeTable, ChunkGrid},
	coords::AlignedBox,
};

use std::{sync::Arc, time::Duration};

/// Represents an `AlignedBox`-shaped object that has physics or something like that.
pub(crate) struct AlignedPhysBox {
	pub(crate) aligned_box: AlignedBox,
	pub(crate) motion: cgmath::Vector3<f32>,
	/// Gravity's acceleration of this box is influenced by this parameter.
	/// It may not be exactly analog to weight but it's not too far.
	pub(crate) gravity_factor: f32,
}

pub(crate) fn apply_on_physics_step(
	player_phys: &mut AlignedPhysBox,
	chunk_grid: &ChunkGrid,
	block_type_table: &Arc<BlockTypeTable>,
	dt: Duration,
) {
	// TODO: Work out something better here,
	// although it is not very important at the moment.
	let player_bottom = player_phys.aligned_box.pos
		- cgmath::Vector3::<f32>::from((0.0, 0.0, player_phys.aligned_box.dims.z / 2.0));
	let player_bottom_below = player_phys.aligned_box.pos
		- cgmath::Vector3::<f32>::from((0.0, 0.0, player_phys.aligned_box.dims.z / 2.0 + 0.01));
	let player_bottom_block_coords = player_bottom.map(|x| x.round() as i32);
	let player_bottom_block_coords_below = player_bottom_below.map(|x| x.round() as i32);
	let player_bottom_block_opt = chunk_grid.get_block(player_bottom_block_coords);
	let player_bottom_block_opt_below = chunk_grid.get_block(player_bottom_block_coords_below);
	let is_on_ground = if player_phys.motion.z <= 0.0 {
		if let Some(block_id) = player_bottom_block_opt_below {
			if block_type_table.get(block_id).unwrap().is_opaque() {
				// The player is on the ground, so we make sure we are not overlapping it.
				player_phys.motion.z = 0.0;
				player_phys.aligned_box.pos.z = player_bottom_block_coords_below.z as f32
					+ 0.5 + player_phys.aligned_box.dims.z / 2.0;
				true
			} else {
				false
			}
		} else {
			false
		}
	} else {
		false
	};
	let is_in_ground = if player_phys.motion.z <= 0.0 {
		if let Some(block_id) = player_bottom_block_opt {
			if block_type_table.get(block_id).unwrap().is_opaque() {
				// The player is inside the ground, so we uuh.. do something?
				player_phys.motion.z = 0.0;
				player_phys.aligned_box.pos.z =
					player_bottom_block_coords.z as f32 + 0.5 + player_phys.aligned_box.dims.z / 2.0;
				true
			} else {
				false
			}
		} else {
			false
		}
	} else {
		false
	};
	player_phys.aligned_box.pos += player_phys.motion;
	if !is_on_ground {
		player_phys.motion.z -= player_phys.gravity_factor * 0.3 * dt.as_secs_f32();
	}
	if is_in_ground {
		player_phys.aligned_box.pos.z += 0.01;
	}
}
