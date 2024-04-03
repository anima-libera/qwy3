pub(crate) mod block;
pub(crate) mod block_shadow;
pub(crate) mod part_textured;
pub(crate) mod part_textured_shadow;
pub(crate) mod simple_line;
pub(crate) mod simple_line_2d;
pub(crate) mod simple_texture_2d;
pub(crate) mod skybox;

/// Vector in 3D.
#[derive(Copy, Clone, Debug)]
/// Certified Plain Old Data (so it can be sent to the GPU as a uniform).
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Vector3Pod {
	pub(crate) values: [f32; 3],
}

/// Vector in 2D.
#[derive(Copy, Clone, Debug)]
/// Certified Plain Old Data (so it can be sent to the GPU as a uniform).
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Vector2Pod {
	pub(crate) values: [f32; 2],
}
