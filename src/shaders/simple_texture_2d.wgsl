struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) coords_in_atlas: vec2<f32>,
};

struct VertexOutput {
	@builtin(position) screen_position: vec4<f32>,
	@location(0) coords_in_atlas: vec2<f32>,
};

@group(0) @binding(0) var<uniform> uniform_aspect_ratio: f32;
@group(0) @binding(1) var uniform_atlas_texture: texture_2d<f32>;
@group(0) @binding(2) var uniform_atlas_sampler: sampler;

@vertex
fn vertex_shader_main(vertex_input: VertexInput) -> VertexOutput {
	var vertex_output: VertexOutput;
	vertex_output.screen_position = vec4<f32>(vertex_input.position, 1.0);
	vertex_output.screen_position.y *= uniform_aspect_ratio;
	vertex_output.coords_in_atlas = vertex_input.coords_in_atlas;
	return vertex_output;
}

@fragment
fn fragment_shader_main(the: VertexOutput) -> @location(0) vec4<f32> {
	var out_color = textureSample(uniform_atlas_texture, uniform_atlas_sampler, the.coords_in_atlas);

	// Full transparency.
	if out_color.a < 0.5 {
		discard;
	}

	return out_color;
}
