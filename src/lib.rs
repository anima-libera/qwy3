#![allow(clippy::items_after_test_module)]

mod atlas;
mod block_types;
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
mod skybox;
mod texture_gen;
mod threadpool;
mod widgets;
mod world_gen;

use std::{
	collections::HashMap,
	f32::consts::TAU,
	sync::{atomic::AtomicI32, Arc},
};

use cgmath::{point3, ElementWise, EuclideanSpace, InnerSpace, MetricSpace};
use rand::Rng;
use skybox::SkyboxFaces;
use wgpu::util::DeviceExt;
use winit::event_loop::ControlFlow;

use camera::{aspect_ratio, CameraOrthographicSettings, CameraPerspectiveSettings, CameraSettings};
use chunks::*;
use coords::*;
use line_meshes::*;
use physics::AlignedPhysBox;
use rendering::*;
use shaders::{simple_texture_2d::SimpleTextureVertexPod, Vector3Pod};
use widgets::{InterfaceMeshesVertices, Widget, WidgetLabel};
use world_gen::WorldGenerator;

use crate::{
	atlas::Atlas,
	lang::LogItem,
	shaders::Vector2Pod,
	skybox::{
		default_skybox_painter, default_skybox_painter_3, generate_skybox_cubemap_faces_images,
		SkyboxMesh,
	},
};

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
	ToggleDisplayInterfaceDebugBoxes,
	ToggleFog,
	ToggleFullscreen,
}

enum WorkerTask {
	GenerateChunkBlocks(
		ChunkCoords,
		std::sync::mpsc::Receiver<(ChunkBlocks, ChunkCullingInfo)>,
	),
	MeshChunk(ChunkCoords, std::sync::mpsc::Receiver<ChunkMesh>),
	/// The counter at the end is the number of faces already finished.
	PaintNewSkybox(std::sync::mpsc::Receiver<SkyboxFaces>, Arc<AtomicI32>),
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
	) -> Vec<SimpleTextureVertexPod> {
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
	/// Chunk to consider regarding generation.
	chunk_generation_front: Vec<ChunkCoords>,
	/// Would be in `chunk_generation_front` if it was not outside the loading radius.
	chunk_generation_front_too_far: Vec<ChunkCoords>,
	/// Like `chunk_generation_front` but these are not the priority and can take their time.
	chunk_generation_front_not_priority: Vec<ChunkCoords>,
	controls_to_trigger: Vec<ControlEvent>,
	control_bindings: HashMap<Control, Action>,
	block_type_table: Arc<BlockTypeTable>,
	rendering: RenderPipelinesAndBindGroups,
	close_after_one_frame: bool,
	cursor_mesh: SimpleLineMesh,
	random_message: &'static str,
	font: font::Font,
	command_line_content: String,
	typing_in_command_line: bool,
	last_command_line_interaction: Option<std::time::Instant>,
	command_confirmed: bool,
	world_generator: Arc<dyn WorldGenerator + Sync + Send>,
	loading_distance: f32,
	margin_before_unloading: f32,
	widget_tree_root: Widget,
	enable_interface_draw_debug_boxes: bool,
	skybox_cubemap_texture: wgpu::Texture,
	fog_center_position_thingy: BindingThingy<wgpu::Buffer>,
	fog_inf_sup_radiuses_thingy: BindingThingy<wgpu::Buffer>,
	fog_inf_sup_radiuses: (f32, f32),

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
	enable_fog: bool,
	enable_fullscreen: bool,
}

fn init_game() -> (Game, winit::event_loop::EventLoop<()>) {
	// Wgpu uses the `log`/`env_logger` crates to log errors and stuff,
	// and we do want to see the errors very much.
	env_logger::init();

	if cfg!(debug_assertions) {
		println!(
			"Running a debug build.\n\
			Note that better performances are possible with a release build,\n\
			using the command `cargo run --release -- [arguments for Qwy3]`"
		);
	}

	let cmdline::CommandLineSettings {
		number_of_threads,
		close_after_one_frame,
		verbose,
		output_atlas,
		world_gen_seed,
		which_world_generator,
		display_world_generator_possible_names,
		loading_distance,
		chunk_edge,
		fullscreen,
		no_vsync,
		fog,
		test_lang,
	} = cmdline::parse_command_line_arguments();

	if display_world_generator_possible_names {
		crate::cmdline::display_world_generator_names();
		std::process::exit(0);
	}

	if let Some(test_id) = test_lang {
		println!("Test lang: test id {test_id}");
		lang::test_lang(test_id);
		std::process::exit(0);
	}

	let enable_fullscreen = fullscreen;

	let event_loop = winit::event_loop::EventLoop::new();
	let window = winit::window::WindowBuilder::new()
		.with_title("Qwy3")
		.with_maximized(true)
		.with_resizable(true)
		.with_fullscreen(fullscreen.then_some(winit::window::Fullscreen::Borderless(None)))
		.build(&event_loop)
		.unwrap();
	let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
		backends: wgpu::Backends::all(),
		dx12_shader_compiler: Default::default(),
	});
	let window_surface = unsafe { instance.create_surface(&window) }.unwrap();

	// Try to get a cool adapter first.
	let adapter = instance.enumerate_adapters(wgpu::Backends::all()).find(|adapter| {
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
	let desired_present_mode = if no_vsync {
		if !surface_capabilities.present_modes.contains(&wgpu::PresentMode::Immediate) {
			println!("Warning: Immediate present mode (V-Sync Off) not available.");
			wgpu::PresentMode::Fifo
		} else {
			wgpu::PresentMode::Immediate
		}
	} else {
		wgpu::PresentMode::Fifo
	};
	assert!(surface_capabilities.present_modes.contains(&desired_present_mode));
	let size = window.inner_size();
	let window_surface_config = wgpu::SurfaceConfiguration {
		usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
		format: surface_format,
		width: size.width,
		height: size.height,
		present_mode: desired_present_mode,
		alpha_mode: surface_capabilities.alpha_modes[0],
		view_formats: vec![],
	};
	window_surface.configure(&device, &window_surface_config);

	let aspect_ratio_thingy = init_aspect_ratio_thingy(Arc::clone(&device));

	let block_type_table = Arc::new(BlockTypeTable::new());

	let atlas = Atlas::new(world_gen_seed);
	if output_atlas {
		let path = "atlas.png";
		println!("Outputting atlas to \"{path}\"");
		atlas.image.save_with_format(path, image::ImageFormat::Png).unwrap();
	}
	let AtlasStuff { atlas_texture_view_thingy, atlas_texture_sampler_thingy } =
		init_atlas_stuff(Arc::clone(&device), &queue, atlas.image.as_ref());

	let font = font::Font::font_01();

	let skybox_faces = generate_skybox_cubemap_faces_images(&default_skybox_painter, None);
	let SkyboxStuff {
		skybox_cubemap_texture_view_thingy,
		skybox_cubemap_texture_sampler_thingy,
		skybox_cubemap_texture,
	} = init_skybox_stuff(Arc::clone(&device), &queue, &skybox_faces.data());
	// The better painter that takes significantly more time will be run on a worker thread.
	let longer_skybox_painter = default_skybox_painter_3(4, world_gen_seed);

	let FogStuff { fog_center_position_thingy, fog_inf_sup_radiuses_thingy } =
		init_fog_stuff(Arc::clone(&device));

	let enable_fog = fog;

	queue.write_buffer(
		&fog_center_position_thingy.resource,
		0,
		bytemuck::cast_slice(&[Vector3Pod { values: [0.0, 0.0, 0.0] }]),
	);
	let fog_inf_sup_radiuses = (0.0, 20.0);
	queue.write_buffer(
		&fog_inf_sup_radiuses_thingy.resource,
		0,
		bytemuck::cast_slice(&[Vector2Pod {
			values: if enable_fog {
				[fog_inf_sup_radiuses.0, fog_inf_sup_radiuses.1]
			} else {
				[10000.0, 10000.0]
			},
		}]),
	);

	let camera_settings = CameraPerspectiveSettings {
		up_direction: (0.0, 0.0, 1.0).into(),
		aspect_ratio: window_surface_config.width as f32 / window_surface_config.height as f32,
		field_of_view_y: TAU / 4.0,
		near_plane: 0.1,
		far_plane: 1000.0,
	};
	let camera_matrix_thingy = init_camera_matrix_thingy(Arc::clone(&device));

	let camera_direction = AngularDirection::from_angle_horizontal(0.0);

	let selected_camera = WhichCameraToUse::FirstPerson;

	let cursor_is_captured = true;
	let cursor_was_actually_captured =
		window.set_cursor_grab(winit::window::CursorGrabMode::Confined).is_ok();
	if cursor_was_actually_captured {
		window.set_cursor_visible(false);
	}

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

	let mut chunk_generation_front = vec![];
	{
		let player_block_coords = (player_phys.aligned_box.pos
			- cgmath::Vector3::<f32>::unit_z() * (player_phys.aligned_box.dims.z / 2.0 + 0.1))
			.map(|x| x.round() as i32);
		let player_chunk_coords = cd.world_coords_to_containing_chunk_coords(player_block_coords);

		chunk_generation_front.push(player_chunk_coords);
	}
	let chunk_generation_front_too_far = vec![];
	let chunk_generation_front_not_priority = vec![];

	let enable_world_generation = true;

	let mut worker_tasks = vec![];
	let pool = threadpool::ThreadPool::new(number_of_threads as usize);

	let face_counter = {
		let (sender, receiver) = std::sync::mpsc::channel();
		let face_counter = Arc::new(AtomicI32::new(0));
		worker_tasks.push(WorkerTask::PaintNewSkybox(
			receiver,
			Arc::clone(&face_counter),
		));
		let cloned_face_counter = Arc::clone(&face_counter);
		pool.enqueue_task(Box::new(move || {
			let skybox_faces =
				generate_skybox_cubemap_faces_images(&longer_skybox_painter, Some(cloned_face_counter));
			let _ = sender.send(skybox_faces);
		}));
		face_counter
	};

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
			skybox_cubemap_texture_view_thingy: &skybox_cubemap_texture_view_thingy,
			skybox_cubemap_texture_sampler_thingy: &skybox_cubemap_texture_sampler_thingy,
			fog_center_position_thingy: &fog_center_position_thingy,
			fog_inf_sup_radiuses_thingy: &fog_inf_sup_radiuses_thingy,
		},
		shadow_map_format,
		window_surface_config.format,
		z_buffer_format,
	);

	let cursor_mesh = SimpleLineMesh::interface_2d_cursor(&device);

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

	let command_line_content = String::new();
	let typing_in_command_line = false;
	let last_command_line_interaction = None;
	let command_confirmed = false;

	let world_generator = which_world_generator.get_the_actual_generator(world_gen_seed);

	let margin_before_unloading = 60.0;

	let enable_display_not_surrounded_chunks_as_boxes = false;

	let widget_tree_root = Widget::new_margins(
		(5.0, 5.0, 0.0, 0.0),
		Box::new(Widget::new_list(
			vec![
				Widget::new_labeled_nothing(WidgetLabel::GeneralDebugInfo),
				Widget::new_smoothly_incoming(
					cgmath::point2(1.0, 0.0),
					std::time::Instant::now(),
					std::time::Duration::from_secs_f32(1.0),
					Box::new(Widget::new_simple_text(
						"nyoom >w<".to_string(),
						font::TextRenderingSettings::with_scale(3.0),
					)),
				),
				Widget::new_label(
					WidgetLabel::LogLineList,
					Box::new(Widget::new_list(
						vec![Widget::new_disappear_when_complete(
							std::time::Duration::from_secs_f32(2.0),
							Box::new(Widget::new_face_counter(
								font::TextRenderingSettings::with_scale(3.0),
								face_counter,
							)),
						)],
						5.0,
					)),
				),
				Widget::new_simple_text(
					"test (stays below log)".to_string(),
					font::TextRenderingSettings::with_scale(3.0),
				),
			],
			5.0,
		)),
	);

	let enable_interface_draw_debug_boxes = false;

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
		chunk_generation_front,
		chunk_generation_front_too_far,
		chunk_generation_front_not_priority,
		controls_to_trigger,
		control_bindings,
		block_type_table,
		rendering,
		close_after_one_frame,
		cursor_mesh,
		random_message,
		font,
		command_line_content,
		typing_in_command_line,
		last_command_line_interaction,
		command_confirmed,
		world_generator,
		loading_distance,
		margin_before_unloading,
		widget_tree_root,
		enable_interface_draw_debug_boxes,
		skybox_cubemap_texture,
		fog_center_position_thingy,
		fog_inf_sup_radiuses_thingy,
		fog_inf_sup_radiuses,

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
		enable_fog,
		enable_fullscreen,
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
				game.window_surface.configure(&game.device, &game.window_surface_config);
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
				game.window.set_cursor_grab(winit::window::CursorGrabMode::Confined).unwrap();
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
			let direction_left_or_right =
				game.camera_direction.to_horizontal().add_to_horizontal_angle(TAU / 4.0 * dx.signum());
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
								game.window.set_cursor_grab(winit::window::CursorGrabMode::None).unwrap();
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
						(Action::ToggleDisplayInterfaceDebugBoxes, true) => {
							game.enable_interface_draw_debug_boxes =
								!game.enable_interface_draw_debug_boxes;
						},
						(Action::ToggleFog, true) => {
							game.enable_fog = !game.enable_fog;
							let (inf, sup) = if game.enable_fog {
								game.fog_inf_sup_radiuses
							} else {
								(10000.0, 10000.0)
							};
							game.queue.write_buffer(
								&game.fog_inf_sup_radiuses_thingy.resource,
								0,
								bytemuck::cast_slice(&[Vector2Pod { values: [inf, sup] }]),
							);
						},
						(Action::ToggleFullscreen, true) => {
							game.enable_fullscreen = !game.enable_fullscreen;
							game.window.set_fullscreen(
								game
									.enable_fullscreen
									.then_some(winit::window::Fullscreen::Borderless(None)),
							);
						},
						(_, false) => {},
					}
				}
			}
			game.controls_to_trigger.clear();

			let mut interface_meshes_vertices = InterfaceMeshesVertices::new();

			// Top left info.
			if let Some(general_debug_info_widget) =
				game.widget_tree_root.find_label_content(WidgetLabel::GeneralDebugInfo)
			{
				let fps = 1.0 / dt.as_secs_f32();
				let chunk_count = game.chunk_grid.map.len();
				let block_count = chunk_count * game.cd.number_of_blocks();
				let chunk_meshed_count = game
					.chunk_grid
					.map
					.iter()
					.filter(|(_chunk_coords, chunk)| chunk.mesh.is_some())
					.count();
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
				let text = format!(
					"fps: {fps}\n\
					chunks loaded: {chunk_count}\n\
					blocks loaded: {block_count}\n\
					chunks meshed: {chunk_meshed_count}\n\
					player coords: {player_block_coords_str}\n\
					{random_message}"
				);
				*general_debug_info_widget = Widget::new_simple_text(text, settings);
			}

			// Command line handling.
			if game.command_confirmed {
				let text = game.command_line_content.clone();

				let mut log = lang::Log::new();
				let res = lang::run(&text, &mut lang::Context::with_builtins(), &mut log);

				let text = if let Err(error) = res {
					format!("{error:?}")
				} else {
					let lines: Vec<_> = log
						.log_items
						.into_iter()
						.map(|item| match item {
							LogItem::Text(text) => text,
						})
						.collect();
					lines.join("\n")
				};

				let widget = if text.is_empty() {
					let scale = rand::thread_rng().gen_range(1..=3) as f32;
					let settings = font::TextRenderingSettings::with_scale(scale);
					let text = "uwu test".to_string();
					Widget::new_simple_text(text, settings)
				} else {
					let settings = font::TextRenderingSettings::with_scale(3.0);
					Widget::new_simple_text(text, settings)
				};

				if let Some(Widget::List { sub_widgets, .. }) =
					game.widget_tree_root.find_label_content(WidgetLabel::LogLineList)
				{
					sub_widgets.push(Widget::new_smoothly_incoming(
						cgmath::point2(0.0, 0.0),
						std::time::Instant::now(),
						std::time::Duration::from_secs_f32(1.0),
						Box::new(widget),
					));

					if sub_widgets.iter().filter(|widget| !widget.is_diappearing()).count() > 25 {
						let window_width = game.window_surface_config.width as f32;
						sub_widgets
							.iter_mut()
							.find(|widget| !widget.is_diappearing())
							.expect("we just checked that there are at least some amout of them")
							.pop_while_smoothly_closing_space(
								std::time::Instant::now(),
								std::time::Duration::from_secs_f32(1.0),
								&game.font,
								window_width,
							);
					}
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
				let y = 0.0 + dimensions.y / 2.0;
				let x = 0.0 - dimensions.x / 2.0;
				// Somehow this makes it pixel perfect, somehow?
				let x = (x * (window_width * 8.0) - 0.5).floor() / (window_width * 8.0);
				let text_displayed = if carret_visible {
					command_line_content_with_carret.as_str()
				} else {
					command_line_content
				};
				let simple_texture_vertices = game.font.simple_texture_vertices_from_text(
					window_width,
					cgmath::point3(x, y, 0.5),
					settings,
					text_displayed,
				);
				interface_meshes_vertices.add_simple_texture_vertices(simple_texture_vertices);
			}

			// Interface widget tree.
			{
				let window_width = game.window_surface_config.width as f32;
				let window_height = game.window_surface_config.height as f32;

				game.widget_tree_root.for_each_rec(&mut |widget| {
					if let Widget::DisappearWhenComplete {
						sub_widget,
						completed_time,
						delay_before_disappearing,
					} = widget
					{
						if sub_widget.is_completed() && completed_time.is_none() {
							*completed_time = Some(std::time::Instant::now());
						} else if completed_time.is_some_and(|completed_time| {
							completed_time.elapsed() > *delay_before_disappearing
						}) {
							widget.pop_while_smoothly_closing_space(
								std::time::Instant::now(),
								std::time::Duration::from_secs_f32(0.5),
								&game.font,
								window_width,
							);
						}
					}
				});

				game.widget_tree_root.generate_mesh_vertices(
					cgmath::point3(-1.0, window_height / window_width, 0.5),
					&mut interface_meshes_vertices,
					&game.font,
					window_width,
					game.enable_interface_draw_debug_boxes,
				);
			}

			// Recieve task results from workers.
			game.worker_tasks.retain_mut(|worker_task| {
				let is_not_done_yet = match worker_task {
					WorkerTask::GenerateChunkBlocks(chunk_coords, receiver) => {
						let chunk_coords_and_result_opt =
							receiver.try_recv().ok().map(|(chunk_blocks, chunk_culling_info)| {
								(*chunk_coords, chunk_blocks, chunk_culling_info)
							});
						let is_not_done_yet = chunk_coords_and_result_opt.is_none();
						if let Some((chunk_coords, chunk_blocks, chunk_culling_info)) =
							chunk_coords_and_result_opt
						{
							let coords_span = ChunkCoordsSpan { cd: game.cd, chunk_coords };
							let mut chunk = Chunk::new_empty(coords_span);

							chunk.blocks = Some(chunk_blocks);
							chunk.culling_info = Some(chunk_culling_info.clone());
							game.chunk_grid.map.insert(chunk_coords, chunk);

							for neighbor_chunk_coords in iter_3d_cube_center_radius(chunk_coords, 2) {
								if let Some(neighbor_chunk) =
									game.chunk_grid.map.get_mut(&neighbor_chunk_coords)
								{
									neighbor_chunk.remeshing_required = true;
								}
							}

							for straight_direction in OrientedAxis::all_the_six_possible_directions() {
								if !chunk_culling_info.all_opaque_faces.contains(&straight_direction) {
									let delta = straight_direction.delta();
									let adjacent_chunk_coords = chunk_coords + delta;
									let is_priority =
										!chunk_culling_info.all_air_faces.contains(&straight_direction);
									(if is_priority {
										&mut game.chunk_generation_front
									} else {
										&mut game.chunk_generation_front_not_priority
									})
									.push(adjacent_chunk_coords);
								}
							}
						}
						is_not_done_yet
					},
					WorkerTask::MeshChunk(chunk_coords, receiver) => {
						let chunk_coords_and_result_opt =
							receiver.try_recv().ok().map(|chunk_mesh| (*chunk_coords, chunk_mesh));
						let is_not_done_yet = chunk_coords_and_result_opt.is_none();
						if let Some((chunk_coords, chunk_mesh)) = chunk_coords_and_result_opt {
							if let Some(chunk) = game.chunk_grid.map.get_mut(&chunk_coords) {
								chunk.mesh = Some(chunk_mesh);
							} else {
								// The chunk have been unloaded since the meshing was ordered.
								// It really can happen, for example when the player travels very fast.
							}
						}
						is_not_done_yet
					},
					WorkerTask::PaintNewSkybox(receiver, _face_counter) => {
						let result_opt = receiver.try_recv().ok();
						let is_not_done_yet = result_opt.is_none();
						if let Some(skybox_faces) = result_opt {
							update_skybox_texture(
								&game.queue,
								&game.skybox_cubemap_texture,
								&skybox_faces.data(),
							);
						}
						is_not_done_yet
					},
				};
				is_not_done_yet
			});

			// Request meshing for chunks that can be meshed or should be re-meshed.
			let mut closest_unmeshed_chunk_distance: Option<f32> = None;
			let chunk_coords_list: Vec<_> = game.chunk_grid.map.keys().copied().collect();
			for chunk_coords in chunk_coords_list.iter().copied() {
				let already_has_mesh =
					game.chunk_grid.map.get(&chunk_coords).map(|chunk| chunk.mesh.is_some()).unwrap();

				if !already_has_mesh {
					let chunk_span = ChunkCoordsSpan { cd: game.cd, chunk_coords };
					let center = (chunk_span.block_coords_inf().map(|x| x as f32)
						+ chunk_span.block_coords_sup_excluded().map(|x| x as f32 - 1.0).to_vec())
						/ 2.0;
					let distance = center.distance(game.player_phys.aligned_box.pos);
					// Remove the longest radius of the chunk to get the worst case distance.
					let sqrt_3 = 3.0_f32.sqrt();
					let distance = distance - game.cd.edge as f32 * sqrt_3 / 2.0;
					if let Some(previous_distance) = closest_unmeshed_chunk_distance {
						if previous_distance > distance {
							closest_unmeshed_chunk_distance = Some(distance);
						}
					} else {
						closest_unmeshed_chunk_distance = Some(distance);
					}
				}

				let doesnt_need_mesh = game.chunk_grid.map.get(&chunk_coords).is_some_and(|chunk| {
					chunk.culling_info.as_ref().is_some_and(|culling_info| culling_info.all_air)
				});
				let is_being_meshed = game.worker_tasks.iter().any(|worker_task| match worker_task {
					WorkerTask::MeshChunk(chunk_coords_uwu, ..) => *chunk_coords_uwu == chunk_coords,
					_ => false,
				});
				let should_be_remeshed =
					game.chunk_grid.map.get(&chunk_coords).is_some_and(|chunk| chunk.remeshing_required);
				let shall_be_meshed = (!doesnt_need_mesh)
					&& (((!already_has_mesh) && (!is_being_meshed)) || should_be_remeshed)
					&& game.worker_tasks.len() < game.pool.number_of_workers();
				if shall_be_meshed {
					// Asking a worker for the meshing or remeshing of the chunk
					game.chunk_grid.map.get_mut(&chunk_coords).unwrap().remeshing_required = false;
					let (sender, receiver) = std::sync::mpsc::channel();
					game.worker_tasks.push(WorkerTask::MeshChunk(chunk_coords, receiver));
					let opaqueness_layer = game.chunk_grid.get_opaqueness_layer_around_chunk(
						chunk_coords,
						true,
						Arc::clone(&game.block_type_table),
					);
					let opaqueness_layer_for_ambiant_occlusion =
						game.chunk_grid.get_opaqueness_layer_around_chunk(
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
							opaqueness_layer_for_ambiant_occlusion,
							block_type_table,
						);
						mesh.update_gpu_data(&device);
						let _ = sender.send(mesh);
					}));
				}
			}

			// Handle fog adjustment.
			if let Some(distance) = closest_unmeshed_chunk_distance {
				let delta = distance - game.fog_inf_sup_radiuses.1;
				let delta_normalized = if delta < 0.0 {
					-1.0
				} else if delta > 0.0 {
					1.0
				} else {
					0.0
				};
				let evolution = delta_normalized * dt.as_secs_f32() * 15.0;
				let evolution = evolution.clamp(-delta.abs(), delta.abs());
				if evolution != 0.0 {
					game.fog_inf_sup_radiuses.1 += evolution;
					game.fog_inf_sup_radiuses.1 = game.fog_inf_sup_radiuses.1.max(20.0);
					game.fog_inf_sup_radiuses.0 = game.fog_inf_sup_radiuses.1 - 20.0;
					if game.enable_fog {
						game.queue.write_buffer(
							&game.fog_inf_sup_radiuses_thingy.resource,
							0,
							bytemuck::cast_slice(&[Vector2Pod {
								values: [game.fog_inf_sup_radiuses.0, game.fog_inf_sup_radiuses.1],
							}]),
						);
					}
				}
			}

			// Request generation of chunk blocks for not-generated not-being-generated close chunks.
			if game.enable_world_generation {
				let workers_dedicated_to_meshing = 1;
				let available_workers_to_generate = (game.pool.number_of_workers()
					- workers_dedicated_to_meshing)
					.saturating_sub(game.worker_tasks.len());

				if available_workers_to_generate >= 1 {
					let player_block_coords = (game.player_phys.aligned_box.pos
						- cgmath::Vector3::<f32>::unit_z()
							* (game.player_phys.aligned_box.dims.z / 2.0 + 0.1))
						.map(|x| x.round() as i32);
					let player_chunk_coords =
						game.cd.world_coords_to_containing_chunk_coords(player_block_coords);

					let generation_distance_in_chunks = game.loading_distance / game.cd.edge as f32;
					let unloading_distance_in_chunks =
						(game.loading_distance + game.margin_before_unloading) / game.cd.edge as f32;

					if game.chunk_generation_front.is_empty() {
						game.chunk_generation_front.append(&mut game.chunk_generation_front_not_priority);
					} else if let Some(front_chunk_coords) =
						game.chunk_generation_front_not_priority.pop()
					{
						game.chunk_generation_front.push(front_chunk_coords);
					}

					game.chunk_generation_front.retain(|front_chunk_coords| {
						let too_far = front_chunk_coords
							.map(|x| x as f32)
							.distance(player_chunk_coords.map(|x| x as f32))
							> generation_distance_in_chunks;
						if too_far {
							game.chunk_generation_front_too_far.push(*front_chunk_coords);
						}
						!too_far
					});

					game.chunk_generation_front_too_far.retain(|front_chunk_coords| {
						let way_too_far = front_chunk_coords
							.map(|x| x as f32)
							.distance(player_chunk_coords.map(|x| x as f32))
							> unloading_distance_in_chunks;
						!way_too_far
					});

					if !game.chunk_generation_front_too_far.is_empty() {
						for _ in 0..3 {
							// Just checking a few per frame at random should be enough.
							if game.chunk_generation_front_too_far.is_empty() {
								break;
							}
							let index =
								rand::thread_rng().gen_range(0..game.chunk_generation_front_too_far.len());
							let front_chunk_coords = game.chunk_generation_front_too_far[index];
							let still_too_far = front_chunk_coords
								.map(|x| x as f32)
								.distance(player_chunk_coords.map(|x| x as f32))
								> generation_distance_in_chunks;
							if !still_too_far {
								game.chunk_generation_front_too_far.remove(index);
								game.chunk_generation_front.push(front_chunk_coords);
							}
						}
					}

					game.chunk_generation_front.push(player_chunk_coords);
					for direction in OrientedAxis::all_the_six_possible_directions() {
						game.chunk_generation_front.push(player_chunk_coords + direction.delta());
					}

					game.chunk_generation_front.retain(|front_chunk_coords| {
						let blocks_was_generated = game
							.chunk_grid
							.map
							.get(front_chunk_coords)
							.is_some_and(|chunk| chunk.blocks.is_some());
						let blocks_is_being_generated =
							game.worker_tasks.iter().any(|worker_task| match worker_task {
								WorkerTask::GenerateChunkBlocks(chunk_coords, ..) =>
									chunk_coords == front_chunk_coords,
								_ => false,
							});
						(!blocks_was_generated) && (!blocks_is_being_generated)
					});

					// Sort to put closer chunks at the end.
					game.chunk_generation_front.sort_unstable_by_key(|chunk_coords| {
						-(chunk_coords.map(|x| x as f32).distance2(player_chunk_coords.map(|x| x as f32))
							* 10.0) as i64
					});

					let mut slot_count = available_workers_to_generate;
					while slot_count >= 1 {
						let considered_chunk_coords = game.chunk_generation_front.pop();
						let considered_chunk_coords = match considered_chunk_coords {
							Some(chunk_coords) => chunk_coords,
							None => break,
						};

						let blocks_was_generated = game
							.chunk_grid
							.map
							.get(&considered_chunk_coords)
							.is_some_and(|chunk| chunk.blocks.is_some());
						let blocks_is_being_generated =
							game.worker_tasks.iter().any(|worker_task| match worker_task {
								WorkerTask::GenerateChunkBlocks(chunk_coords, ..) =>
									*chunk_coords == considered_chunk_coords,
								_ => false,
							});

						if (!blocks_was_generated) && (!blocks_is_being_generated) {
							slot_count -= 1;

							// Asking a worker for the generation of chunk blocks.
							let chunk_coords = considered_chunk_coords;
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

								let chunk_blocks = chunk_generator
									.generate_chunk_blocks(coords_span, Arc::clone(&block_type_table));
								let chunk_culling_info =
									ChunkCullingInfo::compute_from_blocks(&chunk_blocks, block_type_table);
								let _ = sender.send((chunk_blocks, chunk_culling_info));
							}));
						}
					}
				}
			}

			// Unload chunks that are a bit too far.
			{
				let player_block_coords = (game.player_phys.aligned_box.pos
					- cgmath::Vector3::<f32>::unit_z()
						* (game.player_phys.aligned_box.dims.z / 2.0 + 0.1))
					.map(|x| x.round() as i32);
				let player_chunk_coords =
					game.cd.world_coords_to_containing_chunk_coords(player_block_coords);

				let unloading_distance = game.loading_distance + game.margin_before_unloading;
				let unloading_distance_in_chunks = unloading_distance / game.cd.edge as f32;
				for chunk_coords in chunk_coords_list.into_iter() {
					let dist_in_chunks =
						chunk_coords.map(|x| x as f32).distance(player_chunk_coords.map(|x| x as f32));
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
				let walking_rightward_direction =
					game.camera_direction.to_horizontal().add_to_horizontal_angle(-TAU / 4.0).to_vec3()
						* walking_rightward_factor as f32;
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

			game.queue.write_buffer(
				&game.fog_center_position_thingy.resource,
				0,
				bytemuck::cast_slice(&[Vector3Pod { values: game.player_phys.aligned_box.pos.into() }]),
			);

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

			let (camera_view_projection_matrix, camera_position_ifany) = {
				if matches!(game.selected_camera, WhichCameraToUse::Sun) {
					(sun_camera_view_projection_matrix, None)
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
					let camera_up_vector =
						game.camera_direction.add_to_vertical_angle(-TAU / 4.0).to_vec3();
					let camera_view_projection_matrix = game.camera_settings.view_projection_matrix(
						camera_position,
						camera_direction_vector,
						camera_up_vector,
					);
					(camera_view_projection_matrix, Some(camera_position))
				}
			};
			game.queue.write_buffer(
				&game.camera_matrix_thingy.resource,
				0,
				bytemuck::cast_slice(&[camera_view_projection_matrix]),
			);

			let skybox_mesh = SkyboxMesh::new(
				&game.device,
				camera_position_ifany.unwrap_or(point3(0.0, 0.0, 0.0)),
			);

			let sun_light_direction =
				Vector3Pod { values: (-game.sun_position_in_sky.to_vec3()).into() };
			game.queue.write_buffer(
				&game.sun_light_direction_thingy.resource,
				0,
				bytemuck::cast_slice(&[sun_light_direction]),
			);

			let interface_simple_texture_mesh = SimpleTextureMesh::from_vertices(
				&game.device,
				interface_meshes_vertices.simple_texture_vertices,
			);
			let interface_simple_line_mesh = SimpleLineMesh::from_vertices(
				&game.device,
				interface_meshes_vertices.simple_line_vertices,
			);

			let mut encoder = game.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
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

			// Render pass to render the skybox to the screen.
			let window_texture = game.window_surface.get_current_texture().unwrap();
			{
				let window_texture_view =
					window_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
				let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: Some("Render Pass to render the skybox"),
					color_attachments: &[Some(wgpu::RenderPassColorAttachment {
						view: &window_texture_view,
						resolve_target: None,
						ops: wgpu::Operations {
							load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.7, b: 1.0, a: 1.0 }),
							store: true,
						},
					})],
					depth_stencil_attachment: None,
				});

				if matches!(game.selected_camera, WhichCameraToUse::Sun) {
					let scale = game.window_surface_config.height as f32 / game.sun_camera.height;
					let w = game.sun_camera.width * scale;
					let h = game.sun_camera.height * scale;
					let x = game.window_surface_config.width as f32 / 2.0 - w / 2.0;
					let y = game.window_surface_config.height as f32 / 2.0 - h / 2.0;
					render_pass.set_viewport(x, y, w, h, 0.0, 1.0);
				}

				render_pass.set_pipeline(&game.rendering.skybox_render_pipeline);
				render_pass.set_bind_group(0, &game.rendering.skybox_bind_group, &[]);
				render_pass.set_vertex_buffer(0, skybox_mesh.vertex_buffer.slice(..));
				render_pass.draw(0..(skybox_mesh.vertices.len() as u32), 0..1);
			}

			// Render pass to render the world to the screen.
			{
				let window_texture_view =
					window_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
				let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: Some("Render Pass to render the world"),
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
				let window_texture_view =
					window_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
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

				if game.enable_display_interface
					&& !matches!(game.selected_camera, WhichCameraToUse::Sun)
					&& !game.typing_in_command_line
				{
					render_pass.set_pipeline(&game.rendering.simple_line_2d_render_pipeline);
					render_pass.set_bind_group(0, &game.rendering.simple_line_2d_bind_group, &[]);
					render_pass.set_vertex_buffer(0, game.cursor_mesh.vertex_buffer.slice(..));
					render_pass.draw(0..(game.cursor_mesh.vertices.len() as u32), 0..1);
				}

				if game.enable_display_interface
					&& !matches!(game.selected_camera, WhichCameraToUse::Sun)
				{
					render_pass.set_pipeline(&game.rendering.simple_texture_2d_render_pipeline);
					render_pass.set_bind_group(0, &game.rendering.simple_texture_2d_bind_group, &[]);
					let mesh = &interface_simple_texture_mesh;
					render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
					render_pass.draw(0..(mesh.vertices.len() as u32), 0..1);

					render_pass.set_pipeline(&game.rendering.simple_line_2d_render_pipeline);
					render_pass.set_bind_group(0, &game.rendering.simple_line_2d_bind_group, &[]);
					let mesh = &interface_simple_line_mesh;
					render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
					render_pass.draw(0..(mesh.vertices.len() as u32), 0..1);
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
