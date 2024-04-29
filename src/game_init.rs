use std::{
	collections::HashMap,
	f32::consts::TAU,
	io::{Read, Write},
	sync::{atomic::AtomicI32, Arc},
	time::Duration,
};

use crate::{
	atlas::Atlas,
	block_types::BlockTypeTable,
	camera::{CameraOrthographicSettings, CameraPerspectiveSettings},
	chunk_blocks::Block,
	chunk_loading::LoadingManager,
	chunks::ChunkGrid,
	cmdline, commands,
	coords::{AlignedBox, AngularDirection, ChunkCoords, ChunkDimensions, OrientedFaceCoords},
	entity_parts::{PartTables, TextureMappingAndColoringTable},
	font::{self, Font},
	interface::Interface,
	lang,
	line_meshes::SimpleLineMesh,
	physics::{AlignedPhysBox, PlayerJumpManager},
	rendering_init::{
		self, init_aspect_ratio_thingy, init_atlas_stuff, init_camera_matrix_thingy, init_fog_stuff,
		init_shadow_map_stuff, init_skybox_stuff, init_sun_camera_matrices_thingy,
		init_sun_light_direction_thingy, init_texturing_and_coloring_array_thingy,
		make_z_buffer_texture_view, AllBindingThingies, AtlasStuff, BindingThingy, FogStuff,
		RenderPipelinesAndBindGroups, ShadowMapStuff, SkyboxStuff, SunCameraStuff,
	},
	saves::Save,
	shaders::{Vector2Pod, Vector3Pod},
	skybox::{
		default_skybox_painter, default_skybox_painter_3, generate_skybox_cubemap_faces_images,
		SkyboxFaces,
	},
	threadpool,
	unsorted::{
		Action, Control, ControlEvent, CurrentWorkerTasks, PlayingMode, WhichCameraToUse, WorkerTask,
	},
	widgets::Widget,
	world_gen::{WhichWorldGenerator, WorldGenerator},
};

use fxhash::FxHashSet;
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct StateSavable {
	chunk_dimensions_edge: i32,
	world_gen_seed: i32,
	which_world_generator: WhichWorldGenerator,
	only_save_modified_chunks: bool,
	set_of_already_generated_chunks: FxHashSet<ChunkCoords>,
	player_pos: [f32; 3],
	player_angular_direction: [f32; 2],
	world_time: Duration,
	player_held_block: Option<Block>,
}

pub(crate) fn save_savable_state(game: &Game) {
	let mut state_file =
		std::fs::File::create(&game.save.as_ref().unwrap().state_file_path).unwrap();
	let savable = StateSavable {
		chunk_dimensions_edge: game.cd.edge,
		world_gen_seed: game.world_gen_seed,
		which_world_generator: game.which_world_generator,
		only_save_modified_chunks: game.only_save_modified_chunks,
		set_of_already_generated_chunks: game.chunk_grid.set_of_already_generated_chunks().clone(),
		player_pos: game.player_phys.aligned_box().pos.into(),
		player_angular_direction: game.camera_direction.into(),
		world_time: game.world_time,
		player_held_block: game.player_held_block.clone(),
	};
	let data = rmp_serde::encode::to_vec(&savable).unwrap();
	state_file.write_all(&data).unwrap();
}

fn load_savable_state_from_save(save: &Arc<Save>) -> Option<StateSavable> {
	let mut state_file = std::fs::File::open(&save.state_file_path).ok()?;
	let mut data = vec![];
	state_file.read_to_end(&mut data).unwrap();
	let savable: StateSavable = rmp_serde::decode::from_slice(&data).unwrap();
	Some(savable)
}

pub(crate) struct Game {
	/// The window is in an Arc because the window_surface wants a reference to it.
	pub(crate) window: Arc<winit::window::Window>,
	pub(crate) window_surface: wgpu::Surface<'static>,
	pub(crate) device: Arc<wgpu::Device>,
	pub(crate) queue: wgpu::Queue,
	pub(crate) window_surface_config: wgpu::SurfaceConfiguration,
	pub(crate) aspect_ratio_thingy: BindingThingy<wgpu::Buffer>,
	pub(crate) z_buffer_view: wgpu::TextureView,
	pub(crate) z_buffer_format: wgpu::TextureFormat,
	pub(crate) camera_direction: AngularDirection,
	pub(crate) camera_settings: CameraPerspectiveSettings,
	pub(crate) camera_matrix_thingy: BindingThingy<wgpu::Buffer>,
	pub(crate) sun_position_in_sky: AngularDirection,
	pub(crate) sun_light_direction_thingy: BindingThingy<wgpu::Buffer>,
	pub(crate) sun_cameras: Vec<CameraOrthographicSettings>,
	pub(crate) sun_camera_matrices_thingy: BindingThingy<wgpu::Buffer>,
	pub(crate) sun_camera_single_matrix_thingy: BindingThingy<wgpu::Buffer>,
	pub(crate) shadow_map_cascade_view_thingies: Vec<BindingThingy<wgpu::TextureView>>,
	pub(crate) targeted_face: Option<OrientedFaceCoords>,
	pub(crate) player_phys: AlignedPhysBox,
	pub(crate) player_jump_manager: PlayerJumpManager,
	pub(crate) cd: ChunkDimensions,
	pub(crate) chunk_grid: ChunkGrid,
	pub(crate) loading_manager: LoadingManager,
	pub(crate) controls_to_trigger: Vec<ControlEvent>,
	pub(crate) control_bindings: HashMap<Control, Action>,
	pub(crate) block_type_table: Arc<BlockTypeTable>,
	pub(crate) rendering: RenderPipelinesAndBindGroups,
	pub(crate) close_after_one_frame: bool,
	pub(crate) cursor_mesh: SimpleLineMesh,
	pub(crate) random_message: &'static str,
	pub(crate) font: Arc<font::Font>,
	pub(crate) command_line_content: String,
	pub(crate) typing_in_command_line: bool,
	pub(crate) last_command_line_interaction: Option<std::time::Instant>,
	pub(crate) command_confirmed: bool,
	pub(crate) world_generator: Arc<dyn WorldGenerator + Sync + Send>,
	pub(crate) which_world_generator: WhichWorldGenerator,
	pub(crate) world_gen_seed: i32,
	pub(crate) interface: Interface,
	pub(crate) enable_interface_draw_debug_boxes: bool,
	pub(crate) skybox_cubemap_texture: wgpu::Texture,
	pub(crate) fog_center_position_thingy: BindingThingy<wgpu::Buffer>,
	pub(crate) fog_inf_sup_radiuses_thingy: BindingThingy<wgpu::Buffer>,
	pub(crate) fog_inf_sup_radiuses: (f32, f32),
	pub(crate) fog_margin: f32,
	pub(crate) output_atlas_when_generated: bool,
	pub(crate) atlas_texture: wgpu::Texture,
	pub(crate) save: Option<Arc<Save>>,
	pub(crate) only_save_modified_chunks: bool,
	pub(crate) max_fps: Option<i32>,
	pub(crate) part_tables: PartTables,
	pub(crate) texturing_and_coloring_array_thingy: BindingThingy<wgpu::Buffer>,
	pub(crate) texture_mapping_table: TextureMappingAndColoringTable,
	pub(crate) player_held_block: Option<Block>,
	pub(crate) world_time: Duration,
	pub(crate) playing_mode: PlayingMode,
	pub(crate) player_health: Option<u32>,

	pub(crate) worker_tasks: CurrentWorkerTasks,
	pub(crate) pool: threadpool::ThreadPool,

	pub(crate) time_beginning: std::time::Instant,
	pub(crate) time_from_last_iteration: std::time::Instant,

	pub(crate) walking_forward: bool,
	pub(crate) walking_backward: bool,
	pub(crate) walking_leftward: bool,
	pub(crate) walking_rightward: bool,
	pub(crate) enable_physics: bool,
	pub(crate) enable_world_generation: bool,
	pub(crate) selected_camera: WhichCameraToUse,
	pub(crate) enable_display_phys_box: bool,
	pub(crate) cursor_is_captured: bool,
	pub(crate) enable_display_interface: bool,
	pub(crate) enable_display_not_surrounded_chunks_as_boxes: bool,
	pub(crate) enable_display_chunks_with_entities_as_boxes: bool,
	pub(crate) enable_display_entity_boxes: bool,
	pub(crate) enable_fog: bool,
	pub(crate) enable_fullscreen: bool,
}

pub(crate) fn init_game() -> (Game, winit::event_loop::EventLoop<()>) {
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
		max_fps,
		no_fog,
		fog_margin,
		save_name,
		only_save_modified_chunks,
		playing_mode,
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

	let event_loop = winit::event_loop::EventLoop::new().unwrap();
	event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

	let enable_fullscreen = fullscreen;
	let window = winit::window::WindowBuilder::new()
		.with_title("Qwy3")
		.with_maximized(true)
		.with_resizable(true)
		.with_fullscreen(enable_fullscreen.then_some(winit::window::Fullscreen::Borderless(None)))
		.build(&event_loop)
		.unwrap();
	let window = Arc::new(window);

	let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
	let window_surface = instance.create_surface(Arc::clone(&window)).unwrap();

	// Try to get a cool adapter first.
	let adapter = instance.enumerate_adapters(wgpu::Backends::all()).into_iter().find(|adapter| {
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
					required_features: wgpu::Features::empty(),
					required_limits: wgpu::Limits { ..wgpu::Limits::default() },
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
		desired_maximum_frame_latency: 2,
		alpha_mode: surface_capabilities.alpha_modes[0],
		view_formats: vec![],
	};
	window_surface.configure(&device, &window_surface_config);

	let aspect_ratio_thingy = init_aspect_ratio_thingy(Arc::clone(&device));

	let save = save_name.map(|name| Arc::new(Save::create(name)));
	let saved_state = save.as_ref().and_then(load_savable_state_from_save);

	if save.is_none() {
		println!("Warning: No save specified, nothing will persist.");
		println!("A save name can be specified using `-s <NAME>` or `--save <NAME>`.");
	}

	let only_save_modified_chunks = saved_state
		.as_ref()
		.map(|state| state.only_save_modified_chunks)
		.unwrap_or(only_save_modified_chunks);

	let world_gen_seed = saved_state
		.as_ref()
		.map(|state| state.world_gen_seed)
		.unwrap_or(world_gen_seed.unwrap_or_else(|| rand::thread_rng().gen()));

	let block_type_table = Arc::new(BlockTypeTable::new());

	let atlas_loaded_from_save = save.as_ref().and_then(Atlas::load_from_save);
	let need_generation_of_the_complete_atlas = atlas_loaded_from_save.is_none();

	let atlas = atlas_loaded_from_save.unwrap_or_else(Atlas::new_fast_incomplete);
	let AtlasStuff {
		atlas_texture_view_thingy,
		atlas_texture_sampler_thingy,
		atlas_texture,
	} = init_atlas_stuff(Arc::clone(&device), &queue, atlas.image.as_ref());
	let output_atlas_when_generated = output_atlas;

	let font = Arc::new(Font::font_02());

	let skybox_faces_loaded_from_save = save.as_ref().and_then(SkyboxFaces::load_from_save);
	let need_generation_of_the_better_skybox = skybox_faces_loaded_from_save.is_none();
	let skybox_faces = skybox_faces_loaded_from_save
		.unwrap_or_else(|| generate_skybox_cubemap_faces_images(&default_skybox_painter, None));

	let SkyboxStuff {
		skybox_cubemap_texture_view_thingy,
		skybox_cubemap_texture_sampler_thingy,
		skybox_cubemap_texture,
	} = init_skybox_stuff(Arc::clone(&device), &queue, &skybox_faces.data());
	// The better painter that takes significantly more time will be run on a worker thread.
	let longer_skybox_painter = default_skybox_painter_3(4, world_gen_seed);

	let FogStuff { fog_center_position_thingy, fog_inf_sup_radiuses_thingy } =
		init_fog_stuff(Arc::clone(&device));

	let enable_fog = !no_fog;

	queue.write_buffer(
		&fog_center_position_thingy.resource,
		0,
		bytemuck::cast_slice(&[Vector3Pod { values: [0.0, 0.0, 0.0] }]),
	);
	let fog_inf_sup_radiuses = (0.0, fog_margin);
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

	let camera_direction: AngularDirection = saved_state
		.as_ref()
		.map(|state| (&state.player_angular_direction).into())
		.unwrap_or(AngularDirection::from_angle_horizontal(0.0));

	let selected_camera = WhichCameraToUse::FirstPerson;

	let cursor_is_captured = true;
	let cursor_was_actually_captured =
		window.set_cursor_grab(winit::window::CursorGrabMode::Confined).is_ok();
	if cursor_was_actually_captured {
		window.set_cursor_visible(false);
	}

	let targeted_face = None;

	let walking_forward = false;
	let walking_backward = false;
	let walking_leftward = false;
	let walking_rightward = false;

	let player_pos: cgmath::Point3<f32> =
		(*saved_state.as_ref().map(|state| &state.player_pos).unwrap_or(&[0.0, 0.0, 2.0])).into();
	let player_phys = AlignedPhysBox::new(
		AlignedBox { pos: player_pos, dims: (0.8, 0.8, 1.8).into() },
		cgmath::vec3(0.0, 0.0, 0.0),
	);
	let player_jump_manager = PlayerJumpManager::new();
	let enable_physics = true;
	let enable_display_phys_box = false;

	let player_held_block = saved_state.as_ref().and_then(|state| state.player_held_block.clone());

	let player_health = (playing_mode == PlayingMode::Play).then_some(5);

	let sun_position_in_sky = AngularDirection::from_angles(TAU / 16.0, TAU / 8.0);
	let sun_light_direction_thingy = init_sun_light_direction_thingy(Arc::clone(&device));

	let world_time =
		saved_state.as_ref().map_or(Duration::from_secs_f32(0.0), |state| state.world_time);

	let sun_cameras = vec![
		CameraOrthographicSettings {
			up_direction: (0.0, 0.0, 1.0).into(),
			width: 45.0,
			height: 45.0,
			depth: 800.0,
		},
		CameraOrthographicSettings {
			up_direction: (0.0, 0.0, 1.0).into(),
			width: 750.0,
			height: 750.0,
			depth: 800.0,
		},
	];
	let shadow_map_cascade_count = sun_cameras.len() as u32;
	let SunCameraStuff { sun_camera_matrices_thingy, sun_camera_single_matrix_thingy } =
		init_sun_camera_matrices_thingy(Arc::clone(&device), shadow_map_cascade_count);

	let ShadowMapStuff {
		shadow_map_format,
		shadow_map_view_thingy,
		shadow_map_sampler_thingy,
		shadow_map_cascade_view_thingies,
	} = init_shadow_map_stuff(Arc::clone(&device), shadow_map_cascade_count);

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

	let chunk_edge =
		saved_state.as_ref().map(|state| state.chunk_dimensions_edge).unwrap_or(chunk_edge as i32);
	let cd = ChunkDimensions::from(chunk_edge as i32);
	let already_generated_set = saved_state.as_ref().map(|state| {
		// TODO: Avoid cloning here.
		state.set_of_already_generated_chunks.clone()
	});
	let chunk_grid = ChunkGrid::new(cd, already_generated_set);

	let margin_before_unloading = 60.0;
	let loading_manager = LoadingManager::new(loading_distance, margin_before_unloading);

	let enable_world_generation = true;

	let mut worker_tasks = CurrentWorkerTasks { tasks: vec![] };
	let pool = threadpool::ThreadPool::new(number_of_threads as usize);

	if need_generation_of_the_complete_atlas {
		let (sender, receiver) = std::sync::mpsc::channel();
		worker_tasks.tasks.push(WorkerTask::GenerateAtlas(receiver));
		pool.enqueue_task(Box::new(move || {
			let atlas = Atlas::new_slow_complete(world_gen_seed);
			let _ = sender.send(atlas);
		}));
	}

	let face_counter = need_generation_of_the_better_skybox.then(|| {
		let (sender, receiver) = std::sync::mpsc::channel();
		let face_counter = Arc::new(AtomicI32::new(0));
		worker_tasks.tasks.push(WorkerTask::PaintNewSkybox(
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
	});

	let part_tables = PartTables::new(&device);

	let texturing_and_coloring_array_thingy =
		init_texturing_and_coloring_array_thingy(Arc::clone(&device));
	let texture_mapping_table = TextureMappingAndColoringTable::new();

	let rendering = rendering_init::init_rendering_stuff(
		Arc::clone(&device),
		AllBindingThingies {
			aspect_ratio_thingy: &aspect_ratio_thingy,
			camera_matrix_thingy: &camera_matrix_thingy,
			sun_light_direction_thingy: &sun_light_direction_thingy,
			sun_camera_matrices_thingy: &sun_camera_matrices_thingy,
			sun_camera_single_matrix_thingy: &sun_camera_single_matrix_thingy,
			shadow_map_view_thingy: &shadow_map_view_thingy,
			shadow_map_sampler_thingy: &shadow_map_sampler_thingy,
			atlas_texture_view_thingy: &atlas_texture_view_thingy,
			atlas_texture_sampler_thingy: &atlas_texture_sampler_thingy,
			skybox_cubemap_texture_view_thingy: &skybox_cubemap_texture_view_thingy,
			skybox_cubemap_texture_sampler_thingy: &skybox_cubemap_texture_sampler_thingy,
			fog_center_position_thingy: &fog_center_position_thingy,
			fog_inf_sup_radiuses_thingy: &fog_inf_sup_radiuses_thingy,
			texturing_and_coloring_array_thingy: &texturing_and_coloring_array_thingy,
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

	let which_world_generator = saved_state
		.as_ref()
		.map(|state| state.which_world_generator)
		.unwrap_or(which_world_generator);
	let world_generator =
		which_world_generator.get_the_actual_generator(world_gen_seed, &block_type_table);

	let enable_display_not_surrounded_chunks_as_boxes = false;

	let enable_display_chunks_with_entities_as_boxes = false;
	let enable_display_entity_boxes = false;

	let mut interface = Interface::new();

	if let Some(face_counter) = face_counter {
		interface.log_widget(Widget::new_disappear_when_complete(
			std::time::Duration::from_secs_f32(2.0),
			Box::new(Widget::new_face_counter(
				font::TextRenderingSettings::with_scale(3.0),
				face_counter,
			)),
		));
	}

	if let Some(save) = save.as_ref() {
		let settings = font::TextRenderingSettings::with_scale(2.0);
		let save_name = &save.name;
		let save_path = save.main_directory.display();
		interface.log_widget(Widget::new_simple_text(
			format!("Save \"{save_name}\""),
			settings.clone(),
		));
		interface.log_widget(Widget::new_simple_text(
			format!("Save path \"{save_path}\""),
			settings,
		));
	} else {
		let mut settings = font::TextRenderingSettings::with_scale(3.0);
		settings.color = [0.4, 0.0, 0.0];
		interface.log_widget(Widget::new_simple_text(
			"No save, nothing will persist".to_string(),
			settings.clone(),
		));
	}

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
		sun_cameras,
		sun_camera_matrices_thingy,
		sun_camera_single_matrix_thingy,
		shadow_map_cascade_view_thingies,
		targeted_face,
		player_phys,
		player_jump_manager,
		cd,
		chunk_grid,
		loading_manager,
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
		which_world_generator,
		world_gen_seed,
		interface,
		enable_interface_draw_debug_boxes,
		skybox_cubemap_texture,
		fog_center_position_thingy,
		fog_inf_sup_radiuses_thingy,
		fog_inf_sup_radiuses,
		fog_margin,
		output_atlas_when_generated,
		atlas_texture,
		save,
		only_save_modified_chunks,
		max_fps,
		part_tables,
		texturing_and_coloring_array_thingy,
		texture_mapping_table,
		player_held_block,
		world_time,
		playing_mode,
		player_health,

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
		enable_display_chunks_with_entities_as_boxes,
		enable_display_entity_boxes,
		enable_fog,
		enable_fullscreen,
	};
	(game, event_loop)
}

impl Game {
	pub(crate) fn player_chunk(&self) -> ChunkCoords {
		let player_block_coords = (self.player_phys.aligned_box().pos
			- cgmath::Vector3::<f32>::unit_z() * (self.player_phys.aligned_box().dims.z / 2.0 + 0.1))
			.map(|x| x.round() as i32);
		self.cd.world_coords_to_containing_chunk_coords(player_block_coords)
	}
}
