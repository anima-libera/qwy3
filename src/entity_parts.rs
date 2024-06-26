//! Entity parts handling.
//!
//! An entity part is an instanced model that is rendered with the world.
//! An entity can have as many or as few parts as it desires.
//!
//! The parts are instanced, so there is one table of instances per model.
//! The models can be simple shapes (such as a cube), or whatever.
//!
//! The models are not textured nor colored, the texturing/coloring is done per instance.
//! But the actual texturing/coloring data for all the vertices of the model is not inside
//! the instance, rather it is in a table of such data, and the instace points to the offset
//! at which sits the texturing/coloring data it wants.
//! Thus there are 3 Wgpu buffers:
//!   - the model mesh (one per part table).
//!   - the instance table (one per part table).
//!   - the texturing/coloring data table (one for all the tables).
//!
//! One part table owns one model and owns all the instances of that model.
//!
//! The buffer of the actual texturing/coloring data is `texturing_and_coloring_array_thingy`.

use std::{
	collections::HashMap,
	marker::PhantomData,
	sync::{Arc, Mutex, RwLock},
};

use bytemuck::Zeroable;
use cgmath::{EuclideanSpace, Matrix, SquareMatrix};
use fxhash::FxHashMap;
use wgpu::util::DeviceExt;

use crate::{
	block_types::{BlockType, BlockTypeId, BlockTypeTable},
	coords::{AxisOrientation, NonOrientedAxis, OrientedAxis},
	rendering_init::BindingThingy,
	shaders::{part_colored::PartColoredInstancePod, part_textured::PartTexturedInstancePod},
	table_allocator::{AllocationDecision, FreeingAdvice, TableAllocator},
};

use self::{
	colored_cube::ColoredCubePartKind, colored_icosahedron::ColoredIcosahedronPartKind,
	textured_cube::TexturedCubePartKind,
};

/// Handler to an entity part instance of type `T` which may or may not have been allocated yet.
/// Entities should use these to handle their parts.
///
/// Instead of being serialized and deserialized with the other entity data, it should be
/// skipped by marking its field with `#[serde(skip)]`. These fields will be filled with their
/// `Default` implementation, here it would be the `NotAllocatedYet` variant.
///
/// For this reason, entities should handle their part creations via
/// `PartHandler::ensure_is_allocated` at each physics step so that their rendering is ensured
/// (if the entities in question want these parts to be rendered at the moment),
/// no matter how they are loaded. It works quite well!
//
// TODO: The variant discriminant doubles the size of the type. Make it smaller.
#[derive(Clone, Default)]
pub(crate) enum PartHandler<T: PartKind> {
	#[default]
	NotAllocatedYet,
	Allocated {
		/// Index of the instance in the `instance_table` of the `PartTable<T>`.
		index: u32,
		/// Rust bullied me into putting that here >_<'
		/// The handler does not "own" a `T` with the Rust meaning of ownership.
		/// This should be harmless, but who knows, the documentation of `PhantomData` acts all
		/// mysterious about what this really does to the compilation (but it can have an influence,
		/// that was clear at least).
		/// TODO: Look into it? (very low priority)
		_marker: PhantomData<T::Instance>,
	},
}

impl<T: PartKind> PartHandler<T> {
	/// If the handler does not refer to an allocated part instance yet,
	/// then now it does and the newly allocated part instance is initialized by `initialize`.
	#[inline]
	pub(crate) fn ensure_is_allocated(
		&mut self,
		part_table: &mut PartTable<T>,
		initialize: impl FnOnce() -> T::Instance,
	) {
		if let PartHandler::NotAllocatedYet = self {
			let instance = initialize();
			let index = part_table.allocate_instance(instance) as u32;
			*self = PartHandler::Allocated { index, _marker: PhantomData }
		}
	}

	/// If the handler refer to an allocated part instance,
	/// then it is modified via `callback`.
	#[inline]
	pub(crate) fn modify_instance(
		&mut self,
		part_table: &mut PartTable<T>,
		callback: impl FnOnce(&mut T::Instance),
	) {
		if let PartHandler::Allocated { index, .. } = self {
			if let Some(instance) = part_table.instance_table.get_mut(*index as usize) {
				callback(instance);
				part_table.cpu_to_gpu_update_required_for_instances = true;
			} else {
				panic!("Bug: Out of bounds part instance index");
			}
		}
	}

	/// If the handler refered to an allocated instance,
	/// then the instance is released from its existence.
	pub(crate) fn delete(&self, part_table: &mut PartTable<T>) {
		if let PartHandler::Allocated { index, .. } = self {
			part_table.delete_instance(*index as usize);
		}
	}
}

/// The tables of the tables of the parts.
/// One part table holds a model mesh and all its instances.
/// All these tables are gathered in here.
pub(crate) struct PartTables {
	pub(crate) textured_cubes: Mutex<PartTable<TexturedCubePartKind>>,
	pub(crate) colored_cubes: Mutex<PartTable<ColoredCubePartKind>>,
	pub(crate) colored_icosahedron: Mutex<PartTable<ColoredIcosahedronPartKind>>,
	// NOTE: Added tables should also be handled in `PartTables::part_tables_for_rendering` and
	// in `PartTables::cup_to_gpu_update_if_required`.
}

/// Shared portion of the `PartTables` that is enough to render them.
pub(crate) struct PartTablesForRendering {
	pub(crate) textured: [DataForPartTableRendering; 1],
	pub(crate) colored: [DataForPartTableRendering; 2],
}

impl PartTables {
	pub(crate) fn new(device: &wgpu::Device) -> PartTables {
		PartTables {
			textured_cubes: Mutex::new(textured_cube::textured_cube_part_table(device)),
			colored_cubes: Mutex::new(colored_cube::colored_cube_part_table(device)),
			colored_icosahedron: Mutex::new(colored_icosahedron::colored_icosahedron_part_table(
				device,
			)),
		}
	}

	pub(crate) fn cup_to_gpu_update_if_required(
		&mut self,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
	) {
		self.textured_cubes.lock().unwrap().cup_to_gpu_update_if_required(device, queue);
		self.colored_cubes.lock().unwrap().cup_to_gpu_update_if_required(device, queue);
		self.colored_icosahedron.lock().unwrap().cup_to_gpu_update_if_required(device, queue);
	}

	/// Returns what is needed to render the part tables, in a shared way.
	pub(crate) fn part_tables_for_rendering(&self) -> PartTablesForRendering {
		PartTablesForRendering {
			textured: [self.textured_cubes.lock().unwrap().get_data_for_rendering()],
			colored: [
				self.colored_cubes.lock().unwrap().get_data_for_rendering(),
				self.colored_icosahedron.lock().unwrap().get_data_for_rendering(),
			],
		}
	}
}

/// Trait that declares that a type can be the raw data of a part instance.
/// There can be a `PartTable` of these.
pub(crate) trait PartInstance: bytemuck::Pod + bytemuck::Zeroable + Clone {
	/// Set the transform matrix of the instance.
	fn set_model_matrix(&mut self, model_matrix: &cgmath::Matrix4<f32>);
}

pub(crate) trait PartKind {
	type Instance: PartInstance;
}

/// A table of entity parts.
/// One such table holds a model (mesh) and all the instances of that model.
/// It also handles the syncing of this data with the GPU.
pub(crate) struct PartTable<T: PartKind> {
	/// The mesh of the model for this table of instances.
	mesh: Mesh,
	/// The CPU-side table of all the instances for the model of this table.
	/// As per `PartInstance`'s trait bounds requirements, all this data can be copied raw
	/// to the GPU.
	instance_table: Vec<T::Instance>,
	/// The GPU-side buffer that is given the data from `instance_table`.
	instance_table_buffer: Arc<wgpu::Buffer>,
	/// The allocator that manages the allocation and freeing of the instances.
	instance_table_allocator: TableAllocator,
	/// If an instance is modified, then the new data must be sent to the GPU.
	cpu_to_gpu_update_required_for_instances: bool,
	/// If the size of the instance table was modified, the buffer must be recreated to fit.
	cpu_to_gpu_update_required_for_buffer_length_change: bool,
	name: &'static str,
}

impl<T: PartKind> PartTable<T> {
	pub(crate) fn allocate_instance(&mut self, instance: T::Instance) -> usize {
		match self.instance_table_allocator.allocate_one() {
			AllocationDecision::AllocateIndex(index) => {
				self.instance_table[index] = instance;
				self.cpu_to_gpu_update_required_for_instances = true;
				index
			},
			AllocationDecision::NeedsBiggerBuffer => {
				let growing_factor = 1.25;
				let new_length = (self.instance_table.len() as f32 * growing_factor) as usize + 4;
				self.instance_table.resize(new_length, T::Instance::zeroed());
				self.instance_table_allocator.length_increased_to(new_length);
				let AllocationDecision::AllocateIndex(index) =
					self.instance_table_allocator.allocate_one()
				else {
					unreachable!("The length of the table increased by at least 4, there must be room");
				};
				self.instance_table[index] = instance;
				self.cpu_to_gpu_update_required_for_instances = true;
				self.cpu_to_gpu_update_required_for_buffer_length_change = true;
				index
				// TODO: This does not even require to leave safe Rust to make it faster.
				// Actually, what really needs manual resizing is the wgpu buffer, not the rust vec.
				// We could decide that the wgpu buffer has the size of the allocator and
				// let the vec manage its size independently. That would require to make the allocator
				// allocate at the beginning of the last interval instead of at the end of it (to
				// make it worth it).
			},
		}
	}

	pub(crate) fn delete_instance(&mut self, index: usize) {
		self.instance_table[index] = T::Instance::zeroed();
		self.cpu_to_gpu_update_required_for_instances = true;
		match self.instance_table_allocator.free_one(index) {
			FreeingAdvice::NothingToDo => {},
			FreeingAdvice::CanShortenToLengthOf(advised_new_smaller_length) => {
				self.instance_table.resize_with(advised_new_smaller_length, || {
					panic!("There should be no element creation, we only shrink")
				});
				self.cpu_to_gpu_update_required_for_buffer_length_change = true;
				self.instance_table_allocator.length_shriked_to(advised_new_smaller_length);
			},
		}
	}

	fn cup_to_gpu_update_if_required(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
		if self.cpu_to_gpu_update_required_for_buffer_length_change {
			// TODO: See the TODO at the end of `allocate_instance`.
			self.cpu_to_gpu_update_required_for_buffer_length_change = false;
			self.cpu_to_gpu_update_required_for_instances = false;
			let name = self.name;
			self.instance_table_buffer = Arc::new(device.create_buffer_init(
				&wgpu::util::BufferInitDescriptor {
					label: Some(&format!("{name} Instance Buffer")),
					contents: bytemuck::cast_slice(&self.instance_table),
					usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
				},
			));
		} else if self.cpu_to_gpu_update_required_for_instances {
			self.cpu_to_gpu_update_required_for_instances = false;
			queue.write_buffer(
				&self.instance_table_buffer,
				0,
				bytemuck::cast_slice(&self.instance_table),
			);
		}
	}

	fn get_data_for_rendering(&self) -> DataForPartTableRendering {
		DataForPartTableRendering {
			mesh_vertices_count: self.mesh.vertex_count,
			mesh_vertex_buffer: Arc::clone(&self.mesh.buffer),
			instances_count: self.instance_table.len() as u32,
			instance_buffer: Arc::clone(&self.instance_table_buffer),
		}
	}
}

/// Just what is needed to render the instances of a part table.
/// Note that this type is the same no matter the PartInstance type parameter
/// of the part table it comes from.
pub(crate) struct DataForPartTableRendering {
	pub(crate) mesh_vertices_count: u32,
	pub(crate) mesh_vertex_buffer: Arc<wgpu::Buffer>,
	pub(crate) instances_count: u32,
	pub(crate) instance_buffer: Arc<wgpu::Buffer>,
}

/// A mesh of a model. Its data is all on the GPU side.
struct Mesh {
	vertex_count: u32,
	buffer: Arc<wgpu::Buffer>,
}

/// The table in which are stored the texture mappings and the colorings of the instances.
/// A model for textured/colored entity parts is not textured/colored itself, instead
/// each instance of that model refers to its desired texture mappings/coloring in this table.
pub(crate) struct TextureMappingAndColoringTable {
	/// Maps each possible texturings/colorings to the offset (in `f32`s) of
	/// the texture mapping/coloring in the array.
	map_to_offset: FxHashMap<WhichTextureMappingOrColoring, u32>,
	/// Next offset in the Wgpu buffer, in bytes.
	next_offset_in_buffer_in_bytes: u32,
	/// Next offset in `f32`s to be given to instances.
	next_offset: u32,
}

pub(crate) struct TextureMappingAndColoringTableRwLock(
	pub(crate) RwLock<TextureMappingAndColoringTable>,
);

#[derive(Hash, PartialEq, Eq)]
enum WhichTextureMappingOrColoring {
	BlockTextureMapping(BlockTypeId),
	CubeColoringUni([u8; 3]),
	IcosahedronColoring(WhichIcosahedronColoring),
}

#[derive(Hash, PartialEq, Eq)]
pub(crate) enum WhichIcosahedronColoring {
	Test,
}

/// An offset that points to some texture mappings made for a cube model.
#[derive(Clone, Copy)]
pub(crate) struct CubeTextureMappingOffset(u32);
/// An offset that points to some coloring made for a cube model.
#[derive(Clone, Copy)]
pub(crate) struct CubeColoringOffset(u32);
/// An offset that points to some coloring made for an icosahedron model.
#[derive(Clone, Copy)]
pub(crate) struct IcosahedrongColoringOffset(u32);

impl TextureMappingAndColoringTable {
	pub(crate) fn new() -> TextureMappingAndColoringTable {
		TextureMappingAndColoringTable {
			map_to_offset: HashMap::default(),
			next_offset_in_buffer_in_bytes: 0,
			next_offset: 0,
		}
	}
}

impl TextureMappingAndColoringTableRwLock {
	/// Get an offset in the array of texture mappings, specifically for a textured cube part,
	/// and with the textures of a block. The resulting offset may be given to an instance of
	/// the textured cube model. If the requested texture mapping is not in the table, it is added.
	/// Returns `None` if given a `block_type_id` that does not correspond to a solid block.
	pub(crate) fn get_offset_of_block(
		&self,
		block_type_id: BlockTypeId,
		block_type_table: &Arc<BlockTypeTable>,
		texturing_and_coloring_array_thingy: &BindingThingy<wgpu::Buffer>,
		queue: &wgpu::Queue,
	) -> Option<CubeTextureMappingOffset> {
		let which_mapping = WhichTextureMappingOrColoring::BlockTextureMapping(block_type_id);
		{
			let read = self.0.read().unwrap();
			let offset = read.map_to_offset.get(&which_mapping);
			if let Some(offset) = offset {
				return Some(CubeTextureMappingOffset(*offset));
			}
		}
		// Not found, we have to write it in.
		let texture_coords_on_atlas = match block_type_table.get(block_type_id)? {
			BlockType::Solid { texture_coords_on_atlas } => *texture_coords_on_atlas,
			_ => return None,
		};
		let mappings = textured_cube::texture_mappings_for_cube(texture_coords_on_atlas);
		let data = bytemuck::cast_slice(&mappings);
		let (data_offset, offset) = {
			let mut write = self.0.write().unwrap();
			let data_offset = write.next_offset_in_buffer_in_bytes;
			write.next_offset_in_buffer_in_bytes += data.len() as u32;
			let offset = write.next_offset;
			let length_of_the_mapping_for_one_vertex = 2;
			write.next_offset += mappings.len() as u32 * length_of_the_mapping_for_one_vertex;
			write.map_to_offset.insert(which_mapping, offset);
			(data_offset, offset)
		};
		queue.write_buffer(
			&texturing_and_coloring_array_thingy.resource,
			data_offset as u64,
			data,
		);
		Some(CubeTextureMappingOffset(offset))
	}

	/// Get an offset in the array of colorings, specifically for a colored cube part.
	/// The resulting offset may be given to an instance of the colored cube model.
	/// If the requested coloring is not in the table, it is added.
	pub(crate) fn get_offset_of_cube_coloring_uni(
		&self,
		color: [u8; 3],
		texturing_and_coloring_array_thingy: &BindingThingy<wgpu::Buffer>,
		queue: &wgpu::Queue,
	) -> CubeColoringOffset {
		let which_mapping = WhichTextureMappingOrColoring::CubeColoringUni(color);
		{
			let read = self.0.read().unwrap();
			let offset = read.map_to_offset.get(&which_mapping);
			if let Some(offset) = offset {
				return CubeColoringOffset(*offset);
			}
		}
		// Not found, we have to write it in.
		let coloring = colored_cube::unicolor_for_cube(cgmath::vec3(
			color[0] as f32 / 255.0,
			color[1] as f32 / 255.0,
			color[2] as f32 / 255.0,
		));
		let data = bytemuck::cast_slice(&coloring);
		let (data_offset, offset) = {
			let mut write = self.0.write().unwrap();
			let data_offset = write.next_offset_in_buffer_in_bytes;
			write.next_offset_in_buffer_in_bytes += data.len() as u32;
			let offset = write.next_offset;
			let length_of_the_coloring_for_one_vertex = 3;
			write.next_offset += coloring.len() as u32 * length_of_the_coloring_for_one_vertex;
			write.map_to_offset.insert(which_mapping, offset);
			(data_offset, offset)
		};
		queue.write_buffer(
			&texturing_and_coloring_array_thingy.resource,
			data_offset as u64,
			data,
		);
		CubeColoringOffset(offset)
	}

	/// Get an offset in the array of colorings, specifically for a colored icosahedron part.
	/// The resulting offset may be given to an instance of the colored icosahedron model.
	/// If the requested coloring is not in the table, it is added.
	pub(crate) fn get_offset_of_icosahedron_coloring(
		&self,
		which_coloring: WhichIcosahedronColoring,
		texturing_and_coloring_array_thingy: &BindingThingy<wgpu::Buffer>,
		queue: &wgpu::Queue,
	) -> IcosahedrongColoringOffset {
		let which_mapping = WhichTextureMappingOrColoring::IcosahedronColoring(which_coloring);
		{
			let read = self.0.read().unwrap();
			let offset = read.map_to_offset.get(&which_mapping);
			if let Some(offset) = offset {
				return IcosahedrongColoringOffset(*offset);
			}
		}
		// Not found, we have to write it in.
		let coloring = colored_icosahedron::coloring_for_icosahedron();
		let data = bytemuck::cast_slice(&coloring);
		let (data_offset, offset) = {
			let mut write = self.0.write().unwrap();
			let data_offset = write.next_offset_in_buffer_in_bytes;
			write.next_offset_in_buffer_in_bytes += data.len() as u32;
			let offset = write.next_offset;
			let length_of_the_coloring_for_one_vertex = 3;
			write.next_offset += coloring.len() as u32 * length_of_the_coloring_for_one_vertex;
			write.map_to_offset.insert(which_mapping, offset);
			(data_offset, offset)
		};
		queue.write_buffer(
			&texturing_and_coloring_array_thingy.resource,
			data_offset as u64,
			data,
		);
		IcosahedrongColoringOffset(offset)
	}
}

impl PartInstance for PartTexturedInstancePod {
	fn set_model_matrix(&mut self, model_matrix: &cgmath::Matrix4<f32>) {
		let inv_trans_model_matrix =
			cgmath::conv::array4x4(model_matrix.transpose().invert().unwrap());
		let model_matrix = cgmath::conv::array4x4(*model_matrix);
		self.model_matrix_1_of_4 = model_matrix[0];
		self.model_matrix_2_of_4 = model_matrix[1];
		self.model_matrix_3_of_4 = model_matrix[2];
		self.model_matrix_4_of_4 = model_matrix[3];
		self.inv_trans_model_matrix_1_of_4 = inv_trans_model_matrix[0];
		self.inv_trans_model_matrix_2_of_4 = inv_trans_model_matrix[1];
		self.inv_trans_model_matrix_3_of_4 = inv_trans_model_matrix[2];
		self.inv_trans_model_matrix_4_of_4 = inv_trans_model_matrix[3];
	}
}

impl PartInstance for PartColoredInstancePod {
	fn set_model_matrix(&mut self, model_matrix: &cgmath::Matrix4<f32>) {
		let inv_trans_model_matrix =
			cgmath::conv::array4x4(model_matrix.transpose().invert().unwrap());
		let model_matrix = cgmath::conv::array4x4(*model_matrix);
		self.model_matrix_1_of_4 = model_matrix[0];
		self.model_matrix_2_of_4 = model_matrix[1];
		self.model_matrix_3_of_4 = model_matrix[2];
		self.model_matrix_4_of_4 = model_matrix[3];
		self.inv_trans_model_matrix_1_of_4 = inv_trans_model_matrix[0];
		self.inv_trans_model_matrix_2_of_4 = inv_trans_model_matrix[1];
		self.inv_trans_model_matrix_3_of_4 = inv_trans_model_matrix[2];
		self.inv_trans_model_matrix_4_of_4 = inv_trans_model_matrix[3];
	}
}

pub(crate) mod textured_cube {
	//! Here are hanled the matters specific to the
	//! textured cube entity parts and their `PartTable`.

	use cgmath::{Matrix, SquareMatrix};

	use crate::shaders::{part_textured::PartVertexPod, Vector2Pod};

	use super::*;

	#[derive(Clone)]
	pub(crate) struct TexturedCubePartKind;
	impl PartKind for TexturedCubePartKind {
		type Instance = PartTexturedInstancePod;
	}

	pub(super) fn textured_cube_part_table(
		device: &wgpu::Device,
	) -> PartTable<TexturedCubePartKind> {
		let name = "Textured Cube Part";
		PartTable {
			mesh: cube_mesh(device, name),
			instance_table: vec![],
			instance_table_buffer: Arc::new(device.create_buffer_init(
				&wgpu::util::BufferInitDescriptor {
					label: Some(&format!("{name} Instance Buffer")),
					contents: &[],
					usage: wgpu::BufferUsages::VERTEX,
				},
			)),
			instance_table_allocator: TableAllocator::new(0, 200),
			cpu_to_gpu_update_required_for_instances: false,
			cpu_to_gpu_update_required_for_buffer_length_change: false,
			name,
		}
	}

	/// A nicer form of an instance that is yet to be converted into its raw counterpart (the raw
	/// counterpart will be the one that gets stored in a `PartTable`).
	pub(crate) struct PartTexturedCubeInstanceData {
		model_matrix: cgmath::Matrix4<f32>,
		texture_mapping_offset: u32,
	}

	impl PartTexturedCubeInstanceData {
		pub(crate) fn new(
			pos: cgmath::Point3<f32>,
			texture_mapping_offset: CubeTextureMappingOffset,
		) -> PartTexturedCubeInstanceData {
			let model_matrix = cgmath::Matrix4::<f32>::from_translation(pos.to_vec());
			PartTexturedCubeInstanceData {
				model_matrix,
				texture_mapping_offset: texture_mapping_offset.0,
			}
		}

		/// Converts into the form that can be stored in a `PartTable`.
		pub(crate) fn into_pod(self) -> PartTexturedInstancePod {
			let model_matrix = cgmath::conv::array4x4(self.model_matrix);
			let inv_trans_model_matrix =
				cgmath::conv::array4x4(self.model_matrix.transpose().invert().unwrap());
			PartTexturedInstancePod {
				model_matrix_1_of_4: model_matrix[0],
				model_matrix_2_of_4: model_matrix[1],
				model_matrix_3_of_4: model_matrix[2],
				model_matrix_4_of_4: model_matrix[3],
				inv_trans_model_matrix_1_of_4: inv_trans_model_matrix[0],
				inv_trans_model_matrix_2_of_4: inv_trans_model_matrix[1],
				inv_trans_model_matrix_3_of_4: inv_trans_model_matrix[2],
				inv_trans_model_matrix_4_of_4: inv_trans_model_matrix[3],
				texture_mapping_offset: self.texture_mapping_offset,
			}
		}
	}

	/// Creates the mesh of the cube model.
	fn cube_mesh(device: &wgpu::Device, name: &str) -> Mesh {
		// There is a lot of code duplicated from `chunk_meshing::generate_block_face_mesh`.
		// TODO: Factorize some code with there.

		let mut vertices: Vec<PartVertexPod> = vec![];

		let cube_center = cgmath::point3(0.0, 0.0, 0.0);
		for direction in OrientedAxis::all_the_six_possible_directions() {
			let normal: [f32; 3] = cgmath::conv::array3(direction.delta().map(|x| x as f32));

			let face_center = cube_center + direction.delta().map(|x| x as f32) * 0.5;

			let [other_axis_a, other_axis_b] = direction.axis.the_other_two_axes();

			let mut coords_array = [face_center; 4];

			coords_array[0][other_axis_a.index()] -= 0.5;
			coords_array[0][other_axis_b.index()] -= 0.5;
			coords_array[1][other_axis_a.index()] -= 0.5;
			coords_array[1][other_axis_b.index()] += 0.5;
			coords_array[2][other_axis_a.index()] += 0.5;
			coords_array[2][other_axis_b.index()] -= 0.5;
			coords_array[3][other_axis_a.index()] += 0.5;
			coords_array[3][other_axis_b.index()] += 0.5;

			let indices = [1, 0, 3, 3, 0, 2];

			// Adjusting the order of the vertices to makeup for face culling.
			let reverse_order = match direction.axis {
				NonOrientedAxis::X => direction.orientation == AxisOrientation::Negativewards,
				NonOrientedAxis::Y => direction.orientation == AxisOrientation::Positivewards,
				NonOrientedAxis::Z => direction.orientation == AxisOrientation::Negativewards,
			};
			let indices_indices_normal = [0, 1, 2, 3, 4, 5];
			let indices_indices_reversed = [0, 2, 1, 3, 5, 4];
			let mut handle_index = |index: usize| {
				vertices.push(PartVertexPod { position: coords_array[index].into(), normal });
			};
			if !reverse_order {
				for indices_index in indices_indices_normal {
					handle_index(indices[indices_index]);
				}
			} else {
				for indices_index in indices_indices_reversed {
					handle_index(indices[indices_index]);
				}
			}
		}

		let buffer = Arc::new(
			device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some(&format!("{name} Vertex Buffer")),
				contents: bytemuck::cast_slice(&vertices),
				usage: wgpu::BufferUsages::VERTEX,
			}),
		);

		Mesh { vertex_count: vertices.len() as u32, buffer }
	}

	/// Creates the texture mappings (to apply to the cube mesh)
	/// with the given texture rect in the atlas.
	pub(crate) fn texture_mappings_for_cube(
		texture_coords_on_atlas: cgmath::Point2<i32>,
	) -> Vec<Vector2Pod> {
		// There is a lot of code duplicated from `chunk_meshing::generate_block_face_mesh`.
		// TODO: Factorize some code with there.

		let mut mappings_coords_on_atlas: Vec<Vector2Pod> = vec![];

		for direction in OrientedAxis::all_the_six_possible_directions() {
			let texture_rect_in_atlas_xy: cgmath::Point2<f32> =
				texture_coords_on_atlas.map(|x| x as f32) * (1.0 / 512.0);
			let texture_rect_in_atlas_wh: cgmath::Vector2<f32> =
				cgmath::vec2(16.0, 16.0) * (1.0 / 512.0);
			let mut coords_in_atlas_array: [cgmath::Point2<f32>; 4] = [
				texture_rect_in_atlas_xy,
				texture_rect_in_atlas_xy,
				texture_rect_in_atlas_xy,
				texture_rect_in_atlas_xy,
			];
			// We flip horizontally the texture for some face orientations so that
			// we don't observe a "mirror" effect on some vertical block edges.
			let order = if direction
				== (OrientedAxis {
					axis: NonOrientedAxis::X,
					orientation: AxisOrientation::Positivewards,
				}) || direction
				== (OrientedAxis {
					axis: NonOrientedAxis::Y,
					orientation: AxisOrientation::Negativewards,
				}) {
				[2, 3, 0, 1]
			} else {
				[0, 1, 2, 3]
			};
			coords_in_atlas_array[order[0]].x += texture_rect_in_atlas_wh.x * 0.0;
			coords_in_atlas_array[order[0]].y += texture_rect_in_atlas_wh.y * 0.0;
			coords_in_atlas_array[order[1]].x += texture_rect_in_atlas_wh.x * 0.0;
			coords_in_atlas_array[order[1]].y += texture_rect_in_atlas_wh.y * 1.0;
			coords_in_atlas_array[order[2]].x += texture_rect_in_atlas_wh.x * 1.0;
			coords_in_atlas_array[order[2]].y += texture_rect_in_atlas_wh.y * 0.0;
			coords_in_atlas_array[order[3]].x += texture_rect_in_atlas_wh.x * 1.0;
			coords_in_atlas_array[order[3]].y += texture_rect_in_atlas_wh.y * 1.0;

			let indices = [1, 0, 3, 3, 0, 2];

			let reverse_order = match direction.axis {
				NonOrientedAxis::X => direction.orientation == AxisOrientation::Negativewards,
				NonOrientedAxis::Y => direction.orientation == AxisOrientation::Positivewards,
				NonOrientedAxis::Z => direction.orientation == AxisOrientation::Negativewards,
			};
			let indices_indices_normal = [0, 1, 2, 3, 4, 5];
			let indices_indices_reversed = [0, 2, 1, 3, 5, 4];
			let mut handle_index = |index: usize| {
				mappings_coords_on_atlas
					.push(Vector2Pod { values: coords_in_atlas_array[index].into() });
			};
			if !reverse_order {
				for indices_index in indices_indices_normal {
					handle_index(indices[indices_index]);
				}
			} else {
				for indices_index in indices_indices_reversed {
					handle_index(indices[indices_index]);
				}
			}
		}

		mappings_coords_on_atlas
	}
}

pub(crate) mod colored_cube {
	//! Here are hanled the matters specific to the
	//! colored cube entity parts and their `PartTable`.

	use cgmath::{Matrix, SquareMatrix};

	use crate::shaders::{part_textured::PartVertexPod, Vector3Pod};

	use super::*;

	#[derive(Clone)]
	pub(crate) struct ColoredCubePartKind;
	impl PartKind for ColoredCubePartKind {
		type Instance = PartColoredInstancePod;
	}

	pub(super) fn colored_cube_part_table(device: &wgpu::Device) -> PartTable<ColoredCubePartKind> {
		let name = "Colored Cube Part";
		PartTable {
			mesh: cube_mesh(device, name),
			instance_table: vec![],
			instance_table_buffer: Arc::new(device.create_buffer_init(
				&wgpu::util::BufferInitDescriptor {
					label: Some(&format!("{name} Instance Buffer")),
					contents: &[],
					usage: wgpu::BufferUsages::VERTEX,
				},
			)),
			instance_table_allocator: TableAllocator::new(0, 200),
			cpu_to_gpu_update_required_for_instances: false,
			cpu_to_gpu_update_required_for_buffer_length_change: false,
			name,
		}
	}

	/// A nicer form of an instance that is yet to be converted into its raw counterpart (the raw
	/// counterpart will be the one that gets stored in a `PartTable`).
	pub(crate) struct PartColoredCubeInstanceData {
		model_matrix: cgmath::Matrix4<f32>,
		coloring_offset: u32,
	}

	impl PartColoredCubeInstanceData {
		pub(crate) fn new(
			pos: cgmath::Point3<f32>,
			coloring_offset: CubeColoringOffset,
		) -> PartColoredCubeInstanceData {
			let model_matrix = cgmath::Matrix4::<f32>::from_translation(pos.to_vec());
			PartColoredCubeInstanceData { model_matrix, coloring_offset: coloring_offset.0 }
		}

		/// Converts into the form that can be stored in a `PartTable`.
		pub(crate) fn into_pod(self) -> PartColoredInstancePod {
			let model_matrix = cgmath::conv::array4x4(self.model_matrix);
			let inv_trans_model_matrix =
				cgmath::conv::array4x4(self.model_matrix.transpose().invert().unwrap());
			PartColoredInstancePod {
				model_matrix_1_of_4: model_matrix[0],
				model_matrix_2_of_4: model_matrix[1],
				model_matrix_3_of_4: model_matrix[2],
				model_matrix_4_of_4: model_matrix[3],
				inv_trans_model_matrix_1_of_4: inv_trans_model_matrix[0],
				inv_trans_model_matrix_2_of_4: inv_trans_model_matrix[1],
				inv_trans_model_matrix_3_of_4: inv_trans_model_matrix[2],
				inv_trans_model_matrix_4_of_4: inv_trans_model_matrix[3],
				coloring_offset: self.coloring_offset,
			}
		}
	}

	// TODO: Recycle the textured cube mesh, these are exactly the same mesh, they could be the same
	// Wgpu buffer that it would still work just fine.
	//
	/// Creates the mesh of the cube model.
	fn cube_mesh(device: &wgpu::Device, name: &str) -> Mesh {
		// There is a lot of code duplicated from `chunk_meshing::generate_block_face_mesh`.
		// TODO: Factorize some code with there.

		let mut vertices: Vec<PartVertexPod> = vec![];

		let cube_center = cgmath::point3(0.0, 0.0, 0.0);
		for direction in OrientedAxis::all_the_six_possible_directions() {
			let normal: [f32; 3] = cgmath::conv::array3(direction.delta().map(|x| x as f32));

			let face_center = cube_center + direction.delta().map(|x| x as f32) * 0.5;

			let [other_axis_a, other_axis_b] = direction.axis.the_other_two_axes();

			let mut coords_array = [face_center; 4];

			coords_array[0][other_axis_a.index()] -= 0.5;
			coords_array[0][other_axis_b.index()] -= 0.5;
			coords_array[1][other_axis_a.index()] -= 0.5;
			coords_array[1][other_axis_b.index()] += 0.5;
			coords_array[2][other_axis_a.index()] += 0.5;
			coords_array[2][other_axis_b.index()] -= 0.5;
			coords_array[3][other_axis_a.index()] += 0.5;
			coords_array[3][other_axis_b.index()] += 0.5;

			let indices = [1, 0, 3, 3, 0, 2];

			// Adjusting the order of the vertices to makeup for face culling.
			let reverse_order = match direction.axis {
				NonOrientedAxis::X => direction.orientation == AxisOrientation::Negativewards,
				NonOrientedAxis::Y => direction.orientation == AxisOrientation::Positivewards,
				NonOrientedAxis::Z => direction.orientation == AxisOrientation::Negativewards,
			};
			let indices_indices_normal = [0, 1, 2, 3, 4, 5];
			let indices_indices_reversed = [0, 2, 1, 3, 5, 4];
			let mut handle_index = |index: usize| {
				vertices.push(PartVertexPod { position: coords_array[index].into(), normal });
			};
			if !reverse_order {
				for indices_index in indices_indices_normal {
					handle_index(indices[indices_index]);
				}
			} else {
				for indices_index in indices_indices_reversed {
					handle_index(indices[indices_index]);
				}
			}
		}

		let buffer = Arc::new(
			device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some(&format!("{name} Vertex Buffer")),
				contents: bytemuck::cast_slice(&vertices),
				usage: wgpu::BufferUsages::VERTEX,
			}),
		);

		Mesh { vertex_count: vertices.len() as u32, buffer }
	}

	/// Creates a coloring (to apply to the cube mesh)
	/// with the given color applied to all the vertices, the coube will have that one single color.
	pub(crate) fn unicolor_for_cube(color: cgmath::Vector3<f32>) -> Vec<Vector3Pod> {
		let mut colors: Vec<Vector3Pod> = vec![];

		let number_of_faces = 6;
		let number_of_triangles = 2 * number_of_faces;
		let number_of_vertices = 3 * number_of_triangles;
		for _i in 0..number_of_vertices {
			colors.push(Vector3Pod { values: color.into() });
		}

		colors
	}
}

pub(crate) mod colored_icosahedron {
	//! Here are hanled the matters specific to the
	//! colored icosahedron entity parts and their `PartTable`.

	use cgmath::{InnerSpace, Matrix, SquareMatrix};

	use crate::shaders::{
		part_colored::PartColoredInstancePod, part_textured::PartVertexPod, Vector3Pod,
	};

	use super::*;

	#[derive(Clone)]
	pub(crate) struct ColoredIcosahedronPartKind;
	impl PartKind for ColoredIcosahedronPartKind {
		type Instance = PartColoredInstancePod;
	}

	pub(super) fn colored_icosahedron_part_table(
		device: &wgpu::Device,
	) -> PartTable<ColoredIcosahedronPartKind> {
		let name = "Colored Icosahedron Part";
		PartTable {
			mesh: icosahedron_mesh(device, name),
			instance_table: vec![],
			instance_table_buffer: Arc::new(device.create_buffer_init(
				&wgpu::util::BufferInitDescriptor {
					label: Some(&format!("{name} Instance Buffer")),
					contents: &[],
					usage: wgpu::BufferUsages::VERTEX,
				},
			)),
			instance_table_allocator: TableAllocator::new(0, 200),
			cpu_to_gpu_update_required_for_instances: false,
			cpu_to_gpu_update_required_for_buffer_length_change: false,
			name,
		}
	}

	/// A nicer form of an instance that is yet to be converted into its raw counterpart (the raw
	/// counterpart will be the one that gets stored in a `PartTable`).
	pub(crate) struct PartColoredIcosahedronInstanceData {
		model_matrix: cgmath::Matrix4<f32>,
		coloring_offset: u32,
	}

	impl PartColoredIcosahedronInstanceData {
		pub(crate) fn new(
			pos: cgmath::Point3<f32>,
			texture_mapping_offset: IcosahedrongColoringOffset,
		) -> PartColoredIcosahedronInstanceData {
			let model_matrix = cgmath::Matrix4::<f32>::from_translation(pos.to_vec());
			PartColoredIcosahedronInstanceData {
				model_matrix,
				coloring_offset: texture_mapping_offset.0,
			}
		}

		/// Converts into the form that can be stored in a `PartTable`.
		pub(crate) fn into_pod(self) -> PartColoredInstancePod {
			let model_matrix = cgmath::conv::array4x4(self.model_matrix);
			let inv_trans_model_matrix =
				cgmath::conv::array4x4(self.model_matrix.transpose().invert().unwrap());
			PartColoredInstancePod {
				model_matrix_1_of_4: model_matrix[0],
				model_matrix_2_of_4: model_matrix[1],
				model_matrix_3_of_4: model_matrix[2],
				model_matrix_4_of_4: model_matrix[3],
				inv_trans_model_matrix_1_of_4: inv_trans_model_matrix[0],
				inv_trans_model_matrix_2_of_4: inv_trans_model_matrix[1],
				inv_trans_model_matrix_3_of_4: inv_trans_model_matrix[2],
				inv_trans_model_matrix_4_of_4: inv_trans_model_matrix[3],
				coloring_offset: self.coloring_offset,
			}
		}
	}

	// Some resources:
	// https://schneide.blog/2016/07/15/generating-an-icosphere-in-c/
	// https://web.archive.org/web/20180808214504/http://donhavey.com:80/blog/tutorials/tutorial-3-the-icosahedron-sphere/
	const GOLD: f32 = 1.618034; // Golden ratio.
	const VERTICES_FOR_REF: [cgmath::Point3<f32>; 12] = [
		cgmath::point3(-1.0, 0.0, GOLD),
		cgmath::point3(1.0, 0.0, GOLD),
		cgmath::point3(-1.0, 0.0, -GOLD),
		cgmath::point3(1.0, 0.0, -GOLD),
		cgmath::point3(0.0, GOLD, 1.0),
		cgmath::point3(0.0, GOLD, -1.0),
		cgmath::point3(0.0, -GOLD, 1.0),
		cgmath::point3(0.0, -GOLD, -1.0),
		cgmath::point3(GOLD, 1.0, 0.0),
		cgmath::point3(-GOLD, 1.0, 0.0),
		cgmath::point3(GOLD, -1.0, 0.0),
		cgmath::point3(-GOLD, -1.0, 0.0),
	];
	const TRIANGLES_INDICES_IN_REFS: [[usize; 3]; 20] = [
		[1, 4, 0],
		[4, 9, 0],
		[4, 5, 9],
		[8, 5, 4],
		[1, 8, 4],
		[1, 10, 8],
		[10, 3, 8],
		[8, 3, 5],
		[3, 2, 5],
		[3, 7, 2],
		[3, 10, 7],
		[10, 6, 7],
		[6, 11, 7],
		[6, 0, 11],
		[6, 1, 0],
		[10, 1, 6],
		[11, 0, 9],
		[2, 11, 9],
		[5, 2, 9],
		[11, 2, 7],
	];

	struct Face([cgmath::Point3<f32>; 3]);

	impl Face {
		fn normal(&self) -> cgmath::Vector3<f32> {
			// We choose to have one normal per face instead of interpolated per-vertex normals,
			// because we embrace the low poly visual style (by artistic choice).
			let mut normal = cgmath::vec3(0.0, 0.0, 0.0);
			for position in self.0.iter().copied() {
				normal += position.to_vec();
			}
			normal.normalize()
		}

		fn subdivide(&self) -> [Face; 4] {
			// A d B
			//  f e
			//   C
			let a = self.0[0];
			let b = self.0[1];
			let c = self.0[2];
			let d = a.midpoint(b);
			let e = b.midpoint(c);
			let f = c.midpoint(a);
			[
				Face([a, d, f]),
				Face([d, b, e]),
				Face([d, e, f]),
				Face([f, e, c]),
			]
		}
	}

	fn faces() -> impl Iterator<Item = Face> {
		TRIANGLES_INDICES_IN_REFS
			.into_iter()
			.map(|triangle_indices_in_refs| {
				Face(triangle_indices_in_refs.map(|index_in_refs| VERTICES_FOR_REF[index_in_refs]))
			})
			.flat_map(|face| face.subdivide())
			.map(|face| {
				let mut positions = face
					.0
					.into_iter()
					.map(|position| cgmath::Point3::from_vec(position.to_vec().normalize() / 2.0));
				Face([
					positions.next().unwrap(),
					positions.next().unwrap(),
					positions.next().unwrap(),
				])
			})
	}

	/// Creates the mesh of the icosahedron model.
	fn icosahedron_mesh(device: &wgpu::Device, name: &str) -> Mesh {
		let mut vertices: Vec<PartVertexPod> = vec![];
		for face in faces() {
			let normal = face.normal();
			for position in face.0.into_iter() {
				vertices.push(PartVertexPod { position: position.into(), normal: normal.into() });
			}
		}
		let buffer = Arc::new(
			device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some(&format!("{name} Vertex Buffer")),
				contents: bytemuck::cast_slice(&vertices),
				usage: wgpu::BufferUsages::VERTEX,
			}),
		);
		Mesh { vertex_count: vertices.len() as u32, buffer }
	}

	/// Creates coloring (to apply to the icosahedron mesh).
	pub(crate) fn coloring_for_icosahedron() -> Vec<Vector3Pod> {
		let mut colors: Vec<Vector3Pod> = vec![];
		for face in faces() {
			for position in face.0 {
				let mut color = cgmath::conv::array3(position + cgmath::vec3(0.5, 0.5, 0.5));
				if color.iter().sum::<f32>() < 1.5 {
					for component in color.iter_mut() {
						*component = 1.0 - *component;
					}
				}
				colors.push(Vector3Pod { values: color });
			}
		}
		colors
	}
}
