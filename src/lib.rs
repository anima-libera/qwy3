#![allow(clippy::items_after_test_module)]

mod camera;
mod chunks;
mod commands;
mod coords;
mod line_meshes;
mod noise;
mod rendering;
mod shaders;
mod threadpool;

use std::{collections::HashMap, f32::consts::TAU, sync::Arc};

use cgmath::{InnerSpace, MetricSpace};
use rand::Rng;
use winit::event_loop::ControlFlow;

use camera::{aspect_ratio, CameraOrthographicSettings, CameraPerspectiveSettings, CameraSettings};
use chunks::*;
use coords::*;
use line_meshes::*;
use rendering::*;

/// Just a 3D rectangular axis-aligned box.
/// It cannot rotate as it stays aligned on the axes.
struct AlignedBox {
	/// Position of the center of the box.
	pos: cgmath::Point3<f32>,
	/// Width of the box along each axis.
	dims: cgmath::Vector3<f32>,
}

/// Represents an `AlignedBox`-shaped object that has physics or something like that.
struct AlignedPhysBox {
	aligned_box: AlignedBox,
	motion: cgmath::Vector3<f32>,
	/// Gravity's acceleration of this box is influenced by this parameter.
	/// It may not be exactly analog to weight but it's not too far.
	gravity_factor: f32,
}

/// Vector in 3D.
#[derive(Copy, Clone, Debug)]
/// Certified Plain Old Data (so it can be sent to the GPU as a uniform).
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vector3Pod {
	values: [f32; 3],
}

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
}

enum WorkerTask {
	GenerateChunkBlocks(ChunkCoords, std::sync::mpsc::Receiver<ChunkBlocks>),
	MeshChunk(ChunkCoords, std::sync::mpsc::Receiver<ChunkMesh>),
}

struct Game {
	window: winit::window::Window,
	window_surface: wgpu::Surface,
	device: Arc<wgpu::Device>,
	queue: wgpu::Queue,
	window_surface_config: wgpu::SurfaceConfiguration,
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
}

fn init_game() -> (Game, winit::event_loop::EventLoop<()>) {
	// Wgpu uses the `log`/`env_logger` crates to log errors and stuff,
	// and we do want to see the errors very much.
	env_logger::init();

	let mut number_of_threads = 12;
	let mut close_after_one_frame = false;

	let mut args = std::env::args().enumerate();
	args.next(); // Path to binary.
	while let Some((arg_index, arg_name)) = args.next() {
		match arg_name.as_str() {
			"--threads" => match args
				.next()
				.map(|(second_index, second_arg)| (second_index, str::parse::<u32>(&second_arg)))
			{
				Some((_second_index, Ok(number))) => number_of_threads = number,
				Some((second_index, Err(parsing_error))) => {
					println!(
						"Error in command line arguments at argument {second_index}: \
						Argument \"--threads\" is expected to be followed by an unsigned 32-bits \
						integer argument, but parsing failed: {parsing_error}"
					);
				},
				None => {
					println!(
						"Error in command line arguments at the end: \
						Argument \"--threads\" is expected to be followed by an unsigned 32-bits \
						integer argument, but no argument followed"
					);
				},
			},
			"--close-after-one-frame" => {
				println!("Will close after one frame");
				close_after_one_frame = true;
			},
			unknown_arg_name => {
				println!(
					"Error in command line arguments at argument {arg_index}: \
					Argument name \"{unknown_arg_name}\" is unknown"
				);
			},
		}
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

	if false {
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
			if tx * 2 < y || 16 * 2 - tx * 2 < y {
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
	let AtlasStuff { atlas_texture_view_thingy, atlas_texture_sampler_thingy } =
		init_atlas_stuff(Arc::clone(&device), &queue, &atlas_data);

	let camera_settings = CameraPerspectiveSettings {
		up_direction: (0.0, 0.0, 1.0).into(),
		aspect_ratio: window_surface_config.width as f32 / window_surface_config.height as f32,
		field_of_view_y: TAU / 4.0,
		near_plane: 0.001,
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
		aligned_box: AlignedBox { pos: (5.5, 5.5, 5.5).into(), dims: (0.8, 0.8, 1.8).into() },
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

	let cd = ChunkDimensions::from(16);
	let chunk_grid = ChunkGrid::new(cd);

	let enable_world_generation = true;

	let worker_tasks = vec![];
	let pool = threadpool::ThreadPool::new(number_of_threads as usize);

	let rendering = rendering::init_rendering_stuff(
		Arc::clone(&device),
		AllBindingThingies {
			camera_matrix_thingy: &camera_matrix_thingy,
			sun_light_direction_thingy: &sun_light_direction_thingy,
			sun_camera_matrix_thingy: &sun_camera_matrix_thingy,
			shadow_map_view_thingy: &shadow_map_view_thingy,
			shadow_map_sampler_thingy: &shadow_map_sampler_thingy,
			atlas_texture_view_thingy: &atlas_texture_view_thingy,
			atlas_texture_sampler_thingy: &atlas_texture_sampler_thingy,
		},
		shadow_map_format,
		window_surface_config.format,
		z_buffer_format,
	);

	let game = Game {
		window,
		window_surface,
		device,
		queue,
		window_surface_config,
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
				game.controls_to_trigger.push(ControlEvent {
					control: Control::KeyboardKey(*key),
					pressed: *state == ElementState::Pressed,
				});
			},

			WindowEvent::MouseInput { state, button, .. } if game.cursor_is_captured => {
				game.controls_to_trigger.push(ControlEvent {
					control: Control::MouseButton(*button),
					pressed: *state == ElementState::Pressed,
				});
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
						(_, false) => {},
					}
				}
			}
			game.controls_to_trigger.clear();

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
					WorkerTask::MeshChunk(chunk_coords, receiver) => {
						let chunk_coords_and_result_opt = receiver
							.try_recv()
							.ok()
							.map(|chunk_mesh| (*chunk_coords, chunk_mesh));
						let is_not_done_yet = chunk_coords_and_result_opt.is_none();
						if let Some((chunk_coords, chunk_mesh)) = chunk_coords_and_result_opt {
							let chunk = game.chunk_grid.map.get_mut(&chunk_coords).unwrap();
							chunk.mesh = Some(chunk_mesh);
						}
						is_not_done_yet
					},
				};
				is_not_done_yet
			});

			// Request meshing for chunks that can be meshed or should be re-meshed.
			let chunk_coords_list: Vec<_> = game.chunk_grid.map.keys().copied().collect();
			for chunk_coords in chunk_coords_list.into_iter() {
				let already_has_mesh = game
					.chunk_grid
					.map
					.get(&chunk_coords)
					.unwrap()
					.mesh
					.is_some();
				let is_being_meshed = game
					.worker_tasks
					.iter()
					.any(|worker_task| match worker_task {
						WorkerTask::MeshChunk(chunk_coords_uwu, ..) => *chunk_coords_uwu == chunk_coords,
						_ => false,
					});
				let can_be_meshed = 'can_be_meshed: {
					for neighbor_chunk_coords in iter_3d_cube_center_radius(chunk_coords, 2) {
						let blocks_was_generated = game
							.chunk_grid
							.map
							.get(&neighbor_chunk_coords)
							.is_some_and(|chunk| chunk.blocks.is_some());
						if !blocks_was_generated {
							break 'can_be_meshed false;
						}
					}
					true
				};
				let should_be_remeshed = game
					.chunk_grid
					.map
					.get(&chunk_coords)
					.is_some_and(|chunk| chunk.remeshing_required);
				if (((!already_has_mesh) && (!is_being_meshed)) || should_be_remeshed)
					&& can_be_meshed
					&& game.worker_tasks.len() < game.pool.number_of_workers()
				{
					// Asking a worker for the meshing or remeshing of the chunk
					game
						.chunk_grid
						.map
						.get_mut(&chunk_coords)
						.unwrap()
						.remeshing_required = false;
					let (sender, receiver) = std::sync::mpsc::channel();
					game
						.worker_tasks
						.push(WorkerTask::MeshChunk(chunk_coords, receiver));
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

				let mut neighbor_chunk_coords_array: Vec<_> =
					iter_3d_cube_center_radius(player_chunk_coords, 8).collect();
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
					if (!blocks_was_generated)
						&& (!blocks_is_being_generated)
						&& game.worker_tasks.len() < (game.pool.number_of_workers() - 2)
					{
						// Asking a worker for the generation of chunk blocks
						let chunk_coords = neighbor_chunk_coords;
						let (sender, receiver) = std::sync::mpsc::channel();
						game
							.worker_tasks
							.push(WorkerTask::GenerateChunkBlocks(chunk_coords, receiver));
						let chunk_generator = ChunkGenerator {};
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
				// TODO: Work out something better here,
				// although it is not very important at the moment.
				let player_bottom = game.player_phys.aligned_box.pos
					- cgmath::Vector3::<f32>::from((
						0.0,
						0.0,
						game.player_phys.aligned_box.dims.z / 2.0,
					));
				let player_bottom_below = game.player_phys.aligned_box.pos
					- cgmath::Vector3::<f32>::from((
						0.0,
						0.0,
						game.player_phys.aligned_box.dims.z / 2.0 + 0.01,
					));
				let player_bottom_block_coords = player_bottom.map(|x| x.round() as i32);
				let player_bottom_block_coords_below = player_bottom_below.map(|x| x.round() as i32);
				let player_bottom_block_opt = game.chunk_grid.get_block(player_bottom_block_coords);
				let player_bottom_block_opt_below =
					game.chunk_grid.get_block(player_bottom_block_coords_below);
				let is_on_ground = if game.player_phys.motion.z <= 0.0 {
					if let Some(block_id) = player_bottom_block_opt_below {
						if game.block_type_table.get(block_id).unwrap().is_opaque() {
							// The player is on the ground, so we make sure we are not overlapping it.
							game.player_phys.motion.z = 0.0;
							game.player_phys.aligned_box.pos.z =
								player_bottom_block_coords_below.z as f32
									+ 0.5 + game.player_phys.aligned_box.dims.z / 2.0;
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
				let is_in_ground = if game.player_phys.motion.z <= 0.0 {
					if let Some(block_id) = player_bottom_block_opt {
						if game.block_type_table.get(block_id).unwrap().is_opaque() {
							// The player is inside the ground, so we uuh.. do something?
							game.player_phys.motion.z = 0.0;
							game.player_phys.aligned_box.pos.z =
								player_bottom_block_coords.z as f32
									+ 0.5 + game.player_phys.aligned_box.dims.z / 2.0;
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
				game.player_phys.aligned_box.pos += game.player_phys.motion;
				if !is_on_ground {
					game.player_phys.motion.z -=
						game.player_phys.gravity_factor * 0.3 * dt.as_secs_f32();
				}
				if is_in_ground {
					game.player_phys.aligned_box.pos.z += 0.01;
				}
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

			let cursor_mesh = SimpleLineMesh::interface_2d_cursor(
				&game.device,
				(
					game.window_surface_config.width,
					game.window_surface_config.height,
				),
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
					render_pass.set_bind_group(0, &game.rendering.simple_line_render_bind_group, &[]);
					render_pass.set_vertex_buffer(0, player_box_mesh.vertex_buffer.slice(..));
					render_pass.draw(0..(player_box_mesh.vertices.len() as u32), 0..1);
				}

				if let Some(targeted_block_box_mesh) = &targeted_block_box_mesh_opt {
					render_pass.set_pipeline(&game.rendering.simple_line_render_pipeline);
					render_pass.set_bind_group(0, &game.rendering.simple_line_render_bind_group, &[]);
					render_pass.set_vertex_buffer(0, targeted_block_box_mesh.vertex_buffer.slice(..));
					render_pass.draw(0..(targeted_block_box_mesh.vertices.len() as u32), 0..1);
				}
			}

			// Render pass to draw the interface.
			{
				let window_texture_view = window_texture
					.texture
					.create_view(&wgpu::TextureViewDescriptor::default());
				let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: Some("Render Pass to render "),
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
				if !matches!(game.selected_camera, WhichCameraToUse::Sun) {
					render_pass.set_vertex_buffer(0, cursor_mesh.vertex_buffer.slice(..));
					render_pass.draw(0..(cursor_mesh.vertices.len() as u32), 0..1);
				}
			}

			game.queue.submit(std::iter::once(encoder.finish()));
			window_texture.present();

			if game.close_after_one_frame {
				println!("Closing after one frame, as asked via command line arguments");
				*control_flow = ControlFlow::Exit;
			}
		},
		_ => {},
	});
}
