use crate::BindingResourceable;
pub(crate) use crate::BindingThingy;

/// Vertex type used in chunk block meshes.
#[derive(Copy, Clone, Debug)]
/// Certified Plain Old Data (so it can be sent to the GPU as a uniform).
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
pub struct BlockVertexPod {
	pub position: [f32; 3],
	pub coords_in_atlas: [f32; 2],
	pub normal: [f32; 3],
	pub ambiant_occlusion: f32,
}

pub struct BindingThingies<'a> {
	pub(crate) camera_matrix_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) sun_light_direction_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) sun_camera_matrix_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) shadow_map_view_thingy: &'a BindingThingy<wgpu::TextureView>,
	pub(crate) shadow_map_sampler_thingy: &'a BindingThingy<wgpu::Sampler>,
	pub(crate) atlas_texture_view_thingy: &'a BindingThingy<wgpu::TextureView>,
	pub(crate) atlas_texture_sampler_thingy: &'a BindingThingy<wgpu::Sampler>,
	pub(crate) fog_center_position_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) fog_inf_sup_radiuses_thingy: &'a BindingThingy<wgpu::Buffer>,
}

pub fn render_pipeline_and_bind_group(
	device: &wgpu::Device,
	binding_thingies: BindingThingies,
	output_format: wgpu::TextureFormat,
	z_buffer_format: wgpu::TextureFormat,
) -> (wgpu::RenderPipeline, wgpu::BindGroup) {
	let block_vertex_buffer_layout = wgpu::VertexBufferLayout {
		array_stride: std::mem::size_of::<BlockVertexPod>() as wgpu::BufferAddress,
		step_mode: wgpu::VertexStepMode::Vertex,
		attributes: &[
			wgpu::VertexAttribute {
				offset: 0,
				shader_location: 0,
				format: wgpu::VertexFormat::Float32x3,
			},
			wgpu::VertexAttribute {
				offset: (std::mem::size_of::<f32>() * 3) as wgpu::BufferAddress,
				shader_location: 1,
				format: wgpu::VertexFormat::Float32x2,
			},
			wgpu::VertexAttribute {
				offset: (std::mem::size_of::<f32>() * 5) as wgpu::BufferAddress,
				shader_location: 2,
				format: wgpu::VertexFormat::Float32x3,
			},
			wgpu::VertexAttribute {
				offset: (std::mem::size_of::<f32>() * 8) as wgpu::BufferAddress,
				shader_location: 3,
				format: wgpu::VertexFormat::Float32,
			},
		],
	};

	let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
		label: Some("Block Shader Bind Group Layout"),
		entries: &[
			binding_thingies
				.camera_matrix_thingy
				.binding_type
				.layout_entry(0, wgpu::ShaderStages::VERTEX),
			binding_thingies
				.sun_light_direction_thingy
				.binding_type
				.layout_entry(1, wgpu::ShaderStages::VERTEX),
			binding_thingies
				.sun_camera_matrix_thingy
				.binding_type
				.layout_entry(2, wgpu::ShaderStages::FRAGMENT),
			binding_thingies
				.shadow_map_view_thingy
				.binding_type
				.layout_entry(3, wgpu::ShaderStages::FRAGMENT),
			binding_thingies
				.shadow_map_sampler_thingy
				.binding_type
				.layout_entry(4, wgpu::ShaderStages::FRAGMENT),
			binding_thingies
				.atlas_texture_view_thingy
				.binding_type
				.layout_entry(5, wgpu::ShaderStages::FRAGMENT),
			binding_thingies
				.atlas_texture_sampler_thingy
				.binding_type
				.layout_entry(6, wgpu::ShaderStages::FRAGMENT),
			binding_thingies
				.fog_center_position_thingy
				.binding_type
				.layout_entry(7, wgpu::ShaderStages::FRAGMENT),
			binding_thingies
				.fog_inf_sup_radiuses_thingy
				.binding_type
				.layout_entry(8, wgpu::ShaderStages::FRAGMENT),
		],
	});
	let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
		label: Some("Block Shader Bind Group"),
		layout: &bind_group_layout,
		entries: &[
			wgpu::BindGroupEntry {
				binding: 0,
				resource: binding_thingies
					.camera_matrix_thingy
					.resource
					.as_binding_resource(),
			},
			wgpu::BindGroupEntry {
				binding: 1,
				resource: binding_thingies
					.sun_light_direction_thingy
					.resource
					.as_binding_resource(),
			},
			wgpu::BindGroupEntry {
				binding: 2,
				resource: binding_thingies
					.sun_camera_matrix_thingy
					.resource
					.as_binding_resource(),
			},
			wgpu::BindGroupEntry {
				binding: 3,
				resource: binding_thingies
					.shadow_map_view_thingy
					.resource
					.as_binding_resource(),
			},
			wgpu::BindGroupEntry {
				binding: 4,
				resource: binding_thingies
					.shadow_map_sampler_thingy
					.resource
					.as_binding_resource(),
			},
			wgpu::BindGroupEntry {
				binding: 5,
				resource: binding_thingies
					.atlas_texture_view_thingy
					.resource
					.as_binding_resource(),
			},
			wgpu::BindGroupEntry {
				binding: 6,
				resource: binding_thingies
					.atlas_texture_sampler_thingy
					.resource
					.as_binding_resource(),
			},
			wgpu::BindGroupEntry {
				binding: 7,
				resource: binding_thingies
					.fog_center_position_thingy
					.resource
					.as_binding_resource(),
			},
			wgpu::BindGroupEntry {
				binding: 8,
				resource: binding_thingies
					.fog_inf_sup_radiuses_thingy
					.resource
					.as_binding_resource(),
			},
		],
	});

	let block_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
		label: Some("Block Shader"),
		source: wgpu::ShaderSource::Wgsl(include_str!("block.wgsl").into()),
	});
	let block_render_pipeline_layout =
		device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("Block Render Pipeline Layout"),
			bind_group_layouts: &[&bind_group_layout],
			push_constant_ranges: &[],
		});

	let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
		label: Some("Block Render Pipeline"),
		layout: Some(&block_render_pipeline_layout),
		vertex: wgpu::VertexState {
			module: &block_shader,
			entry_point: "vertex_shader_main",
			buffers: &[block_vertex_buffer_layout],
		},
		fragment: Some(wgpu::FragmentState {
			module: &block_shader,
			entry_point: "fragment_shader_main",
			targets: &[Some(wgpu::ColorTargetState {
				format: output_format,
				blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
			depth_compare: wgpu::CompareFunction::Less,
			stencil: wgpu::StencilState::default(),
			bias: wgpu::DepthBiasState::default(),
		}),
		multisample: wgpu::MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false },
		multiview: None,
	});

	(render_pipeline, bind_group)
}
