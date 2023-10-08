use super::block::BlockVertexPod;
use crate::BindingResourceable;
pub(crate) use crate::BindingThingy;

pub struct BindingThingies<'a> {
	pub(crate) sun_camera_matrix_thingy: &'a BindingThingy<wgpu::Buffer>,
}

pub fn render_pipeline_and_bind_group(
	device: &wgpu::Device,
	binding_thingies: BindingThingies,
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
		label: Some("Block Shadow Shader Bind Group Layout"),
		entries: &[binding_thingies
			.sun_camera_matrix_thingy
			.binding_type
			.layout_entry(0, wgpu::ShaderStages::VERTEX)],
	});
	let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
		label: Some("Block Shadow Shader Bind Group"),
		layout: &bind_group_layout,
		entries: &[wgpu::BindGroupEntry {
			binding: 0,
			resource: binding_thingies
				.sun_camera_matrix_thingy
				.resource
				.as_binding_resource(),
		}],
	});

	let block_shadow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
		label: Some("Block Shadow Shader"),
		source: wgpu::ShaderSource::Wgsl(include_str!("block_shadow.wgsl").into()),
	});
	let block_shadow_render_pipeline_layout =
		device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("Block Shadow Render Pipeline Layout"),
			bind_group_layouts: &[&bind_group_layout],
			push_constant_ranges: &[],
		});

	let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
	});

	(render_pipeline, bind_group)
}
