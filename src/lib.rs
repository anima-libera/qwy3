mod camera;
mod coords;

use std::{collections::HashMap, f32::consts::TAU};

use bytemuck::Zeroable;
use wgpu::util::DeviceExt;
use winit::{
	event_loop::{ControlFlow, EventLoop},
	window::WindowBuilder,
};

use camera::{aspect_ratio, CameraPerspectiveSettings, Matrix4x4Pod};
use coords::*;

/// Vertex type used in chunk block meshes.
#[derive(Copy, Clone, Debug)]
/// Certified Plain Old Data (so it can be sent to the GPU as a uniform).
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
struct BlockVertexPod {
	position: [f32; 3],
	color: [f32; 3],
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
		vertices.push(BlockVertexPod { position: b.into(), color: [0.0, 0.0, 1.0] });
		vertices.push(BlockVertexPod { position: b.into(), color: [0.0, 0.0, 1.0] });
		vertices.push(BlockVertexPod { position: c.into(), color: [0.0, 1.0, 0.0] });
		vertices.push(BlockVertexPod { position: d.into(), color: [1.0, 1.0, 0.0] });
	} else {
		vertices.push(BlockVertexPod { position: a.into(), color: [1.0, 0.0, 0.0] });
		vertices.push(BlockVertexPod { position: b.into(), color: [0.0, 1.0, 0.0] });
		vertices.push(BlockVertexPod { position: c.into(), color: [0.0, 0.0, 1.0] });
		vertices.push(BlockVertexPod { position: b.into(), color: [0.0, 1.0, 0.0] });
		vertices.push(BlockVertexPod { position: d.into(), color: [1.0, 1.0, 0.0] });
		vertices.push(BlockVertexPod { position: c.into(), color: [0.0, 0.0, 1.0] });
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
	fn mesh(
		&self,
		device: &wgpu::Device,
		cd: ChunkDimensions,
		chunk_coords: ChunkCoords,
	) -> ChunkMesh {
		let mut block_vertices = Vec::new();
		for internal_coords in cd.iter_internal_block_coords() {
			if self.internal_block(cd, internal_coords).is_not_air {
				for direction in OrientedAxis::all_the_six_possible_directions() {
					let covered = {
						if let Some(internal_neighbor) = internal_coords.internal_neighbor(cd, direction)
						{
							self.internal_block(cd, internal_neighbor).is_not_air
						} else {
							false
						}
					};
					if !covered {
						let world_coords =
							cd.chunk_internal_coords_to_world_coords(chunk_coords, internal_coords);
						let BlockCoords { x, y, z } = world_coords;
						generate_face(
							&mut block_vertices,
							direction,
							(x as f32, y as f32, z as f32).into(),
						);
					}
				}
			}
		}
		ChunkMesh::from_vertices(device, block_vertices)
	}
}

struct ChunkMesh {
	block_vertices: Vec<BlockVertexPod>,
	block_vertex_buffer: wgpu::Buffer,
}

impl ChunkMesh {
	fn from_vertices(device: &wgpu::Device, block_vertices: Vec<BlockVertexPod>) -> ChunkMesh {
		let block_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Block Vertex Buffer"),
			contents: bytemuck::cast_slice(&block_vertices),
			usage: wgpu::BufferUsages::VERTEX,
		});
		ChunkMesh { block_vertices, block_vertex_buffer }
	}
}

struct Chunk {
	blocks: ChunkBlocks,
	mesh: Option<ChunkMesh>,
}

struct ChunkGrid {
	map: HashMap<ChunkCoords, Chunk>,
}

impl ChunkGrid {
	fn set_block(&mut self, cd: ChunkDimensions, coords: BlockCoords, block: BlockTypeId) {
		let (chunk_coords, internal_coords) = cd.world_coords_to_chunk_internal_coords(coords);
		let chunk = self.map.get_mut(&chunk_coords).unwrap();
		let block_dst = chunk.blocks.internal_block_mut(cd, internal_coords);
		*block_dst = block;
	}

	fn get_block(&self, cd: ChunkDimensions, coords: BlockCoords) -> Option<BlockTypeId> {
		let (chunk_coords, internal_coords) = cd.world_coords_to_chunk_internal_coords(coords);
		let chunk = self.map.get(&chunk_coords)?;
		Some(chunk.blocks.internal_block(cd, internal_coords))
	}
}

struct AlignedPhysBox {
	/// Position of the center of the box.
	pos: cgmath::Point3<f32>,
	motion: cgmath::Vector3<f32>,
	dims: cgmath::Vector3<f32>,
	gravity_factor: f32,
}

/// Vertex type used in debugging line meshes.
#[derive(Copy, Clone, Debug)]
/// Certified Plain Old Data (so it can be sent to the GPU as a uniform).
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
struct SimpleLineVertexPod {
	position: [f32; 3],
	color: [f32; 3],
}

struct SimpleLineMesh {
	vertices: Vec<SimpleLineVertexPod>,
	vertex_buffer: wgpu::Buffer,
}

impl SimpleLineMesh {
	fn from_vertices(device: &wgpu::Device, vertices: Vec<SimpleLineVertexPod>) -> SimpleLineMesh {
		let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Simple Line Vertex Buffer"),
			contents: bytemuck::cast_slice(&vertices),
			usage: wgpu::BufferUsages::VERTEX,
		});
		SimpleLineMesh { vertices, vertex_buffer }
	}

	fn from_aligned_box(device: &wgpu::Device, aligned_box: &AlignedPhysBox) -> SimpleLineMesh {
		let color = [1.0, 1.0, 1.0];
		let mut vertices = Vec::new();

		// A---B  +--->   The L square and the H square are horizontal.
		// |   |  |   X+  L has lower value of Z coord.
		// C---D  v Y+    H has heigher value of Z coord.
		let al = aligned_box.pos - aligned_box.dims / 2.0;
		let bl = al + cgmath::Vector3::<f32>::unit_x() * aligned_box.dims.x;
		let cl = al + cgmath::Vector3::<f32>::unit_y() * aligned_box.dims.y;
		let dl = bl + cgmath::Vector3::<f32>::unit_y() * aligned_box.dims.y;
		let ah = al + cgmath::Vector3::<f32>::unit_z() * aligned_box.dims.z;
		let bh = bl + cgmath::Vector3::<f32>::unit_z() * aligned_box.dims.z;
		let ch = cl + cgmath::Vector3::<f32>::unit_z() * aligned_box.dims.z;
		let dh = dl + cgmath::Vector3::<f32>::unit_z() * aligned_box.dims.z;
		// L square
		vertices.push(SimpleLineVertexPod { position: al.into(), color });
		vertices.push(SimpleLineVertexPod { position: bl.into(), color });
		vertices.push(SimpleLineVertexPod { position: bl.into(), color });
		vertices.push(SimpleLineVertexPod { position: dl.into(), color });
		vertices.push(SimpleLineVertexPod { position: dl.into(), color });
		vertices.push(SimpleLineVertexPod { position: cl.into(), color });
		vertices.push(SimpleLineVertexPod { position: cl.into(), color });
		vertices.push(SimpleLineVertexPod { position: al.into(), color });
		// H square
		vertices.push(SimpleLineVertexPod { position: ah.into(), color });
		vertices.push(SimpleLineVertexPod { position: bh.into(), color });
		vertices.push(SimpleLineVertexPod { position: bh.into(), color });
		vertices.push(SimpleLineVertexPod { position: dh.into(), color });
		vertices.push(SimpleLineVertexPod { position: dh.into(), color });
		vertices.push(SimpleLineVertexPod { position: ch.into(), color });
		vertices.push(SimpleLineVertexPod { position: ch.into(), color });
		vertices.push(SimpleLineVertexPod { position: ah.into(), color });
		// Edges between L square and H square corresponding vertices.
		vertices.push(SimpleLineVertexPod { position: al.into(), color });
		vertices.push(SimpleLineVertexPod { position: ah.into(), color });
		vertices.push(SimpleLineVertexPod { position: bl.into(), color });
		vertices.push(SimpleLineVertexPod { position: bh.into(), color });
		vertices.push(SimpleLineVertexPod { position: cl.into(), color });
		vertices.push(SimpleLineVertexPod { position: ch.into(), color });
		vertices.push(SimpleLineVertexPod { position: dl.into(), color });
		vertices.push(SimpleLineVertexPod { position: dh.into(), color });

		SimpleLineMesh::from_vertices(device, vertices)
	}
}

pub fn run() {
	env_logger::init();
	let event_loop = EventLoop::new();
	let window = WindowBuilder::new()
		.with_title("Qwy3")
		.with_maximized(true)
		.with_resizable(true)
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
	// In case we didn't find any cool adapter, at least we can try to get a bad adapter.
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

	let mut camera = CameraPerspectiveSettings {
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

	let mut camera_angle_horizontal: f32 = 0.0;
	let mut camera_angle_vertical: f32 = TAU / 4.0;
	fn direction_from_angles(angle_horizontal: f32, angle_vertical: f32) -> cgmath::Vector3<f32> {
		let direction_vertical = f32::cos(angle_vertical);
		let direction_horizontal =
			cgmath::Vector2::<f32>::from((f32::cos(angle_horizontal), f32::sin(angle_horizontal)))
				* f32::sqrt(1.0 - direction_vertical.powi(2))
				* if angle_vertical < 0.0 { -1.0 } else { 1.0 };
		cgmath::Vector3::<f32>::from((
			direction_horizontal.x,
			direction_horizontal.y,
			direction_vertical,
		))
	}

	let mut player_phys = AlignedPhysBox {
		pos: (5.5, 5.5, 5.5).into(),
		motion: (0.0, 0.0, 0.0).into(),
		dims: (0.8, 0.8, 1.8).into(),
		gravity_factor: 1.0,
	};
	let mut enable_physics = true;

	let mut moving_forward = false;
	let mut moving_backward = false;
	let mut moving_leftward = false;
	let mut moving_rightward = false;

	window
		.set_cursor_grab(winit::window::CursorGrabMode::Confined)
		.unwrap();
	window.set_cursor_visible(false);

	let cd = ChunkDimensions::from(10);

	let mut chunk_grid = ChunkGrid { map: HashMap::new() };
	for chunk_coords in iter_3d_rect_inf_sup((-3, -3, -3), (4, 4, 4)) {
		let (x, y, z) = chunk_coords;
		let chunk_coords = ChunkCoords { x, y, z };
		let chunk = Chunk { blocks: ChunkBlocks::new(cd), mesh: None };
		chunk_grid.map.insert(chunk_coords, chunk);
	}

	for (chunk_coords, chunk) in chunk_grid.map.iter_mut() {
		for internal_coords in cd.iter_internal_block_coords() {
			let coords = cd.chunk_internal_coords_to_world_coords(*chunk_coords, internal_coords);
			// Test chunk generation.
			*chunk.blocks.internal_block_mut(cd, internal_coords) = BlockTypeId {
				is_not_air: coords.z as f32
					- f32::cos(coords.x as f32 * 0.3)
					- f32::cos(coords.y as f32 * 0.3)
					- 3.0 < 0.0,
			};
		}
	}

	for (&chunk_coords, chunk) in chunk_grid.map.iter_mut() {
		let mesh = chunk.blocks.mesh(&device, cd, chunk_coords);
		chunk.mesh = Some(mesh);
	}

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

	let block_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
		label: Some("Block Shader"),
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
			module: &block_shader,
			entry_point: "vertex_shader_main",
			buffers: &[block_vertex_buffer_layout],
		},
		fragment: Some(wgpu::FragmentState {
			module: &block_shader,
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
		source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/simple_line.wgsl").into()),
	});
	let simple_line_render_pipeline_layout =
		device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: Some("Simple Line Render Pipeline Layout"),
			bind_group_layouts: &[&camera_bind_group_layout],
			push_constant_ranges: &[],
		});
	let simple_line_render_pipeline =
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
					format: config.format,
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
			multisample: wgpu::MultisampleState {
				count: 1,
				mask: !0,
				alpha_to_coverage_enabled: false,
			},
			multiview: None,
		});

	let time_beginning = std::time::Instant::now();
	let mut time_from_last_iteration = std::time::Instant::now();

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
				camera.aspect_ratio = aspect_ratio(width, height);
			},
			WindowEvent::KeyboardInput {
				input: KeyboardInput { state, virtual_keycode: Some(VirtualKeyCode::Z), .. },
				..
			} => {
				moving_forward = *state == ElementState::Pressed;
			},
			WindowEvent::KeyboardInput {
				input: KeyboardInput { state, virtual_keycode: Some(VirtualKeyCode::S), .. },
				..
			} => {
				moving_backward = *state == ElementState::Pressed;
			},
			WindowEvent::KeyboardInput {
				input: KeyboardInput { state, virtual_keycode: Some(VirtualKeyCode::Q), .. },
				..
			} => {
				moving_leftward = *state == ElementState::Pressed;
			},
			WindowEvent::KeyboardInput {
				input: KeyboardInput { state, virtual_keycode: Some(VirtualKeyCode::D), .. },
				..
			} => {
				moving_rightward = *state == ElementState::Pressed;
			},
			WindowEvent::KeyboardInput {
				input: KeyboardInput { state: ElementState::Pressed, virtual_keycode: Some(key), .. },
				..
			} => match key {
				VirtualKeyCode::P => enable_physics = !enable_physics,
				VirtualKeyCode::H => {
					dbg!(player_phys.pos);
					let player_bottom = player_phys.pos
						- cgmath::Vector3::<f32>::from((0.0, 0.0, player_phys.dims.z / 2.0));
					dbg!(player_bottom);
					let player_bottom_block_coords = (
						player_bottom.x.round() as i32,
						player_bottom.y.round() as i32,
						player_bottom.z.round() as i32,
					);
					dbg!(player_bottom_block_coords);
					println!();
				},
				VirtualKeyCode::O => {
					let player_bottom = player_phys.pos
						- cgmath::Vector3::<f32>::from((0.0, 0.0, player_phys.dims.z / 2.0));
					let player_bottom_block_coords = BlockCoords {
						x: player_bottom.x.round() as i32,
						y: player_bottom.y.round() as i32,
						z: player_bottom.z.round() as i32,
					};
					let player_bottom_block_opt = chunk_grid.get_block(cd, player_bottom_block_coords);
					if let Some(block) = player_bottom_block_opt {
						chunk_grid.set_block(
							cd,
							player_bottom_block_coords,
							BlockTypeId { is_not_air: !block.is_not_air },
						);

						let chunk_coords =
							cd.world_coords_to_containing_chunk_coords(player_bottom_block_coords);
						let chunk = chunk_grid.map.get_mut(&chunk_coords).unwrap();
						let mesh = chunk.blocks.mesh(&device, cd, chunk_coords);
						chunk.mesh = Some(mesh);
					}
				},
				_ => {},
			},
			WindowEvent::MouseInput {
				state: winit::event::ElementState::Pressed,
				button: winit::event::MouseButton::Right,
				..
			} => {
				player_phys.motion.z = 0.1;
			},
			_ => {},
		},
		Event::DeviceEvent { event: winit::event::DeviceEvent::MouseMotion { delta }, .. } => {
			let sensitivity = 0.01;
			camera_angle_horizontal += -1.0 * delta.0 as f32 * sensitivity;
			camera_angle_vertical += delta.1 as f32 * sensitivity;
			if camera_angle_vertical < 0.0 {
				camera_angle_vertical = 0.0;
			}
			if TAU / 2.0 < camera_angle_vertical {
				camera_angle_vertical = TAU / 2.0;
			}
		},
		Event::DeviceEvent { event: winit::event::DeviceEvent::MouseWheel { delta }, .. } => {
			let (dx, dy) = match delta {
				MouseScrollDelta::LineDelta(horizontal, vertical) => (horizontal, vertical),
				MouseScrollDelta::PixelDelta(position) => (position.x as f32, position.y as f32),
			};
			let sensitivity = 0.01;
			player_phys.pos.z -= dy * sensitivity;
			player_phys.pos +=
				direction_from_angles(camera_angle_horizontal + TAU / 4.0 * dx.signum(), TAU / 4.0)
					* f32::abs(dx) * sensitivity;
		},
		Event::MainEventsCleared => {
			let _time_since_beginning = time_beginning.elapsed();
			let now = std::time::Instant::now();
			let dt = now - time_from_last_iteration;
			time_from_last_iteration = now;

			let moving_factor = if enable_physics { 12.0 } else { 35.0 } * dt.as_secs_f32();
			let moving_forward_factor =
				if moving_forward { 1 } else { 0 } + if moving_backward { -1 } else { 0 };
			let moving_rightward_factor =
				if moving_rightward { 1 } else { 0 } + if moving_leftward { -1 } else { 0 };
			player_phys.pos += direction_from_angles(camera_angle_horizontal, TAU / 4.0)
				* moving_forward_factor as f32
				* moving_factor;
			player_phys.pos += direction_from_angles(camera_angle_horizontal - TAU / 4.0, TAU / 4.0)
				* moving_rightward_factor as f32
				* moving_factor;

			if enable_physics {
				let player_bottom =
					player_phys.pos - cgmath::Vector3::<f32>::from((0.0, 0.0, player_phys.dims.z / 2.0));
				let player_bottom_block_coords = BlockCoords {
					x: player_bottom.x.round() as i32,
					y: player_bottom.y.round() as i32,
					z: player_bottom.z.round() as i32,
				};
				let player_bottom_block_opt = chunk_grid.get_block(cd, player_bottom_block_coords);
				let is_on_ground = if player_phys.motion.z <= 0.0 {
					if let Some(block) = player_bottom_block_opt {
						if block.is_not_air {
							player_phys.motion.z = 0.0;
							player_phys.pos.z =
								player_bottom_block_coords.z as f32 + 0.5 + player_phys.dims.z / 2.0;
							true
						} else {
							false
						}
					} else {
						false
					}
				} else {
					false
				};
				player_phys.pos += player_phys.motion;
				if !is_on_ground {
					player_phys.motion.z -= player_phys.gravity_factor * 0.3 * dt.as_secs_f32();
				}
			}

			let player_box_mesh = SimpleLineMesh::from_aligned_box(&device, &player_phys);

			let camera_position = player_phys.pos
				+ cgmath::Vector3::<f32>::from((0.0, 0.0, player_phys.dims.z / 2.0)) * 0.7;
			let direction = direction_from_angles(camera_angle_horizontal, camera_angle_vertical);
			let up_head =
				direction_from_angles(camera_angle_horizontal, camera_angle_vertical - TAU / 4.0);
			let camera_view_projection_matrix =
				camera.view_projection_matrix(camera_position, direction, up_head);
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
				for (_chunk_coords, chunk) in chunk_grid.map.iter() {
					if let Some(ref mesh) = chunk.mesh {
						render_pass.set_vertex_buffer(0, mesh.block_vertex_buffer.slice(..));
						render_pass.draw(0..(mesh.block_vertices.len() as u32), 0..1);
					}
				}

				render_pass.set_pipeline(&simple_line_render_pipeline);
				render_pass.set_bind_group(0, &camera_bind_group, &[]);
				render_pass.set_vertex_buffer(0, player_box_mesh.vertex_buffer.slice(..));
				render_pass.draw(0..(player_box_mesh.vertices.len() as u32), 0..1);
			}
			queue.submit(std::iter::once(encoder.finish()));
			window_texture.present();
		},
		_ => {},
	});
}
