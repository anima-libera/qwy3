#![allow(clippy::items_after_test_module)]

mod camera;
mod chunks;
mod cmdline;
mod commands;
mod coords;
mod font;
mod lang;
mod line_meshes;
mod noise;
mod physics;
mod rendering;
mod shaders;
mod threadpool;
mod world_gen;

use std::{collections::HashMap, f32::consts::TAU, sync::Arc};

use cgmath::{ElementWise, InnerSpace, MetricSpace};
use rand::Rng;
use wgpu::util::DeviceExt;
use winit::event_loop::ControlFlow;

use camera::{aspect_ratio, CameraOrthographicSettings, CameraPerspectiveSettings, CameraSettings};
use chunks::*;
use coords::*;
use line_meshes::*;
use physics::AlignedPhysBox;
use rendering::*;
use shaders::Vector3Pod;
use world_gen::WorldGenerator;

use crate::font::TextRenderingSettings;

enum WhichCameraToUse {
	FirstPerson,
	ThirdPersonNear,
	ThirdPersonFar,
	ThirdPersonVeryFar,
	Sun,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Control {
	KeyboardKey(winit::event::VirtualKeyCode),
	MouseButton(winit::event::MouseButton),
}
struct ControlEvent {
	control: Control,
	pressed: bool,
}
enum Action {
	WalkForward,
	WalkBackward,
	WalkLeftward,
	WalkRightward,
	Jump,
	TogglePhysics,
	ToggleWorldGeneration,
	CycleFirstAndThirdPersonViews,
	ToggleDisplayPlayerBox,
	ToggleSunView,
	ToggleCursorCaptured,
	PrintCoords,
	PlaceOrRemoveBlockUnderPlayer,
	PlaceBlockAtTarget,
	RemoveBlockAtTarget,
	ToggleDisplayInterface,
	OpenCommandLine,
	ToggleDisplayNotSurroundedChunksAsBoxes,
}

enum WorkerTask {
	GenerateChunkBlocks(ChunkCoords, std::sync::mpsc::Receiver<ChunkBlocks>),
	/// The bool at the end is `meshed_with_all_the_surrounding_chunks`.
	MeshChunk(ChunkCoords, std::sync::mpsc::Receiver<ChunkMesh>, bool),
}

pub struct SimpleTextureMesh {
	pub vertices: Vec<shaders::simple_texture_2d::SimpleTextureVertexPod>,
	pub vertex_buffer: wgpu::Buffer,
}

impl SimpleTextureMesh {
	fn from_vertices(
		device: &wgpu::Device,
		vertices: Vec<shaders::simple_texture_2d::SimpleTextureVertexPod>,
	) -> SimpleTextureMesh {
		let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Simple Texture Vertex Buffer"),
			contents: bytemuck::cast_slice(&vertices),
			usage: wgpu::BufferUsages::VERTEX,
		});
		SimpleTextureMesh { vertices, vertex_buffer }
	}

	fn _from_rect(
		device: &wgpu::Device,
		center: cgmath::Point3<f32>,
		dimensions: cgmath::Vector2<f32>,
		texture_rect_in_atlas_xy: cgmath::Point2<f32>,
		texture_rect_in_atlas_wh: cgmath::Vector2<f32>,
	) -> SimpleTextureMesh {
		let vertices = SimpleTextureMesh::vertices_for_rect(
			center,
			dimensions,
			texture_rect_in_atlas_xy,
			texture_rect_in_atlas_wh,
			[1.0, 1.0, 1.0],
		);
		SimpleTextureMesh::from_vertices(device, vertices)
	}

	fn vertices_for_rect(
		top_left: cgmath::Point3<f32>,
		dimensions: cgmath::Vector2<f32>,
		texture_rect_in_atlas_xy: cgmath::Point2<f32>,
		texture_rect_in_atlas_wh: cgmath::Vector2<f32>,
		color_factor: [f32; 3],
	) -> Vec<shaders::simple_texture_2d::SimpleTextureVertexPod> {
		use shaders::simple_texture_2d::SimpleTextureVertexPod;
		let mut vertices = vec![];

		let a = top_left + cgmath::vec3(0.0, 0.0, 0.0);
		let b = top_left + cgmath::vec3(dimensions.x, 0.0, 0.0);
		let c = top_left + cgmath::vec3(0.0, -dimensions.y, 0.0);
		let d = top_left + cgmath::vec3(dimensions.x, -dimensions.y, 0.0);
		let atlas_a = texture_rect_in_atlas_xy
			+ texture_rect_in_atlas_wh.mul_element_wise(cgmath::vec2(0.0, 0.0));
		let atlas_b = texture_rect_in_atlas_xy
			+ texture_rect_in_atlas_wh.mul_element_wise(cgmath::vec2(1.0, 0.0));
		let atlas_c = texture_rect_in_atlas_xy
			+ texture_rect_in_atlas_wh.mul_element_wise(cgmath::vec2(0.0, 1.0));
		let atlas_d = texture_rect_in_atlas_xy
			+ texture_rect_in_atlas_wh.mul_element_wise(cgmath::vec2(1.0, 1.0));

		vertices.push(SimpleTextureVertexPod {
			position: a.into(),
			coords_in_atlas: atlas_a.into(),
			color_factor,
		});
		vertices.push(SimpleTextureVertexPod {
			position: b.into(),
			coords_in_atlas: atlas_b.into(),
			color_factor,
		});
		vertices.push(SimpleTextureVertexPod {
			position: c.into(),
			coords_in_atlas: atlas_c.into(),
			color_factor,
		});
		vertices.push(SimpleTextureVertexPod {
			position: c.into(),
			coords_in_atlas: atlas_c.into(),
			color_factor,
		});
		vertices.push(SimpleTextureVertexPod {
			position: b.into(),
			coords_in_atlas: atlas_b.into(),
			color_factor,
		});
		vertices.push(SimpleTextureVertexPod {
			position: d.into(),
			coords_in_atlas: atlas_d.into(),
			color_factor,
		});

		vertices
	}
}

#[derive(Clone, Copy)]
struct RectInAtlas {
	texture_rect_in_atlas_xy: cgmath::Point2<f32>,
	texture_rect_in_atlas_wh: cgmath::Vector2<f32>,
}

struct LogLine {
	text: String,
	settings: font::TextRenderingSettings,
	dimensions: (f32, f32),
	target_position: (f32, f32),
	current_position: (f32, f32),
	creation_time: std::time::Instant,
	mesh: SimpleTextureMesh,
}

struct Game {
	window: winit::window::Window,
	window_surface: wgpu::Surface,
	device: Arc<wgpu::Device>,
	queue: wgpu::Queue,
	window_surface_config: wgpu::SurfaceConfiguration,
	aspect_ratio_thingy: BindingThingy<wgpu::Buffer>,
	z_buffer_view: wgpu::TextureView,
	z_buffer_format: wgpu::TextureFormat,
	camera_direction: AngularDirection,
	camera_settings: CameraPerspectiveSettings,
	camera_matrix_thingy: BindingThingy<wgpu::Buffer>,
	sun_position_in_sky: AngularDirection,
	sun_light_direction_thingy: BindingThingy<wgpu::Buffer>,
	sun_camera: CameraOrthographicSettings,
	sun_camera_matrix_thingy: BindingThingy<wgpu::Buffer>,
	shadow_map_view_thingy: BindingThingy<wgpu::TextureView>,
	/// First is the block of matter that is targeted,
	/// second is the empty block near it that would be filled if a block was placed now.
	targeted_block_coords: Option<(BlockCoords, BlockCoords)>,
	player_phys: AlignedPhysBox,
	cd: ChunkDimensions,
	chunk_grid: ChunkGrid,
	controls_to_trigger: Vec<ControlEvent>,
	control_bindings: HashMap<Control, Action>,
	block_type_table: Arc<BlockTypeTable>,
	rendering: RenderPipelinesAndBindGroups,
	close_after_one_frame: bool,
	cursor_mesh: SimpleLineMesh,
	top_left_info_mesh: SimpleTextureMesh,
	random_message: &'static str,
	font: font::Font,
	command_line_mesh: SimpleTextureMesh,
	command_line_content: String,
	typing_in_command_line: bool,
	last_command_line_interaction: Option<std::time::Instant>,
	command_confirmed: bool,
	world_generator: Arc<dyn WorldGenerator + Sync + Send>,
	loading_distance: f32,
	margin_before_unloading: f32,
	log: Vec<LogLine>,
	offset_for_2d_thingy: BindingThingy<wgpu::Buffer>,

	worker_tasks: Vec<WorkerTask>,
	pool: threadpool::ThreadPool,

	time_beginning: std::time::Instant,
	time_from_last_iteration: std::time::Instant,

	walking_forward: bool,
	walking_backward: bool,
	walking_leftward: bool,
	walking_rightward: bool,
	enable_physics: bool,
	enable_world_generation: bool,
	selected_camera: WhichCameraToUse,
	enable_display_phys_box: bool,
	cursor_is_captured: bool,
	enable_display_interface: bool,
	enable_display_not_surrounded_chunks_as_boxes: bool,
	enable_temporary_meshing_of_not_surrounded_chunks: bool,
}

fn init_game() -> (Game, winit::event_loop::EventLoop<()>) {
	// Wgpu uses the `log`/`env_logger` crates to log errors and stuff,
	// and we do want to see the errors very much.
	env_logger::init();

	let cmdline::CommandLineSettings {
		number_of_threads,
		close_after_one_frame,
		verbose,
		output_atlas,
		world_gen_seed,
		which_world_generator,
		loading_distance,
		chunk_edge,
		test_lang,
	} = cmdline::parse_command_line_arguments();

	if let Some(test_id) = test_lang {
		println!("Test lang: test id {test_id}");
		lang::test_lang(test_id);
		std::process::exit(0);
	}

	let event_loop = winit::event_loop::EventLoop::new();
	let window = winit::window::WindowBuilder::new()
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

	if verbose {
		// At some point it could be nice to allow the user to choose their preferred adapter.
		// No one should have to struggle to make some game use the big GPU instead of the tiny one.
		println!("AVAILABLE ADAPTERS:");
		for adapter in instance.enumerate_adapters(wgpu::Backends::all()) {
			dbg!(adapter.get_info());
			dbg!(adapter.limits().max_bind_groups);
		}
		println!("SELECTED ADAPTER:");
		dbg!(adapter.get_info());
	}

	let (device, queue) = futures::executor::block_on(async {
		adapter
			.request_device(
				&wgpu::DeviceDescriptor {
					features: wgpu::Features::empty(),
					limits: wgpu::Limits { ..wgpu::Limits::default() },
					label: None,
				},
				None,
			)
			.await
	})
	.unwrap();
	let device = Arc::new(device);

	let surface_capabilities = window_surface.get_capabilities(&adapter);
	let surface_format = surface_capabilities
		.formats
		.iter()
		.copied()
		.find(|f| f.is_srgb())
		.unwrap_or(surface_capabilities.formats[0]);
	assert!(surface_capabilities
		.present_modes
		.contains(&wgpu::PresentMode::Fifo));
	let size = window.inner_size();
	let window_surface_config = wgpu::SurfaceConfiguration {
		usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
		format: surface_format,
		width: size.width,
		height: size.height,
		present_mode: wgpu::PresentMode::Fifo,
		alpha_mode: surface_capabilities.alpha_modes[0],
		view_formats: vec![],
	};
	window_surface.configure(&device, &window_surface_config);

	let aspect_ratio_thingy = init_aspect_ratio_thingy(Arc::clone(&device));

	let block_type_table = Arc::new(BlockTypeTable::new());

	let mut atlas_data: [u8; 4 * ATLAS_DIMS.0 * ATLAS_DIMS.1] = [0; 4 * ATLAS_DIMS.0 * ATLAS_DIMS.1];
	for y in 0..16 {
		for x in 0..16 {
			let index = 4 * (y * ATLAS_DIMS.0 + x);
			let grey = rand::thread_rng().gen_range(240..=255);
			atlas_data[index..(index + 4)].clone_from_slice(&[grey, grey, grey, 255]);
		}
	}
	for y in 0..16 {
		for x in (0..16).map(|x| x + 16) {
			let index = 4 * (y * ATLAS_DIMS.0 + x);
			atlas_data[index..(index + 4)].clone_from_slice(&[
				rand::thread_rng().gen_range(80..100),
				rand::thread_rng().gen_range(230..=255),
				rand::thread_rng().gen_range(10..30),
				255,
			]);
		}
	}
	for y in 0..16 {
		for x in (0..16).map(|x| x + 32) {
			let index = 4 * (y * ATLAS_DIMS.0 + x);
			let tx = x - 32;
			let tp = cgmath::vec2(tx as f32, y as f32 / 2.0);
			let bottom_center = cgmath::vec2(8.0, 0.0);
			if bottom_center.distance(tp) > 8.0 {
				atlas_data[index..(index + 4)].clone_from_slice(&[0, 0, 0, 0]);
			} else {
				atlas_data[index..(index + 4)].clone_from_slice(&[
					rand::thread_rng().gen_range(80..100),
					rand::thread_rng().gen_range(230..=255),
					rand::thread_rng().gen_range(10..30),
					255,
				]);
			}
		}
	}

	let font_image = image::load_from_memory(include_bytes!("../assets/font-01.png")).unwrap();
	for y in 0..font_image.height() {
		for x in 0..font_image.width() {
			let pixel_from_image = font_image.as_rgba8().unwrap().get_pixel(x, y);
			// We keep black as white (to multiply with colors) and discard everything else.
			let color = if pixel_from_image.0 == [0, 0, 0, 255] {
				[255, 255, 255, 255]
			} else {
				[0, 0, 0, 0]
			};
			let index = 4 * ((y as usize + 32) * ATLAS_DIMS.0 + x as usize);
			atlas_data[index..(index + 4)].clone_from_slice(&color);
		}
	}
	let font = font::Font::font_01();

	if output_atlas {
		let path = "atlas.png";
		println!("Outputting atlas to \"{path}\"");
		image::save_buffer_with_format(
			path,
			&atlas_data,
			ATLAS_DIMS.0 as u32,
			ATLAS_DIMS.1 as u32,
			image::ColorType::Rgba8,
			image::ImageFormat::Png,
		)
		.unwrap();
	}
	let AtlasStuff { atlas_texture_view_thingy, atlas_texture_sampler_thingy } =
		init_atlas_stuff(Arc::clone(&device), &queue, &atlas_data);

	let camera_settings = CameraPerspectiveSettings {
		up_direction: (0.0, 0.0, 1.0).into(),
		aspect_ratio: window_surface_config.width as f32 / window_surface_config.height as f32,
		field_of_view_y: TAU / 4.0,
		near_plane: 0.1,
		far_plane: 400.0,
	};
	let camera_matrix_thingy = init_camera_matrix_thingy(Arc::clone(&device));

	let camera_direction = AngularDirection::from_angle_horizontal(0.0);

	let selected_camera = WhichCameraToUse::FirstPerson;

	let cursor_is_captured = true;
	window
		.set_cursor_grab(winit::window::CursorGrabMode::Confined)
		.unwrap();
	window.set_cursor_visible(false);

	// First is the block of matter that is targeted,
	// second is the empty block near it that would be filled if a block was placed now.
	let targeted_block_coords: Option<(BlockCoords, BlockCoords)> = None;

	let walking_forward = false;
	let walking_backward = false;
	let walking_leftward = false;
	let walking_rightward = false;

	let player_phys = AlignedPhysBox {
		aligned_box: AlignedBox { pos: (0.0, 0.0, 2.0).into(), dims: (0.8, 0.8, 1.8).into() },
		motion: (0.0, 0.0, 0.0).into(),
		gravity_factor: 1.0,
	};
	let enable_physics = true;
	let enable_display_phys_box = false;

	let sun_position_in_sky = AngularDirection::from_angles(TAU / 16.0, TAU / 8.0);
	let sun_light_direction_thingy = init_sun_light_direction_thingy(Arc::clone(&device));

	let sun_camera = CameraOrthographicSettings {
		up_direction: (0.0, 0.0, 1.0).into(),
		width: 85.0,
		height: 85.0,
		depth: 200.0,
	};
	let sun_camera_matrix_thingy = init_sun_camera_matrix_thingy(Arc::clone(&device));

	let ShadowMapStuff {
		shadow_map_format,
		shadow_map_view_thingy,
		shadow_map_sampler_thingy,
	} = init_shadow_map_stuff(Arc::clone(&device));

	let z_buffer_format = wgpu::TextureFormat::Depth32Float;
	let z_buffer_view = make_z_buffer_texture_view(
		&device,
		z_buffer_format,
		window_surface_config.width,
		window_surface_config.height,
	);

	let time_beginning = std::time::Instant::now();
	let time_from_last_iteration = std::time::Instant::now();

	let control_bindings = commands::parse_control_binding_file();
	let controls_to_trigger: Vec<ControlEvent> = vec![];

	let cd = ChunkDimensions::from(chunk_edge as i32);
	let chunk_grid = ChunkGrid::new(cd);

	let enable_world_generation = true;

	let worker_tasks = vec![];
	let pool = threadpool::ThreadPool::new(number_of_threads as usize);

	let offset_for_2d_thingy = init_offset_for_2d_thingy(Arc::clone(&device));

	let rendering = rendering::init_rendering_stuff(
		Arc::clone(&device),
		AllBindingThingies {
			aspect_ratio_thingy: &aspect_ratio_thingy,
			camera_matrix_thingy: &camera_matrix_thingy,
			sun_light_direction_thingy: &sun_light_direction_thingy,
			sun_camera_matrix_thingy: &sun_camera_matrix_thingy,
			shadow_map_view_thingy: &shadow_map_view_thingy,
			shadow_map_sampler_thingy: &shadow_map_sampler_thingy,
			atlas_texture_view_thingy: &atlas_texture_view_thingy,
			atlas_texture_sampler_thingy: &atlas_texture_sampler_thingy,
			offset_for_2d_thingy: &offset_for_2d_thingy,
		},
		shadow_map_format,
		window_surface_config.format,
		z_buffer_format,
	);

	let cursor_mesh = SimpleLineMesh::interface_2d_cursor(&device);

	let top_left_info_mesh = SimpleTextureMesh::from_vertices(&device, vec![]);

	// Most useful feature in the known universe.
	let random_message_pool = [
		"hewwo :3",
		"uwu",
		"owo",
		"jaaj",
		"trans rights",
		"qwy3 best game",
		":3",
		"^^",
		"drink water!",
		"rust best lang",
		"when the",
		"voxels!",
		">w<",
		"<3",
		"gaming",
		"nyaa",
	];
	let random_message =
		random_message_pool[rand::thread_rng().gen_range(0..random_message_pool.len())];

	let enable_display_interface = true;

	let command_line_mesh = SimpleTextureMesh::from_vertices(&device, vec![]);
	let command_line_content = String::new();
	let typing_in_command_line = false;
	let last_command_line_interaction = None;
	let command_confirmed = false;

	let world_generator = which_world_generator.get_the_actual_generator(world_gen_seed);

	let margin_before_unloading = 60.0;

	let enable_display_not_surrounded_chunks_as_boxes = false;
	let enable_temporary_meshing_of_not_surrounded_chunks = true;

	let log = vec![];

	queue.write_buffer(
		&offset_for_2d_thingy.resource,
		0,
		bytemuck::cast_slice(&[Vector3Pod { values: [0.0, 0.0, 0.0] }]),
	);

	if verbose {
		println!("End of initialization");
	}

	let game = Game {
		window,
		window_surface,
		device,
		queue,
		window_surface_config,
		aspect_ratio_thingy,
		z_buffer_format,
		z_buffer_view,
		camera_direction,
		camera_settings,
		camera_matrix_thingy,
		sun_position_in_sky,
		sun_light_direction_thingy,
		sun_camera,
		sun_camera_matrix_thingy,
		shadow_map_view_thingy,
		targeted_block_coords,
		player_phys,
		cd,
		chunk_grid,
		controls_to_trigger,
		control_bindings,
		block_type_table,
		rendering,
		close_after_one_frame,
		cursor_mesh,
		top_left_info_mesh,
		random_message,
		font,
		command_line_mesh,
		command_line_content,
		typing_in_command_line,
		last_command_line_interaction,
		command_confirmed,
		world_generator,
		loading_distance,
		margin_before_unloading,
		log,
		offset_for_2d_thingy,

		worker_tasks,
		pool,

		time_beginning,
		time_from_last_iteration,

		walking_forward,
		walking_backward,
		walking_leftward,
		walking_rightward,
		enable_physics,
		enable_world_generation,
		selected_camera,
		enable_display_phys_box,
		cursor_is_captured,
		enable_display_interface,
		enable_display_not_surrounded_chunks_as_boxes,
		enable_temporary_meshing_of_not_surrounded_chunks,
	};
	(game, event_loop)
}

pub fn run() {
	let (mut game, event_loop) = init_game();

	use winit::event::*;
	event_loop.run(move |event, _, control_flow| match event {
		Event::WindowEvent { ref event, window_id } if window_id == game.window.id() => match event {
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
				game.window_surface_config.width = width;
				game.window_surface_config.height = height;
				game
					.window_surface
					.configure(&game.device, &game.window_surface_config);
				game.z_buffer_view =
					make_z_buffer_texture_view(&game.device, game.z_buffer_format, width, height);
				game.camera_settings.aspect_ratio = aspect_ratio(width, height);

				game.queue.write_buffer(
					&game.aspect_ratio_thingy.resource,
					0,
					bytemuck::cast_slice(&[game.camera_settings.aspect_ratio]),
				);
			},

			WindowEvent::MouseInput {
				state: winit::event::ElementState::Pressed,
				button: winit::event::MouseButton::Left,
				..
			} if !game.cursor_is_captured => {
				game.cursor_is_captured = true;
				game
					.window
					.set_cursor_grab(winit::window::CursorGrabMode::Confined)
					.unwrap();
				game.window.set_cursor_visible(false);
			},

			WindowEvent::KeyboardInput {
				input: KeyboardInput { state, virtual_keycode: Some(key), .. },
				..
			} => {
				if game.typing_in_command_line && *state == ElementState::Pressed {
					if matches!(key, VirtualKeyCode::Return) {
						game.command_confirmed = true;
						game.typing_in_command_line = false;
						game.last_command_line_interaction = Some(std::time::Instant::now());
					} else if matches!(key, VirtualKeyCode::Back) {
						game.command_line_content.pop();
						game.last_command_line_interaction = Some(std::time::Instant::now());
					} else {
						// Handeled by the `winit::WindowEvent::ReceivedCharacter` case.
					}
				} else {
					game.controls_to_trigger.push(ControlEvent {
						control: Control::KeyboardKey(*key),
						pressed: *state == ElementState::Pressed,
					});
				}
			},

			WindowEvent::MouseInput { state, button, .. } if game.cursor_is_captured => {
				game.controls_to_trigger.push(ControlEvent {
					control: Control::MouseButton(*button),
					pressed: *state == ElementState::Pressed,
				});
			},

			WindowEvent::ReceivedCharacter(character) => {
				const BACKSPACE: char = '\u{8}';
				if game.typing_in_command_line && *character != BACKSPACE {
					game.command_line_content.push(*character);
					game.last_command_line_interaction = Some(std::time::Instant::now());
				}
			},

			_ => {},
		},

		Event::DeviceEvent { event: winit::event::DeviceEvent::MouseMotion { delta }, .. }
			if game.cursor_is_captured =>
		{
			// Move camera.
			let sensitivity = 0.0025;
			game.camera_direction.angle_horizontal += -1.0 * delta.0 as f32 * sensitivity;
			game.camera_direction.angle_vertical += delta.1 as f32 * sensitivity;
			if game.camera_direction.angle_vertical < 0.0 {
				game.camera_direction.angle_vertical = 0.0;
			}
			if TAU / 2.0 < game.camera_direction.angle_vertical {
				game.camera_direction.angle_vertical = TAU / 2.0;
			}
		},

		Event::DeviceEvent { event: winit::event::DeviceEvent::MouseWheel { delta }, .. } => {
			// Wheel moves the player along the vertical axis.
			// Useful when physics are disabled.
			let (dx, dy) = match delta {
				MouseScrollDelta::LineDelta(horizontal, vertical) => (horizontal, vertical),
				MouseScrollDelta::PixelDelta(position) => (position.x as f32, position.y as f32),
			};
			let sensitivity = 0.01;
			let direction_left_or_right = game
				.camera_direction
				.to_horizontal()
				.add_to_horizontal_angle(TAU / 4.0 * dx.signum());
			game.player_phys.aligned_box.pos.z -= dy * sensitivity;
			game.player_phys.aligned_box.pos +=
				direction_left_or_right.to_vec3() * f32::abs(dx) * sensitivity;
		},

		Event::MainEventsCleared => {
			let _time_since_beginning = game.time_beginning.elapsed();
			let now = std::time::Instant::now();
			let dt = now - game.time_from_last_iteration;
			game.time_from_last_iteration = now;

			// Perform actions triggered by controls.
			for control_event in game.controls_to_trigger.iter() {
				let pressed = control_event.pressed;
				if let Some(action) = game.control_bindings.get(&control_event.control) {
					match (action, pressed) {
						(Action::WalkForward, pressed) => {
							game.walking_forward = pressed;
						},
						(Action::WalkBackward, pressed) => {
							game.walking_backward = pressed;
						},
						(Action::WalkLeftward, pressed) => {
							game.walking_leftward = pressed;
						},
						(Action::WalkRightward, pressed) => {
							game.walking_rightward = pressed;
						},
						(Action::Jump, true) => {
							game.player_phys.motion.z = 0.1;
						},
						(Action::TogglePhysics, true) => {
							game.enable_physics = !game.enable_physics;
						},
						(Action::ToggleWorldGeneration, true) => {
							game.enable_world_generation = !game.enable_world_generation;
						},
						(Action::CycleFirstAndThirdPersonViews, true) => {
							game.selected_camera = match game.selected_camera {
								WhichCameraToUse::FirstPerson => WhichCameraToUse::ThirdPersonNear,
								WhichCameraToUse::ThirdPersonNear => WhichCameraToUse::ThirdPersonFar,
								WhichCameraToUse::ThirdPersonFar => WhichCameraToUse::ThirdPersonVeryFar,
								WhichCameraToUse::ThirdPersonVeryFar => WhichCameraToUse::FirstPerson,
								WhichCameraToUse::Sun => WhichCameraToUse::FirstPerson,
							};
						},
						(Action::ToggleDisplayPlayerBox, true) => {
							game.enable_display_phys_box = !game.enable_display_phys_box;
						},
						(Action::ToggleSunView, true) => {
							game.selected_camera = match game.selected_camera {
								WhichCameraToUse::Sun => WhichCameraToUse::FirstPerson,
								_ => WhichCameraToUse::Sun,
							};
						},
						(Action::ToggleCursorCaptured, true) => {
							game.cursor_is_captured = !game.cursor_is_captured;
							if game.cursor_is_captured {
								game
									.window
									.set_cursor_grab(winit::window::CursorGrabMode::Confined)
									.unwrap();
								game.window.set_cursor_visible(false);
							} else {
								game
									.window
									.set_cursor_grab(winit::window::CursorGrabMode::None)
									.unwrap();
								game.window.set_cursor_visible(true);
							}
						},
						(Action::PrintCoords, true) => {
							dbg!(game.player_phys.aligned_box.pos);
							let player_bottom = game.player_phys.aligned_box.pos
								- cgmath::Vector3::<f32>::from((
									0.0,
									0.0,
									game.player_phys.aligned_box.dims.z / 2.0,
								));
							dbg!(player_bottom);
						},
						(Action::PlaceOrRemoveBlockUnderPlayer, true) => {
							let player_bottom = game.player_phys.aligned_box.pos
								- cgmath::Vector3::<f32>::unit_z()
									* (game.player_phys.aligned_box.dims.z / 2.0 + 0.1);
							let player_bottom_block_coords = player_bottom.map(|x| x.round() as i32);
							let player_bottom_block_opt =
								game.chunk_grid.get_block(player_bottom_block_coords);
							if let Some(block_id) = player_bottom_block_opt {
								game.chunk_grid.set_block_and_request_updates_to_meshes(
									player_bottom_block_coords,
									if game.block_type_table.get(block_id).unwrap().is_opaque() {
										game.block_type_table.air_id()
									} else {
										game.block_type_table.ground_id()
									},
								);
							}
						},
						(Action::PlaceBlockAtTarget, true) => {
							if let Some((_, coords)) = game.targeted_block_coords {
								game.chunk_grid.set_block_and_request_updates_to_meshes(
									coords,
									game.block_type_table.ground_id(),
								);
							}
						},
						(Action::RemoveBlockAtTarget, true) => {
							if let Some((coords, _)) = game.targeted_block_coords {
								game.chunk_grid.set_block_and_request_updates_to_meshes(
									coords,
									game.block_type_table.air_id(),
								);
							}
						},
						(Action::ToggleDisplayInterface, true) => {
							game.enable_display_interface = !game.enable_display_interface;
						},
						(Action::OpenCommandLine, true) => {
							game.typing_in_command_line = true;
							game.last_command_line_interaction = Some(std::time::Instant::now());
						},
						(Action::ToggleDisplayNotSurroundedChunksAsBoxes, true) => {
							game.enable_display_not_surrounded_chunks_as_boxes =
								!game.enable_display_not_surrounded_chunks_as_boxes;
						},
						(_, false) => {},
					}
				}
			}
			game.controls_to_trigger.clear();

			// Top left info.
			{
				let window_width = game.window_surface_config.width as f32;
				let window_height = game.window_surface_config.height as f32;
				let fps = 1.0 / dt.as_secs_f32();
				let chunk_count = game.chunk_grid.map.len();
				let block_count = chunk_count * game.cd.number_of_blocks();
				let chunk_def_meshed_count = game
					.chunk_grid
					.map
					.iter()
					.filter(|(_chunk_coords, chunk)| {
						chunk.mesh.is_some() && chunk.meshed_with_all_the_surrounding_chunks
					})
					.count();
				let chunk_tmp_meshed_count = game
					.chunk_grid
					.map
					.iter()
					.filter(|(_chunk_coords, chunk)| {
						chunk.mesh.is_some() && !chunk.meshed_with_all_the_surrounding_chunks
					})
					.count();
				let chunk_meshed_count = chunk_def_meshed_count + chunk_tmp_meshed_count;
				let player_block_coords = (game.player_phys.aligned_box.pos
					- cgmath::Vector3::<f32>::unit_z()
						* (game.player_phys.aligned_box.dims.z / 2.0 + 0.1))
					.map(|x| x.round() as i32);
				let player_block_coords_str = {
					let cgmath::Point3 { x, y, z } = player_block_coords;
					format!("{x},{y},{z}")
				};
				let random_message = game.random_message;
				let settings = font::TextRenderingSettings::with_scale(3.0);
				game.top_left_info_mesh = game.font.simple_texture_mesh_from_text(
					&game.device,
					window_width,
					cgmath::point3(
						-1.0 + 4.0 / window_width,
						(-4.0 + window_height) / window_width,
						0.5,
					),
					settings,
					&format!(
						"fps: {fps}\n\
						chunks loaded: {chunk_count}\n\
						blocks loaded: {block_count}\n\
						chunks meshed: {chunk_def_meshed_count} def + {chunk_tmp_meshed_count} tmp = \
							{chunk_meshed_count}\n\
						player coords: {player_block_coords_str}\n\
						{random_message}"
					),
				);
			}

			// Command line handling.
			if game.command_confirmed {
				println!("Executing command \"{}\"", game.command_line_content);

				let text = game.command_line_content.clone();
				let settings = font::TextRenderingSettings::with_scale(3.0);
				let window_width = game.window_surface_config.width as f32;
				let window_height = game.window_surface_config.height as f32;
				let dimensions =
					game
						.font
						.dimensions_of_text(window_width, settings.clone(), text.as_str());
				let mesh = game.font.simple_texture_mesh_from_text(
					&game.device,
					window_width,
					cgmath::point3(0.0, 0.0, 0.0),
					settings.clone(),
					&text,
				);
				game.log.insert(
					0,
					LogLine {
						text,
						settings,
						dimensions,
						target_position: (0.0, 0.0),
						current_position: (0.0, 0.0),
						creation_time: std::time::Instant::now(),
						mesh,
					},
				);
				for (i, log_line) in game.log.iter_mut().enumerate() {
					let y =
						(i + 1) as f32 / window_width * 3.0 * 6.0 * 2.0 - window_height / window_width;
					let x = -1.0 + 10.0 / window_width;
					// Somehow this makes it pixel perfect, somehow?
					let x = (x * (window_width * 8.0) - 0.5).floor() / (window_width * 8.0);
					let position = (x, y);
					log_line.target_position = position;
					log_line.current_position = position;
				}

				game.command_line_content.clear();
				game.command_confirmed = false;
			}
			{
				let carret_blinking_speed = 1.5;
				let carret_blinking_visibility_ratio = 0.5;
				let carret_text_representation = "█";
				let carret_visible = game.typing_in_command_line
					&& game.last_command_line_interaction.is_some_and(|time| {
						(time.elapsed().as_secs_f32() * carret_blinking_speed).fract()
							< carret_blinking_visibility_ratio
					});
				let window_width = game.window_surface_config.width as f32;
				let command_line_content = game.command_line_content.as_str();
				let command_line_content_with_carret =
					command_line_content.to_string() + carret_text_representation;
				let settings = font::TextRenderingSettings::with_scale(4.0);
				let dimensions = game.font.dimensions_of_text(
					window_width,
					settings.clone(),
					command_line_content_with_carret.as_str(),
				);
				let y = 0.0 + dimensions.1 / 2.0;
				let x = 0.0 - dimensions.0 / 2.0;
				// Somehow this makes it pixel perfect, somehow?
				let x = (x * (window_width * 8.0) - 0.5).floor() / (window_width * 8.0);
				let text_displayed = if carret_visible {
					command_line_content_with_carret.as_str()
				} else {
					command_line_content
				};
				game.command_line_mesh = game.font.simple_texture_mesh_from_text(
					&game.device,
					window_width,
					cgmath::point3(x, y, 0.5),
					settings,
					text_displayed,
				);
			}

			// Recieve task results from workers.
			game.worker_tasks.retain_mut(|worker_task| {
				let is_not_done_yet = match worker_task {
					WorkerTask::GenerateChunkBlocks(chunk_coords, receiver) => {
						let chunk_coords_and_result_opt = receiver
							.try_recv()
							.ok()
							.map(|chunk_blocks| (*chunk_coords, chunk_blocks));
						let is_not_done_yet = chunk_coords_and_result_opt.is_none();
						if let Some((chunk_coords, chunk_blocks)) = chunk_coords_and_result_opt {
							let coords_span = ChunkCoordsSpan { cd: game.cd, chunk_coords };
							let mut chunk = Chunk::new_empty(coords_span);
							chunk.blocks = Some(chunk_blocks);
							game.chunk_grid.map.insert(chunk_coords, chunk);

							for neighbor_chunk_coords in iter_3d_cube_center_radius(chunk_coords, 2) {
								if let Some(neighbor_chunk) =
									game.chunk_grid.map.get_mut(&neighbor_chunk_coords)
								{
									neighbor_chunk.remeshing_required = true;
								}
							}
						}
						is_not_done_yet
					},
					WorkerTask::MeshChunk(
						chunk_coords,
						receiver,
						meshed_with_all_the_surrounding_chunks,
					) => {
						let chunk_coords_and_result_opt = receiver
							.try_recv()
							.ok()
							.map(|chunk_mesh| (*chunk_coords, chunk_mesh));
						let is_not_done_yet = chunk_coords_and_result_opt.is_none();
						if let Some((chunk_coords, chunk_mesh)) = chunk_coords_and_result_opt {
							if let Some(chunk) = game.chunk_grid.map.get_mut(&chunk_coords) {
								chunk.mesh = Some(chunk_mesh);
								if *meshed_with_all_the_surrounding_chunks {
									chunk.meshed_with_all_the_surrounding_chunks =
										*meshed_with_all_the_surrounding_chunks;
								}
							} else {
								// The chunk have been unloaded since the meshing was ordered.
								// It really can happen, for example when the player travels very fast.
							}
						}
						is_not_done_yet
					},
				};
				is_not_done_yet
			});

			// Request meshing for chunks that can be meshed or should be re-meshed.
			let chunk_coords_list: Vec<_> = game.chunk_grid.map.keys().copied().collect();
			for chunk_coords in chunk_coords_list.iter().copied() {
				let tmp_meshing_allowed = game.enable_temporary_meshing_of_not_surrounded_chunks;
				let (already_has_mesh, already_has_def_mesh) = game
					.chunk_grid
					.map
					.get(&chunk_coords)
					.map(|chunk| {
						(
							chunk.mesh.is_some(),
							chunk.meshed_with_all_the_surrounding_chunks,
						)
					})
					.unwrap();
				let is_being_meshed = game
					.worker_tasks
					.iter()
					.any(|worker_task| match worker_task {
						WorkerTask::MeshChunk(chunk_coords_uwu, ..) => *chunk_coords_uwu == chunk_coords,
						_ => false,
					});
				let can_be_def_meshed = 'can_be_def_meshed: {
					for neighbor_chunk_coords in iter_3d_cube_center_radius(chunk_coords, 2) {
						let blocks_was_generated = game
							.chunk_grid
							.map
							.get(&neighbor_chunk_coords)
							.is_some_and(|chunk| chunk.blocks.is_some());
						if !blocks_was_generated {
							break 'can_be_def_meshed false;
						}
					}
					true
				};
				let should_be_remeshed = game
					.chunk_grid
					.map
					.get(&chunk_coords)
					.is_some_and(|chunk| chunk.remeshing_required);
				let shall_be_def_meshed = (((!already_has_mesh) && (!is_being_meshed))
					|| should_be_remeshed)
					&& can_be_def_meshed
					&& game.worker_tasks.len() < game.pool.number_of_workers();
				let shall_be_tmp_meshed = tmp_meshing_allowed
					&& !is_being_meshed
					&& !shall_be_def_meshed
					&& !already_has_mesh
					&& !already_has_def_mesh
					&& game.worker_tasks.len() < game.pool.number_of_workers();
				if shall_be_def_meshed || shall_be_tmp_meshed {
					// Asking a worker for the meshing or remeshing of the chunk
					game
						.chunk_grid
						.map
						.get_mut(&chunk_coords)
						.unwrap()
						.remeshing_required = false;
					let (sender, receiver) = std::sync::mpsc::channel();
					game.worker_tasks.push(WorkerTask::MeshChunk(
						chunk_coords,
						receiver,
						shall_be_def_meshed,
					));
					let opaqueness_layer = game.chunk_grid.get_opaqueness_layer_around_chunk(
						chunk_coords,
						false,
						Arc::clone(&game.block_type_table),
					);
					let chunk_blocks = game
						.chunk_grid
						.map
						.get(&chunk_coords)
						.unwrap()
						.blocks
						.clone() // TODO: Find a way to avoid cloning all these blocks ><.
						.unwrap();
					let device = Arc::clone(&game.device);
					let block_type_table = Arc::clone(&game.block_type_table);
					game.pool.enqueue_task(Box::new(move || {
						// TODO: Remove the sleeping!
						// Test delay to make sure that the main thread keeps working even
						// when the workers tasks take very long.
						//std::thread::sleep(std::time::Duration::from_secs_f32(
						//	rand::thread_rng().gen_range(0.1..0.3),
						//));

						let mut mesh = chunk_blocks.generate_mesh_given_surrounding_opaqueness(
							opaqueness_layer,
							block_type_table,
						);
						mesh.update_gpu_data(&device);
						let _ = sender.send(mesh);
					}));
				}
			}

			// Request generation of chunk blocks for not-generated not-being-generated close chunks.
			if game.enable_world_generation {
				let player_block_coords = (game.player_phys.aligned_box.pos
					- cgmath::Vector3::<f32>::unit_z()
						* (game.player_phys.aligned_box.dims.z / 2.0 + 0.1))
					.map(|x| x.round() as i32);
				let player_chunk_coords = game
					.cd
					.world_coords_to_containing_chunk_coords(player_block_coords);

				let generation_distance_in_chunks = game.loading_distance / game.cd.edge as f32;
				let generation_distance_in_chunks_up = generation_distance_in_chunks.ceil() as i32;

				let mut neighbor_chunk_coords_array: Vec<_> =
					iter_3d_cube_center_radius(player_chunk_coords, generation_distance_in_chunks_up)
						.filter(|chunk_coords| {
							chunk_coords
								.map(|x| x as f32)
								.distance(player_chunk_coords.map(|x| x as f32))
								<= generation_distance_in_chunks
						})
						.collect();
				// No early optimizations! This is an (in)valid excuse to not optimize this!
				neighbor_chunk_coords_array.sort_unstable_by_key(|chunk_coords| {
					(chunk_coords
						.map(|x| x as f32)
						.distance2(player_chunk_coords.map(|x| x as f32))
						* 10.0) as u64
				});

				for neighbor_chunk_coords in neighbor_chunk_coords_array.into_iter() {
					let blocks_was_generated = game
						.chunk_grid
						.map
						.get(&neighbor_chunk_coords)
						.is_some_and(|chunk| chunk.blocks.is_some());
					let blocks_is_being_generated =
						game
							.worker_tasks
							.iter()
							.any(|worker_task| match worker_task {
								WorkerTask::GenerateChunkBlocks(chunk_coords, ..) => {
									*chunk_coords == neighbor_chunk_coords
								},
								_ => false,
							});
					let workers_dedicated_to_meshing = 1;
					if (!blocks_was_generated)
						&& (!blocks_is_being_generated)
						&& game.worker_tasks.len()
							< (game.pool.number_of_workers() - workers_dedicated_to_meshing)
					{
						// Asking a worker for the generation of chunk blocks
						let chunk_coords = neighbor_chunk_coords;
						let (sender, receiver) = std::sync::mpsc::channel();
						game
							.worker_tasks
							.push(WorkerTask::GenerateChunkBlocks(chunk_coords, receiver));
						let chunk_generator = Arc::clone(&game.world_generator);
						let coords_span = ChunkCoordsSpan { cd: game.cd, chunk_coords };
						let block_type_table = Arc::clone(&game.block_type_table);
						game.pool.enqueue_task(Box::new(move || {
							// TODO: Remove the sleeping!
							// Test delay to make sure that the main thread keeps working even
							// when the workers tasks take very long.
							//std::thread::sleep(std::time::Duration::from_secs_f32(
							//	rand::thread_rng().gen_range(0.1..0.3),
							//));

							let chunk_blocks =
								chunk_generator.generate_chunk_blocks(coords_span, block_type_table);
							let _ = sender.send(chunk_blocks);
						}));
					}
				}
			}

			// Unload chunks that are a bit too far.
			{
				let player_block_coords = (game.player_phys.aligned_box.pos
					- cgmath::Vector3::<f32>::unit_z()
						* (game.player_phys.aligned_box.dims.z / 2.0 + 0.1))
					.map(|x| x.round() as i32);
				let player_chunk_coords = game
					.cd
					.world_coords_to_containing_chunk_coords(player_block_coords);

				let unloading_distance = game.loading_distance + game.margin_before_unloading;
				let unloading_distance_in_chunks = unloading_distance / game.cd.edge as f32;
				for chunk_coords in chunk_coords_list.into_iter() {
					let dist_in_chunks = chunk_coords
						.map(|x| x as f32)
						.distance(player_chunk_coords.map(|x| x as f32));
					if dist_in_chunks > unloading_distance_in_chunks {
						// TODO: Save blocks to database on disk or something.
						game.chunk_grid.map.remove(&chunk_coords);
					}
				}
			}

			let walking_vector = {
				let walking_factor = if game.enable_physics { 12.0 } else { 35.0 } * dt.as_secs_f32();
				let walking_forward_factor = if game.walking_forward { 1 } else { 0 }
					+ if game.walking_backward { -1 } else { 0 };
				let walking_rightward_factor = if game.walking_rightward { 1 } else { 0 }
					+ if game.walking_leftward { -1 } else { 0 };
				let walking_forward_direction =
					game.camera_direction.to_horizontal().to_vec3() * walking_forward_factor as f32;
				let walking_rightward_direction = game
					.camera_direction
					.to_horizontal()
					.add_to_horizontal_angle(-TAU / 4.0)
					.to_vec3() * walking_rightward_factor as f32;
				let walking_vector_direction = walking_forward_direction + walking_rightward_direction;
				(if walking_vector_direction.magnitude() == 0.0 {
					walking_vector_direction
				} else {
					walking_vector_direction.normalize()
				} * walking_factor)
			};
			game.player_phys.aligned_box.pos += walking_vector;

			if game.enable_physics {
				physics::apply_on_physics_step(
					&mut game.player_phys,
					&game.chunk_grid,
					&game.block_type_table,
					dt,
				);
			}

			let player_box_mesh =
				SimpleLineMesh::from_aligned_box(&game.device, &game.player_phys.aligned_box);

			let first_person_camera_position = game.player_phys.aligned_box.pos
				+ cgmath::Vector3::<f32>::from((0.0, 0.0, game.player_phys.aligned_box.dims.z / 2.0))
					* 0.7;

			// Targeted block coords update.
			let direction = game.camera_direction.to_vec3();
			let mut position = first_person_camera_position;
			let mut last_position_int: Option<BlockCoords> = None;
			game.targeted_block_coords = loop {
				if first_person_camera_position.distance(position) > 6.0 {
					break None;
				}
				let position_int = position.map(|x| x.round() as i32);
				if game
					.chunk_grid
					.get_block(position_int)
					.is_some_and(|block_id| !game.block_type_table.get(block_id).unwrap().is_air())
				{
					if let Some(last_position_int) = last_position_int {
						break Some((position_int, last_position_int));
					} else {
						break None;
					}
				}
				if last_position_int != Some(position_int) {
					last_position_int = Some(position_int);
				}
				// TODO: Advance directly to the next block with exactly the right step distance,
				// also do not skip blocks (even a small arbitrary step can be too big sometimes).
				position += direction * 0.01;
			};

			let targeted_block_box_mesh_opt = game.targeted_block_coords.map(|(coords, _)| {
				SimpleLineMesh::from_aligned_box(
					&game.device,
					&AlignedBox {
						pos: coords.map(|x| x as f32),
						dims: cgmath::vec3(1.01, 1.01, 1.01),
					},
				)
			});

			let mut chunk_box_meshes = vec![];
			if game.enable_display_not_surrounded_chunks_as_boxes {
				for chunk_coords in game.chunk_grid.map.keys().copied() {
					let is_surrounded = 'is_surrounded: {
						for neighbor_chunk_coords in iter_3d_cube_center_radius(chunk_coords, 2) {
							let blocks_was_generated =
								game.chunk_grid.map.contains_key(&neighbor_chunk_coords);
							if !blocks_was_generated {
								break 'is_surrounded false;
							}
						}
						true
					};
					if !is_surrounded {
						let coords_span = ChunkCoordsSpan { cd: game.cd, chunk_coords };
						let inf = coords_span.block_coords_inf().map(|x| x as f32);
						let dims = coords_span.cd._dimensions().map(|x| x as f32);
						let pos = inf + dims / 2.0;
						chunk_box_meshes.push(SimpleLineMesh::from_aligned_box(
							&game.device,
							&AlignedBox { pos, dims },
						));
					}
				}
			}

			game.sun_position_in_sky.angle_horizontal += (TAU / 150.0) * dt.as_secs_f32();

			let sun_camera_view_projection_matrix = {
				let camera_position = first_person_camera_position;
				let camera_direction_vector = -game.sun_position_in_sky.to_vec3();
				let camera_up_vector = (0.0, 0.0, 1.0).into();
				game.sun_camera.view_projection_matrix(
					camera_position,
					camera_direction_vector,
					camera_up_vector,
				)
			};
			game.queue.write_buffer(
				&game.sun_camera_matrix_thingy.resource,
				0,
				bytemuck::cast_slice(&[sun_camera_view_projection_matrix]),
			);

			let camera_view_projection_matrix = {
				if matches!(game.selected_camera, WhichCameraToUse::Sun) {
					sun_camera_view_projection_matrix
				} else {
					let mut camera_position = first_person_camera_position;
					let camera_direction_vector = game.camera_direction.to_vec3();
					if matches!(game.selected_camera, WhichCameraToUse::ThirdPersonNear) {
						camera_position -= camera_direction_vector * 5.0;
					} else if matches!(game.selected_camera, WhichCameraToUse::ThirdPersonFar) {
						camera_position -= camera_direction_vector * 40.0;
					} else if matches!(game.selected_camera, WhichCameraToUse::ThirdPersonVeryFar) {
						camera_position -= camera_direction_vector * 200.0;
					}
					let camera_up_vector = game
						.camera_direction
						.add_to_vertical_angle(-TAU / 4.0)
						.to_vec3();
					game.camera_settings.view_projection_matrix(
						camera_position,
						camera_direction_vector,
						camera_up_vector,
					)
				}
			};
			game.queue.write_buffer(
				&game.camera_matrix_thingy.resource,
				0,
				bytemuck::cast_slice(&[camera_view_projection_matrix]),
			);

			let sun_light_direction =
				Vector3Pod { values: (-game.sun_position_in_sky.to_vec3()).into() };
			game.queue.write_buffer(
				&game.sun_light_direction_thingy.resource,
				0,
				bytemuck::cast_slice(&[sun_light_direction]),
			);

			let mut encoder = game
				.device
				.create_command_encoder(&wgpu::CommandEncoderDescriptor {
					label: Some("Render Encoder"),
				});

			// Render pass to generate the shadow map.
			{
				let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: Some("Render Pass for Shadow Map"),
					color_attachments: &[],
					depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
						view: &game.shadow_map_view_thingy.resource,
						depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: true }),
						stencil_ops: None,
					}),
				});

				render_pass.set_pipeline(&game.rendering.block_shadow_render_pipeline);
				render_pass.set_bind_group(0, &game.rendering.block_shadow_bind_group, &[]);
				for chunk in game.chunk_grid.map.values() {
					if let Some(ref mesh) = chunk.mesh {
						render_pass
							.set_vertex_buffer(0, mesh.block_vertex_buffer.as_ref().unwrap().slice(..));
						render_pass.draw(0..(mesh.block_vertices.len() as u32), 0..1);
					}
				}
			}

			// Render pass to render the world to the screen.
			let window_texture = game.window_surface.get_current_texture().unwrap();
			{
				let window_texture_view = window_texture
					.texture
					.create_view(&wgpu::TextureViewDescriptor::default());
				let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: Some("Render Pass to render the world"),
					color_attachments: &[Some(wgpu::RenderPassColorAttachment {
						view: &window_texture_view,
						resolve_target: None,
						ops: wgpu::Operations {
							load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.7, b: 1.0, a: 1.0 }),
							store: true,
						},
					})],
					depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
						view: &game.z_buffer_view,
						depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: true }),
						stencil_ops: None,
					}),
				});

				if matches!(game.selected_camera, WhichCameraToUse::Sun) {
					let scale = game.window_surface_config.height as f32 / game.sun_camera.height;
					let w = game.sun_camera.width * scale;
					let h = game.sun_camera.height * scale;
					let x = game.window_surface_config.width as f32 / 2.0 - w / 2.0;
					let y = game.window_surface_config.height as f32 / 2.0 - h / 2.0;
					render_pass.set_viewport(x, y, w, h, 0.0, 1.0);
				}

				render_pass.set_pipeline(&game.rendering.block_render_pipeline);
				render_pass.set_bind_group(0, &game.rendering.block_bind_group, &[]);
				for chunk in game.chunk_grid.map.values() {
					if let Some(ref mesh) = chunk.mesh {
						render_pass
							.set_vertex_buffer(0, mesh.block_vertex_buffer.as_ref().unwrap().slice(..));
						render_pass.draw(0..(mesh.block_vertices.len() as u32), 0..1);
					}
				}

				if game.enable_display_phys_box {
					render_pass.set_pipeline(&game.rendering.simple_line_render_pipeline);
					render_pass.set_bind_group(0, &game.rendering.simple_line_bind_group, &[]);
					render_pass.set_vertex_buffer(0, player_box_mesh.vertex_buffer.slice(..));
					render_pass.draw(0..(player_box_mesh.vertices.len() as u32), 0..1);
				}

				if let Some(targeted_block_box_mesh) = &targeted_block_box_mesh_opt {
					if game.enable_display_interface {
						render_pass.set_pipeline(&game.rendering.simple_line_render_pipeline);
						render_pass.set_bind_group(0, &game.rendering.simple_line_bind_group, &[]);
						render_pass.set_vertex_buffer(0, targeted_block_box_mesh.vertex_buffer.slice(..));
						render_pass.draw(0..(targeted_block_box_mesh.vertices.len() as u32), 0..1);
					}
				}

				for chunk_box_mesh in chunk_box_meshes.iter() {
					render_pass.set_pipeline(&game.rendering.simple_line_render_pipeline);
					render_pass.set_bind_group(0, &game.rendering.simple_line_bind_group, &[]);
					render_pass.set_vertex_buffer(0, chunk_box_mesh.vertex_buffer.slice(..));
					render_pass.draw(0..(chunk_box_mesh.vertices.len() as u32), 0..1);
				}
			}

			// Render pass to draw the interface.
			{
				game.queue.write_buffer(
					&game.offset_for_2d_thingy.resource,
					0,
					bytemuck::cast_slice(&[0.0f32, 0.0f32, 0.0f32]),
				);

				let window_texture_view = window_texture
					.texture
					.create_view(&wgpu::TextureViewDescriptor::default());
				let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: Some("Render Pass to render the interface"),
					color_attachments: &[Some(wgpu::RenderPassColorAttachment {
						view: &window_texture_view,
						resolve_target: None,
						ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: true },
					})],
					depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
						view: &game.z_buffer_view,
						depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: true }),
						stencil_ops: None,
					}),
				});

				render_pass.set_pipeline(&game.rendering.simple_line_2d_render_pipeline);
				render_pass.set_bind_group(0, &game.rendering.simple_line_2d_bind_group, &[]);
				if game.enable_display_interface
					&& !matches!(game.selected_camera, WhichCameraToUse::Sun)
					&& !game.typing_in_command_line
				{
					render_pass.set_vertex_buffer(0, game.cursor_mesh.vertex_buffer.slice(..));
					render_pass.draw(0..(game.cursor_mesh.vertices.len() as u32), 0..1);
				}

				render_pass.set_pipeline(&game.rendering.simple_texture_2d_render_pipeline);
				render_pass.set_bind_group(0, &game.rendering.simple_texture_2d_bind_group, &[]);
				if game.enable_display_interface
					&& !matches!(game.selected_camera, WhichCameraToUse::Sun)
				{
					render_pass.set_vertex_buffer(0, game.top_left_info_mesh.vertex_buffer.slice(..));
					render_pass.draw(0..(game.top_left_info_mesh.vertices.len() as u32), 0..1);

					render_pass.set_vertex_buffer(0, game.command_line_mesh.vertex_buffer.slice(..));
					render_pass.draw(0..(game.command_line_mesh.vertices.len() as u32), 0..1);
				}
			}

			game.queue.submit(std::iter::once(encoder.finish()));

			// Render passes to draw the log.
			// TODO: MAKE IT SO THAT WE DONT SUBMIT A WHOLE THING FOR EACH LINE
			// or make sure that it is not a big deal (but I would not count on that ><).
			for log_line in game.log.iter() {
				let mut encoder = game
					.device
					.create_command_encoder(&wgpu::CommandEncoderDescriptor {
						label: Some("Render Encoder for a log line"),
					});

				{
					let window_texture_view = window_texture
						.texture
						.create_view(&wgpu::TextureViewDescriptor::default());
					let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
						label: Some("Render Pass to render a log line"),
						color_attachments: &[Some(wgpu::RenderPassColorAttachment {
							view: &window_texture_view,
							resolve_target: None,
							ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: true },
						})],
						depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
							view: &game.z_buffer_view,
							depth_ops: Some(wgpu::Operations {
								load: wgpu::LoadOp::Clear(1.0),
								store: true,
							}),
							stencil_ops: None,
						}),
					});

					render_pass.set_pipeline(&game.rendering.simple_texture_2d_render_pipeline);
					render_pass.set_bind_group(0, &game.rendering.simple_texture_2d_bind_group, &[]);

					let offset = {
						let (x, y) = log_line.target_position;
						[x, y, 0.0]
					};
					game.queue.write_buffer(
						&game.offset_for_2d_thingy.resource,
						0,
						bytemuck::cast_slice(&[offset]),
					);

					render_pass.set_vertex_buffer(0, log_line.mesh.vertex_buffer.slice(..));
					render_pass.draw(0..(log_line.mesh.vertices.len() as u32), 0..1);
				}

				game.queue.submit(std::iter::once(encoder.finish()));
			}

			window_texture.present();

			if game.close_after_one_frame {
				println!("Closing after one frame, as asked via command line arguments");
				*control_flow = ControlFlow::Exit;
			}
		},
		_ => {},
	});
}
