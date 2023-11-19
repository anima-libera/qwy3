struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) coords_in_atlas: vec2<f32>,
	@location(2) color_factor: vec3<f32>,
};

struct VertexOutput {
	@builtin(position) screen_position: vec4<f32>,
	@location(0) coords_in_atlas: vec2<f32>,
	@location(1) color_factor: vec3<f32>,
};

@group(0) @binding(0) var<uniform> uniform_aspect_ratio: f32;
@group(0) @binding(1) var uniform_atlas_texture: texture_2d<f32>;
@group(0) @binding(2) var uniform_atlas_sampler: sampler;
@group(0) @binding(3) var<uniform> uniform_offset_for_2d: vec3<f32>;

@vertex
fn vertex_shader_main(vertex_input: VertexInput) -> VertexOutput {
	var vertex_output: VertexOutput;
	var position_xyz = vertex_input.position + uniform_offset_for_2d;
	vertex_output.screen_position = vec4<f32>(position_xyz, 1.0);
	vertex_output.screen_position.y *= uniform_aspect_ratio;
	vertex_output.coords_in_atlas = vertex_input.coords_in_atlas;
	vertex_output.color_factor = vertex_input.color_factor;
	return vertex_output;
}

@fragment
fn fragment_shader_main(the: VertexOutput) -> @location(0) vec4<f32> {
	var out_color = textureSample(uniform_atlas_texture, uniform_atlas_sampler, the.coords_in_atlas);

	// Full transparency.
	if out_color.a < 0.5 {
		discard;
	}

	out_color = vec4(out_color.rgb * the.color_factor, 1.0);

	return out_color;
}
