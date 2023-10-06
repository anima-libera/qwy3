struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) coords_in_atlas: vec2<f32>,
	@location(2) normal: vec3<f32>,
	@location(3) ambiant_occlusion: f32,
};

struct VertexOutput {
	@builtin(position) screen_position: vec4<f32>,
	@location(0) coords_in_atlas: vec2<f32>,
	@location(1) shade: f32,
	@location(2) ambiant_occlusion: f32,
	@location(3) world_position: vec3<f32>,
};

@group(0) @binding(0) var<uniform> uniform_camera: mat4x4<f32>;
@group(1) @binding(0) var<uniform> uniform_sun_light_direction: vec3<f32>;
@group(2) @binding(0) var<uniform> uniform_sun_camera: mat4x4<f32>;
@group(3) @binding(0) var uniform_shadow_map_texture: texture_depth_2d;
@group(3) @binding(1) var uniform_shadow_map_sampler: sampler_comparison;
@group(4) @binding(0) var uniform_atlas_texture: texture_2d<f32>;
@group(4) @binding(1) var uniform_atlas_sampler: sampler;

@vertex
fn vertex_shader_main(vertex_input: VertexInput) -> VertexOutput {
	var vertex_output: VertexOutput;
	vertex_output.screen_position = uniform_camera * vec4<f32>(vertex_input.position, 1.0);
	var shade = dot(vertex_input.normal, -uniform_sun_light_direction);
	shade = clamp(shade, 0.0, 1.0);
	vertex_output.coords_in_atlas = vertex_input.coords_in_atlas;
	vertex_output.shade = shade;
	vertex_output.ambiant_occlusion = vertex_input.ambiant_occlusion;
	vertex_output.world_position = vertex_input.position;
	return vertex_output;
}

@fragment
fn fragment_shader_main(the: VertexOutput) -> @location(0) vec4<f32> {
	// Use the shadow map to know if the fragment is in shadows from other geometries.
	var position_in_sun_screen = uniform_sun_camera * vec4<f32>(the.world_position, 1.0);
	// Stealing some stuff from
	// https://github.com/gfx-rs/wgpu/blob/trunk/examples/shadow/src/shader.wgsl
	var position_in_shadow_map =
		position_in_sun_screen.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
	var not_in_shadow = textureSampleCompare(
		uniform_shadow_map_texture, uniform_shadow_map_sampler,
		position_in_shadow_map, position_in_sun_screen.z);
	if position_in_shadow_map.x < 0.0 || 1.0 < position_in_shadow_map.x ||
		position_in_shadow_map.y < 0.0 || 1.0 < position_in_shadow_map.y
	{
		// If we are outside of the shadow map then we get sun light.
		not_in_shadow = 1.0;
	}

	var out_color = textureSample(uniform_atlas_texture, uniform_atlas_sampler, the.coords_in_atlas);

	// Apply the darkenning due to the shadows and ambiant occlusion.
	var shade = the.shade * not_in_shadow;
	var out_color_rgb = out_color.rgb;
	let shade_ratio = 0.7; // How dark can in get in the shadows.
	out_color_rgb *= shade * shade_ratio + (1.0 - shade_ratio);
	let ambiant_occlusion_ratio = 0.7; // How dark can it get in the corners.
	out_color_rgb *= the.ambiant_occlusion * ambiant_occlusion_ratio + (1.0 - ambiant_occlusion_ratio);

	// Apply a touch of the sun light color over exposed surfaces.
	let sun_light_color = vec3<f32>(0.5, 0.35, 0.0) * 0.8;
	out_color_rgb = mix(
		out_color_rgb * (vec3<f32>(1.0, 1.0, 1.0) + sun_light_color), out_color_rgb,
		1.0 - shade);

	return vec4<f32>(out_color_rgb, out_color.a);
}
