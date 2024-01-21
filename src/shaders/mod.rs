pub mod block;
pub mod block_shadow;
pub mod simple_line;
pub mod simple_line_2d;
pub mod simple_texture_2d;
pub mod skybox;

/// Vector in 3D.
#[derive(Copy, Clone, Debug)]
/// Certified Plain Old Data (so it can be sent to the GPU as a uniform).
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vector3Pod {
	pub values: [f32; 3],
}
