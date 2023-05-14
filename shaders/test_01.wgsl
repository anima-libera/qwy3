struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) color: vec3<f32>,
};

struct VertexOutput {
	@builtin(position) clip_position: vec4<f32>,
	@location(0) color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uniform_camera: mat4x4<f32>;

@vertex
fn vs_main(vertex_input: VertexInput) -> VertexOutput {
	var vertex_output: VertexOutput;
	vertex_output.clip_position = uniform_camera * vec4<f32>(vertex_input.position, 1.0);
	vertex_output.color = vec4<f32>(vertex_input.color, 1.0);
	return vertex_output;
}

@fragment
fn fs_main(the: VertexOutput) -> @location(0) vec4<f32> {
	return the.color;
}
