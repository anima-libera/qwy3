#![allow(clippy::items_after_test_module)]

mod atlas;
mod block_types;
mod camera;
mod chunk_loading;
mod chunk_meshing;
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
mod rendering_init;
mod saves;
mod shaders;
mod skybox;
mod texture_gen;
mod threadpool;
mod widgets;
mod world_gen;

use std::{
	collections::HashMap,
	f32::consts::TAU,
	io::{Read, Write},
	sync::{atomic::AtomicI32, Arc},
};

use cgmath::{point3, ElementWise, InnerSpace, MetricSpace};
use chunk_loading::LoadingManager;
use chunk_meshing::{ChunkMesh, DataForChunkMeshing};
use rand::Rng;
use saves::Save;
use serde::{Deserialize, Serialize};
use skybox::SkyboxFaces;
use threadpool::ThreadPool;
use wgpu::util::DeviceExt;

use camera::{aspect_ratio, CameraOrthographicSettings, CameraPerspectiveSettings, CameraSettings};
use chunks::*;
use coords::*;
use line_meshes::*;
use physics::AlignedPhysBox;
use rendering_init::*;
use shaders::{simple_texture_2d::SimpleTextureVertexPod, Vector3Pod};
use widgets::{InterfaceMeshesVertices, Widget, WidgetLabel};
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use world_gen::WorldGenerator;

use crate::{
	atlas::Atlas,
	font::Font,
	lang::LogItem,
	shaders::Vector2Pod,
	skybox::{
		default_skybox_painter, default_skybox_painter_3, generate_skybox_cubemap_faces_images,
		SkyboxMesh,
	},
};

#[derive(Clone, Copy)]
enum WhichCameraToUse {
	FirstPerson,
	ThirdPersonNear,
	ThirdPersonFar,
	ThirdPersonVeryFar,
	ThirdPersonTooFar,
	Sun,
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum Control {
	KeyboardKey(winit::keyboard::Key),
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

/// The main-thread reciever for the results of a task that was given to a worker thread.
enum WorkerTask {
	LoadChunkBlocks(
		ChunkCoords,
		std::sync::mpsc::Receiver<(ChunkBlocks, ChunkCullingInfo)>,
	),
	MeshChunk(ChunkCoords, std::sync::mpsc::Receiver<ChunkMesh>),
	/// The counter at the end is the number of faces already finished.
	PaintNewSkybox(std::sync::mpsc::Receiver<SkyboxFaces>, Arc<AtomicI32>),
	GenerateAtlas(std::sync::mpsc::Receiver<Atlas>),
}

struct CurrentWorkerTasks {
	tasks: Vec<WorkerTask>,
}

impl CurrentWorkerTasks {
	fn run_chunk_meshing_task(
		&mut self,
		pool: &mut ThreadPool,
		chunk_coords: ChunkCoords,
		data_for_chunk_meshing: DataForChunkMeshing,
		device: Arc<wgpu::Device>,
	) {
		let (sender, receiver) = std::sync::mpsc::channel();
		self.tasks.push(WorkerTask::MeshChunk(chunk_coords, receiver));
		pool.enqueue_task(Box::new(move || {
			let mut mesh = data_for_chunk_meshing.generate_mesh();
			mesh.update_gpu_data(&device);
			let _ = sender.send(mesh);
		}));
	}

	fn is_being_meshed(&self, chunk_coords: ChunkCoords) -> bool {
		self.tasks.iter().any(|worker_task| match worker_task {
			WorkerTask::MeshChunk(chunk_coords_uwu, ..) => *chunk_coords_uwu == chunk_coords,
			_ => false,
		})
	}

	fn run_chunk_loading_task(
		&mut self,
		pool: &mut ThreadPool,
		chunk_coords: ChunkCoords,
		world_generator: &Arc<dyn WorldGenerator + Sync + Send>,
		block_type_table: &Arc<BlockTypeTable>,
		save: Option<&Arc<Save>>,
		cd: ChunkDimensions,
	) {
		let (sender, receiver) = std::sync::mpsc::channel();
		self.tasks.push(WorkerTask::LoadChunkBlocks(chunk_coords, receiver));
		let chunk_generator = Arc::clone(world_generator);
		let coords_span = ChunkCoordsSpan { cd, chunk_coords };
		let block_type_table = Arc::clone(block_type_table);
		let save = save.map(Arc::clone);
		pool.enqueue_task(Box::new(move || {
			// Loading a chunk means either loading from save (disk)
			// if there is a save and the chunk was already generated and saved in the past,
			// or else generating it.
			let chunk_blocks =
				save.and_then(|save| ChunkBlocks::load_from_save(coords_span, &save)).unwrap_or_else(
					|| chunk_generator.generate_chunk_blocks(coords_span, &block_type_table),
				);
			let chunk_culling_info =
				ChunkCullingInfo::compute_from_blocks(&chunk_blocks, &block_type_table);
			let _ = sender.send((chunk_blocks, chunk_culling_info));
		}));
	}

	fn is_being_loaded(&self, chunk_coords: ChunkCoords) -> bool {
		self.tasks.iter().any(|worker_task| match worker_task {
			WorkerTask::LoadChunkBlocks(chunk_coords_uwu, ..) => *chunk_coords_uwu == chunk_coords,
			_ => false,
		})
	}
}

pub(crate) struct SimpleTextureMesh {
	pub(crate) vertices: Vec<shaders::simple_texture_2d::SimpleTextureVertexPod>,
	pub(crate) vertex_buffer: wgpu::Buffer,
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
			position: c.into(),
			coords_in_atlas: atlas_c.into(),
			color_factor,
		});
		vertices.push(SimpleTextureVertexPod {
			position: d.into(),
			coords_in_atlas: atlas_d.into(),
			color_factor,
		});
		vertices.push(SimpleTextureVertexPod {
			position: b.into(),
			coords_in_atlas: atlas_b.into(),
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

#[derive(Serialize, Deserialize)]
struct StateSavable {
	player_pos: [f32; 3],
	player_angular_direction: [f32; 2],
}

fn save_savable_state(game: &Game) {
	let mut state_file =
		std::fs::File::create(&game.save.as_ref().unwrap().state_file_path).unwrap();
	let player_pos = game.player_phys.aligned_box().pos.into();
	let player_angular_direction = game.camera_direction.into();
	let savable = StateSavable { player_pos, player_angular_direction };
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

struct Game {
	/// The window is in an Arc because the window_surface wants a reference to it.
	window: Arc<winit::window::Window>,
	window_surface: wgpu::Surface<'static>,
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
	sun_cameras: Vec<CameraOrthographicSettings>,
	sun_camera_matrices_thingy: BindingThingy<wgpu::Buffer>,
	sun_camera_single_matrix_thingy: BindingThingy<wgpu::Buffer>,
	shadow_map_cascade_view_thingies: Vec<BindingThingy<wgpu::TextureView>>,
	/// First is the block of matter that is targeted,
	/// second is the empty block near it that would be filled if a block was placed now.
	targeted_block_coords: Option<(BlockCoords, BlockCoords)>,
	player_phys: AlignedPhysBox,
	cd: ChunkDimensions,
	chunk_grid: ChunkGrid,
	loading_manager: LoadingManager,
	controls_to_trigger: Vec<ControlEvent>,
	control_bindings: HashMap<Control, Action>,
	block_type_table: Arc<BlockTypeTable>,
	rendering: RenderPipelinesAndBindGroups,
	close_after_one_frame: bool,
	cursor_mesh: SimpleLineMesh,
	random_message: &'static str,
	font: Arc<font::Font>,
	command_line_content: String,
	typing_in_command_line: bool,
	last_command_line_interaction: Option<std::time::Instant>,
	command_confirmed: bool,
	world_generator: Arc<dyn WorldGenerator + Sync + Send>,
	widget_tree_root: Widget,
	enable_interface_draw_debug_boxes: bool,
	skybox_cubemap_texture: wgpu::Texture,
	fog_center_position_thingy: BindingThingy<wgpu::Buffer>,
	fog_inf_sup_radiuses_thingy: BindingThingy<wgpu::Buffer>,
	fog_inf_sup_radiuses: (f32, f32),
	fog_margin: f32,
	output_atlas_when_generated: bool,
	atlas_texture: wgpu::Texture,
	save: Option<Arc<Save>>,

	worker_tasks: CurrentWorkerTasks,
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
		no_fog,
		fog_margin,
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

	let save = Some(Arc::new(Save::create("testies".to_string())));
	let saved_state = save.as_ref().and_then(load_savable_state_from_save);

	let block_type_table = Arc::new(BlockTypeTable::new());

	let atlas = Atlas::new_fast_incomplete();
	let AtlasStuff {
		atlas_texture_view_thingy,
		atlas_texture_sampler_thingy,
		atlas_texture,
	} = init_atlas_stuff(Arc::clone(&device), &queue, atlas.image.as_ref());
	let output_atlas_when_generated = output_atlas;

	let font = Arc::new(Font::font_01());

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

	// First is the block of matter that is targeted,
	// second is the empty block near it that would be filled if a block was placed now.
	let targeted_block_coords: Option<(BlockCoords, BlockCoords)> = None;

	let walking_forward = false;
	let walking_backward = false;
	let walking_leftward = false;
	let walking_rightward = false;

	let player_pos: cgmath::Point3<f32> =
		(*saved_state.as_ref().map(|state| &state.player_pos).unwrap_or(&[0.0, 0.0, 2.0])).into();
	let player_phys =
		AlignedPhysBox::new(AlignedBox { pos: player_pos, dims: (0.8, 0.8, 1.8).into() });
	let enable_physics = true;
	let enable_display_phys_box = false;

	let sun_position_in_sky = AngularDirection::from_angles(TAU / 16.0, TAU / 8.0);
	let sun_light_direction_thingy = init_sun_light_direction_thingy(Arc::clone(&device));

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

	let cd = ChunkDimensions::from(chunk_edge as i32);
	let chunk_grid = ChunkGrid::new(cd);

	let margin_before_unloading = 60.0;
	let loading_manager = LoadingManager::new(loading_distance, margin_before_unloading);

	let enable_world_generation = true;

	let mut worker_tasks = CurrentWorkerTasks { tasks: vec![] };
	let pool = threadpool::ThreadPool::new(number_of_threads as usize);

	{
		let (sender, receiver) = std::sync::mpsc::channel();
		worker_tasks.tasks.push(WorkerTask::GenerateAtlas(receiver));
		pool.enqueue_task(Box::new(move || {
			let atlas = Atlas::new_slow_complete(world_gen_seed);
			let _ = sender.send(atlas);
		}));
	}

	let face_counter = {
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
	};

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

	let enable_display_not_surrounded_chunks_as_boxes = false;

	let mut widget_tree_root = Widget::new_margins(
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

	if let Some(save) = save.as_ref() {
		if let Some(Widget::List { sub_widgets, .. }) =
			widget_tree_root.find_label_content(WidgetLabel::LogLineList)
		{
			let settings = font::TextRenderingSettings::with_scale(2.0);
			let save_name = &save.name;
			let save_path = save.main_directory.display();
			sub_widgets.push(Widget::new_simple_text(
				format!("Save \"{save_name}\""),
				settings.clone(),
			));
			sub_widgets.push(Widget::new_simple_text(
				format!("Save path \"{save_path}\""),
				settings,
			));
		}
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
		targeted_block_coords,
		player_phys,
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
		widget_tree_root,
		enable_interface_draw_debug_boxes,
		skybox_cubemap_texture,
		fog_center_position_thingy,
		fog_inf_sup_radiuses_thingy,
		fog_inf_sup_radiuses,
		fog_margin,
		output_atlas_when_generated,
		atlas_texture,
		save,

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

impl Game {
	fn player_chunk(&self) -> ChunkCoords {
		let player_block_coords = (self.player_phys.aligned_box().pos
			- cgmath::Vector3::<f32>::unit_z() * (self.player_phys.aligned_box().dims.z / 2.0 + 0.1))
			.map(|x| x.round() as i32);
		self.cd.world_coords_to_containing_chunk_coords(player_block_coords)
	}
}

pub fn run() {
	let (mut game, event_loop) = init_game();

	use winit::event::*;
	use winit::keyboard::*;
	let res = event_loop.run(move |event, elwt| match event {
		Event::WindowEvent { ref event, window_id } if window_id == game.window.id() => match event {
			WindowEvent::CloseRequested
			| WindowEvent::KeyboardInput {
				event:
					KeyEvent {
						logical_key: Key::Named(NamedKey::Escape),
						state: ElementState::Pressed,
						..
					},
				..
			} => elwt.exit(),

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
				event: event @ KeyEvent { logical_key, state, repeat, .. },
				..
			} => {
				if game.typing_in_command_line && *state == ElementState::Pressed {
					if matches!(logical_key, Key::Named(NamedKey::Enter)) {
						game.command_confirmed = true;
						game.typing_in_command_line = false;
						game.last_command_line_interaction = Some(std::time::Instant::now());
					} else if matches!(logical_key, Key::Named(NamedKey::Backspace)) {
						game.command_line_content.pop();
						game.last_command_line_interaction = Some(std::time::Instant::now());
					} else if let Key::Character(string) = logical_key {
						game.command_line_content += string;
						game.last_command_line_interaction = Some(std::time::Instant::now());
					}
				} else if !repeat {
					game.controls_to_trigger.push(ControlEvent {
						control: Control::KeyboardKey(event.key_without_modifiers()),
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
			let mut pos = game.player_phys.aligned_box().pos;
			pos.z -= dy * sensitivity;
			pos += direction_left_or_right.to_vec3() * f32::abs(dx) * sensitivity;
			game.player_phys.impose_new_pos(pos);
		},

		Event::AboutToWait => {
			// Here shall begin the body of the gameloop.

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
							game.player_phys.jump();
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
								WhichCameraToUse::ThirdPersonVeryFar => WhichCameraToUse::ThirdPersonTooFar,
								WhichCameraToUse::ThirdPersonTooFar => WhichCameraToUse::FirstPerson,
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
							dbg!(game.player_phys.aligned_box().pos);
							let player_bottom = game.player_phys.aligned_box().pos
								- cgmath::Vector3::<f32>::from((
									0.0,
									0.0,
									game.player_phys.aligned_box().dims.z / 2.0,
								));
							dbg!(player_bottom);
						},
						(Action::PlaceOrRemoveBlockUnderPlayer, true) => {
							let player_bottom = game.player_phys.aligned_box().pos
								- cgmath::Vector3::<f32>::unit_z()
									* (game.player_phys.aligned_box().dims.z / 2.0 + 0.1);
							let player_bottom_block_coords = player_bottom.map(|x| x.round() as i32);
							let player_bottom_block_opt =
								game.chunk_grid.get_block(player_bottom_block_coords);
							if let Some(block) = player_bottom_block_opt {
								game.chunk_grid.set_block_and_request_updates_to_meshes(
									player_bottom_block_coords,
									if game.block_type_table.get(block.type_id).unwrap().is_opaque() {
										game.block_type_table.air_id().into()
									} else {
										game.block_type_table.ground_id().into()
									},
								);
							}
						},
						(Action::PlaceBlockAtTarget, true) => {
							if let Some((_, coords)) = game.targeted_block_coords {
								game.chunk_grid.set_block_and_request_updates_to_meshes(
									coords,
									//game.block_type_table.ground_id().into(),
									Block {
										type_id: game.block_type_table.text_id(),
										data: Some(BlockData::Text("Jaaj".to_string())),
									},
								);
							}
						},
						(Action::RemoveBlockAtTarget, true) => {
							if let Some((coords, _)) = game.targeted_block_coords {
								game.chunk_grid.set_block_and_request_updates_to_meshes(
									coords,
									game.block_type_table.air_id().into(),
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
				let chunk_count = game.chunk_grid.count_chunks_that_have_blocks();
				let block_count = chunk_count * game.cd.number_of_blocks();
				let chunk_meshed_count = game.chunk_grid.count_chunks_that_have_meshes();
				let player_block_coords = (game.player_phys.aligned_box().pos
					- cgmath::Vector3::<f32>::unit_z()
						* (game.player_phys.aligned_box().dims.z / 2.0 + 0.1))
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
			game.worker_tasks.tasks.retain_mut(|worker_task| {
				let is_not_done_yet = match worker_task {
					WorkerTask::LoadChunkBlocks(chunk_coords, receiver) => {
						let chunk_coords_and_result_opt =
							receiver.try_recv().ok().map(|(chunk_blocks, chunk_culling_info)| {
								(*chunk_coords, chunk_blocks, chunk_culling_info)
							});
						let is_not_done_yet = chunk_coords_and_result_opt.is_none();
						if let Some((chunk_coords, chunk_blocks, chunk_culling_info)) =
							chunk_coords_and_result_opt
						{
							game.loading_manager.handle_chunk_loading_results(
								chunk_coords,
								chunk_blocks,
								chunk_culling_info,
								&mut game.chunk_grid,
							);
						}
						is_not_done_yet
					},
					WorkerTask::MeshChunk(chunk_coords, receiver) => {
						let chunk_coords_and_result_opt =
							receiver.try_recv().ok().map(|chunk_mesh| (*chunk_coords, chunk_mesh));
						let is_not_done_yet = chunk_coords_and_result_opt.is_none();
						if let Some((chunk_coords, chunk_mesh)) = chunk_coords_and_result_opt {
							game.chunk_grid.add_chunk_meshing_results(chunk_coords, chunk_mesh);
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
					WorkerTask::GenerateAtlas(receiver) => {
						let result_opt = receiver.try_recv().ok();
						let is_not_done_yet = result_opt.is_none();
						if let Some(completed_atlas) = result_opt {
							if game.output_atlas_when_generated {
								let path = "atlas.png";
								println!("Outputting atlas to \"{path}\"");
								completed_atlas
									.image
									.save_with_format(path, image::ImageFormat::Png)
									.unwrap();
							}
							update_atlas_texture(
								&game.queue,
								&game.atlas_texture,
								&completed_atlas.image.as_ref(),
							);
						}
						is_not_done_yet
					},
				};
				is_not_done_yet
			});

			// Request meshing for chunks that can be meshed or should be re-meshed.
			game.chunk_grid.run_some_required_remeshing_tasks(
				&mut game.worker_tasks,
				&mut game.pool,
				&game.block_type_table,
				&game.font,
				&game.device,
			);

			// Handle fog adjustment.
			// Current fog fix,
			// works fine when the loading of chunks is finished or almost finished.
			let sqrt_3 = 3.0_f32.sqrt();
			let distance = game.loading_manager.loading_distance - game.cd.edge as f32 * sqrt_3 / 2.0;
			game.fog_inf_sup_radiuses.1 = distance.max(game.fog_margin);
			game.fog_inf_sup_radiuses.0 = game.fog_inf_sup_radiuses.1 - game.fog_margin;
			if game.enable_fog {
				game.queue.write_buffer(
					&game.fog_inf_sup_radiuses_thingy.resource,
					0,
					bytemuck::cast_slice(&[Vector2Pod {
						values: [game.fog_inf_sup_radiuses.0, game.fog_inf_sup_radiuses.1],
					}]),
				);
			}

			// Request generation of chunk blocks for not-generated not-being-generated close chunks.
			let player_chunk = game.player_chunk();
			game.loading_manager.handle_loading(
				&mut game.chunk_grid,
				&mut game.worker_tasks,
				&mut game.pool,
				player_chunk,
				&game.world_generator,
				&game.block_type_table,
				game.save.as_ref(),
			);

			// Unload chunks that are a bit too far.
			let unloading_distance =
				game.loading_manager.loading_distance + game.loading_manager.margin_before_unloading;
			game.chunk_grid.unload_chunks_too_far(
				game.player_chunk(),
				unloading_distance,
				game.save.as_ref(),
			);

			// Walking.
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
			game.player_phys.walk(walking_vector, !game.enable_physics);

			if game.enable_physics {
				game.player_phys.apply_on_physics_step(&game.chunk_grid, &game.block_type_table, dt);
			}

			game.queue.write_buffer(
				&game.fog_center_position_thingy.resource,
				0,
				bytemuck::cast_slice(&[Vector3Pod {
					values: game.player_phys.aligned_box().pos.into(),
				}]),
			);

			let player_box_mesh =
				SimpleLineMesh::from_aligned_box(&game.device, game.player_phys.aligned_box());

			let player_blocks_box_mesh = SimpleLineMesh::from_aligned_box(
				&game.device,
				&game.player_phys.aligned_box().overlapping_block_coords_span().to_aligned_box(),
			);

			let first_person_camera_position = game.player_phys.aligned_box().pos
				+ cgmath::Vector3::<f32>::from((0.0, 0.0, game.player_phys.aligned_box().dims.z / 2.0))
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
					.is_some_and(|block| !game.block_type_table.get(block.type_id).unwrap().is_air())
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
				for chunk_coords in game.chunk_grid.iter_loaded_chunk_coords() {
					let is_surrounded = 'is_surrounded: {
						for neighbor_chunk_coords in iter_3d_cube_center_radius(chunk_coords, 2) {
							let blocks_was_generated = game.chunk_grid.is_loaded(neighbor_chunk_coords);
							if !blocks_was_generated {
								break 'is_surrounded false;
							}
						}
						true
					};
					if !is_surrounded {
						let coords_span = ChunkCoordsSpan { cd: game.cd, chunk_coords };
						let inf = coords_span.block_coords_inf().map(|x| x as f32);
						let dims = coords_span.cd._dimensions().map(|x| x as f32 - 1.0);
						let pos = inf + dims / 2.0;
						chunk_box_meshes.push(SimpleLineMesh::from_aligned_box(
							&game.device,
							&AlignedBox { pos, dims },
						));
					}
				}
			}

			game.sun_position_in_sky.angle_horizontal += (TAU / 150.0) * dt.as_secs_f32();

			let sun_camera_view_projection_matrices: Vec<_> = game
				.sun_cameras
				.iter()
				.map(|camera| {
					let camera_position = first_person_camera_position;
					let camera_direction_vector = -game.sun_position_in_sky.to_vec3();
					let camera_up_vector = (0.0, 0.0, 1.0).into();
					camera.view_projection_matrix(
						camera_position,
						camera_direction_vector,
						camera_up_vector,
					)
				})
				.collect();
			game.queue.write_buffer(
				&game.sun_camera_matrices_thingy.resource,
				0,
				bytemuck::cast_slice(&sun_camera_view_projection_matrices),
			);

			let (camera_view_projection_matrix, camera_position_ifany) = {
				if matches!(game.selected_camera, WhichCameraToUse::Sun) {
					(sun_camera_view_projection_matrices[0], None)
				} else {
					let mut camera_position = first_person_camera_position;
					let camera_direction_vector = game.camera_direction.to_vec3();
					match game.selected_camera {
						WhichCameraToUse::FirstPerson | WhichCameraToUse::Sun => {},
						WhichCameraToUse::ThirdPersonNear => {
							camera_position -= camera_direction_vector * 5.0;
						},
						WhichCameraToUse::ThirdPersonFar => {
							camera_position -= camera_direction_vector * 40.0;
						},
						WhichCameraToUse::ThirdPersonVeryFar => {
							camera_position -= camera_direction_vector * 200.0;
						},
						WhichCameraToUse::ThirdPersonTooFar => {
							camera_position -= camera_direction_vector
								* (game.loading_manager.loading_distance + 250.0).max(300.0);
						},
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

			let data_for_rendering = rendering::DataForRendering {
				device: &game.device,
				queue: &game.queue,
				window_surface: &game.window_surface,
				window_surface_config: &game.window_surface_config,
				rendering: &game.rendering,
				sun_cameras: &game.sun_cameras,
				sun_camera_matrices_thingy: &game.sun_camera_matrices_thingy,
				sun_camera_single_matrix_thingy: &game.sun_camera_single_matrix_thingy,
				shadow_map_cascade_view_thingies: &game.shadow_map_cascade_view_thingies,
				chunk_grid: &game.chunk_grid,
				z_buffer_view: &game.z_buffer_view,
				selected_camera: game.selected_camera,
				enable_display_phys_box: game.enable_display_phys_box,
				player_box_mesh: &player_box_mesh,
				player_blocks_box_mesh: &player_blocks_box_mesh,
				targeted_block_box_mesh_opt: &targeted_block_box_mesh_opt,
				enable_display_interface: game.enable_display_interface,
				chunk_box_meshes: &chunk_box_meshes,
				skybox_mesh: &skybox_mesh,
				typing_in_command_line: game.typing_in_command_line,
				cursor_mesh: &game.cursor_mesh,
				interface_simple_texture_mesh: &interface_simple_texture_mesh,
				interface_simple_line_mesh: &interface_simple_line_mesh,
			};
			data_for_rendering.render();

			if game.close_after_one_frame {
				println!("Closing after one frame, as asked via command line arguments");
				elwt.exit();
			}
		},

		Event::LoopExiting => {
			if game.save.is_some() {
				save_savable_state(&game);
				game.chunk_grid.unload_all_chunks(game.save.as_ref());
			}
		},

		_ => {},
	});
	res.unwrap();
}
