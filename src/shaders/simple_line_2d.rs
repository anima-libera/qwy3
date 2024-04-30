use super::simple_line::SimpleLineVertexPod;
use crate::rendering_init::BindingThingy;

pub(crate) struct BindingThingies<'a> {
	pub(crate) aspect_ratio_thingy: &'a BindingThingy<wgpu::Buffer>,
}

pub(crate) fn render_pipeline(
	device: &wgpu::Device,
	binding_thingies: BindingThingies,
	output_format: wgpu::TextureFormat,
	z_buffer_format: wgpu::TextureFormat,
) -> (wgpu::RenderPipeline, wgpu::BindGroup) {
	let vertex_buffer_layout = wgpu::VertexBufferLayout {
		array_stride: std::mem::size_of::<SimpleLineVertexPod>() as wgpu::BufferAddress,
		step_mode: wgpu::VertexStepMode::Vertex,
		attributes: &SimpleLineVertexPod::vertex_attributes(),
	};

	use wgpu::ShaderStages as S;
	let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
		label: Some("Simple Line 2D Shader Bind Group Layout"),
		entries: &[binding_thingies.aspect_ratio_thingy.layout_entry(0, S::VERTEX)],
	});
	let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
		label: Some("Simple Line 2D Shader Bind Group"),
		layout: &bind_group_layout,
		entries: &[binding_thingies.aspect_ratio_thingy.bind_group_entry(0)],
	});

	let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
		label: Some("Simple Line 2D Shader"),
		source: wgpu::ShaderSource::Wgsl(include_str!("simple_line_2d.wgsl").into()),
	});
	let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
		label: Some("Simple Line 2D Render Pipeline Layout"),
		bind_group_layouts: &[&bind_group_layout],
		push_constant_ranges: &[],
	});

	let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
		label: Some("Simple Line 2D Render Pipeline"),
		layout: Some(&render_pipeline_layout),
		vertex: wgpu::VertexState {
			module: &shader,
			entry_point: "vertex_shader_main",
			compilation_options: wgpu::PipelineCompilationOptions::default(),
			buffers: &[vertex_buffer_layout],
		},
		fragment: Some(wgpu::FragmentState {
			module: &shader,
			entry_point: "fragment_shader_main",
			compilation_options: wgpu::PipelineCompilationOptions::default(),
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
			depth_compare: wgpu::CompareFunction::LessEqual,
			stencil: wgpu::StencilState::default(),
			bias: wgpu::DepthBiasState::default(),
		}),
		multisample: wgpu::MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false },
		multiview: None,
	});

	(render_pipeline, bind_group)
}
