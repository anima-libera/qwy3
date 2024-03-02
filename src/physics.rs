use cgmath::MetricSpace;

use crate::{
	chunks::{BlockTypeTable, ChunkGrid},
	coords::AlignedBox,
	AxisOrientation, BlockCoords, NonOrientedAxis, OrientedAxis,
};

use std::{sync::Arc, time::Duration};

/// Represents an `AlignedBox`-shaped object that has physics or something like that.
#[derive(Clone)]
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
	let is_opaque = |coords: BlockCoords| -> bool {
		chunk_grid
			.get_block(coords)
			.is_some_and(|block_id| block_type_table.get(block_id).unwrap().is_opaque())
	};

	// Bubble up through solid matter if the hit box happens to already be inside matter.
	let bottom_block = (player_phys.aligned_box.pos
		+ cgmath::vec3(0.0, 0.0, -player_phys.aligned_box.dims.z / 2.0 + 0.3))
	.map(|x| x.round() as i32);
	if is_opaque(bottom_block) {
		player_phys.aligned_box.pos.z += 100.0 * dt.as_secs_f32();
		player_phys.motion = cgmath::vec3(0.0, 0.0, 0.0);
		return;
	}

	// Consider the given directions and attempt to solve collisions
	// with solid blocks in these directions.
	let one_pass = |mut player_phys: AlignedPhysBox,
	                directions: &[OrientedAxis]|
	 -> AlignedPhysBox {
		// Here is a rectangular shape in block coords that contains all the blocks that
		// the player box overlaps.
		// Note the negative margins on horizontal axes, it happens to fix some issues.
		let block_span = player_phys
			.aligned_box
			.clone()
			.added_margins(cgmath::vec3(-0.01, -0.01, 0.0))
			.overlapping_block_coords_span();

		'outer: for coords in block_span.iter() {
			if is_opaque(coords) {
				for direction in directions {
					let axis = direction.axis.index();
					let sign = direction.orientation.sign() as f32;
					let is_on_side = !block_span.contains(coords + direction.delta());
					let is_a_face = !is_opaque(coords - direction.delta());
					if is_on_side && is_a_face {
						// There is a collision to be solved.
						if player_phys.motion[axis] * sign > 0.0 {
							player_phys.motion[axis] = 0.0;
						}
						let mut player_side = player_phys.aligned_box.pos;
						player_side[axis] += (player_phys.aligned_box.dims[axis] / 2.0 + 0.0001) * sign;
						player_side = player_side.map(|x| x.round());
						let mut new_pos = player_phys.aligned_box.pos;
						new_pos[axis] =
							player_side[axis] - (0.5 + player_phys.aligned_box.dims[axis] / 2.0) * sign;
						if new_pos.distance(player_phys.aligned_box.pos) < 0.2 {
							player_phys.aligned_box.pos = new_pos;
							continue 'outer;
						}
					}
				}
			}
		}
		player_phys
	};

	// Solving vertical collisions first.
	*player_phys = one_pass(
		player_phys.clone(),
		&[
			OrientedAxis {
				axis: NonOrientedAxis::Z,
				orientation: AxisOrientation::Negativewards,
			},
			OrientedAxis {
				axis: NonOrientedAxis::Z,
				orientation: AxisOrientation::Positivewards,
			},
		],
	);

	// Try to solve collisions on both horizontal axes separately.
	// Then we apply the smallest resolution because the smallest one is surely the best one
	// (the lengtier displacement is likely the more unnatural resolution).
	let player_phys_x = one_pass(
		player_phys.clone(),
		&[
			OrientedAxis {
				axis: NonOrientedAxis::X,
				orientation: AxisOrientation::Negativewards,
			},
			OrientedAxis {
				axis: NonOrientedAxis::X,
				orientation: AxisOrientation::Positivewards,
			},
		],
	);
	let player_phys_y = one_pass(
		player_phys.clone(),
		&[
			OrientedAxis {
				axis: NonOrientedAxis::Y,
				orientation: AxisOrientation::Negativewards,
			},
			OrientedAxis {
				axis: NonOrientedAxis::Y,
				orientation: AxisOrientation::Positivewards,
			},
		],
	);
	if player_phys.aligned_box.pos.distance(player_phys_x.aligned_box.pos)
		> player_phys.aligned_box.pos.distance(player_phys_y.aligned_box.pos)
	{
		*player_phys = player_phys_y;
	} else {
		*player_phys = player_phys_x;
	}

	// It so happens that some cases require additional resolutions, and these seem to work fine.
	*player_phys = one_pass(
		player_phys.clone(),
		&[
			OrientedAxis {
				axis: NonOrientedAxis::X,
				orientation: AxisOrientation::Negativewards,
			},
			OrientedAxis {
				axis: NonOrientedAxis::X,
				orientation: AxisOrientation::Positivewards,
			},
			OrientedAxis {
				axis: NonOrientedAxis::Y,
				orientation: AxisOrientation::Negativewards,
			},
			OrientedAxis {
				axis: NonOrientedAxis::Y,
				orientation: AxisOrientation::Positivewards,
			},
		],
	);
	*player_phys = one_pass(
		player_phys.clone(),
		&[
			OrientedAxis {
				axis: NonOrientedAxis::Y,
				orientation: AxisOrientation::Positivewards,
			},
			OrientedAxis {
				axis: NonOrientedAxis::Y,
				orientation: AxisOrientation::Negativewards,
			},
			OrientedAxis {
				axis: NonOrientedAxis::X,
				orientation: AxisOrientation::Positivewards,
			},
			OrientedAxis {
				axis: NonOrientedAxis::X,
				orientation: AxisOrientation::Negativewards,
			},
		],
	);

	player_phys.aligned_box.pos += player_phys.motion;
	player_phys.motion.z -= player_phys.gravity_factor * 0.3 * dt.as_secs_f32();
	player_phys.motion *= 0.998;
}
