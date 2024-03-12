use super::block::BlockVertexPod;
pub(crate) use crate::BindingThingy;

pub(crate) struct BindingThingies<'a> {
	pub(crate) sun_camera_single_matrix_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) atlas_texture_view_thingy: &'a BindingThingy<wgpu::TextureView>,
	pub(crate) atlas_texture_sampler_thingy: &'a BindingThingy<wgpu::Sampler>,
	pub(crate) fog_center_position_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) fog_inf_sup_radiuses_thingy: &'a BindingThingy<wgpu::Buffer>,
}

pub(crate) fn render_pipeline_and_bind_group(
	device: &wgpu::Device,
	binding_thingies: BindingThingies,
	z_buffer_format: wgpu::TextureFormat,
) -> (wgpu::RenderPipeline, wgpu::BindGroup) {
	let block_vertex_buffer_layout = wgpu::VertexBufferLayout {
		array_stride: std::mem::size_of::<BlockVertexPod>() as wgpu::BufferAddress,
		step_mode: wgpu::VertexStepMode::Vertex,
		attributes: &BlockVertexPod::vertex_attributes(),
	};

	use wgpu::ShaderStages as S;
	let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
		label: Some("Block Shadow Shader Bind Group Layout"),
		entries: &[
			binding_thingies.sun_camera_single_matrix_thingy.layout_entry(0, S::VERTEX),
			binding_thingies.atlas_texture_view_thingy.layout_entry(1, S::FRAGMENT),
			binding_thingies.atlas_texture_sampler_thingy.layout_entry(2, S::FRAGMENT),
			binding_thingies.fog_center_position_thingy.layout_entry(3, S::FRAGMENT),
			binding_thingies.fog_inf_sup_radiuses_thingy.layout_entry(4, S::FRAGMENT),
		],
	});
	let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
		label: Some("Block Shadow Shader Bind Group"),
		layout: &bind_group_layout,
		entries: &[
			binding_thingies.sun_camera_single_matrix_thingy.bind_group_entry(0),
			binding_thingies.atlas_texture_view_thingy.bind_group_entry(1),
			binding_thingies.atlas_texture_sampler_thingy.bind_group_entry(2),
			binding_thingies.fog_center_position_thingy.bind_group_entry(3),
			binding_thingies.fog_inf_sup_radiuses_thingy.bind_group_entry(4),
		],
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
		fragment: Some(wgpu::FragmentState {
			module: &block_shadow_shader,
			entry_point: "fragment_shader_main",
			targets: &[],
		}),
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
