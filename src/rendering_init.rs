use std::mem::size_of;
use std::sync::Arc;

use bytemuck::Zeroable;
use wgpu::util::DeviceExt;

use crate::shaders::Vector2Pod;
use crate::{camera::Matrix4x4Pod, shaders, Vector3Pod};

/// Type representation for the `ty` and `count` fields of a `wgpu::BindGroupLayoutEntry`.
#[derive(Clone)]
pub(crate) struct BindingType {
	pub(crate) ty: wgpu::BindingType,
	pub(crate) count: Option<std::num::NonZeroU32>,
}

impl BindingType {
	pub(crate) fn layout_entry(
		&self,
		binding: u32,
		visibility: wgpu::ShaderStages,
	) -> wgpu::BindGroupLayoutEntry {
		wgpu::BindGroupLayoutEntry { binding, visibility, ty: self.ty, count: self.count }
	}
}

/// Can be a `wgpu::BindingResource`.
pub(crate) trait AsBindingResource {
	fn as_binding_resource(&self) -> wgpu::BindingResource;
}
impl AsBindingResource for wgpu::Buffer {
	fn as_binding_resource(&self) -> wgpu::BindingResource {
		self.as_entire_binding()
	}
}
impl AsBindingResource for wgpu::TextureView {
	fn as_binding_resource(&self) -> wgpu::BindingResource {
		wgpu::BindingResource::TextureView(self)
	}
}
impl AsBindingResource for wgpu::Sampler {
	fn as_binding_resource(&self) -> wgpu::BindingResource {
		wgpu::BindingResource::Sampler(self)
	}
}

/// Resource and associated information required for creations of both
/// a `wgpu::BindGroupLayoutEntry` and a `wgpu::BindGroupEntry`.
pub(crate) struct BindingThingy<T: AsBindingResource> {
	pub(crate) binding_type: BindingType,
	pub(crate) resource: T,
}

impl<T: AsBindingResource> BindingThingy<T> {
	pub(crate) fn layout_entry(
		&self,
		binding: u32,
		visibility: wgpu::ShaderStages,
	) -> wgpu::BindGroupLayoutEntry {
		self.binding_type.layout_entry(binding, visibility)
	}

	pub(crate) fn bind_group_entry(&self, binding: u32) -> wgpu::BindGroupEntry {
		wgpu::BindGroupEntry { binding, resource: self.resource.as_binding_resource() }
	}
}

pub(crate) fn make_z_buffer_texture_view(
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

pub(crate) struct RenderPipelinesAndBindGroups {
	pub(crate) block_shadow_render_pipeline: wgpu::RenderPipeline,
	pub(crate) block_shadow_bind_group: wgpu::BindGroup,
	pub(crate) block_render_pipeline: wgpu::RenderPipeline,
	pub(crate) block_bind_group: wgpu::BindGroup,
	pub(crate) simple_line_render_pipeline: wgpu::RenderPipeline,
	pub(crate) simple_line_bind_group: wgpu::BindGroup,
	pub(crate) simple_line_2d_render_pipeline: wgpu::RenderPipeline,
	pub(crate) simple_line_2d_bind_group: wgpu::BindGroup,
	pub(crate) simple_texture_2d_render_pipeline: wgpu::RenderPipeline,
	pub(crate) simple_texture_2d_bind_group: wgpu::BindGroup,
	pub(crate) skybox_render_pipeline: wgpu::RenderPipeline,
	pub(crate) skybox_bind_group: wgpu::BindGroup,
}

pub(crate) struct AllBindingThingies<'a> {
	pub(crate) aspect_ratio_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) camera_matrix_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) sun_light_direction_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) sun_camera_matrices_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) sun_camera_single_matrix_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) shadow_map_view_thingy: &'a BindingThingy<wgpu::TextureView>,
	pub(crate) shadow_map_sampler_thingy: &'a BindingThingy<wgpu::Sampler>,
	pub(crate) atlas_texture_view_thingy: &'a BindingThingy<wgpu::TextureView>,
	pub(crate) atlas_texture_sampler_thingy: &'a BindingThingy<wgpu::Sampler>,
	pub(crate) skybox_cubemap_texture_view_thingy: &'a BindingThingy<wgpu::TextureView>,
	pub(crate) skybox_cubemap_texture_sampler_thingy: &'a BindingThingy<wgpu::Sampler>,
	pub(crate) fog_center_position_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) fog_inf_sup_radiuses_thingy: &'a BindingThingy<wgpu::Buffer>,
}

pub(crate) fn init_rendering_stuff(
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
				sun_camera_single_matrix_thingy: all_binding_thingies.sun_camera_single_matrix_thingy,
				atlas_texture_view_thingy: all_binding_thingies.atlas_texture_view_thingy,
				atlas_texture_sampler_thingy: all_binding_thingies.atlas_texture_sampler_thingy,
			},
			shadow_map_format,
		);

	let (block_render_pipeline, block_bind_group) = shaders::block::render_pipeline_and_bind_group(
		&device,
		shaders::block::BindingThingies {
			camera_matrix_thingy: all_binding_thingies.camera_matrix_thingy,
			sun_light_direction_thingy: all_binding_thingies.sun_light_direction_thingy,
			sun_camera_matrices_thingy: all_binding_thingies.sun_camera_matrices_thingy,
			shadow_map_view_thingy: all_binding_thingies.shadow_map_view_thingy,
			shadow_map_sampler_thingy: all_binding_thingies.shadow_map_sampler_thingy,
			atlas_texture_view_thingy: all_binding_thingies.atlas_texture_view_thingy,
			atlas_texture_sampler_thingy: all_binding_thingies.atlas_texture_sampler_thingy,
			fog_center_position_thingy: all_binding_thingies.fog_center_position_thingy,
			fog_inf_sup_radiuses_thingy: all_binding_thingies.fog_inf_sup_radiuses_thingy,
		},
		window_surface_format,
		z_buffer_format,
	);

	let (simple_line_render_pipeline, simple_line_bind_group) =
		shaders::simple_line::render_pipeline_and_bind_group(
			&device,
			shaders::simple_line::BindingThingies {
				camera_matrix_thingy: all_binding_thingies.camera_matrix_thingy,
			},
			window_surface_format,
			z_buffer_format,
		);

	let (simple_line_2d_render_pipeline, simple_line_2d_bind_group) =
		shaders::simple_line_2d::render_pipeline(
			&device,
			shaders::simple_line_2d::BindingThingies {
				aspect_ratio_thingy: all_binding_thingies.aspect_ratio_thingy,
			},
			window_surface_format,
			z_buffer_format,
		);

	let (simple_texture_2d_render_pipeline, simple_texture_2d_bind_group) =
		shaders::simple_texture_2d::render_pipeline(
			&device,
			shaders::simple_texture_2d::BindingThingies {
				aspect_ratio_thingy: all_binding_thingies.aspect_ratio_thingy,
				atlas_texture_view_thingy: all_binding_thingies.atlas_texture_view_thingy,
				atlas_texture_sampler_thingy: all_binding_thingies.atlas_texture_sampler_thingy,
			},
			window_surface_format,
			z_buffer_format,
		);

	let (skybox_render_pipeline, skybox_bind_group) =
		shaders::skybox::render_pipeline_and_bind_group(
			&device,
			shaders::skybox::BindingThingies {
				camera_matrix_thingy: all_binding_thingies.camera_matrix_thingy,
				skybox_cubemap_texture_view_thingy: all_binding_thingies
					.skybox_cubemap_texture_view_thingy,
				skybox_cubemap_texture_sampler_thingy: all_binding_thingies
					.skybox_cubemap_texture_sampler_thingy,
			},
			window_surface_format,
		);

	RenderPipelinesAndBindGroups {
		block_shadow_render_pipeline,
		block_shadow_bind_group,
		block_render_pipeline,
		block_bind_group,
		simple_line_render_pipeline,
		simple_line_bind_group,
		simple_line_2d_render_pipeline,
		simple_line_2d_bind_group,
		simple_texture_2d_render_pipeline,
		simple_texture_2d_bind_group,
		skybox_render_pipeline,
		skybox_bind_group,
	}
}

pub(crate) fn init_aspect_ratio_thingy(device: Arc<wgpu::Device>) -> BindingThingy<wgpu::Buffer> {
	let aspect_ratio_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Aspect Ratio Buffer"),
		contents: bytemuck::cast_slice(&[f32::zeroed()]),
		usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
	});
	let aspect_ratio_binding_type = BindingType {
		ty: wgpu::BindingType::Buffer {
			ty: wgpu::BufferBindingType::Uniform,
			has_dynamic_offset: false,
			min_binding_size: None,
		},
		count: None,
	};
	BindingThingy {
		binding_type: aspect_ratio_binding_type,
		resource: aspect_ratio_buffer,
	}
}

pub(crate) struct ShadowMapStuff {
	pub(crate) shadow_map_format: wgpu::TextureFormat,
	/// View on the whole texture array, all cascades included.
	pub(crate) shadow_map_view_thingy: BindingThingy<wgpu::TextureView>,
	pub(crate) shadow_map_sampler_thingy: BindingThingy<wgpu::Sampler>,
	/// Views on each of the textures of the array, every cascade texture gets its own view.
	pub(crate) shadow_map_cascade_view_thingies: Vec<BindingThingy<wgpu::TextureView>>,
}
pub(crate) fn init_shadow_map_stuff(
	device: Arc<wgpu::Device>,
	shadow_map_cascade_count: u32,
) -> ShadowMapStuff {
	let shadow_map_format = wgpu::TextureFormat::Depth32Float;
	let shadow_map_texture = device.create_texture(&wgpu::TextureDescriptor {
		label: Some("Shadow Map Texture"),
		size: wgpu::Extent3d {
			width: 8192,
			height: 8192,
			depth_or_array_layers: shadow_map_cascade_count,
		},
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
			view_dimension: wgpu::TextureViewDimension::D2Array,
			multisampled: false,
		},
		count: None,
	};
	let shadow_map_view_thingy = BindingThingy {
		binding_type: shadow_map_view_binding_type.clone(),
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

	let shadow_map_cascade_views: Vec<_> = (0..shadow_map_cascade_count)
		.map(|cascade_index| {
			shadow_map_texture.create_view(&wgpu::TextureViewDescriptor {
				dimension: Some(wgpu::TextureViewDimension::D2),
				base_array_layer: cascade_index,
				array_layer_count: Some(1),
				..wgpu::TextureViewDescriptor::default()
			})
		})
		.collect();
	let shadow_map_cascade_view_thingies = shadow_map_cascade_views
		.into_iter()
		.map(|shadow_map_cascade_view| BindingThingy {
			binding_type: shadow_map_view_binding_type.clone(),
			resource: shadow_map_cascade_view,
		})
		.collect();

	ShadowMapStuff {
		shadow_map_format,
		shadow_map_view_thingy,
		shadow_map_sampler_thingy,
		shadow_map_cascade_view_thingies,
	}
}

pub(crate) struct SunCameraStuff {
	pub(crate) sun_camera_matrices_thingy: BindingThingy<wgpu::Buffer>,
	pub(crate) sun_camera_single_matrix_thingy: BindingThingy<wgpu::Buffer>,
}
pub(crate) fn init_sun_camera_matrices_thingy(
	device: Arc<wgpu::Device>,
	shadow_map_cascade_count: u32,
) -> SunCameraStuff {
	let sun_camera_matrices_buffer = device.create_buffer(&wgpu::BufferDescriptor {
		label: Some("Sun Camera Matrices Buffer"),
		size: size_of::<Matrix4x4Pod>() as u64 * shadow_map_cascade_count as u64,
		usage: wgpu::BufferUsages::STORAGE
			| wgpu::BufferUsages::COPY_DST
			| wgpu::BufferUsages::COPY_SRC,
		mapped_at_creation: false,
	});
	let sun_camera_matrices_binding_type = BindingType {
		ty: wgpu::BindingType::Buffer {
			ty: wgpu::BufferBindingType::Storage { read_only: true },
			has_dynamic_offset: false,
			min_binding_size: None,
		},
		count: None,
	};
	let sun_camera_matrices_binding_thingy = BindingThingy {
		binding_type: sun_camera_matrices_binding_type,
		resource: sun_camera_matrices_buffer,
	};

	let sun_camera_single_matrix_buffer = device.create_buffer(&wgpu::BufferDescriptor {
		label: Some("Sun Camera Single Matrix Buffer"),
		size: size_of::<Matrix4x4Pod>() as u64,
		usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		mapped_at_creation: false,
	});
	let sun_camera_single_matrix_binding_type = BindingType {
		ty: wgpu::BindingType::Buffer {
			ty: wgpu::BufferBindingType::Uniform,
			has_dynamic_offset: false,
			min_binding_size: None,
		},
		count: None,
	};
	let sun_camera_single_matrix_binding_thingy = BindingThingy {
		binding_type: sun_camera_single_matrix_binding_type,
		resource: sun_camera_single_matrix_buffer,
	};

	SunCameraStuff {
		sun_camera_matrices_thingy: sun_camera_matrices_binding_thingy,
		sun_camera_single_matrix_thingy: sun_camera_single_matrix_binding_thingy,
	}
}

pub(crate) fn init_sun_light_direction_thingy(
	device: Arc<wgpu::Device>,
) -> BindingThingy<wgpu::Buffer> {
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

pub(crate) fn init_camera_matrix_thingy(device: Arc<wgpu::Device>) -> BindingThingy<wgpu::Buffer> {
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

use crate::atlas::ATLAS_DIMS;

pub(crate) struct AtlasStuff {
	pub(crate) atlas_texture_view_thingy: BindingThingy<wgpu::TextureView>,
	pub(crate) atlas_texture_sampler_thingy: BindingThingy<wgpu::Sampler>,
	pub(crate) atlas_texture: wgpu::Texture,
}
pub(crate) fn init_atlas_stuff(
	device: Arc<wgpu::Device>,
	queue: &wgpu::Queue,
	atlas_data: &[u8],
) -> AtlasStuff {
	assert_eq!(atlas_data.len(), 4 * ATLAS_DIMS.0 * ATLAS_DIMS.1);

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

	AtlasStuff {
		atlas_texture_view_thingy,
		atlas_texture_sampler_thingy,
		atlas_texture,
	}
}

pub(crate) type AtlasData<'a> = &'a [u8];
pub(crate) fn update_atlas_texture(
	queue: &wgpu::Queue,
	atlas_texture: &wgpu::Texture,
	atlas_data: &AtlasData,
) {
	queue.write_texture(
		wgpu::ImageCopyTexture {
			texture: atlas_texture,
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
		wgpu::Extent3d {
			width: ATLAS_DIMS.0 as u32,
			height: ATLAS_DIMS.1 as u32,
			depth_or_array_layers: 1,
		},
	);
}

use crate::skybox::SKYBOX_SIDE_DIMS;

pub(crate) struct SkyboxStuff {
	pub(crate) skybox_cubemap_texture_view_thingy: BindingThingy<wgpu::TextureView>,
	pub(crate) skybox_cubemap_texture_sampler_thingy: BindingThingy<wgpu::Sampler>,
	pub(crate) skybox_cubemap_texture: wgpu::Texture,
}
pub(crate) fn init_skybox_stuff(
	device: Arc<wgpu::Device>,
	queue: &wgpu::Queue,
	skybox_data: &SkyboxData,
) -> SkyboxStuff {
	for face_data in skybox_data {
		assert_eq!(face_data.len(), 4 * SKYBOX_SIDE_DIMS.0 * SKYBOX_SIDE_DIMS.1);
	}

	let skybox_cubemap_texture_size = wgpu::Extent3d {
		width: SKYBOX_SIDE_DIMS.0 as u32,
		height: SKYBOX_SIDE_DIMS.1 as u32,
		depth_or_array_layers: 6,
	};
	let skybox_cubemap_texture = device.create_texture(&wgpu::TextureDescriptor {
		label: Some("Skybox Cubemap Texture"),
		size: skybox_cubemap_texture_size,
		mip_level_count: 1,
		sample_count: 1,
		dimension: wgpu::TextureDimension::D2,
		format: wgpu::TextureFormat::Rgba8UnormSrgb,
		usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
		view_formats: &[],
	});
	update_skybox_texture(queue, &skybox_cubemap_texture, skybox_data);
	let skybox_cubemap_texture_view =
		skybox_cubemap_texture.create_view(&wgpu::TextureViewDescriptor {
			label: Some("Skybox Cubemap Texture View"),
			dimension: Some(wgpu::TextureViewDimension::Cube),
			..Default::default()
		});
	let skybox_cubemap_texture_view_binding_type = BindingType {
		ty: wgpu::BindingType::Texture {
			multisampled: false,
			view_dimension: wgpu::TextureViewDimension::Cube,
			sample_type: wgpu::TextureSampleType::Float { filterable: true },
		},
		count: None, // I mean there is only one cube and we already said it is a cube in `ty`..
	};
	let skybox_cubemap_texture_view_thingy = BindingThingy {
		binding_type: skybox_cubemap_texture_view_binding_type,
		resource: skybox_cubemap_texture_view,
	};
	let skybox_cubemap_texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
		label: Some("Skybox Cubemap Texture Sampler"),
		address_mode_u: wgpu::AddressMode::ClampToEdge,
		address_mode_v: wgpu::AddressMode::ClampToEdge,
		address_mode_w: wgpu::AddressMode::ClampToEdge,
		mag_filter: wgpu::FilterMode::Nearest,
		min_filter: wgpu::FilterMode::Nearest,
		mipmap_filter: wgpu::FilterMode::Nearest,
		..Default::default()
	});
	let skybox_cubemap_texture_sampler_binding_type = BindingType {
		ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
		count: None,
	};
	let skybox_cubemap_texture_sampler_thingy = BindingThingy {
		binding_type: skybox_cubemap_texture_sampler_binding_type,
		resource: skybox_cubemap_texture_sampler,
	};

	SkyboxStuff {
		skybox_cubemap_texture_view_thingy,
		skybox_cubemap_texture_sampler_thingy,
		skybox_cubemap_texture,
	}
}

pub(crate) type SkyboxData<'a> = [&'a [u8]; 6];
pub(crate) fn update_skybox_texture(
	queue: &wgpu::Queue,
	skybox_cubemap_texture: &wgpu::Texture,
	skybox_data: &SkyboxData,
) {
	for (face_index, face_data) in skybox_data.iter().enumerate() {
		queue.write_texture(
			wgpu::ImageCopyTexture {
				texture: skybox_cubemap_texture,
				mip_level: 0,
				origin: wgpu::Origin3d { x: 0, y: 0, z: face_index as u32 },
				aspect: wgpu::TextureAspect::All,
			},
			face_data,
			wgpu::ImageDataLayout {
				offset: 0,
				bytes_per_row: Some(4 * SKYBOX_SIDE_DIMS.0 as u32),
				rows_per_image: Some(SKYBOX_SIDE_DIMS.1 as u32),
			},
			wgpu::Extent3d {
				width: SKYBOX_SIDE_DIMS.0 as u32,
				height: SKYBOX_SIDE_DIMS.1 as u32,
				depth_or_array_layers: 1,
			},
		);
	}
}

pub(crate) struct FogStuff {
	pub(crate) fog_center_position_thingy: BindingThingy<wgpu::Buffer>,
	pub(crate) fog_inf_sup_radiuses_thingy: BindingThingy<wgpu::Buffer>,
}
pub(crate) fn init_fog_stuff(device: Arc<wgpu::Device>) -> FogStuff {
	let fog_center_position_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Fog Center Position Buffer"),
		contents: bytemuck::cast_slice(&[Vector3Pod::zeroed()]),
		usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
	});
	let fog_center_position_binding_type = BindingType {
		ty: wgpu::BindingType::Buffer {
			ty: wgpu::BufferBindingType::Uniform,
			has_dynamic_offset: false,
			min_binding_size: None,
		},
		count: None,
	};
	let fog_center_position_thingy = BindingThingy {
		binding_type: fog_center_position_binding_type,
		resource: fog_center_position_buffer,
	};

	let fog_inf_sup_radiuses_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Fog Inf & Sup Radiuses Buffer"),
		contents: bytemuck::cast_slice(&[Vector2Pod::zeroed()]),
		usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
	});
	let fog_inf_sup_radiuses_binding_type = BindingType {
		ty: wgpu::BindingType::Buffer {
			ty: wgpu::BufferBindingType::Uniform,
			has_dynamic_offset: false,
			min_binding_size: None,
		},
		count: None,
	};
	let fog_inf_sup_radiuses_thingy = BindingThingy {
		binding_type: fog_inf_sup_radiuses_binding_type,
		resource: fog_inf_sup_radiuses_buffer,
	};

	FogStuff { fog_center_position_thingy, fog_inf_sup_radiuses_thingy }
}
