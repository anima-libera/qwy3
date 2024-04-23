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
	@location(10) coloring_point_offset: u32,
};

struct VertexOutput {
	@builtin(position) screen_position: vec4<f32>,
	@location(0) world_position: vec3<f32>,
};

@group(0) @binding(0) var<uniform> uniform_sun_camera: mat4x4<f32>;
@group(0) @binding(1) var<uniform> uniform_fog_center_position: vec3<f32>;
@group(0) @binding(2) var<uniform> uniform_fog_inf_sup_radiuses: vec2<f32>;

// TODO: There is a lot of code duplication between here, `block_shadow.wgsl` and
// `part_textured_shadow.wgsl`, we have to factorize!

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

	var world_position = model_matrix * vec4<f32>(vertex_input.position, 1.0);

	var vertex_output: VertexOutput;
	vertex_output.screen_position = uniform_sun_camera * world_position;
	vertex_output.world_position = world_position.xyz;
	return vertex_output;
}

@fragment
fn fragment_shader_main(the: VertexOutput) {
	// Simple fog effect.
	// TODO: Make it so that fog transparency on shadow caster makes the shadow 'transparent'
	// so that the shadow opacity matches the caster's opacity. This may prove way more complicated
	// than just making changes in this shader.
	var distance_to_fog_center = distance(uniform_fog_center_position, the.world_position);
	var fog_inf_radius = uniform_fog_inf_sup_radiuses.x;
	var fog_sup_radius = uniform_fog_inf_sup_radiuses.y;
	var arbitrary_radius = (fog_inf_radius * 1.0 + fog_sup_radius * 5.0) / 6.0;
	if arbitrary_radius < distance_to_fog_center {
		discard;
	}
}
