use wgpu::vertex_attr_array;

use crate::{rendering_init::BindingThingy, shaders::part_textured::PartVertexPod};

/// Instance type used for each entity part that has texture mappings.
#[derive(Copy, Clone, Debug)]
/// Certified Plain Old Data (so it can be sent to the GPU as a uniform).
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PartColoredInstancePod {
	pub(crate) model_matrix_1_of_4: [f32; 4],
	pub(crate) model_matrix_2_of_4: [f32; 4],
	pub(crate) model_matrix_3_of_4: [f32; 4],
	pub(crate) model_matrix_4_of_4: [f32; 4],
	pub(crate) inv_trans_model_matrix_1_of_4: [f32; 4],
	pub(crate) inv_trans_model_matrix_2_of_4: [f32; 4],
	pub(crate) inv_trans_model_matrix_3_of_4: [f32; 4],
	pub(crate) inv_trans_model_matrix_4_of_4: [f32; 4],
	pub(crate) coloring_offset: u32,
}
impl PartColoredInstancePod {
	pub(crate) fn vertex_attributes() -> [wgpu::VertexAttribute; 9] {
		vertex_attr_array![
			2 => Float32x4,
			3 => Float32x4,
			4 => Float32x4,
			5 => Float32x4,
			6 => Float32x4,
			7 => Float32x4,
			8 => Float32x4,
			9 => Float32x4,
			10 => Uint32,
		]
	}
}

pub(crate) struct BindingThingies<'a> {
	pub(crate) camera_matrix_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) texturing_and_coloring_array_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) sun_light_direction_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) sun_camera_matrices_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) shadow_map_view_thingy: &'a BindingThingy<wgpu::TextureView>,
	pub(crate) shadow_map_sampler_thingy: &'a BindingThingy<wgpu::Sampler>,
	pub(crate) fog_center_position_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) fog_inf_sup_radiuses_thingy: &'a BindingThingy<wgpu::Buffer>,
}

pub(crate) fn render_pipeline_and_bind_group(
	device: &wgpu::Device,
	binding_thingies: BindingThingies,
	output_format: wgpu::TextureFormat,
	z_buffer_format: wgpu::TextureFormat,
) -> (wgpu::RenderPipeline, wgpu::BindGroup) {
	let vertex_buffer_layout = wgpu::VertexBufferLayout {
		array_stride: std::mem::size_of::<PartVertexPod>() as wgpu::BufferAddress,
		step_mode: wgpu::VertexStepMode::Vertex,
		attributes: &PartVertexPod::vertex_attributes(),
	};
	let instance_buffer_layout = wgpu::VertexBufferLayout {
		array_stride: std::mem::size_of::<PartColoredInstancePod>() as wgpu::BufferAddress,
		step_mode: wgpu::VertexStepMode::Instance,
		attributes: &PartColoredInstancePod::vertex_attributes(),
	};

	use wgpu::ShaderStages as S;
	let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
		label: Some("Part Colored Shader Bind Group Layout"),
		entries: &[
			binding_thingies.camera_matrix_thingy.layout_entry(0, S::VERTEX),
			binding_thingies.texturing_and_coloring_array_thingy.layout_entry(1, S::VERTEX),
			binding_thingies.sun_light_direction_thingy.layout_entry(2, S::VERTEX),
			binding_thingies.sun_camera_matrices_thingy.layout_entry(3, S::FRAGMENT),
			binding_thingies.shadow_map_view_thingy.layout_entry(4, S::FRAGMENT),
			binding_thingies.shadow_map_sampler_thingy.layout_entry(5, S::FRAGMENT),
			binding_thingies.fog_center_position_thingy.layout_entry(6, S::FRAGMENT),
			binding_thingies.fog_inf_sup_radiuses_thingy.layout_entry(7, S::FRAGMENT),
		],
	});
	let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
		label: Some("Part Colored Shader Bind Group"),
		layout: &bind_group_layout,
		entries: &[
			binding_thingies.camera_matrix_thingy.bind_group_entry(0),
			binding_thingies.texturing_and_coloring_array_thingy.bind_group_entry(1),
			binding_thingies.sun_light_direction_thingy.bind_group_entry(2),
			binding_thingies.sun_camera_matrices_thingy.bind_group_entry(3),
			binding_thingies.shadow_map_view_thingy.bind_group_entry(4),
			binding_thingies.shadow_map_sampler_thingy.bind_group_entry(5),
			binding_thingies.fog_center_position_thingy.bind_group_entry(6),
			binding_thingies.fog_inf_sup_radiuses_thingy.bind_group_entry(7),
		],
	});

	let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
		label: Some("Part Colored Shader"),
		source: wgpu::ShaderSource::Wgsl(include_str!("part_colored.wgsl").into()),
	});
	let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
		label: Some("Part Colored Render Pipeline Layout"),
		bind_group_layouts: &[&bind_group_layout],
		push_constant_ranges: &[],
	});

	let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
		label: Some("Part Colored Render Pipeline"),
		layout: Some(&render_pipeline_layout),
		vertex: wgpu::VertexState {
			module: &shader,
			entry_point: "vertex_shader_main",
			compilation_options: wgpu::PipelineCompilationOptions::default(),
			buffers: &[vertex_buffer_layout, instance_buffer_layout],
		},
		fragment: Some(wgpu::FragmentState {
			module: &shader,
			entry_point: "fragment_shader_main",
			compilation_options: wgpu::PipelineCompilationOptions::default(),
			targets: &[Some(wgpu::ColorTargetState {
				format: output_format,
				// See same spot in the `block` pipeline for an explanation.
				blend: Some(wgpu::BlendState::REPLACE),
				write_mask: wgpu::ColorWrites::ALL,
			})],
		}),
		primitive: wgpu::PrimitiveState {
			topology: wgpu::PrimitiveTopology::TriangleList,
			strip_index_format: None,
			front_face: wgpu::FrontFace::Ccw,
			cull_mode: Some(wgpu::Face::Back),
			polygon_mode: wgpu::PolygonMode::Fill,
			unclipped_depth: false,
			conservative: false,
		},
		depth_stencil: Some(wgpu::DepthStencilState {
			format: z_buffer_format,
			depth_write_enabled: true,
			depth_compare: wgpu::CompareFunction::LessEqual,
			stencil: wgpu::StencilState::default(),
			bias: wgpu::DepthBiasState::default(),
		}),
		multisample: wgpu::MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false },
		multiview: None,
	});

	(render_pipeline, bind_group)
}
