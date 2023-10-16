use std::sync::Arc;

use bytemuck::Zeroable;
use wgpu::util::DeviceExt;

use crate::{camera::Matrix4x4Pod, shaders, Vector3Pod};

/// Type representation for the `ty` and `count` fields of a `wgpu::BindGroupLayoutEntry`.
pub struct BindingType {
	pub ty: wgpu::BindingType,
	pub count: Option<std::num::NonZeroU32>,
}

impl BindingType {
	pub fn layout_entry(
		&self,
		binding: u32,
		visibility: wgpu::ShaderStages,
	) -> wgpu::BindGroupLayoutEntry {
		wgpu::BindGroupLayoutEntry { binding, visibility, ty: self.ty, count: self.count }
	}
}

pub trait BindingResourceable {
	fn as_binding_resource(&self) -> wgpu::BindingResource;
}
impl BindingResourceable for wgpu::Buffer {
	fn as_binding_resource(&self) -> wgpu::BindingResource {
		self.as_entire_binding()
	}
}
impl BindingResourceable for wgpu::TextureView {
	fn as_binding_resource(&self) -> wgpu::BindingResource {
		wgpu::BindingResource::TextureView(self)
	}
}
impl BindingResourceable for wgpu::Sampler {
	fn as_binding_resource(&self) -> wgpu::BindingResource {
		wgpu::BindingResource::Sampler(self)
	}
}

/// Resource and associated information required for creations of both
/// a `wgpu::BindGroupLayoutEntry` and a `wgpu::BindGroupEntry`.
pub struct BindingThingy<T: BindingResourceable> {
	pub binding_type: BindingType,
	pub resource: T,
}

pub fn make_z_buffer_texture_view(
	device: &wgpu::Device,
	format: wgpu::TextureFormat,
	width: u32,
	height: u32,
) -> wgpu::TextureView {
	let z_buffer_texture_description = wgpu::TextureDescriptor {
		label: Some("Z Buffer"),
		size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format,
		view_formats: &[],
		usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
	};
	let z_buffer_texture = device.create_texture(&z_buffer_texture_description);
	z_buffer_texture.create_view(&wgpu::TextureViewDescriptor::default())
}

pub struct RenderPipelinesAndBindGroups {
	pub block_shadow_render_pipeline: wgpu::RenderPipeline,
	pub block_shadow_bind_group: wgpu::BindGroup,
	pub block_render_pipeline: wgpu::RenderPipeline,
	pub block_bind_group: wgpu::BindGroup,
	pub simple_line_render_pipeline: wgpu::RenderPipeline,
	pub simple_line_render_bind_group: wgpu::BindGroup,
	pub simple_line_2d_render_pipeline: wgpu::RenderPipeline,
}

pub struct AllBindingThingies<'a> {
	pub(crate) camera_matrix_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) sun_light_direction_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) sun_camera_matrix_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) shadow_map_view_thingy: &'a BindingThingy<wgpu::TextureView>,
	pub(crate) shadow_map_sampler_thingy: &'a BindingThingy<wgpu::Sampler>,
	pub(crate) atlas_texture_view_thingy: &'a BindingThingy<wgpu::TextureView>,
	pub(crate) atlas_texture_sampler_thingy: &'a BindingThingy<wgpu::Sampler>,
}

pub fn init_rendering_stuff(
	device: Arc<wgpu::Device>,
	all_binding_thingies: AllBindingThingies,
	shadow_map_format: wgpu::TextureFormat,
	window_surface_format: wgpu::TextureFormat,
	z_buffer_format: wgpu::TextureFormat,
) -> RenderPipelinesAndBindGroups {
	let (block_shadow_render_pipeline, block_shadow_bind_group) =
		shaders::block_shadow::render_pipeline_and_bind_group(
			&device,
			shaders::block_shadow::BindingThingies {
				sun_camera_matrix_thingy: all_binding_thingies.sun_camera_matrix_thingy,
			},
			shadow_map_format,
		);

	let (block_render_pipeline, block_bind_group) = shaders::block::render_pipeline_and_bind_group(
		&device,
		shaders::block::BindingThingies {
			camera_matrix_thingy: all_binding_thingies.camera_matrix_thingy,
			sun_light_direction_thingy: all_binding_thingies.sun_light_direction_thingy,
			sun_camera_matrix_thingy: all_binding_thingies.sun_camera_matrix_thingy,
			shadow_map_view_thingy: all_binding_thingies.shadow_map_view_thingy,
			shadow_map_sampler_thingy: all_binding_thingies.shadow_map_sampler_thingy,
			atlas_texture_view_thingy: all_binding_thingies.atlas_texture_view_thingy,
			atlas_texture_sampler_thingy: all_binding_thingies.atlas_texture_sampler_thingy,
		},
		window_surface_format,
		z_buffer_format,
	);

	let (simple_line_render_pipeline, simple_line_render_bind_group) =
		shaders::simple_line::render_pipeline_and_bind_group(
			&device,
			shaders::simple_line::BindingThingies {
				camera_matrix_thingy: all_binding_thingies.camera_matrix_thingy,
			},
			window_surface_format,
			z_buffer_format,
		);

	let simple_line_2d_render_pipeline = shaders::simple_line_2d::render_pipeline(
		&device,
		shaders::simple_line_2d::BindingThingies {},
		window_surface_format,
		z_buffer_format,
	);

	RenderPipelinesAndBindGroups {
		block_shadow_render_pipeline,
		block_shadow_bind_group,
		block_render_pipeline,
		block_bind_group,
		simple_line_render_pipeline,
		simple_line_render_bind_group,
		simple_line_2d_render_pipeline,
	}
}

pub struct ShadowMapStuff {
	pub shadow_map_format: wgpu::TextureFormat,
	pub shadow_map_view_thingy: BindingThingy<wgpu::TextureView>,
	pub shadow_map_sampler_thingy: BindingThingy<wgpu::Sampler>,
}
pub fn init_shadow_map_stuff(device: Arc<wgpu::Device>) -> ShadowMapStuff {
	let shadow_map_format = wgpu::TextureFormat::Depth32Float;
	let shadow_map_texture = device.create_texture(&wgpu::TextureDescriptor {
		label: Some("Shadow Map Texture"),
		size: wgpu::Extent3d { width: 8192, height: 8192, depth_or_array_layers: 1 },
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: shadow_map_format,
		usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
		view_formats: &[],
	});
	let shadow_map_view = shadow_map_texture.create_view(&wgpu::TextureViewDescriptor::default());
	let shadow_map_view_binding_type = BindingType {
		ty: wgpu::BindingType::Texture {
			sample_type: wgpu::TextureSampleType::Depth,
			view_dimension: wgpu::TextureViewDimension::D2,
			multisampled: false,
		},
		count: None,
	};
	let shadow_map_view_thingy = BindingThingy {
		binding_type: shadow_map_view_binding_type,
		resource: shadow_map_view,
	};
	let shadow_map_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
		label: Some("Shadow Map Sampler"),
		address_mode_u: wgpu::AddressMode::ClampToEdge,
		address_mode_v: wgpu::AddressMode::ClampToEdge,
		address_mode_w: wgpu::AddressMode::ClampToEdge,
		mag_filter: wgpu::FilterMode::Linear,
		min_filter: wgpu::FilterMode::Linear,
		mipmap_filter: wgpu::FilterMode::Nearest,
		compare: Some(wgpu::CompareFunction::LessEqual),
		..Default::default()
	});
	let shadow_map_sampler_binding_type = BindingType {
		ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
		count: None,
	};
	let shadow_map_sampler_thingy = BindingThingy {
		binding_type: shadow_map_sampler_binding_type,
		resource: shadow_map_sampler,
	};

	ShadowMapStuff {
		shadow_map_format,
		shadow_map_view_thingy,
		shadow_map_sampler_thingy,
	}
}

pub fn init_sun_camera_matrix_thingy(device: Arc<wgpu::Device>) -> BindingThingy<wgpu::Buffer> {
	let sun_camera_matrix_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Sun Camera Buffer"),
		contents: bytemuck::cast_slice(&[Matrix4x4Pod::zeroed()]),
		usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
	});
	let sun_camera_matrix_binding_type = BindingType {
		ty: wgpu::BindingType::Buffer {
			ty: wgpu::BufferBindingType::Uniform,
			has_dynamic_offset: false,
			min_binding_size: None,
		},
		count: None,
	};
	BindingThingy {
		binding_type: sun_camera_matrix_binding_type,
		resource: sun_camera_matrix_buffer,
	}
}

pub fn init_sun_light_direction_thingy(device: Arc<wgpu::Device>) -> BindingThingy<wgpu::Buffer> {
	let sun_light_direction_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Sun Light Direction Buffer"),
		contents: bytemuck::cast_slice(&[Vector3Pod::zeroed()]),
		usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
	});
	let sun_light_direction_binding_type = BindingType {
		ty: wgpu::BindingType::Buffer {
			ty: wgpu::BufferBindingType::Uniform,
			has_dynamic_offset: false,
			min_binding_size: None,
		},
		count: None,
	};
	BindingThingy {
		binding_type: sun_light_direction_binding_type,
		resource: sun_light_direction_buffer,
	}
}

pub fn init_camera_matrix_thingy(device: Arc<wgpu::Device>) -> BindingThingy<wgpu::Buffer> {
	let camera_matrix_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Camera Buffer"),
		contents: bytemuck::cast_slice(&[Matrix4x4Pod::zeroed()]),
		usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
	});
	let camera_matrix_binding_type = BindingType {
		ty: wgpu::BindingType::Buffer {
			ty: wgpu::BufferBindingType::Uniform,
			has_dynamic_offset: false,
			min_binding_size: None,
		},
		count: None,
	};
	BindingThingy {
		binding_type: camera_matrix_binding_type,
		resource: camera_matrix_buffer,
	}
}

pub const ATLAS_DIMS: (usize, usize) = (512, 512);

pub struct AtlasStuff {
	pub atlas_texture_view_thingy: BindingThingy<wgpu::TextureView>,
	pub atlas_texture_sampler_thingy: BindingThingy<wgpu::Sampler>,
}
pub fn init_atlas_stuff(
	device: Arc<wgpu::Device>,
	queue: &wgpu::Queue,
	atlas_data: &[u8; 4 * ATLAS_DIMS.0 * ATLAS_DIMS.1],
) -> AtlasStuff {
	let atlas_texture_size = wgpu::Extent3d {
		width: ATLAS_DIMS.0 as u32,
		height: ATLAS_DIMS.1 as u32,
		depth_or_array_layers: 1,
	};
	let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
		label: Some("Atlas Texture"),
		size: atlas_texture_size,
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: wgpu::TextureFormat::Rgba8UnormSrgb,
		usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
		view_formats: &[],
	});
	queue.write_texture(
		wgpu::ImageCopyTexture {
			texture: &atlas_texture,
			mip_level: 0,
			origin: wgpu::Origin3d::ZERO,
			aspect: wgpu::TextureAspect::All,
		},
		atlas_data,
		wgpu::ImageDataLayout {
			offset: 0,
			bytes_per_row: Some(4 * ATLAS_DIMS.0 as u32),
			rows_per_image: Some(ATLAS_DIMS.1 as u32),
		},
		atlas_texture_size,
	);
	let atlas_texture_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
	let atlas_texture_view_binding_type = BindingType {
		ty: wgpu::BindingType::Texture {
			multisampled: false,
			view_dimension: wgpu::TextureViewDimension::D2,
			sample_type: wgpu::TextureSampleType::Float { filterable: true },
		},
		count: None,
	};
	let atlas_texture_view_thingy = BindingThingy {
		binding_type: atlas_texture_view_binding_type,
		resource: atlas_texture_view,
	};
	let atlas_texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
		address_mode_u: wgpu::AddressMode::ClampToEdge,
		address_mode_v: wgpu::AddressMode::ClampToEdge,
		address_mode_w: wgpu::AddressMode::ClampToEdge,
		mag_filter: wgpu::FilterMode::Nearest,
		min_filter: wgpu::FilterMode::Nearest,
		mipmap_filter: wgpu::FilterMode::Nearest,
		..Default::default()
	});
	let atlas_texture_sampler_binding_type = BindingType {
		ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
		count: None,
	};
	let atlas_texture_sampler_thingy = BindingThingy {
		binding_type: atlas_texture_sampler_binding_type,
		resource: atlas_texture_sampler,
	};

	AtlasStuff { atlas_texture_view_thingy, atlas_texture_sampler_thingy }
}
