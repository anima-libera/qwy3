struct PartTableTable {
	cubes: PartTypedTable<PartTexturedCubeInstanceData>,
}

struct PartTypedTable<T> {
	mesh_vertex_count: u32,
	mesh: wgpu::Buffer,
	instance_table: Vec<T>,
	instance_table_buffer: wgpu::Buffer,
}

struct PartTexturedCubeInstanceData {
	transformation_matrix: [f32; 16],
	/// 2D points in the atlas, one for each of the 4 vertices of a square face, times 6 faces.
	texture_uv_mappings: [f32; 48],
}
