struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) color: vec3<f32>,
	@location(2) normal: vec3<f32>,
	@location(3) ambiant_occlusion: f32,
};

struct VertexOutput {
	@builtin(position) screen_position: vec4<f32>,
	@location(0) color: vec4<f32>,
	@location(1) shade: f32,
	@location(2) ambiant_occlusion: f32,
	@location(3) world_position: vec3<f32>,
};

@group(0) @binding(0) var<uniform> uniform_camera: mat4x4<f32>;
@group(1) @binding(0) var<uniform> uniform_sun_light_direction: vec3<f32>;
@group(2) @binding(0) var<uniform> uniform_sun_camera: mat4x4<f32>;
@group(3) @binding(0) var uniform_shadow_map_texture: texture_depth_2d;
@group(3) @binding(1) var uniform_shadow_map_sampler: sampler_comparison;

@vertex
fn vertex_shader_main(vertex_input: VertexInput) -> VertexOutput {
	var vertex_output: VertexOutput;
	vertex_output.screen_position = uniform_camera * vec4<f32>(vertex_input.position, 1.0);
	var shade = dot(vertex_input.normal, -uniform_sun_light_direction);
	shade = clamp(shade, 0.0, 1.0);
	vertex_output.color = vec4<f32>(vertex_input.color, 1.0);
	vertex_output.shade = shade;
	vertex_output.ambiant_occlusion = vertex_input.ambiant_occlusion;
	vertex_output.world_position = vertex_input.position;
	return vertex_output;
}

@fragment
fn fragment_shader_main(the: VertexOutput) -> @location(0) vec4<f32> {
	var position_in_sun_screen = uniform_sun_camera * vec4<f32>(the.world_position, 1.0);
	// Stealing some stuff from
	// https://github.com/gfx-rs/wgpu/blob/trunk/examples/shadow/src/shader.wgsl
	var position_in_shadow_map =
		position_in_sun_screen.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
	var not_in_shadow = textureSampleCompare(
		uniform_shadow_map_texture, uniform_shadow_map_sampler,
		position_in_shadow_map, position_in_sun_screen.z);
	if position_in_shadow_map.x < 0.0 || 1.0 < position_in_shadow_map.x || position_in_shadow_map.y < 0.0 || 1.0 < position_in_shadow_map.y {
		not_in_shadow = 1.0;
	}
	var shade = the.shade * not_in_shadow;
	var out_color_rgb = the.color.rgb;
	out_color_rgb *= shade * 0.5 + 0.5;
	out_color_rgb *= the.ambiant_occlusion * 0.5 + 0.5;
	return vec4<f32>(out_color_rgb, the.color.a);
}
