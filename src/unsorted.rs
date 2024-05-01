//! TODO: Move everything in here to more appropriate modules!

use cgmath::ElementWise;
use clap::ValueEnum;
use std::sync::{atomic::AtomicI32, Arc};
use wgpu::util::DeviceExt;

use crate::{
	atlas::Atlas,
	chunk_blocks::{ChunkBlocks, ChunkCullingInfo},
	chunk_loading::DataForChunkLoading,
	chunk_meshing::{ChunkMesh, DataForChunkMeshing},
	coords::{ChunkCoords, ChunkCoordsSpan},
	entities::ChunkEntities,
	shaders::{self, simple_texture_2d::SimpleTextureVertexPod},
	skybox::SkyboxFaces,
	threadpool::ThreadPool,
};

#[derive(Clone, Copy)]
pub(crate) enum WhichCameraToUse {
	FirstPerson,
	ThirdPersonNear,
	ThirdPersonFar,
	ThirdPersonVeryFar,
	ThirdPersonTooFar,
	Sun,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) enum Control {
	KeyboardKey(winit::keyboard::Key),
	MouseButton(winit::event::MouseButton),
}
pub(crate) struct ControlEvent {
	pub(crate) control: Control,
	pub(crate) pressed: bool,
}
pub(crate) enum Action {
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
	ThrowBlock,
	ToggleDisplayChunksWithEntitiesAsBoxes,
}

/// The main-thread reciever for the results of a task that was given to a worker thread.
pub(crate) enum WorkerTask {
	LoadChunkBlocksAndEntities(
		ChunkCoords,
		std::sync::mpsc::Receiver<(ChunkBlocks, ChunkCullingInfo, Option<ChunkEntities>)>,
	),
	MeshChunk(ChunkCoords, std::sync::mpsc::Receiver<Option<ChunkMesh>>),
	/// The counter at the end is the number of faces already finished.
	PaintNewSkybox(std::sync::mpsc::Receiver<SkyboxFaces>, Arc<AtomicI32>),
	GenerateAtlas(std::sync::mpsc::Receiver<Atlas>),
}

pub(crate) struct CurrentWorkerTasks {
	pub(crate) tasks: Vec<WorkerTask>,
}

impl CurrentWorkerTasks {
	pub(crate) fn run_chunk_meshing_task(
		&mut self,
		pool: &mut ThreadPool,
		chunk_coords: ChunkCoords,
		data_for_chunk_meshing: DataForChunkMeshing,
		device: Arc<wgpu::Device>,
	) {
		let (sender, receiver) = std::sync::mpsc::channel();
		self.tasks.push(WorkerTask::MeshChunk(chunk_coords, receiver));
		pool.enqueue_task(Box::new(move || {
			let vertices = data_for_chunk_meshing.generate_mesh_vertices();
			let non_empty_mesh = !vertices.is_empty();
			let mesh = non_empty_mesh.then(|| ChunkMesh::from_vertices(&device, vertices));
			let _ = sender.send(mesh);
		}));
	}

	pub(crate) fn is_being_meshed(&self, chunk_coords: ChunkCoords) -> bool {
		self.tasks.iter().any(|worker_task| match worker_task {
			WorkerTask::MeshChunk(chunk_coords_uwu, ..) => *chunk_coords_uwu == chunk_coords,
			_ => false,
		})
	}

	pub(crate) fn run_chunk_loading_task(
		&mut self,
		pool: &mut ThreadPool,
		chunk_coords: ChunkCoords,
		data_for_chunk_loading: DataForChunkLoading,
	) {
		let (sender, receiver) = std::sync::mpsc::channel();
		self.tasks.push(WorkerTask::LoadChunkBlocksAndEntities(
			chunk_coords,
			receiver,
		));
		pool.enqueue_task(Box::new(move || {
			let DataForChunkLoading {
				was_already_generated_before,
				world_generator,
				block_type_table,
				save,
				cd,
			} = data_for_chunk_loading;
			let coords_span = ChunkCoordsSpan { cd, chunk_coords };

			// Loading a chunk means either loading from save (disk)
			// if there is a save and the chunk was already generated and saved in the past,
			// or else generating it.

			// The block data and the entities are not to be handled in the same way.
			//
			// The blocks may or may not have been saved even if already generated (it depends
			// on if they were modified since generation and the `only_save_modified_chunks` setting).
			//
			// The entities are always saved, and sometimes even saved in chunks that were never
			// generated (it can happen if an entity goes into a chunk that is outside of the area
			// in which chunks are allowed to be loaded). An entity that does not decide to disappear
			// is never lost (always saved) so once it is generated it must not be generated again
			// because the first one is still around.

			// First we load what we have from the save (if any).
			let blocks_from_save =
				save.as_ref().and_then(|save| ChunkBlocks::load_from_save(coords_span, save));
			let entities_from_save = save.as_ref().and_then(|save| {
				ChunkEntities::load_from_save_while_removing_the_save(coords_span, save)
			});

			// If the entities were already generated, then they have been saved, and we must not
			// generate then an other time to avoid duplicating them.
			let keep_generated_entities = !was_already_generated_before;
			// If the blocks were not saved, then we have to generate to get the blocks.
			let generation_needed = blocks_from_save.is_none() || keep_generated_entities;

			// Now the generation happens if needed.
			let blocks_and_entities_from_gen = generation_needed.then(|| {
				world_generator.generate_chunk_blocks_and_entities(coords_span, &block_type_table)
			});
			let (blocks_from_gen, entities_from_gen) = match blocks_and_entities_from_gen {
				Some((blocks, entities)) => (Some(blocks), Some(entities)),
				None => (None, None),
			};
			let entities_from_gen = keep_generated_entities.then_some(entities_from_gen).flatten();

			// Sorting what we got. At the end, we must have one `ChunkBlocks`
			// and one `Option<ChunkEntities>` (which should be `None` if empty).
			let blocks = blocks_from_save.or(blocks_from_gen).unwrap();
			let entities = match (entities_from_save, entities_from_gen) {
				(Some(entities_save), Some(entities_gen)) => Some(entities_save.merged(entities_gen)),
				(Some(entities), _) | (_, Some(entities)) => Some(entities),
				(None, None) => None,
			};
			let entities = entities.filter(|entities| entities.count_entities() >= 1);

			let culling_info = ChunkCullingInfo::compute_from_blocks(&blocks, &block_type_table);

			let _ = sender.send((blocks, culling_info, entities));
		}));
	}

	pub(crate) fn is_being_loaded(&self, chunk_coords: ChunkCoords) -> bool {
		self.tasks.iter().any(|worker_task| match worker_task {
			WorkerTask::LoadChunkBlocksAndEntities(chunk_coords_uwu, ..) => {
				*chunk_coords_uwu == chunk_coords
			},
			_ => false,
		})
	}
}

pub(crate) struct SimpleTextureMesh {
	pub(crate) vertex_count: u32,
	pub(crate) vertex_buffer: wgpu::Buffer,
}

impl SimpleTextureMesh {
	pub(crate) fn from_vertices(
		device: &wgpu::Device,
		vertices: Vec<shaders::simple_texture_2d::SimpleTextureVertexPod>,
	) -> SimpleTextureMesh {
		let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Simple Texture Vertex Buffer"),
			contents: bytemuck::cast_slice(&vertices),
			usage: wgpu::BufferUsages::VERTEX,
		});
		SimpleTextureMesh { vertex_count: vertices.len() as u32, vertex_buffer }
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

	pub(crate) fn vertices_for_rect(
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
pub(crate) struct RectInAtlas {
	pub(crate) texture_rect_in_atlas_xy: cgmath::Point2<f32>,
	pub(crate) texture_rect_in_atlas_wh: cgmath::Vector2<f32>,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum PlayingMode {
	/// Playing the game and facing its challenges without cheating being allowed by the game.
	Play,
	/// Free from the limitations of the `Play` mode.
	Free,
}
