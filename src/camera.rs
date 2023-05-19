/// Camera settings with perspective.
///
/// It doesn't contain the camera position and target or vector,
/// it only contains all the other setting values used in the
/// computation of the view projection matrix.
pub struct CameraPerspectiveSettings {
	pub up_direction: cgmath::Vector3<f32>,
	/// Width / height.
	pub aspect_ratio: f32,
	/// Angle (unsigned, in radians) of view on the vertical axis, "fovy".
	pub field_of_view_y: f32,
	pub near_plane: f32,
	pub far_plane: f32,
}

/// Width / height.
pub type AspectRatio = f32;
pub fn aspect_ratio(width: u32, height: u32) -> AspectRatio {
	width as f32 / height as f32
}

impl CameraPerspectiveSettings {
	/// Get the view projection matrix that can be sent to the GPU.
	pub fn view_projection_matrix(
		&self,
		position: cgmath::Point3<f32>,
		target: cgmath::Point3<f32>,
	) -> Matrix4x4Pod {
		let view_matrix = cgmath::Matrix4::look_at_rh(position, target, self.up_direction);
		let projection_matrix = cgmath::perspective(
			cgmath::Rad(self.field_of_view_y),
			self.aspect_ratio,
			self.near_plane,
			self.far_plane,
		);
		let view_projection_matrix = projection_matrix * view_matrix;

		// (https://sotrh.github.io/learn-wgpu/beginner/tutorial6-uniforms/#a-perspective-camera)
		// suggests to use this `OPENGL_TO_WGPU_MATRIX` transformation to account for the fact that
		// in OpenGL the view projection transformation should get the frustum to fit in the cube
		// from (-1, -1, -1) to (1, 1, 1), but in Wgpu the frustum should fit in the rectangular
		// area from (-1, -1, 0) to (1, 1, 1). The difference is that on the Z axis (depth) the
		// range is not (-1, 1) but instead is (0, 1).
		// `cgmath` assumes OpenGL-like conventions and here we correct these assumptions to Wgpu.
		#[rustfmt::skip]
		pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
			1.0, 0.0, 0.0, 0.0,
			0.0, 1.0, 0.0, 0.0,
			0.0, 0.0, 0.5, 0.0,
			0.0, 0.0, 0.5, 1.0,
		);
		let view_projection_matrix = OPENGL_TO_WGPU_MATRIX * view_projection_matrix;

		Matrix4x4Pod { values: view_projection_matrix.into() }
	}
}

/// Matrix 4×4.
#[derive(Copy, Clone, Debug)]
/// Certified Plain Old Data (so it can be sent to the GPU as a uniform).
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
pub struct Matrix4x4Pod {
	values: [[f32; 4]; 4],
}
