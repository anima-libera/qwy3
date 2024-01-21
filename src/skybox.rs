use cgmath::{point3, EuclideanSpace, Point3};
use wgpu::util::DeviceExt;

use crate::shaders::skybox::SkyboxVertexPod;

pub const SKYBOX_SIDE_DIMS: (usize, usize) = (512, 512);

pub struct SkyboxMesh {
	pub vertices: Vec<SkyboxVertexPod>,
	pub vertex_buffer: wgpu::Buffer,
}

impl SkyboxMesh {
	pub fn new(device: &wgpu::Device, center_coords: Point3<f32>) -> SkyboxMesh {
		let new_vertex = |position: Point3<f32>| -> SkyboxVertexPod {
			SkyboxVertexPod {
				position: (center_coords + position.to_vec()).into(),
				coords_in_skybox_cubemap: position.into(),
			}
		};
		let new_face = |vertices: &mut Vec<SkyboxVertexPod>,
		                axis_a: usize,
		                axis_b: usize,
		                axis_c_and_value: (usize, f32)| {
			let ab_to_coords = |a: f32, b: f32| -> Point3<f32> {
				let (axis_c, c) = axis_c_and_value;
				let mut coords = point3(0.0, 0.0, 0.0);
				coords[axis_a] = a;
				coords[axis_b] = b;
				coords[axis_c] = c;
				coords
			};
			vertices.push(new_vertex(ab_to_coords(-1.0, -1.0)));
			vertices.push(new_vertex(ab_to_coords(1.0, -1.0)));
			vertices.push(new_vertex(ab_to_coords(-1.0, 1.0)));
			vertices.push(new_vertex(ab_to_coords(1.0, -1.0)));
			vertices.push(new_vertex(ab_to_coords(-1.0, 1.0)));
			vertices.push(new_vertex(ab_to_coords(1.0, 1.0)));
		};

		let mut vertices = vec![];
		new_face(&mut vertices, 0, 1, (2, -1.0));
		new_face(&mut vertices, 0, 1, (2, 1.0));
		new_face(&mut vertices, 0, 2, (1, -1.0));
		new_face(&mut vertices, 0, 2, (1, 1.0));
		new_face(&mut vertices, 1, 2, (0, -1.0));
		new_face(&mut vertices, 1, 2, (0, 1.0));

		let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Skybox Vertex Buffer"),
			contents: bytemuck::cast_slice(&vertices),
			usage: wgpu::BufferUsages::VERTEX,
		});

		SkyboxMesh { vertices, vertex_buffer }
	}
}
