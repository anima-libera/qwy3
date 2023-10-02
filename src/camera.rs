use cgmath::Zero;

/// Width / height.
pub type AspectRatio = f32;
pub fn aspect_ratio(width: u32, height: u32) -> AspectRatio {
	width as f32 / height as f32
}

/// A camera setting type does not contain the position of a camera
/// or of its target, but it contains all the other setting values
/// used in the computation of the view projection matrix.
pub trait CameraSettings {
	/// Get the base projection matrix, like the matrix that does the perspective projection
	/// or the orthographic projection or something.
	fn projection_matrix(&self) -> cgmath::Matrix4<f32>;

	/// Get a vector that describes how the up direction on the screen is maps in the world,
	/// like if it returns (0, 0, 1) (Z-axis is down-up axis in qwy3) then the camera will
	/// make it so that by looking at the screen it appears the camera is filming upright,
	/// but if it returns something like (1, 0, 0) then the camera will film sort of sideways(?).
	fn up_direction(&self) -> cgmath::Vector3<f32>;

	/// Get the view projection matrix that can be sent to the GPU.
	///
	/// The `up_head` parameter does not represent the world's upwards direction
	/// but instead represents the vector from the bottom to the top of the screen
	/// but in 3D world coordinates. It helps when the `direction` is exactly vertical
	/// (because in this case there would be no way to know how to angle the camera).
	fn view_projection_matrix(
		&self,
		position: cgmath::Point3<f32>,
		direction: cgmath::Vector3<f32>,
		up_head: cgmath::Vector3<f32>,
	) -> Matrix4x4Pod {
		let up = if direction.x.is_zero() && direction.y.is_zero() {
			up_head
		} else {
			self.up_direction()
		};
		let view_matrix = cgmath::Matrix4::look_to_rh(position, direction, up);
		let projection_matrix = self.projection_matrix();
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

/// Camera settings with perspective.
pub struct CameraPerspectiveSettings {
	pub up_direction: cgmath::Vector3<f32>,
	/// Width / height.
	pub aspect_ratio: AspectRatio,
	/// Angle (unsigned, in radians) of view on the vertical axis, "fovy".
	pub field_of_view_y: f32,
	pub near_plane: f32,
	pub far_plane: f32,
}

impl CameraSettings for CameraPerspectiveSettings {
	fn projection_matrix(&self) -> cgmath::Matrix4<f32> {
		cgmath::perspective(
			cgmath::Rad(self.field_of_view_y),
			self.aspect_ratio,
			self.near_plane,
			self.far_plane,
		)
	}

	fn up_direction(&self) -> cgmath::Vector3<f32> {
		self.up_direction
	}
}

/// Camera settings with no perspective (orthographic).
pub struct CameraOrthographicSettings {
	pub up_direction: cgmath::Vector3<f32>,
	pub width: f32,
	pub height: f32,
	pub depth: f32,
}

impl CameraSettings for CameraOrthographicSettings {
	fn projection_matrix(&self) -> cgmath::Matrix4<f32> {
		cgmath::ortho(
			-self.width / 2.0,
			self.width / 2.0,
			-self.height / 2.0,
			self.height / 2.0,
			-self.depth / 2.0,
			self.depth / 2.0,
		)
	}

	fn up_direction(&self) -> cgmath::Vector3<f32> {
		self.up_direction
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
