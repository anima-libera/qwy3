struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) coords_in_atlas: vec2<f32>,
	@location(2) normal: vec3<f32>,
	@location(3) ambiant_occlusion: f32,
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

@vertex
fn vertex_shader_main(vertex_input: VertexInput) -> VertexOutput {
	var vertex_output: VertexOutput;
	vertex_output.screen_position = uniform_sun_camera * vec4<f32>(vertex_input.position, 1.0);
	vertex_output.coords_in_atlas = vertex_input.coords_in_atlas;
	vertex_output.world_position = vertex_input.position;
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
	if color.a < 0.5 {
		discard;
	}
}
