use crate::BindingResourceable;
pub(crate) use crate::BindingThingy;

/// Vertex type used in chunk block meshes.
#[derive(Copy, Clone, Debug)]
/// Certified Plain Old Data (so it can be sent to the GPU as a uniform).
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkyboxVertexPod {
	pub position: [f32; 3],
	/// 3D direction vector pointing from the origin to a point on the cubemap.
	/// This is how `Cube`-dimensional cubemap textures are sampled.
	/// See https://www.w3.org/TR/WGSL/#texture-dimensionality and related notions for more.
	pub coords_in_skybox_cubemap: [f32; 3],
}

pub struct BindingThingies<'a> {
	pub(crate) camera_matrix_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) skybox_cubemap_texture_view_thingy: &'a BindingThingy<wgpu::TextureView>,
	pub(crate) skybox_cubemap_texture_sampler_thingy: &'a BindingThingy<wgpu::Sampler>,
}

pub fn render_pipeline_and_bind_group(
	device: &wgpu::Device,
	binding_thingies: BindingThingies,
	output_format: wgpu::TextureFormat,
) -> (wgpu::RenderPipeline, wgpu::BindGroup) {
	let block_vertex_buffer_layout = wgpu::VertexBufferLayout {
		array_stride: std::mem::size_of::<SkyboxVertexPod>() as wgpu::BufferAddress,
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
				format: wgpu::VertexFormat::Float32x3,
			},
		],
	};

	let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
		label: Some("Skybox Shader Bind Group Layout"),
		entries: &[
			binding_thingies
				.camera_matrix_thingy
				.binding_type
				.layout_entry(0, wgpu::ShaderStages::VERTEX),
			binding_thingies
				.skybox_cubemap_texture_view_thingy
				.binding_type
				.layout_entry(1, wgpu::ShaderStages::FRAGMENT),
			binding_thingies
				.skybox_cubemap_texture_sampler_thingy
				.binding_type
				.layout_entry(2, wgpu::ShaderStages::FRAGMENT),
		],
	});
	let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
		label: Some("Skybox Shader Bind Group"),
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
					.skybox_cubemap_texture_view_thingy
					.resource
					.as_binding_resource(),
			},
			wgpu::BindGroupEntry {
				binding: 2,
				resource: binding_thingies
					.skybox_cubemap_texture_sampler_thingy
					.resource
					.as_binding_resource(),
			},
		],
	});

	let skybox_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
		label: Some("Skybox Shader"),
		source: wgpu::ShaderSource::Wgsl(include_str!("skybox.wgsl").into()),
	});
	let skybox_render_pipeline_layout =
		device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("Skybox Render Pipeline Layout"),
			bind_group_layouts: &[&bind_group_layout],
			push_constant_ranges: &[],
		});

	let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
		label: Some("Skybox Render Pipeline"),
		layout: Some(&skybox_render_pipeline_layout),
		vertex: wgpu::VertexState {
			module: &skybox_shader,
			entry_point: "vertex_shader_main",
			buffers: &[block_vertex_buffer_layout],
		},
		fragment: Some(wgpu::FragmentState {
			module: &skybox_shader,
			entry_point: "fragment_shader_main",
			targets: &[Some(wgpu::ColorTargetState {
				format: output_format,
				blend: Some(wgpu::BlendState::REPLACE),
				write_mask: wgpu::ColorWrites::ALL,
			})],
		}),
		primitive: wgpu::PrimitiveState {
			topology: wgpu::PrimitiveTopology::TriangleList,
			strip_index_format: None,
			front_face: wgpu::FrontFace::Ccw,
			cull_mode: None,
			polygon_mode: wgpu::PolygonMode::Fill,
			unclipped_depth: false,
			conservative: false,
		},
		depth_stencil: None,
		multisample: wgpu::MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false },
		multiview: None,
	});

	(render_pipeline, bind_group)
}
