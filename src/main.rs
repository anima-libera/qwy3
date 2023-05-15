use std::f32::consts::TAU;

use bytemuck::Zeroable;
use wgpu::util::DeviceExt;
use winit::{
	event_loop::{ControlFlow, EventLoop},
	window::WindowBuilder,
};

struct CameraPerspectiveConfig {
	up_direction: cgmath::Vector3<f32>,
	/// Width / height.
	aspect_ratio: f32,
	/// Angle (unsigned, in radians) of view on the vertical axis, "fovy".
	field_of_view_y: f32,
	near_plane: f32,
	far_plane: f32,
}

impl CameraPerspectiveConfig {
	/// Get that view projection matrix that can be sent to the GPU.
	fn view_projection_matrix(
		&self,
		position: cgmath::Point3<f32>,
		target: cgmath::Point3<f32>,
	) -> Matrix4x4Pod {
		let view_matrix = cgmath::Matrix4::look_at_rh(position, target, self.up_direction);
		let projection_matrix = cgmath::perspective(
			cgmath::Rad(self.field_of_view_y),
			self.aspect_ratio,
			self.near_plane,
			self.far_plane,
		);
		let view_projection_matrix = projection_matrix * view_matrix;

		// (https://sotrh.github.io/learn-wgpu/beginner/tutorial6-uniforms/#a-perspective-camera)
		// suggests to use this `OPENGL_TO_WGPU_MATRIX` transformation to account for the fact that
		// in OpenGL the view projection transformation should get the frustum to fit in the cube
		// from (-1, -1, -1) to (1, 1, 1), but in Wgpu the frustum should fit in the rectangular
		// area from (-1, -1, 0) to (1, 1, 1). The difference is that on the Z axis (depth) the
		// range is not (-1, 1) but instead is (0, 1).
		// `cgmath` assumes OpenGL-like conventions and here we correct these assumptions to Wgpu.
		#[rustfmt::skip]
		pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
			1.0, 0.0, 0.0, 0.0,
			0.0, 1.0, 0.0, 0.0,
			0.0, 0.0, 0.5, 0.0,
			0.0, 0.0, 0.5, 1.0,
		);
		let view_projection_matrix = OPENGL_TO_WGPU_MATRIX * view_projection_matrix;

		Matrix4x4Pod { values: view_projection_matrix.into() }
	}
}

/// Matrix 4×4.
#[derive(Copy, Clone, Debug)]
/// Certified Plain Old Data (so it can be sent to the GPU as a uniform).
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
struct Matrix4x4Pod {
	values: [[f32; 4]; 4],
}

/// Vertex type used in chunk block meshes.
#[derive(Copy, Clone, Debug)]
/// Certified Plain Old Data (so it can be sent to the GPU as a uniform).
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
struct BlockVertexPod {
	position: [f32; 3],
	color: [f32; 3],
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NonOrientedAxis {
	X,
	Y,
	Z,
}
impl NonOrientedAxis {
	fn iter_over_the_three_possible_axes() -> impl Iterator<Item = NonOrientedAxis> {
		[NonOrientedAxis::X, NonOrientedAxis::Y, NonOrientedAxis::Z].into_iter()
	}
	fn index(self) -> usize {
		match self {
			NonOrientedAxis::X => 0,
			NonOrientedAxis::Y => 1,
			NonOrientedAxis::Z => 2,
		}
	}
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum AxisOrientation {
	Positivewards,
	Negativewards,
}
impl AxisOrientation {
	fn iter_over_the_two_possible_orientations() -> impl Iterator<Item = AxisOrientation> {
		[
			AxisOrientation::Positivewards,
			AxisOrientation::Negativewards,
		]
		.into_iter()
	}
	fn sign(self) -> i32 {
		match self {
			AxisOrientation::Positivewards => 1,
			AxisOrientation::Negativewards => -1,
		}
	}
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct OrientedAxis {
	axis: NonOrientedAxis,
	orientation: AxisOrientation,
}
impl OrientedAxis {
	fn all_the_six_possible_directions() -> impl Iterator<Item = OrientedAxis> {
		NonOrientedAxis::iter_over_the_three_possible_axes().flat_map(|axis| {
			AxisOrientation::iter_over_the_two_possible_orientations()
				.map(move |orientation| OrientedAxis { axis, orientation })
		})
	}
}

fn generate_face(
	vertices: &mut Vec<BlockVertexPod>,
	face_orientation: OrientedAxis,
	block_center: cgmath::Point3<f32>,
) {
	// NO EARLY OPTIMIZATION
	// This shall remain in an unoptimized, unfactorized and flexible state for now!
	let mut a: cgmath::Point3<f32> = block_center;
	let mut b: cgmath::Point3<f32> = block_center;
	let mut c: cgmath::Point3<f32> = block_center;
	let mut d: cgmath::Point3<f32> = block_center;
	a[face_orientation.axis.index()] += 0.5 * face_orientation.orientation.sign() as f32;
	b[face_orientation.axis.index()] += 0.5 * face_orientation.orientation.sign() as f32;
	c[face_orientation.axis.index()] += 0.5 * face_orientation.orientation.sign() as f32;
	d[face_orientation.axis.index()] += 0.5 * face_orientation.orientation.sign() as f32;
	let mut other_axes = [NonOrientedAxis::X, NonOrientedAxis::Y, NonOrientedAxis::Z]
		.into_iter()
		.filter(|&axis| axis != face_orientation.axis);
	let other_axis_a = other_axes.next().unwrap();
	let other_axis_b = other_axes.next().unwrap();
	assert!(other_axes.next().is_none());
	a[other_axis_a.index()] -= 0.5;
	a[other_axis_b.index()] -= 0.5;
	b[other_axis_a.index()] -= 0.5;
	b[other_axis_b.index()] += 0.5;
	c[other_axis_a.index()] += 0.5;
	c[other_axis_b.index()] -= 0.5;
	d[other_axis_a.index()] += 0.5;
	d[other_axis_b.index()] += 0.5;
	let reverse_order = match face_orientation.axis {
		NonOrientedAxis::X => face_orientation.orientation == AxisOrientation::Negativewards,
		NonOrientedAxis::Y => face_orientation.orientation == AxisOrientation::Positivewards,
		NonOrientedAxis::Z => face_orientation.orientation == AxisOrientation::Negativewards,
	};
	if !reverse_order {
		vertices.push(BlockVertexPod { position: a.into(), color: [1.0, 0.0, 0.0] });
		vertices.push(BlockVertexPod { position: c.into(), color: [0.0, 1.0, 0.0] });
		vertices.push(BlockVertexPod { position: b.into(), color: [1.0, 1.0, 0.0] });
		vertices.push(BlockVertexPod { position: b.into(), color: [1.0, 0.0, 1.0] });
		vertices.push(BlockVertexPod { position: c.into(), color: [0.0, 1.0, 1.0] });
		vertices.push(BlockVertexPod { position: d.into(), color: [1.0, 1.0, 1.0] });
	} else {
		vertices.push(BlockVertexPod { position: a.into(), color: [1.0, 0.0, 0.0] });
		vertices.push(BlockVertexPod { position: b.into(), color: [0.0, 1.0, 0.0] });
		vertices.push(BlockVertexPod { position: c.into(), color: [1.0, 1.0, 0.0] });
		vertices.push(BlockVertexPod { position: b.into(), color: [1.0, 0.0, 1.0] });
		vertices.push(BlockVertexPod { position: d.into(), color: [0.0, 1.0, 1.0] });
		vertices.push(BlockVertexPod { position: c.into(), color: [1.0, 1.0, 1.0] });
	}
}

/// Chunks are cubic parts of the world, all of the same size and arranged in a 3D grid.
/// The length (in blocks) of the edges of the chunks is not hardcoded. It can be
/// modified (to some extent) and passed around in a `ChunkDimensions`.
#[derive(Clone, Copy)]
struct ChunkDimensions {
	/// Length (in blocks) of the edge of each (cubic) chunk.
	edge: u32,
}
impl From<ChunkDimensions> for u32 {
	fn from(chunk_side: ChunkDimensions) -> u32 {
		chunk_side.edge
	}
}
impl From<u32> for ChunkDimensions {
	fn from(chunk_side_length: u32) -> ChunkDimensions {
		ChunkDimensions { edge: chunk_side_length }
	}
}
impl ChunkDimensions {
	fn number_of_blocks(self) -> usize {
		self.edge.pow(3) as usize
	}
}

#[derive(Clone, Copy)]
struct ChunkInternalBlockCoords {
	x: u32,
	y: u32,
	z: u32,
}
impl ChunkDimensions {
	fn iter_internal_block_coords(self) -> impl Iterator<Item = ChunkInternalBlockCoords> {
		(0..self.edge).flat_map(move |x| {
			(0..self.edge)
				.flat_map(move |y| (0..self.edge).map(move |z| ChunkInternalBlockCoords { x, y, z }))
		})
	}
	fn internal_index(self, internal_coords: ChunkInternalBlockCoords) -> usize {
		let ChunkInternalBlockCoords { x, y, z } = internal_coords;
		(z * self.edge.pow(2) + y * self.edge + x) as usize
	}
}

#[derive(Clone, Copy)]
struct BlockTypeId {
	is_not_air: bool,
}

struct ChunkBlocks {
	blocks: Vec<BlockTypeId>,
}
impl ChunkBlocks {
	fn new(cd: ChunkDimensions) -> ChunkBlocks {
		ChunkBlocks {
			blocks: Vec::from_iter(
				std::iter::repeat(BlockTypeId { is_not_air: false }).take(cd.number_of_blocks()),
			),
		}
	}
}

impl ChunkBlocks {
	fn internal_block_mut(
		&mut self,
		cd: ChunkDimensions,
		internal_coords: ChunkInternalBlockCoords,
	) -> &mut BlockTypeId {
		&mut self.blocks[cd.internal_index(internal_coords)]
	}

	fn internal_block(
		&self,
		cd: ChunkDimensions,
		internal_coords: ChunkInternalBlockCoords,
	) -> BlockTypeId {
		self.blocks[cd.internal_index(internal_coords)]
	}
}

impl ChunkBlocks {
	fn mesh(&self, cd: ChunkDimensions, vertices: &mut Vec<BlockVertexPod>) {
		for internal_coords in cd.iter_internal_block_coords() {
			if self.internal_block(cd, internal_coords).is_not_air {
				for direction in OrientedAxis::all_the_six_possible_directions() {
					let ChunkInternalBlockCoords { x, y, z } = internal_coords;
					generate_face(vertices, direction, (x as f32, y as f32, z as f32).into());
				}
			}
		}
	}
}

fn main() {
	env_logger::init();
	let event_loop = EventLoop::new();
	let window = WindowBuilder::new()
		.with_title("Qwy3")
		.build(&event_loop)
		.unwrap();
	let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
		backends: wgpu::Backends::all(),
		dx12_shader_compiler: Default::default(),
	});
	let window_surface = unsafe { instance.create_surface(&window) }.unwrap();

	// Try to get a cool adapter first.
	let adapter = instance
		.enumerate_adapters(wgpu::Backends::all())
		.find(|adapter| {
			let info = adapter.get_info();
			info.device_type == wgpu::DeviceType::DiscreteGpu
				&& adapter.is_surface_supported(&window_surface)
		});
	// In case we didn't found any cool adapter, at least we can try to get a bad adapter.
	let adapter = adapter.or_else(|| {
		futures::executor::block_on(async {
			instance
				.request_adapter(&wgpu::RequestAdapterOptions {
					power_preference: wgpu::PowerPreference::HighPerformance,
					compatible_surface: Some(&window_surface),
					force_fallback_adapter: false,
				})
				.await
		})
	});
	let adapter = adapter.unwrap();

	println!("SELECTED ADAPTER:");
	dbg!(adapter.get_info());
	println!("AVAILABLE ADAPTERS:");
	for adapter in instance.enumerate_adapters(wgpu::Backends::all()) {
		dbg!(adapter.get_info());
	}

	let (device, queue) = futures::executor::block_on(async {
		adapter
			.request_device(
				&wgpu::DeviceDescriptor {
					features: wgpu::Features::empty(),
					limits: wgpu::Limits::default(),
					label: None,
				},
				None,
			)
			.await
			.unwrap()
	});

	let surface_caps = window_surface.get_capabilities(&adapter);
	let surface_format = surface_caps
		.formats
		.iter()
		.copied()
		.find(|f| f.is_srgb())
		.unwrap_or(surface_caps.formats[0]);
	assert!(surface_caps
		.present_modes
		.contains(&wgpu::PresentMode::Fifo));
	let size = window.inner_size();
	let mut config = wgpu::SurfaceConfiguration {
		usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
		format: surface_format,
		width: size.width,
		height: size.height,
		present_mode: wgpu::PresentMode::Fifo,
		alpha_mode: surface_caps.alpha_modes[0],
		view_formats: vec![],
	};
	window_surface.configure(&device, &config);

	let mut camera = CameraPerspectiveConfig {
		up_direction: (0.0, 0.0, 1.0).into(),
		aspect_ratio: config.width as f32 / config.height as f32,
		field_of_view_y: TAU / 4.0,
		near_plane: 0.001,
		far_plane: 400.0,
	};
	let camera_matrix_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Camera Buffer"),
		contents: bytemuck::cast_slice(&[Matrix4x4Pod::zeroed()]),
		usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
	});
	let camera_bind_group_layout =
		device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			entries: &[wgpu::BindGroupLayoutEntry {
				binding: 0,
				visibility: wgpu::ShaderStages::VERTEX,
				ty: wgpu::BindingType::Buffer {
					ty: wgpu::BufferBindingType::Uniform,
					has_dynamic_offset: false,
					min_binding_size: None,
				},
				count: None,
			}],
			label: Some("Camera Bind Group Layout"),
		});
	let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
		layout: &camera_bind_group_layout,
		entries: &[wgpu::BindGroupEntry {
			binding: 0,
			resource: camera_matrix_buffer.as_entire_binding(),
		}],
		label: Some("Camera Bind Group"),
	});

	let cd = ChunkDimensions::from(11);
	let mut chunk = ChunkBlocks::new(cd);
	for internal_coords in cd.iter_internal_block_coords() {
		let ChunkInternalBlockCoords { x, y, z } = internal_coords;
		// Test chunk generation.
		chunk.internal_block_mut(cd, internal_coords).is_not_air = z < (x + y) / 4 + 3;
	}

	let mut vertices: Vec<BlockVertexPod> = Vec::new();
	chunk.mesh(cd, &mut vertices);

	let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Vertex Buffer"),
		contents: bytemuck::cast_slice(&vertices),
		usage: wgpu::BufferUsages::VERTEX,
	});

	let vertex_buffer_layout = wgpu::VertexBufferLayout {
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
		],
	};

	fn make_z_buffer_texture_view(
		device: &wgpu::Device,
		format: wgpu::TextureFormat,
		w: u32,
		h: u32,
	) -> wgpu::TextureView {
		let z_buffer_texture_description = wgpu::TextureDescriptor {
			label: Some("Z Buffer"),
			size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
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
	let z_buffer_format = wgpu::TextureFormat::Depth32Float;
	let mut z_buffer_view =
		make_z_buffer_texture_view(&device, z_buffer_format, config.width, config.height);

	let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
		label: Some("Shader"),
		source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/test_01.wgsl").into()),
	});

	let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
		label: Some("Render Pipeline Layout"),
		bind_group_layouts: &[&camera_bind_group_layout],
		push_constant_ranges: &[],
	});
	let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
		label: Some("Render Pipeline"),
		layout: Some(&render_pipeline_layout),
		vertex: wgpu::VertexState {
			module: &shader,
			entry_point: "vertex_shader_main",
			buffers: &[vertex_buffer_layout],
		},
		fragment: Some(wgpu::FragmentState {
			module: &shader,
			entry_point: "fragment_shader_main",
			targets: &[Some(wgpu::ColorTargetState {
				format: config.format,
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
			depth_compare: wgpu::CompareFunction::Less,
			stencil: wgpu::StencilState::default(),
			bias: wgpu::DepthBiasState::default(),
		}),
		multisample: wgpu::MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false },
		multiview: None,
	});

	let time_beginning = std::time::Instant::now();

	use winit::event::*;
	event_loop.run(move |event, _, control_flow| match event {
		Event::WindowEvent { ref event, window_id } if window_id == window.id() => match event {
			WindowEvent::CloseRequested
			| WindowEvent::KeyboardInput {
				input:
					KeyboardInput {
						state: ElementState::Pressed,
						virtual_keycode: Some(VirtualKeyCode::Escape),
						..
					},
				..
			} => *control_flow = ControlFlow::Exit,
			WindowEvent::Resized(new_size) => {
				let winit::dpi::PhysicalSize { width, height } = *new_size;
				config.width = width;
				config.height = height;
				window_surface.configure(&device, &config);
				z_buffer_view = make_z_buffer_texture_view(&device, z_buffer_format, width, height);
				camera.aspect_ratio = width as f32 / height as f32;
			},
			_ => {},
		},
		Event::MainEventsCleared => {
			let time_since_beginning = time_beginning.elapsed();

			let ts = time_since_beginning.as_secs_f32();
			let mut position = (5.5, 5.5, 5.5).into();
			let target = (5.5, 5.5, 5.5).into();
			position += (
				f32::cos(ts * 5.0) * 20.0,
				f32::sin(ts * 5.0) * 20.0,
				f32::cos(ts * 0.8) * 6.0,
			)
				.into();
			let camera_view_projection_matrix = camera.view_projection_matrix(position, target);
			queue.write_buffer(
				&camera_matrix_buffer,
				0,
				bytemuck::cast_slice(&[camera_view_projection_matrix]),
			);

			let window_texture = window_surface.get_current_texture().unwrap();
			let view = window_texture
				.texture
				.create_view(&wgpu::TextureViewDescriptor::default());
			let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
				label: Some("Render Encoder"),
			});

			{
				let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: Some("Render Pass"),
					color_attachments: &[Some(wgpu::RenderPassColorAttachment {
						view: &view,
						resolve_target: None,
						ops: wgpu::Operations {
							load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.7, b: 1.0, a: 1.0 }),
							store: true,
						},
					})],
					depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
						view: &z_buffer_view,
						depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: true }),
						stencil_ops: None,
					}),
				});
				render_pass.set_pipeline(&render_pipeline);
				render_pass.set_bind_group(0, &camera_bind_group, &[]);
				render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
				render_pass.draw(0..(vertices.len() as u32), 0..1);
			}
			queue.submit(std::iter::once(encoder.finish()));
			window_texture.present();
		},
		_ => {},
	});
}
