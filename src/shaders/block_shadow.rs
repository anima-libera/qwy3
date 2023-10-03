use super::block::BlockVertexPod;

pub fn render_pipeline(
	device: &wgpu::Device,
	sun_camera_bind_group_layout: &wgpu::BindGroupLayout,
	z_buffer_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
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
				offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
				shader_location: 1,
				format: wgpu::VertexFormat::Float32x3,
			},
			wgpu::VertexAttribute {
				offset: (std::mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
				shader_location: 2,
				format: wgpu::VertexFormat::Float32x3,
			},
			wgpu::VertexAttribute {
				offset: (std::mem::size_of::<[f32; 3]>() * 3) as wgpu::BufferAddress,
				shader_location: 3,
				format: wgpu::VertexFormat::Float32,
			},
		],
	};

	let block_shadow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
		label: Some("Block Shadow Shader"),
		source: wgpu::ShaderSource::Wgsl(include_str!("block_shadow.wgsl").into()),
	});
	let block_shadow_render_pipeline_layout =
		device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("Block Shadow Render Pipeline Layout"),
			bind_group_layouts: &[sun_camera_bind_group_layout],
			push_constant_ranges: &[],
		});

	device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
		label: Some("Block Shadow Render Pipeline"),
		layout: Some(&block_shadow_render_pipeline_layout),
		vertex: wgpu::VertexState {
			module: &block_shadow_shader,
			entry_point: "vertex_shader_main",
			buffers: &[block_vertex_buffer_layout],
		},
		fragment: None,
		primitive: wgpu::PrimitiveState {
			topology: wgpu::PrimitiveTopology::TriangleList,
			strip_index_format: None,
			front_face: wgpu::FrontFace::Cw,
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
	})
}
