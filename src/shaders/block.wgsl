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
@group(0) @binding(1) var<uniform> uniform_sun_light_direction: vec3<f32>;
@group(0) @binding(2) var<uniform> uniform_sun_camera: mat4x4<f32>;
@group(0) @binding(3) var uniform_shadow_map_texture_array: texture_depth_2d_array;
@group(0) @binding(4) var uniform_shadow_map_sampler: sampler_comparison;
@group(0) @binding(5) var uniform_atlas_texture: texture_2d<f32>;
@group(0) @binding(6) var uniform_atlas_sampler: sampler;
@group(0) @binding(7) var<uniform> uniform_fog_center_position: vec3<f32>;
@group(0) @binding(8) var<uniform> uniform_fog_inf_sup_radiuses: vec2<f32>;

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
	var cascade_index = 0;
	var not_in_shadow = textureSampleCompare(
		uniform_shadow_map_texture_array, uniform_shadow_map_sampler,
		position_in_shadow_map, cascade_index, position_in_sun_screen.z);
	if position_in_shadow_map.x < 0.0 || 1.0 < position_in_shadow_map.x ||
		position_in_shadow_map.y < 0.0 || 1.0 < position_in_shadow_map.y
	{
		// If we are outside of the shadow map then we get sun light.
		not_in_shadow = 1.0;
	}

	var out_color = textureSample(uniform_atlas_texture, uniform_atlas_sampler, the.coords_in_atlas);

	// Fog effect, matter gradually becomes transparent to the skybox when too far away.
	var distance_to_fog_center = distance(uniform_fog_center_position, the.world_position);
	var fog_inf_radius = uniform_fog_inf_sup_radiuses.x;
	var fog_sup_radius = uniform_fog_inf_sup_radiuses.y;
	var fog_transparency = (distance_to_fog_center - fog_inf_radius) / (fog_sup_radius - fog_inf_radius);
	fog_transparency = clamp(fog_transparency, 0.0, 1.0);
	var fog_opacity = 1.0 - fog_transparency;
	out_color.a *= fog_opacity;

	// Full transparency.
	if out_color.a == 0.0 {
		discard;
	}

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
