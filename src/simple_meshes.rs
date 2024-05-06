use cgmath::ElementWise;
use wgpu::util::DeviceExt;

use crate::{
	coords::{AlignedBox, OrientedAxis},
	shaders::{self, simple_line::SimpleLineVertexPod, simple_texture_2d::SimpleTextureVertexPod},
};

/// Mesh of simple lines.
///
/// Can be used (for example) to display hit boxes for debugging purposes.
pub(crate) struct SimpleLineMesh {
	pub(crate) vertex_count: u32,
	pub(crate) vertex_buffer: wgpu::Buffer,
}

impl SimpleLineMesh {
	pub(crate) fn from_vertices(
		device: &wgpu::Device,
		vertices: Vec<SimpleLineVertexPod>,
	) -> SimpleLineMesh {
		let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Simple Line Vertex Buffer"),
			contents: bytemuck::cast_slice(&vertices),
			usage: wgpu::BufferUsages::VERTEX,
		});
		SimpleLineMesh { vertex_count: vertices.len() as u32, vertex_buffer }
	}

	pub(crate) fn from_aligned_box(
		device: &wgpu::Device,
		aligned_box: &AlignedBox,
	) -> SimpleLineMesh {
		// NO EARLY OPTIMIZATION
		// This shall remain in an unoptimized, unfactorized and flexible state for now!

		let color = [1.0, 1.0, 1.0];
		let mut vertices = Vec::new();
		// A---B  +--->   The L square and the H square are horizontal.
		// |   |  |   X+  L has lower value of Z coord.
		// C---D  v Y+    H has heigher value of Z coord.
		let al = aligned_box.pos - aligned_box.dims / 2.0;
		let bl = al + cgmath::Vector3::<f32>::unit_x() * aligned_box.dims.x;
		let cl = al + cgmath::Vector3::<f32>::unit_y() * aligned_box.dims.y;
		let dl = bl + cgmath::Vector3::<f32>::unit_y() * aligned_box.dims.y;
		let ah = al + cgmath::Vector3::<f32>::unit_z() * aligned_box.dims.z;
		let bh = bl + cgmath::Vector3::<f32>::unit_z() * aligned_box.dims.z;
		let ch = cl + cgmath::Vector3::<f32>::unit_z() * aligned_box.dims.z;
		let dh = dl + cgmath::Vector3::<f32>::unit_z() * aligned_box.dims.z;
		// L square
		vertices.push(SimpleLineVertexPod { position: al.into(), color });
		vertices.push(SimpleLineVertexPod { position: bl.into(), color });
		vertices.push(SimpleLineVertexPod { position: bl.into(), color });
		vertices.push(SimpleLineVertexPod { position: dl.into(), color });
		vertices.push(SimpleLineVertexPod { position: dl.into(), color });
		vertices.push(SimpleLineVertexPod { position: cl.into(), color });
		vertices.push(SimpleLineVertexPod { position: cl.into(), color });
		vertices.push(SimpleLineVertexPod { position: al.into(), color });
		// H square
		vertices.push(SimpleLineVertexPod { position: ah.into(), color });
		vertices.push(SimpleLineVertexPod { position: bh.into(), color });
		vertices.push(SimpleLineVertexPod { position: bh.into(), color });
		vertices.push(SimpleLineVertexPod { position: dh.into(), color });
		vertices.push(SimpleLineVertexPod { position: dh.into(), color });
		vertices.push(SimpleLineVertexPod { position: ch.into(), color });
		vertices.push(SimpleLineVertexPod { position: ch.into(), color });
		vertices.push(SimpleLineVertexPod { position: ah.into(), color });
		// Edges between L square and H square corresponding vertices.
		vertices.push(SimpleLineVertexPod { position: al.into(), color });
		vertices.push(SimpleLineVertexPod { position: ah.into(), color });
		vertices.push(SimpleLineVertexPod { position: bl.into(), color });
		vertices.push(SimpleLineVertexPod { position: bh.into(), color });
		vertices.push(SimpleLineVertexPod { position: cl.into(), color });
		vertices.push(SimpleLineVertexPod { position: ch.into(), color });
		vertices.push(SimpleLineVertexPod { position: dl.into(), color });
		vertices.push(SimpleLineVertexPod { position: dh.into(), color });
		SimpleLineMesh::from_vertices(device, vertices)
	}

	/// The `side_offset` parameter moves the generated rectangle along its normal.
	/// It moves towards the outside of the box if the offset is positive.
	pub(crate) fn from_aligned_box_but_only_one_side(
		device: &wgpu::Device,
		aligned_box: &AlignedBox,
		which_side: OrientedAxis,
		side_offset: f32,
	) -> SimpleLineMesh {
		// We are making a rectangle on the plane that contains axis_a and axis_b.
		let [axis_a, axis_b] = which_side.axis.the_other_two_axes();
		// We get the dimensions of that rectangle along its two axes.
		let dim_a = aligned_box.dims[axis_a.index()];
		let dim_b = aligned_box.dims[axis_b.index()];
		// We get the center of the rectangle.
		let displacement_mask =
			which_side.delta().map(|x| x as f32 + side_offset * x.signum() as f32);
		let center = aligned_box.pos + (aligned_box.dims / 2.0).mul_element_wise(displacement_mask);

		// The four vertices of the rectangle.
		let ambm = center + {
			let mut displacement = cgmath::vec3(0.0, 0.0, 0.0);
			displacement[axis_a.index()] = -dim_a / 2.0;
			displacement[axis_b.index()] = -dim_b / 2.0;
			displacement
		};
		let ambp = center + {
			let mut displacement = cgmath::vec3(0.0, 0.0, 0.0);
			displacement[axis_a.index()] = -dim_a / 2.0;
			displacement[axis_b.index()] = dim_b / 2.0;
			displacement
		};
		let apbm = center + {
			let mut displacement = cgmath::vec3(0.0, 0.0, 0.0);
			displacement[axis_a.index()] = dim_a / 2.0;
			displacement[axis_b.index()] = -dim_b / 2.0;
			displacement
		};
		let apbp = center + {
			let mut displacement = cgmath::vec3(0.0, 0.0, 0.0);
			displacement[axis_a.index()] = dim_a / 2.0;
			displacement[axis_b.index()] = dim_b / 2.0;
			displacement
		};

		let color = [1.0, 1.0, 1.0];
		let vertices = vec![
			SimpleLineVertexPod { position: ambm.into(), color },
			SimpleLineVertexPod { position: ambp.into(), color },
			SimpleLineVertexPod { position: ambp.into(), color },
			SimpleLineVertexPod { position: apbp.into(), color },
			SimpleLineVertexPod { position: apbp.into(), color },
			SimpleLineVertexPod { position: apbm.into(), color },
			SimpleLineVertexPod { position: apbm.into(), color },
			SimpleLineVertexPod { position: ambm.into(), color },
		];
		SimpleLineMesh::from_vertices(device, vertices)
	}

	pub(crate) fn interface_2d_cursor(device: &wgpu::Device) -> SimpleLineMesh {
		let color = [1.0, 1.0, 1.0];
		let size = 0.015;
		let vertices = vec![
			SimpleLineVertexPod { position: [-size, 0.0, 0.5], color },
			SimpleLineVertexPod { position: [size, 0.0, 0.5], color },
			SimpleLineVertexPod { position: [0.0, -size, 0.5], color },
			SimpleLineVertexPod { position: [0.0, size, 0.5], color },
		];
		SimpleLineMesh::from_vertices(device, vertices)
	}
}

/// Mesh of simple textured triangles.
///
/// Can be used (for example) to display font character textures in UI text.
pub(crate) struct SimpleTextureMesh {
	pub(crate) vertex_count: u32,
	pub(crate) vertex_buffer: wgpu::Buffer,
}

impl SimpleTextureMesh {
	pub(crate) fn from_vertices(
		device: &wgpu::Device,
		vertices: Vec<shaders::simple_texture_2d::SimpleTextureVertexPod>,
	) -> SimpleTextureMesh {
		let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Simple Texture Vertex Buffer"),
			contents: bytemuck::cast_slice(&vertices),
			usage: wgpu::BufferUsages::VERTEX,
		});
		SimpleTextureMesh { vertex_count: vertices.len() as u32, vertex_buffer }
	}

	fn _from_rect(
		device: &wgpu::Device,
		center: cgmath::Point3<f32>,
		dimensions: cgmath::Vector2<f32>,
		texture_rect_in_atlas_xy: cgmath::Point2<f32>,
		texture_rect_in_atlas_wh: cgmath::Vector2<f32>,
	) -> SimpleTextureMesh {
		let vertices = SimpleTextureMesh::vertices_for_rect(
			center,
			dimensions,
			texture_rect_in_atlas_xy,
			texture_rect_in_atlas_wh,
			[1.0, 1.0, 1.0],
		);
		SimpleTextureMesh::from_vertices(device, vertices)
	}

	pub(crate) fn vertices_for_rect(
		top_left: cgmath::Point3<f32>,
		dimensions: cgmath::Vector2<f32>,
		texture_rect_in_atlas_xy: cgmath::Point2<f32>,
		texture_rect_in_atlas_wh: cgmath::Vector2<f32>,
		color_factor: [f32; 3],
	) -> Vec<SimpleTextureVertexPod> {
		let mut vertices = vec![];

		let a = top_left + cgmath::vec3(0.0, 0.0, 0.0);
		let b = top_left + cgmath::vec3(dimensions.x, 0.0, 0.0);
		let c = top_left + cgmath::vec3(0.0, -dimensions.y, 0.0);
		let d = top_left + cgmath::vec3(dimensions.x, -dimensions.y, 0.0);
		let atlas_a = texture_rect_in_atlas_xy
			+ texture_rect_in_atlas_wh.mul_element_wise(cgmath::vec2(0.0, 0.0));
		let atlas_b = texture_rect_in_atlas_xy
			+ texture_rect_in_atlas_wh.mul_element_wise(cgmath::vec2(1.0, 0.0));
		let atlas_c = texture_rect_in_atlas_xy
			+ texture_rect_in_atlas_wh.mul_element_wise(cgmath::vec2(0.0, 1.0));
		let atlas_d = texture_rect_in_atlas_xy
			+ texture_rect_in_atlas_wh.mul_element_wise(cgmath::vec2(1.0, 1.0));

		vertices.push(SimpleTextureVertexPod {
			position: a.into(),
			coords_in_atlas: atlas_a.into(),
			color_factor,
		});
		vertices.push(SimpleTextureVertexPod {
			position: c.into(),
			coords_in_atlas: atlas_c.into(),
			color_factor,
		});
		vertices.push(SimpleTextureVertexPod {
			position: b.into(),
			coords_in_atlas: atlas_b.into(),
			color_factor,
		});
		vertices.push(SimpleTextureVertexPod {
			position: c.into(),
			coords_in_atlas: atlas_c.into(),
			color_factor,
		});
		vertices.push(SimpleTextureVertexPod {
			position: d.into(),
			coords_in_atlas: atlas_d.into(),
			color_factor,
		});
		vertices.push(SimpleTextureVertexPod {
			position: b.into(),
			coords_in_atlas: atlas_b.into(),
			color_factor,
		});

		vertices
	}
}
