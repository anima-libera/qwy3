struct VertexInput {
	@location(0) position: vec3<f32>,
	// 3D direction vector pointing from the origin to a point on the cubemap.
	// This is how `Cube`-dimensional cubemap textures are sampled.
	// See https://www.w3.org/TR/WGSL/#texture-dimensionality and related notions for more.
	@location(1) coords_in_skybox_cubemap: vec3<f32>,
};

struct VertexOutput {
	@builtin(position) screen_position: vec4<f32>,
	@location(0) coords_in_skybox_cubemap: vec3<f32>,
};

@group(0) @binding(0) var<uniform> uniform_camera: mat4x4<f32>;
@group(0) @binding(1) var uniform_skybox_cubemap_texture: texture_cube<f32>;
@group(0) @binding(2) var uniform_skybox_cubemap_sampler: sampler;

@vertex
fn vertex_shader_main(vertex_input: VertexInput) -> VertexOutput {
	var vertex_output: VertexOutput;
	vertex_output.screen_position = uniform_camera * vec4<f32>(vertex_input.position, 1.0);
	vertex_output.coords_in_skybox_cubemap = vertex_input.coords_in_skybox_cubemap;
	return vertex_output;
}

@fragment
fn fragment_shader_main(the: VertexOutput) -> @location(0) vec4<f32> {
	var out_color = textureSample(
		uniform_skybox_cubemap_texture, uniform_skybox_cubemap_sampler, the.coords_in_skybox_cubemap);
	return out_color;
}
