struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) color: vec3<f32>,
	@location(2) normal: vec3<f32>,
};

struct VertexOutput {
	@builtin(position) screen_position: vec4<f32>,
	@location(0) color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uniform_camera: mat4x4<f32>;
@group(1) @binding(1) var<uniform> uniform_sun_light_direction: vec3<f32>;

@vertex
fn vertex_shader_main(vertex_input: VertexInput) -> VertexOutput {
	var vertex_output: VertexOutput;
	vertex_output.screen_position = uniform_camera * vec4<f32>(vertex_input.position, 1.0);
	var shade = dot(vertex_input.normal, uniform_sun_light_direction);
	shade = clamp(shade, 0.0, 1.0);
	vertex_output.color = vec4<f32>(vertex_input.color * shade, 1.0);
	return vertex_output;
}

@fragment
fn fragment_shader_main(the: VertexOutput) -> @location(0) vec4<f32> {
	return the.color;
}
