/// Vertex type used in debugging line meshes.
#[derive(Copy, Clone, Debug)]
/// Certified Plain Old Data (so it can be sent to the GPU as a uniform).
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
pub struct SimpleLineVertexPod {
	pub position: [f32; 3],
	pub color: [f32; 3],
}

pub fn render_pipeline(
	device: &wgpu::Device,
	camera_bind_group_layout: &wgpu::BindGroupLayout,
	output_format: wgpu::TextureFormat,
	z_buffer_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
	let simple_line_vertex_buffer_layout = wgpu::VertexBufferLayout {
		array_stride: std::mem::size_of::<SimpleLineVertexPod>() as wgpu::BufferAddress,
		step_mode: wgpu::VertexStepMode::Vertex,
		attributes: &[
			wgpu::VertexAttribute {
				offset: 0,
				shader_location: 0,
				format: wgpu::VertexFormat::Float32x3,
			},
			wgpu::VertexAttribute {
				offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
				shader_location: 1,
				format: wgpu::VertexFormat::Float32x3,
			},
		],
	};

	let simple_line_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
		label: Some("Simple Line Shader"),
		source: wgpu::ShaderSource::Wgsl(include_str!("simple_line.wgsl").into()),
	});
	let simple_line_render_pipeline_layout =
		device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("Simple Line Render Pipeline Layout"),
			bind_group_layouts: &[camera_bind_group_layout],
			push_constant_ranges: &[],
		});

	device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
		label: Some("Simple Line Render Pipeline"),
		layout: Some(&simple_line_render_pipeline_layout),
		vertex: wgpu::VertexState {
			module: &simple_line_shader,
			entry_point: "vertex_shader_main",
			buffers: &[simple_line_vertex_buffer_layout],
		},
		fragment: Some(wgpu::FragmentState {
			module: &simple_line_shader,
			entry_point: "fragment_shader_main",
			targets: &[Some(wgpu::ColorTargetState {
				format: output_format,
				blend: Some(wgpu::BlendState::REPLACE),
				write_mask: wgpu::ColorWrites::ALL,
			})],
		}),
		primitive: wgpu::PrimitiveState {
			topology: wgpu::PrimitiveTopology::LineList,
			strip_index_format: None,
			front_face: wgpu::FrontFace::Ccw,
			cull_mode: None,
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
	})
}
