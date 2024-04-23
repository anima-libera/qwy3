struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) normal: vec3<f32>,
};

struct InstanceInput {
	@location(2) model_matrix_1_of_4: vec4<f32>,
	@location(3) model_matrix_2_of_4: vec4<f32>,
	@location(4) model_matrix_3_of_4: vec4<f32>,
	@location(5) model_matrix_4_of_4: vec4<f32>,
	@location(6) inv_trans_model_matrix_1_of_4: vec4<f32>,
	@location(7) inv_trans_model_matrix_2_of_4: vec4<f32>,
	@location(8) inv_trans_model_matrix_3_of_4: vec4<f32>,
	@location(9) inv_trans_model_matrix_4_of_4: vec4<f32>,
	@location(10) texture_mapping_offset: u32,
};

struct VertexOutput {
	@builtin(position) screen_position: vec4<f32>,
	@location(0) coords_in_atlas: vec2<f32>,
	@location(1) world_position: vec3<f32>,
};

@group(0) @binding(0) var<uniform> uniform_sun_camera: mat4x4<f32>;
@group(0) @binding(1) var uniform_atlas_texture: texture_2d<f32>;
@group(0) @binding(2) var uniform_atlas_sampler: sampler;
@group(0) @binding(3) var<uniform> uniform_fog_center_position: vec3<f32>;
@group(0) @binding(4) var<uniform> uniform_fog_inf_sup_radiuses: vec2<f32>;
@group(0) @binding(5) var<storage, read> uniform_texturing_and_coloring_array: array<f32>;

// TODO: There is a lot of code duplication between here and `block_shadow.wgsl`,
// we have to factorize!

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

	var texture_mapping_vertex_offset = instance_input.texture_mapping_offset + vertex_index * 2;
	var x_in_atlas = uniform_texturing_and_coloring_array[texture_mapping_vertex_offset + 0];
	var y_in_atlas = uniform_texturing_and_coloring_array[texture_mapping_vertex_offset + 1];
	var coords_in_atlas = vec2(x_in_atlas, y_in_atlas);

	var world_position = model_matrix * vec4<f32>(vertex_input.position, 1.0);

	var vertex_output: VertexOutput;
	vertex_output.screen_position = uniform_sun_camera * world_position;
	vertex_output.coords_in_atlas = coords_in_atlas;
	vertex_output.world_position = world_position.xyz;
	return vertex_output;
}

@fragment
fn fragment_shader_main(the: VertexOutput) {
	var color = textureSample(uniform_atlas_texture, uniform_atlas_sampler, the.coords_in_atlas);

	// Simple fog effect.
	// TODO: Make it so that fog transparency on shadow caster makes the shadow 'transparent'
	// so that the shadow opacity matches the caster's opacity. This may prove way more complicated
	// than just making changes in this shader.
	var distance_to_fog_center = distance(uniform_fog_center_position, the.world_position);
	var fog_inf_radius = uniform_fog_inf_sup_radiuses.x;
	var fog_sup_radius = uniform_fog_inf_sup_radiuses.y;
	var arbitrary_radius = (fog_inf_radius * 1.0 + fog_sup_radius * 5.0) / 6.0;
	if arbitrary_radius < distance_to_fog_center {
		color.a = 0.0;
	}

	// Full transparency.
	if color.a == 0.0 {
		discard;
	}
}
