struct VertexInput {
	@location(0) position: vec3<f32>,
	@location(1) normal: vec3<f32>,
};

struct InstanceInput {
	@location(2) model_matrix_1_of_4: vec4<f32>,
	@location(3) model_matrix_2_of_4: vec4<f32>,
	@location(4) model_matrix_3_of_4: vec4<f32>,
	@location(5) model_matrix_4_of_4: vec4<f32>,
	@location(6) texture_mapping_point_offset: u32,
};

struct VertexOutput {
	@builtin(position) screen_position: vec4<f32>,
	@location(0) world_position: vec3<f32>,
	@location(1) coords_in_atlas: vec2<f32>,
	@location(2) shade: f32,
};

@group(0) @binding(0) var<uniform> uniform_camera: mat4x4<f32>;
@group(0) @binding(1) var uniform_atlas_texture: texture_2d<f32>;
@group(0) @binding(2) var uniform_atlas_sampler: sampler;
@group(0) @binding(3) var<storage, read> uniform_coords_in_atlas_array: array<vec2<f32> >;
@group(0) @binding(4) var<uniform> uniform_sun_light_direction: vec3<f32>;
@group(0) @binding(5) var<storage, read> uniform_sun_camera_array: array<mat4x4<f32> >;
@group(0) @binding(6) var uniform_shadow_map_texture_array: texture_depth_2d_array;
@group(0) @binding(7) var uniform_shadow_map_sampler: sampler_comparison;
// TODO: Add the fog uniforms.

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

	var coords_in_atlas =
		uniform_coords_in_atlas_array[instance_input.texture_mapping_point_offset + vertex_index];

	var shade = dot(vertex_input.normal, -uniform_sun_light_direction);
	shade = clamp(shade, 0.0, 1.0);

	var world_position = model_matrix * vec4<f32>(vertex_input.position, 1.0);

	var vertex_output: VertexOutput;
	vertex_output.screen_position = uniform_camera * world_position;
	vertex_output.world_position = world_position.xyz;
	vertex_output.coords_in_atlas = coords_in_atlas;
	vertex_output.shade = shade;
	return vertex_output;
}

@fragment
fn fragment_shader_main(the: VertexOutput) -> @location(0) vec4<f32> {
	// TODO: There is a lot of code duplication between here and `blocks.wgsl`,
	// we have to factorize!

	var not_in_shadow = 1.0;

	// Each cascade is a shadow map, from smallest (so more precise) to largest.
	// We querry the smallest shadow map that we are in to get the best available precision.
	var position_in_shadow_map: vec2<f32>;
	var position_in_sun_screen: vec4<f32>;
	var smallest_cascade_index: u32 = 0;
	var cascade_count = arrayLength(&uniform_sun_camera_array);
	for (var cascade_index: u32 = 0; cascade_index < cascade_count; cascade_index++) {
		position_in_sun_screen =
			uniform_sun_camera_array[cascade_index] * vec4<f32>(the.world_position, 1.0);
		// Stealed the coordinate correction from
		// https://github.com/gfx-rs/wgpu/blob/trunk/examples/shadow/src/shader.wgsl
		position_in_shadow_map =
			position_in_sun_screen.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
		var hit_this_cascade =
			!(position_in_shadow_map.x < 0.0 || 1.0 < position_in_shadow_map.x ||
			position_in_shadow_map.y < 0.0 || 1.0 < position_in_shadow_map.y);
		if hit_this_cascade {
			smallest_cascade_index = cascade_index;
			break;
		}
	}
	not_in_shadow = textureSampleCompare(
		uniform_shadow_map_texture_array, uniform_shadow_map_sampler,
		position_in_shadow_map, smallest_cascade_index, position_in_sun_screen.z);
	// Remove shadows outside all the shadow maps.
	var missed_even_last_cascade =
		position_in_shadow_map.x < 0.0 || 1.0 < position_in_shadow_map.x ||
		position_in_shadow_map.y < 0.0 || 1.0 < position_in_shadow_map.y;
	if missed_even_last_cascade {
		not_in_shadow = 1.0;
	} 

	var out_color = textureSample(uniform_atlas_texture, uniform_atlas_sampler, the.coords_in_atlas);

	// Full transparency.
	if out_color.a == 0.0 {
		discard;
	}

	// Apply the darkenning due to the shadows.
	var shade = the.shade * not_in_shadow;
	var out_color_rgb = out_color.rgb;
	let shade_ratio = 0.7; // How dark can in get in the shadows.
	out_color_rgb *= shade * shade_ratio + (1.0 - shade_ratio);

	// Apply a touch of the sun light color over exposed surfaces.
	let sun_light_color = vec3<f32>(0.5, 0.35, 0.0) * 0.8;
	out_color_rgb = mix(
		out_color_rgb * (vec3<f32>(1.0, 1.0, 1.0) + sun_light_color), out_color_rgb,
		1.0 - shade);

	return vec4<f32>(out_color_rgb, out_color.a);
}
