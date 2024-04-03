struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) normal: vec3<f32>,
};

struct InstanceInput {
	@location(2) model_matrix_1_of_4: vec4<f32>,
	@location(3) model_matrix_2_of_4: vec4<f32>,
	@location(4) model_matrix_3_of_4: vec4<f32>,
	@location(5) model_matrix_4_of_4: vec4<f32>,
	@location(6) texture_mapping_point_offset: u32,
};

struct VertexOutput {
	@builtin(position) screen_position: vec4<f32>,
	@location(0) world_position: vec3<f32>,
	@location(1) coords_in_atlas: vec2<f32>,
};

@group(0) @binding(0) var<uniform> uniform_camera: mat4x4<f32>;
@group(0) @binding(1) var uniform_atlas_texture: texture_2d<f32>;
@group(0) @binding(2) var uniform_atlas_sampler: sampler;
@group(0) @binding(3) var<storage, read> uniform_coords_in_atlas_array: array<vec2<f32> >;
// TODO: Add the shadow map and fog uniforms.

@vertex
fn vertex_shader_main(
	@builtin(vertex_index) vertex_index: u32,
	vertex_input: VertexInput,
	instance_input: InstanceInput
) -> VertexOutput {
	var model_matrix = mat4x4(
		instance_input.model_matrix_1_of_4,
		instance_input.model_matrix_2_of_4,
		instance_input.model_matrix_3_of_4,
		instance_input.model_matrix_4_of_4,
	);

	var coords_in_atlas =
		uniform_coords_in_atlas_array[instance_input.texture_mapping_point_offset + vertex_index];

	var vertex_output: VertexOutput;
	vertex_output.screen_position =
		uniform_camera * model_matrix * vec4<f32>(vertex_input.position, 1.0);
	vertex_output.world_position = vertex_input.position;
	vertex_output.coords_in_atlas = coords_in_atlas;
	return vertex_output;
}

@fragment
fn fragment_shader_main(the: VertexOutput) -> @location(0) vec4<f32> {
	var out_color = textureSample(uniform_atlas_texture, uniform_atlas_sampler, the.coords_in_atlas);

	// Full transparency.
	if out_color.a == 0.0 {
		discard;
	}

	return out_color;
}
