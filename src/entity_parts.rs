use cgmath::EuclideanSpace;
use wgpu::util::DeviceExt;

use crate::{
	coords::{AxisOrientation, NonOrientedAxis, OrientedAxis},
	shaders::part_textured::{PartInstancePod, PartVertexPod},
};

pub(crate) struct PartTables {
	pub(crate) textured_cubes: PartTable<PartInstancePod>,
}

impl PartTables {
	pub(crate) fn new(device: &wgpu::Device) -> PartTables {
		PartTables { textured_cubes: textured_cubes::textured_cube_part_table(device) }
	}

	pub(crate) fn cup_to_gpu_update_if_required(
		&mut self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
	) {
		self.textured_cubes.cup_to_gpu_update_if_required(device, queue);
	}

	pub(crate) fn tables_for_rendering(&self) -> [&dyn PartTableRendrable; 1] {
		[&self.textured_cubes]
	}
}

pub(crate) struct PartTable<T: bytemuck::Pod + bytemuck::Zeroable> {
	mesh: Mesh,
	instance_table: Vec<T>,
	instance_table_buffer: wgpu::Buffer,
	cpu_to_gpu_update_required_for_instances: bool,
	cpu_to_gpu_update_required_for_new_instances: bool,
	name: &'static str,
}

impl<T: bytemuck::Pod + bytemuck::Zeroable> PartTable<T> {
	pub(crate) fn add_instance(&mut self, instance: T) {
		self.instance_table.push(instance);
		self.cpu_to_gpu_update_required_for_instances = true;
		self.cpu_to_gpu_update_required_for_new_instances = true;
	}

	fn cup_to_gpu_update_if_required(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
		if self.cpu_to_gpu_update_required_for_new_instances {
			// TODO: Double size like Vec instead of just reallocating for the new instances.
			self.cpu_to_gpu_update_required_for_new_instances = false;
			self.cpu_to_gpu_update_required_for_instances = false;
			let name = self.name;
			self.instance_table_buffer =
				device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
					label: Some(&format!("{name} Instance Buffer")),
					contents: bytemuck::cast_slice(&self.instance_table),
					usage: wgpu::BufferUsages::VERTEX,
				});
		} else if self.cpu_to_gpu_update_required_for_instances {
			self.cpu_to_gpu_update_required_for_instances = false;
			queue.write_buffer(
				&self.instance_table_buffer,
				0,
				bytemuck::cast_slice(&self.instance_table),
			);
		}
	}
}

pub(crate) struct DataForPartTableRendering<'a> {
	pub(crate) mesh_vertices_count: u32,
	pub(crate) mesh_vertex_buffer: &'a wgpu::Buffer,
	pub(crate) instances_count: u32,
	pub(crate) instance_buffer: &'a wgpu::Buffer,
}

pub(crate) trait PartTableRendrable {
	fn get_data_for_rendering(&self) -> DataForPartTableRendering;
}

impl<T: bytemuck::Pod + bytemuck::Zeroable> PartTableRendrable for PartTable<T> {
	fn get_data_for_rendering(&self) -> DataForPartTableRendering {
		DataForPartTableRendering {
			mesh_vertices_count: self.mesh.vertex_count,
			mesh_vertex_buffer: &self.mesh.buffer,
			instances_count: self.instance_table.len() as u32,
			instance_buffer: &self.instance_table_buffer,
		}
	}
}

struct Mesh {
	vertex_count: u32,
	buffer: wgpu::Buffer,
}

pub(crate) mod textured_cubes {
	use super::*;

	pub(super) fn textured_cube_part_table(device: &wgpu::Device) -> PartTable<PartInstancePod> {
		let name = "Textured Cube Part";
		PartTable {
			mesh: cube_mesh(device, name),
			instance_table: vec![],
			instance_table_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some(&format!("{name} Instance Buffer")),
				contents: &[],
				usage: wgpu::BufferUsages::VERTEX,
			}),
			cpu_to_gpu_update_required_for_instances: false,
			cpu_to_gpu_update_required_for_new_instances: false,
			name,
		}
	}

	pub(crate) struct PartTexturedCubeInstanceData {
		model_matrix: [[f32; 4]; 4],
	}

	impl PartTexturedCubeInstanceData {
		pub(crate) fn new(pos: cgmath::Point3<f32>) -> PartTexturedCubeInstanceData {
			let model_matrix = cgmath::Matrix4::<f32>::from_translation(pos.to_vec());
			let model_matrix = cgmath::conv::array4x4(model_matrix);
			PartTexturedCubeInstanceData { model_matrix }
		}

		pub(crate) fn to_pod(&self) -> PartInstancePod {
			PartInstancePod {
				model_matrix_1_of_4: self.model_matrix[0],
				model_matrix_2_of_4: self.model_matrix[1],
				model_matrix_3_of_4: self.model_matrix[2],
				model_matrix_4_of_4: self.model_matrix[3],
			}
		}
	}

	fn cube_mesh(device: &wgpu::Device, name: &str) -> Mesh {
		// There is a lot of code duplicated from `chunk_meshing::generate_block_face_mesh`.
		// TODO: Factorize some code with there.

		let mut vertices: Vec<PartVertexPod> = vec![];

		let cube_center = cgmath::point3(0.0, 0.0, 0.0);
		for direction in OrientedAxis::all_the_six_possible_directions() {
			let normal: [f32; 3] = cgmath::conv::array3(direction.delta().map(|x| x as f32));

			let face_center = cube_center + direction.delta().map(|x| x as f32) * 0.5;

			let mut other_axes = [NonOrientedAxis::X, NonOrientedAxis::Y, NonOrientedAxis::Z]
				.into_iter()
				.filter(|&axis| axis != direction.axis);
			let other_axis_a = other_axes.next().unwrap();
			let other_axis_b = other_axes.next().unwrap();

			let mut coords_array = [face_center; 4];

			coords_array[0][other_axis_a.index()] -= 0.5;
			coords_array[0][other_axis_b.index()] -= 0.5;
			coords_array[1][other_axis_a.index()] -= 0.5;
			coords_array[1][other_axis_b.index()] += 0.5;
			coords_array[2][other_axis_a.index()] += 0.5;
			coords_array[2][other_axis_b.index()] -= 0.5;
			coords_array[3][other_axis_a.index()] += 0.5;
			coords_array[3][other_axis_b.index()] += 0.5;

			let indices = [1, 0, 3, 3, 0, 2];

			// Adjusting the order of the vertices to makeup for face culling.
			let reverse_order = match direction.axis {
				NonOrientedAxis::X => direction.orientation == AxisOrientation::Negativewards,
				NonOrientedAxis::Y => direction.orientation == AxisOrientation::Positivewards,
				NonOrientedAxis::Z => direction.orientation == AxisOrientation::Negativewards,
			};
			let indices_indices_normal = [0, 1, 2, 3, 4, 5];
			let indices_indices_reversed = [0, 2, 1, 3, 5, 4];
			let mut handle_index = |index: usize| {
				vertices.push(PartVertexPod { position: coords_array[index].into(), normal });
			};
			if !reverse_order {
				for indices_index in indices_indices_normal {
					handle_index(indices[indices_index]);
				}
			} else {
				for indices_index in indices_indices_reversed {
					handle_index(indices[indices_index]);
				}
			}
		}

		let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some(&format!("{name} Vertex Buffer")),
			contents: bytemuck::cast_slice(&vertices),
			usage: wgpu::BufferUsages::VERTEX,
		});

		Mesh { vertex_count: vertices.len() as u32, buffer }
	}
}
