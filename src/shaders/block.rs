use wgpu::vertex_attr_array;

use crate::rendering_init::BindingThingy;

/// Vertex type used in chunk block meshes.
#[derive(Copy, Clone, Debug)]
/// Certified Plain Old Data (so it can be sent to the GPU as a uniform).
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BlockVertexPod {
	pub(crate) position: [f32; 3],
	pub(crate) coords_in_atlas: [f32; 2],
	pub(crate) normal: [f32; 3],
	pub(crate) ambiant_occlusion: f32,
}
impl BlockVertexPod {
	pub(crate) fn vertex_attributes() -> [wgpu::VertexAttribute; 4] {
		vertex_attr_array![
			0 => Float32x3,
			1 => Float32x2,
			2 => Float32x3,
			3 => Float32,
		]
	}
}

pub(crate) struct BindingThingies<'a> {
	pub(crate) camera_matrix_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) sun_light_direction_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) sun_camera_matrices_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) shadow_map_view_thingy: &'a BindingThingy<wgpu::TextureView>,
	pub(crate) shadow_map_sampler_thingy: &'a BindingThingy<wgpu::Sampler>,
	pub(crate) atlas_texture_view_thingy: &'a BindingThingy<wgpu::TextureView>,
	pub(crate) atlas_texture_sampler_thingy: &'a BindingThingy<wgpu::Sampler>,
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
		array_stride: std::mem::size_of::<BlockVertexPod>() as wgpu::BufferAddress,
		step_mode: wgpu::VertexStepMode::Vertex,
		attributes: &BlockVertexPod::vertex_attributes(),
	};

	use wgpu::ShaderStages as S;
	let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
		label: Some("Block Shader Bind Group Layout"),
		entries: &[
			binding_thingies.camera_matrix_thingy.layout_entry(0, S::VERTEX),
			binding_thingies.sun_light_direction_thingy.layout_entry(1, S::VERTEX),
			binding_thingies.sun_camera_matrices_thingy.layout_entry(2, S::FRAGMENT),
			binding_thingies.shadow_map_view_thingy.layout_entry(3, S::FRAGMENT),
			binding_thingies.shadow_map_sampler_thingy.layout_entry(4, S::FRAGMENT),
			binding_thingies.atlas_texture_view_thingy.layout_entry(5, S::FRAGMENT),
			binding_thingies.atlas_texture_sampler_thingy.layout_entry(6, S::FRAGMENT),
			binding_thingies.fog_center_position_thingy.layout_entry(7, S::FRAGMENT),
			binding_thingies.fog_inf_sup_radiuses_thingy.layout_entry(8, S::FRAGMENT),
		],
	});
	let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
		label: Some("Block Shader Bind Group"),
		layout: &bind_group_layout,
		entries: &[
			binding_thingies.camera_matrix_thingy.bind_group_entry(0),
			binding_thingies.sun_light_direction_thingy.bind_group_entry(1),
			binding_thingies.sun_camera_matrices_thingy.bind_group_entry(2),
			binding_thingies.shadow_map_view_thingy.bind_group_entry(3),
			binding_thingies.shadow_map_sampler_thingy.bind_group_entry(4),
			binding_thingies.atlas_texture_view_thingy.bind_group_entry(5),
			binding_thingies.atlas_texture_sampler_thingy.bind_group_entry(6),
			binding_thingies.fog_center_position_thingy.bind_group_entry(7),
			binding_thingies.fog_inf_sup_radiuses_thingy.bind_group_entry(8),
		],
	});

	let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
		label: Some("Block Shader"),
		source: wgpu::ShaderSource::Wgsl(include_str!("block.wgsl").into()),
	});
	let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
		label: Some("Block Render Pipeline Layout"),
		bind_group_layouts: &[&bind_group_layout],
		push_constant_ranges: &[],
	});

	let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
		label: Some("Block Render Pipeline"),
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
				// The blocks can get trasparent when far away to create a fog transparency effect
				// that blends in the skybox. It sould only blend in the skybox though, not with blocks
				// behind them, so here we do not do any alpha blending so that blocks do not blend
				// with other blocks, and then the skybox will do the blending in reverse to draw
				// itself behind the blocks.
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
