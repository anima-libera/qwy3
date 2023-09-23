mod camera;
mod coords;
mod shaders;

use std::{collections::HashMap, f32::consts::TAU, ops::Deref};

use bytemuck::Zeroable;
use cgmath::InnerSpace;
use wgpu::util::DeviceExt;
use winit::{
	event_loop::{ControlFlow, EventLoop},
	window::WindowBuilder,
};

use camera::{aspect_ratio, CameraPerspectiveSettings, Matrix4x4Pod};
use coords::*;
use shaders::{block::BlockVertexPod, simple_line::SimpleLineVertexPod};

#[derive(Clone, Copy)]
struct BlockTypeId {
	is_not_air: bool,
}

/// The blocks of a chunk.
struct ChunkBlocks {
	coords_span: ChunkCoordsSpan,
	blocks: Vec<BlockTypeId>,
}

impl ChunkBlocks {
	fn new(coords_span: ChunkCoordsSpan) -> ChunkBlocks {
		ChunkBlocks {
			coords_span,
			blocks: Vec::from_iter(
				std::iter::repeat(BlockTypeId { is_not_air: false })
					.take(coords_span.cd.number_of_blocks()),
			),
		}
	}

	fn get(&self, coords: BlockCoords) -> Option<BlockTypeId> {
		Some(self.blocks[self.coords_span.internal_index(coords)?])
	}

	fn get_mut(&mut self, coords: BlockCoords) -> Option<&mut BlockTypeId> {
		Some(&mut self.blocks[self.coords_span.internal_index(coords)?])
	}
}

/// Information about the opaqueness of each block
/// contained in a 1-block-thick cubic layer around a chunk.
struct OpaquenessLayerAroundChunk {
	/// The coords span of the chunk that is surrounded by the layer that this struct describes.
	/// This is NOT the coords span of the layer. The layer is 1-block thick and encloses that
	/// coords span.
	surrounded_chunk_coords_span: ChunkCoordsSpan,
	data: bitvec::vec::BitVec,
}

impl OpaquenessLayerAroundChunk {
	fn new(surrounded_chunk_coords_span: ChunkCoordsSpan) -> OpaquenessLayerAroundChunk {
		let data = bitvec::vec::BitVec::with_capacity(OpaquenessLayerAroundChunk::data_size(
			surrounded_chunk_coords_span.cd,
		));
		OpaquenessLayerAroundChunk { surrounded_chunk_coords_span, data }
	}

	fn data_size(cd: ChunkDimensions) -> usize {
		let face_size = cd.edge.pow(2);
		let edge_size = cd.edge;
		let corner_size = 1;
		(face_size * 6 + edge_size * 12 + corner_size * 8) as usize
	}

	/// One of the functions of all times, ever!
	fn coords_to_index_in_data(&self, coords: BlockCoords) -> Option<usize> {
		// Ok so here the goal is to map the coords that are in the layer to a unique index.
		if self.surrounded_chunk_coords_span.contains(coords) {
			// If we fall in the chunk that the layer encloses, then we are not in the layer
			// but we are just in the hole in the middle of the layer.
			return None;
		}
		// Get `inf` and `sup` to represent the cube that is the layer (if we ignore the hole
		// in the middle that was already taken care of), `sup` is included.
		let inf: BlockCoords =
			self.surrounded_chunk_coords_span.block_coords_inf() - cgmath::vec3(1, 1, 1);
		let sup: BlockCoords = self
			.surrounded_chunk_coords_span
			.block_coords_sup_excluded();
		let contained_in_the_layer = inf.x <= coords.x
			&& coords.x <= sup.x
			&& inf.y <= coords.y
			&& coords.y <= sup.y
			&& inf.z <= coords.z
			&& coords.z <= sup.z;
		if !contained_in_the_layer {
			// We are outside of the layer.
			return None;
		}
		// If we get here, then it meas we are in the layer and a unique index has to be determined.
		// `layer_edge` is the length in blocks of an edge of the layer.
		let layer_edge = self.surrounded_chunk_coords_span.cd.edge + 2;
		let ix = coords.x - inf.x;
		let iy = coords.y - inf.y;
		let iz = coords.z - inf.z;
		if iz == 0 {
			// Bottom face (lowest Z value).
			Some((ix + iy * layer_edge) as usize)
		} else if iz == layer_edge - 1 {
			// Top face (higest Z value).
			Some((layer_edge.pow(2) + (ix + iy * layer_edge)) as usize)
		} else {
			// One of the side faces that are not in the top/bottom faces.
			// We consider horizontal slices of the layer which are just squares here,
			// `sub_index` is just a unique index in the square we are in, and
			// we have to add enough `square_size` to distinguish between the different squares
			// (for different Z values).
			// `square_size` is the number of blocks in the line of the square (no middle).
			let square_size = (layer_edge - 1) * 4;
			let sub_index = if ix == 0 {
				iy
			} else if ix == layer_edge - 1 {
				layer_edge + iy
			} else if iy == 0 {
				layer_edge * 2 + (ix - 1)
			} else if iy == layer_edge - 1 {
				layer_edge * 2 + (layer_edge - 2) + (ix - 1)
			} else {
				unreachable!()
			};
			Some((layer_edge.pow(2) * 2 + (iz - 1) * square_size + sub_index) as usize)
			// >w<
		}
	}

	fn set(&mut self, coords: BlockCoords, value: bool) {
		let index = self.coords_to_index_in_data(coords).unwrap();
		self.data.set(index, value);
	}

	fn get(&mut self, coords: BlockCoords) -> Option<bool> {
		let index = self.coords_to_index_in_data(coords)?;
		Some(*self.data.get(index).unwrap())
	}
}

#[cfg(test)]
mod test {
	use super::*;
	#[test]
	fn indexing_of_the_funky_layer_data_structure() {
		let cd = ChunkDimensions::from(18);
		let chunk_coords_span = ChunkCoordsSpan { cd, chunk_coords: (2, -3, 0).into() };
		let layer = OpaquenessLayerAroundChunk::new(chunk_coords_span);

		let layer_size = OpaquenessLayerAroundChunk::data_size(cd);
		let mut indices = vec![];

		// We iterate over all the coords in the layer and in the layer hole.
		let inf: BlockCoords = chunk_coords_span.block_coords_inf() - cgmath::vec3(1, 1, 1);
		let sup_excluded: BlockCoords =
			chunk_coords_span.block_coords_sup_excluded() + cgmath::vec3(1, 1, 1);
		for coords in iter_3d_rect_inf_sup_excluded(inf, sup_excluded) {
			let index_opt = layer.coords_to_index_in_data(coords);
			if let Some(index) = index_opt {
				// The layer gave an index to the coords, which means we are supposed
				// to be on the layer.
				// Indices must be unique so we check for that.
				assert!(!indices.contains(&index));
				indices.push(index);
			} else {
				// The layer didn't give an index so it means we are not on the layer.
				// We don't test for coords outside of the layer and its hole so we
				// must be in the hole at the center of the layer.
				assert!(chunk_coords_span.contains(coords));
			}
		}

		// The indices must be unique, and we already checked for that.
		// The indices also must cover all possible indices from 0 to the max expected index,
		// we check for that here.
		for expected_index in 0..layer_size {
			assert!(indices.contains(&expected_index));
		}
	}
}

impl ChunkBlocks {
	fn generate_mesh_assuming_surrounded_by_opaque_or_transparent(
		&self,
		surrounded_by_opaque: bool,
	) -> ChunkMesh {
		let mut block_vertices = Vec::new();
		for coords in self.coords_span.iter_coords() {
			if self.get(coords).unwrap().is_not_air {
				for direction in OrientedAxis::all_the_six_possible_directions() {
					let is_covered_by_neighbor = {
						let neighbor_coords = coords + direction.delta();
						if self.coords_span.contains(neighbor_coords) {
							self.get(neighbor_coords).unwrap().is_not_air
						} else {
							surrounded_by_opaque
						}
					};
					if !is_covered_by_neighbor {
						generate_block_face_mesh(
							&mut block_vertices,
							direction,
							coords.map(|x| x as f32),
						);
					}
				}
			}
		}
		ChunkMesh::from_vertices(block_vertices)
	}

	/// Generates the faces of blocks in the `self` chunk blocks that touch blocks in
	/// the neighbor chunk blocks `neighbor_chunk` and that can be known to be visible
	/// given the blocks in the neighbor chunk.
	fn generate_missing_faces_on_chunk_boarder_in_mesh(
		&self,
		chunk_mesh: &mut ChunkMesh,
		neighbor_chunk: &ChunkBlocks,
	) {
		assert!(is_neighbor_with(
			self.coords_span.chunk_coords,
			neighbor_chunk.coords_span.chunk_coords
		));
		// Note that this is redundent with the `unwrap` of the direction...
		// TODO: Remove?
		let direction = direction_to_neighbor(
			self.coords_span.chunk_coords,
			neighbor_chunk.coords_span.chunk_coords,
		)
		.unwrap();
		for coords in self.coords_span.iter_block_coords_on_chunk_face(direction) {
			if self.get(coords).unwrap().is_not_air {
				let covering_block_coords = coords + direction.delta();
				assert!(neighbor_chunk.coords_span.contains(covering_block_coords));
				let covered = neighbor_chunk
					.get(covering_block_coords)
					.unwrap()
					.is_not_air;
				if !covered {
					generate_block_face_mesh(
						&mut chunk_mesh.block_vertices,
						direction,
						coords.map(|x| x as f32),
					);
				}
			}
		}
		chunk_mesh.cpu_to_gpu_update_required = true;
	}
}

struct ChunkMesh {
	block_vertices: Vec<BlockVertexPod>,
	block_vertex_buffer: Option<wgpu::Buffer>,
	// When `block_vertices` is modified, `block_vertex_buffer` becomes out of sync
	// and must be updated. This is what this field keeps track of.
	cpu_to_gpu_update_required: bool,
}

impl ChunkMesh {
	fn from_vertices(block_vertices: Vec<BlockVertexPod>) -> ChunkMesh {
		let cpu_to_gpu_update_required = !block_vertices.is_empty();
		ChunkMesh {
			block_vertices,
			block_vertex_buffer: None,
			cpu_to_gpu_update_required,
		}
	}

	fn update_gpu_data(&mut self, device: &wgpu::Device) {
		let block_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Block Vertex Buffer"),
			contents: bytemuck::cast_slice(&self.block_vertices),
			usage: wgpu::BufferUsages::VERTEX,
		});
		self.block_vertex_buffer = Some(block_vertex_buffer);
		self.cpu_to_gpu_update_required = false;
	}
}

/// Generate the mesh of a face of a block, adding it to `vertices`.
fn generate_block_face_mesh(
	vertices: &mut Vec<BlockVertexPod>,
	face_orientation: OrientedAxis,
	block_center: cgmath::Point3<f32>,
) {
	// NO EARLY OPTIMIZATION
	// This shall remain in an unoptimized, unfactorized and flexible state for now!

	// We are just meshing a single face, thus a square.
	// We start by 4 points at the center of a block.
	let mut a: cgmath::Point3<f32> = block_center;
	let mut b: cgmath::Point3<f32> = block_center;
	let mut c: cgmath::Point3<f32> = block_center;
	let mut d: cgmath::Point3<f32> = block_center;
	// We move the 4 points to the center of the face we are meshing.
	a[face_orientation.axis.index()] += 0.5 * face_orientation.orientation.sign() as f32;
	b[face_orientation.axis.index()] += 0.5 * face_orientation.orientation.sign() as f32;
	c[face_orientation.axis.index()] += 0.5 * face_orientation.orientation.sign() as f32;
	d[face_orientation.axis.index()] += 0.5 * face_orientation.orientation.sign() as f32;
	// In doing so we moved the points along some axis.
	// The two other axes are the ones that describe a plane in which the 4 points will be moved
	// to make a square, so we get these two other axes.
	let mut other_axes = [NonOrientedAxis::X, NonOrientedAxis::Y, NonOrientedAxis::Z]
		.into_iter()
		.filter(|&axis| axis != face_orientation.axis);
	let other_axis_a = other_axes.next().unwrap();
	let other_axis_b = other_axes.next().unwrap();
	assert!(other_axes.next().is_none());
	// Now we move each point from the center of the face square to one of the square vertex.
	a[other_axis_a.index()] -= 0.5;
	a[other_axis_b.index()] -= 0.5;
	b[other_axis_a.index()] -= 0.5;
	b[other_axis_b.index()] += 0.5;
	c[other_axis_a.index()] += 0.5;
	c[other_axis_b.index()] -= 0.5;
	d[other_axis_a.index()] += 0.5;
	d[other_axis_b.index()] += 0.5;
	// Face culling will discard triangles whose verices don't end up clipped to the screen in
	// a counter-clockwise order. This means that triangles must be counter-clockwise when
	// we look at their front and clockwise when we look at their back.
	// `reverse_order` makes sure that they have the right orientation.
	let reverse_order = match face_orientation.axis {
		NonOrientedAxis::X => face_orientation.orientation == AxisOrientation::Negativewards,
		NonOrientedAxis::Y => face_orientation.orientation == AxisOrientation::Positivewards,
		NonOrientedAxis::Z => face_orientation.orientation == AxisOrientation::Negativewards,
	};
	let normal = {
		let mut normal = [0.0, 0.0, 0.0];
		normal[face_orientation.axis.index()] = face_orientation.orientation.sign() as f32;
		normal
	};
	let color = [0.8, 0.8, 0.8];
	if !reverse_order {
		vertices.push(BlockVertexPod { position: a.into(), color, normal });
		vertices.push(BlockVertexPod { position: c.into(), color, normal });
		vertices.push(BlockVertexPod { position: b.into(), color, normal });
		vertices.push(BlockVertexPod { position: b.into(), color, normal });
		vertices.push(BlockVertexPod { position: c.into(), color, normal });
		vertices.push(BlockVertexPod { position: d.into(), color, normal });
	} else {
		vertices.push(BlockVertexPod { position: a.into(), color, normal });
		vertices.push(BlockVertexPod { position: b.into(), color, normal });
		vertices.push(BlockVertexPod { position: c.into(), color, normal });
		vertices.push(BlockVertexPod { position: b.into(), color, normal });
		vertices.push(BlockVertexPod { position: d.into(), color, normal });
		vertices.push(BlockVertexPod { position: c.into(), color, normal });
	}
}

struct Chunk {
	blocks: ChunkBlocks,
	mesh: Option<ChunkMesh>,
}

impl Chunk {
	fn new_empty(coords: ChunkCoordsSpan) -> Chunk {
		Chunk { blocks: ChunkBlocks::new(coords), mesh: None }
	}

	fn generate_mesh_assuming_surrounded_by_opaque_or_transparent(
		&mut self,
		surrounded_by_opaque: bool,
	) {
		let mesh = self
			.blocks
			.generate_mesh_assuming_surrounded_by_opaque_or_transparent(surrounded_by_opaque);
		self.mesh = Some(mesh);
	}
}

struct ChunkGrid {
	cd: ChunkDimensions,
	map: HashMap<ChunkCoords, Chunk>,
}

impl ChunkGrid {
	fn set_block(&mut self, coords: BlockCoords, block: BlockTypeId) {
		let chunk_coords = self.cd.world_coords_to_containing_chunk_coords(coords);
		match self.map.get_mut(&chunk_coords) {
			Some(chunk) => {
				let block_dst = chunk.blocks.get_mut(coords).unwrap();
				*block_dst = block;
			},
			None => {
				// TODO: Handle this case by storing the fact that a block
				// has to be set when loding the chunk.
				unimplemented!()
			},
		}
	}

	fn get_block(&self, coords: BlockCoords) -> Option<BlockTypeId> {
		let chunk_coords = self.cd.world_coords_to_containing_chunk_coords(coords);
		let chunk = self.map.get(&chunk_coords)?;
		Some(chunk.blocks.get(coords).unwrap())
	}

	fn get_opaqueness_layer_around_chunk(
		&self,
		chunk_coords: ChunkCoords,
		default_to_opaque: bool,
	) -> OpaquenessLayerAroundChunk {
		let surrounded_chunk_coords_span = ChunkCoordsSpan { cd: self.cd, chunk_coords };
		let mut layer = OpaquenessLayerAroundChunk::new(surrounded_chunk_coords_span);

		let inf = surrounded_chunk_coords_span.block_coords_inf() - cgmath::vec3(1, 1, 1);
		let sup_excluded =
			surrounded_chunk_coords_span.block_coords_sup_excluded() + cgmath::vec3(1, 1, 1);
		for z in inf.z..sup_excluded.z {
			for y in inf.y..sup_excluded.y {
				let mut x = inf.x;
				while x < sup_excluded.x {
					let coords: BlockCoords = (x, y, z).into();
					if surrounded_chunk_coords_span.contains(coords) {
						// We skip over the chunk hole in the middle of the layer.
						x = sup_excluded.x - 1;
					} else {
						{
							let opaque = self
								.get_block(coords)
								.map(|block_type_id| block_type_id.is_not_air)
								.unwrap_or(default_to_opaque);
							layer.set(coords, opaque);
						}
						x += 1;
					}
				}
			}
		}

		layer
	}
}

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

/// Mesh of simple lines.
///
/// Can be used (for example) to display hit boxes for debugging purposes.
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

	fn from_aligned_box(device: &wgpu::Device, aligned_box: &AlignedBox) -> SimpleLineMesh {
		// NO EARLY OPTIMIZATION
		// This shall remain in an unoptimized, unfactorized and flexible state for now!

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

/// Vector in 3D.
#[derive(Copy, Clone, Debug)]
/// Certified Plain Old Data (so it can be sent to the GPU as a uniform).
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vector3Pod {
	values: [f32; 3],
}

pub fn run() {
	// Wgpu uses the `log`/`env_logger` crates to log errors and stuff,
	// and we do want to see the errors very much.
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

	// At some point it could be nice to allow the user to choose their preferred adapter.
	// No one should have to struggle to make some game use the big GPU instead of the tiny one.
	println!("AVAILABLE ADAPTERS:");
	for adapter in instance.enumerate_adapters(wgpu::Backends::all()) {
		dbg!(adapter.get_info());
	}
	println!("SELECTED ADAPTER:");
	dbg!(adapter.get_info());

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
	})
	.unwrap();

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
	let mut config = wgpu::SurfaceConfiguration {
		usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
		format: surface_format,
		width: size.width,
		height: size.height,
		present_mode: wgpu::PresentMode::Fifo,
		alpha_mode: surface_capabilities.alpha_modes[0],
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

	let mut camera_direction = AngularDirection::from_angle_horizontal(0.0);
	let mut enable_camera_third_person = false;

	let mut cursor_is_captured = true;
	window
		.set_cursor_grab(winit::window::CursorGrabMode::Confined)
		.unwrap();
	window.set_cursor_visible(false);

	let mut walking_forward = false;
	let mut walking_backward = false;
	let mut walking_leftward = false;
	let mut walking_rightward = false;

	let mut player_phys = AlignedPhysBox {
		aligned_box: AlignedBox { pos: (5.5, 5.5, 5.5).into(), dims: (0.8, 0.8, 1.8).into() },
		motion: (0.0, 0.0, 0.0).into(),
		gravity_factor: 1.0,
	};
	let mut enable_physics = true;
	let mut enable_display_phys_box = true;

	let cd = ChunkDimensions::from(10);

	let mut chunk_grid = ChunkGrid { cd, map: HashMap::new() };
	for chunk_coords in iter_3d_cube_center_radius((0, 0, 0).into(), 3) {
		let chunk = Chunk::new_empty(ChunkCoordsSpan { cd, chunk_coords });
		chunk_grid.map.insert(chunk_coords, chunk);
	}

	for (_chunk_coords, chunk) in chunk_grid.map.iter_mut() {
		for coords in chunk.blocks.coords_span.iter_coords() {
			// Test chunk generation.
			let ground = coords.z as f32
				- f32::cos(coords.x as f32 * 0.3)
				- f32::cos(coords.y as f32 * 0.3)
				- 3.0 < 0.0;
			*chunk.blocks.get_mut(coords).unwrap() = BlockTypeId { is_not_air: ground };
		}
	}

	// It seems pretty hard (or at least it requires a trick that I didn't figure out yet)
	// to iterate over pairs of values of a HashMap and be able to mutate them.
	// The borrow checker is hard but fair, but here it is too hard.
	// Borrowing the chunk map mutably in a more fine grained manner is necessary here,
	// and iterating over the keys without borrowing it is the only way I could find.
	// Also removing a chunk from the map to modify its mesh while iterating over other chunks
	// in the map before putting the chunk back in is the only thing that worked (among the lots
	// of things that I tried).
	// TODO: Find a better way maybe?
	// TODO: Definitely find something better here, this piece of code is highly questionable.
	let chunk_coords_list: Vec<_> = chunk_grid.map.keys().cloned().collect();
	for chunk_coords in chunk_coords_list {
		// We are going to generate the mesh of the `chunk`. It will be inserted back in
		// when we are done with it.
		let mut chunk = chunk_grid.map.remove(&chunk_coords).unwrap();

		chunk.generate_mesh_assuming_surrounded_by_opaque_or_transparent(true);

		for direction in OrientedAxis::all_the_six_possible_directions() {
			let neighbor_chunk_coords = chunk_coords + direction.delta();
			if chunk_grid.map.contains_key(&neighbor_chunk_coords) {
				let neighbor_chunk = chunk_grid.map.get(&neighbor_chunk_coords).unwrap();
				let Chunk { ref blocks, ref mut mesh, .. } = chunk;
				blocks.generate_missing_faces_on_chunk_boarder_in_mesh(
					mesh.as_mut().unwrap(),
					&neighbor_chunk.blocks,
				);
			}
		}

		chunk.mesh.as_mut().unwrap().update_gpu_data(&device);
		chunk_grid.map.insert(chunk_coords, chunk);
	}

	let mut sun_position_in_sky = AngularDirection::from_angles(TAU / 16.0, TAU / 8.0);
	let sun_light_direction_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Sun Light Direction Buffer"),
		contents: bytemuck::cast_slice(&[Vector3Pod::zeroed()]),
		usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
	});
	let sun_light_direction_bind_group_layout =
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
			label: Some("Sun Light Direction Bind Group Layout"),
		});
	let sun_light_direction_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
		layout: &sun_light_direction_bind_group_layout,
		entries: &[wgpu::BindGroupEntry {
			binding: 0,
			resource: sun_light_direction_buffer.as_entire_binding(),
		}],
		label: Some("Sun Light Direction Bind Group"),
	});

	fn make_z_buffer_texture_view(
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
	let z_buffer_format = wgpu::TextureFormat::Depth32Float;
	let mut z_buffer_view =
		make_z_buffer_texture_view(&device, z_buffer_format, config.width, config.height);

	let block_render_pipeline = shaders::block::render_pipeline(
		&device,
		&camera_bind_group_layout,
		&sun_light_direction_bind_group_layout,
		config.format,
		z_buffer_format,
	);

	let simple_line_render_pipeline = shaders::simple_line::render_pipeline(
		&device,
		&camera_bind_group_layout,
		config.format,
		z_buffer_format,
	);

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
				input: KeyboardInput { state, virtual_keycode: Some(key), .. },
				..
			} => match (key, state) {
				(VirtualKeyCode::Z, _)
				| (VirtualKeyCode::S, _)
				| (VirtualKeyCode::Q, _)
				| (VirtualKeyCode::D, _) => {
					let moving_in_some_direction = match key {
						VirtualKeyCode::Z => &mut walking_forward,
						VirtualKeyCode::S => &mut walking_backward,
						VirtualKeyCode::Q => &mut walking_leftward,
						VirtualKeyCode::D => &mut walking_rightward,
						_ => unreachable!(),
					};
					*moving_in_some_direction = *state == ElementState::Pressed;
				},

				(VirtualKeyCode::P, ElementState::Pressed) => enable_physics = !enable_physics,

				(VirtualKeyCode::M, ElementState::Pressed) => {
					enable_camera_third_person = !enable_camera_third_person
				},

				(VirtualKeyCode::L, ElementState::Pressed) => {
					enable_display_phys_box = !enable_display_phys_box
				},

				(VirtualKeyCode::K, ElementState::Pressed) => {
					cursor_is_captured = !cursor_is_captured;
					if cursor_is_captured {
						window
							.set_cursor_grab(winit::window::CursorGrabMode::Confined)
							.unwrap();
						window.set_cursor_visible(false);
					} else {
						window
							.set_cursor_grab(winit::window::CursorGrabMode::None)
							.unwrap();
						window.set_cursor_visible(true);
					}
				},

				(VirtualKeyCode::H, ElementState::Pressed) => {
					dbg!(player_phys.aligned_box.pos);
					let player_bottom = player_phys.aligned_box.pos
						- cgmath::Vector3::<f32>::from((0.0, 0.0, player_phys.aligned_box.dims.z / 2.0));
					dbg!(player_bottom);
				},

				(VirtualKeyCode::O, ElementState::Pressed) => {
					let player_bottom = player_phys.aligned_box.pos
						- cgmath::Vector3::<f32>::unit_z() * (player_phys.aligned_box.dims.z / 2.0 + 0.1);
					let player_bottom_block_coords = player_bottom.map(|x| x.round() as i32);
					let player_bottom_block_opt = chunk_grid.get_block(player_bottom_block_coords);
					if let Some(block) = player_bottom_block_opt {
						chunk_grid.set_block(
							player_bottom_block_coords,
							BlockTypeId { is_not_air: !block.is_not_air },
						);

						let chunk_coords =
							cd.world_coords_to_containing_chunk_coords(player_bottom_block_coords);
						let chunk = chunk_grid.map.get_mut(&chunk_coords).unwrap();
						chunk.generate_mesh_assuming_surrounded_by_opaque_or_transparent(true);
						chunk.mesh.as_mut().unwrap().update_gpu_data(&device);
						// TODO: If the block is on the edge of a chunk we also need to update the
						// meshes of these chunks.
					}
				},
				_ => {},
			},

			WindowEvent::MouseInput {
				state: winit::event::ElementState::Pressed,
				button: winit::event::MouseButton::Right,
				..
			} if cursor_is_captured => {
				player_phys.motion.z = 0.1;
			},

			WindowEvent::MouseInput {
				state: winit::event::ElementState::Pressed,
				button: winit::event::MouseButton::Left,
				..
			} if !cursor_is_captured => {
				cursor_is_captured = true;
				window
					.set_cursor_grab(winit::window::CursorGrabMode::Confined)
					.unwrap();
				window.set_cursor_visible(false);
			},
			_ => {},
		},

		Event::DeviceEvent { event: winit::event::DeviceEvent::MouseMotion { delta }, .. }
			if cursor_is_captured =>
		{
			let sensitivity = 0.01;
			camera_direction.angle_horizontal += -1.0 * delta.0 as f32 * sensitivity;
			camera_direction.angle_vertical += delta.1 as f32 * sensitivity;
			if camera_direction.angle_vertical < 0.0 {
				camera_direction.angle_vertical = 0.0;
			}
			if TAU / 2.0 < camera_direction.angle_vertical {
				camera_direction.angle_vertical = TAU / 2.0;
			}
		},

		Event::DeviceEvent { event: winit::event::DeviceEvent::MouseWheel { delta }, .. } => {
			let (dx, dy) = match delta {
				MouseScrollDelta::LineDelta(horizontal, vertical) => (horizontal, vertical),
				MouseScrollDelta::PixelDelta(position) => (position.x as f32, position.y as f32),
			};
			let sensitivity = 0.01;
			let direction_left_or_right = camera_direction
				.to_horizontal()
				.add_to_horizontal_angle(TAU / 4.0 * dx.signum());
			player_phys.aligned_box.pos.z -= dy * sensitivity;
			player_phys.aligned_box.pos +=
				direction_left_or_right.to_vec3() * f32::abs(dx) * sensitivity;
		},

		Event::MainEventsCleared => {
			let _time_since_beginning = time_beginning.elapsed();
			let now = std::time::Instant::now();
			let dt = now - time_from_last_iteration;
			time_from_last_iteration = now;

			let walking_vector = {
				let walking_factor = if enable_physics { 12.0 } else { 35.0 } * dt.as_secs_f32();
				let walking_forward_factor =
					if walking_forward { 1 } else { 0 } + if walking_backward { -1 } else { 0 };
				let walking_rightward_factor =
					if walking_rightward { 1 } else { 0 } + if walking_leftward { -1 } else { 0 };
				let walking_forward_direction =
					camera_direction.to_horizontal().to_vec3() * walking_forward_factor as f32;
				let walking_rightward_direction = camera_direction
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
			player_phys.aligned_box.pos += walking_vector;

			if enable_physics {
				// TODO: Work out something better here,
				// although it is not very important at the moment.
				let player_bottom = player_phys.aligned_box.pos
					- cgmath::Vector3::<f32>::from((0.0, 0.0, player_phys.aligned_box.dims.z / 2.0));
				let player_bottom_below = player_phys.aligned_box.pos
					- cgmath::Vector3::<f32>::from((
						0.0,
						0.0,
						player_phys.aligned_box.dims.z / 2.0 + 0.01,
					));
				let player_bottom_block_coords = player_bottom.map(|x| x.round() as i32);
				let player_bottom_block_coords_below = player_bottom_below.map(|x| x.round() as i32);
				let player_bottom_block_opt = chunk_grid.get_block(player_bottom_block_coords);
				let player_bottom_block_opt_below =
					chunk_grid.get_block(player_bottom_block_coords_below);
				let is_on_ground = if player_phys.motion.z <= 0.0 {
					if let Some(block) = player_bottom_block_opt_below {
						if block.is_not_air {
							// The player is on the ground, so we make sure we are not overlapping it.
							player_phys.motion.z = 0.0;
							player_phys.aligned_box.pos.z = player_bottom_block_coords_below.z as f32
								+ 0.5 + player_phys.aligned_box.dims.z / 2.0;
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
				let is_in_ground = if player_phys.motion.z <= 0.0 {
					if let Some(block) = player_bottom_block_opt {
						if block.is_not_air {
							// The player is inside the ground, so we uuh.. do something?
							player_phys.motion.z = 0.0;
							player_phys.aligned_box.pos.z = player_bottom_block_coords.z as f32
								+ 0.5 + player_phys.aligned_box.dims.z / 2.0;
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
				player_phys.aligned_box.pos += player_phys.motion;
				if !is_on_ground {
					player_phys.motion.z -= player_phys.gravity_factor * 0.3 * dt.as_secs_f32();
				}
				if is_in_ground {
					player_phys.aligned_box.pos.z += 0.01;
				}
			}

			let player_box_mesh = SimpleLineMesh::from_aligned_box(&device, &player_phys.aligned_box);

			let camera_view_projection_matrix = {
				let mut camera_position = player_phys.aligned_box.pos
					+ cgmath::Vector3::<f32>::from((0.0, 0.0, player_phys.aligned_box.dims.z / 2.0))
						* 0.7;
				let camera_direction_vector = camera_direction.to_vec3();
				if enable_camera_third_person {
					camera_position -= camera_direction_vector * 5.0;
				}
				let camera_up_vector = camera_direction.add_to_vertical_angle(-TAU / 4.0).to_vec3();
				camera.view_projection_matrix(
					camera_position,
					camera_direction_vector,
					camera_up_vector,
				)
			};
			queue.write_buffer(
				&camera_matrix_buffer,
				0,
				bytemuck::cast_slice(&[camera_view_projection_matrix]),
			);

			sun_position_in_sky.angle_horizontal += (TAU / 30.0) * dt.as_secs_f32();
			let sun_light_direction = Vector3Pod { values: sun_position_in_sky.to_vec3().into() };
			queue.write_buffer(
				&sun_light_direction_buffer,
				0,
				bytemuck::cast_slice(&[sun_light_direction]),
			);

			let window_texture = window_surface.get_current_texture().unwrap();
			let window_texture_view = window_texture
				.texture
				.create_view(&wgpu::TextureViewDescriptor::default());
			let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
				label: Some("Render Encoder"),
			});
			let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("Render Pass"),
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &window_texture_view,
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

			render_pass.set_pipeline(&block_render_pipeline);
			render_pass.set_bind_group(0, &camera_bind_group, &[]);
			render_pass.set_bind_group(1, &sun_light_direction_bind_group, &[]);
			for chunk in chunk_grid.map.values() {
				if let Some(ref mesh) = chunk.mesh {
					render_pass
						.set_vertex_buffer(0, mesh.block_vertex_buffer.as_ref().unwrap().slice(..));
					render_pass.draw(0..(mesh.block_vertices.len() as u32), 0..1);
				}
			}

			if enable_display_phys_box {
				render_pass.set_pipeline(&simple_line_render_pipeline);
				render_pass.set_bind_group(0, &camera_bind_group, &[]);
				render_pass.set_vertex_buffer(0, player_box_mesh.vertex_buffer.slice(..));
				render_pass.draw(0..(player_box_mesh.vertices.len() as u32), 0..1);
			}

			// Release `render_pass.parent` which is a ref mut to `encoder`.
			drop(render_pass);

			queue.submit(std::iter::once(encoder.finish()));
			window_texture.present();
		},
		_ => {},
	});
}
