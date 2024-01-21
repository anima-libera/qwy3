//! Skybox. Sky on box.
//!
//! We paint some spherical image on a cubemap texture and render it on the mesh of a cube
//! that whould be placed around the camera and rendered infinitely far.
//! Nothing conceptually complicated here, just lots of small details to be handled just right.

use std::{
	f32::consts::TAU,
	sync::{
		atomic::{self, AtomicI32},
		Arc,
	},
};

use cgmath::{point3, vec3, EuclideanSpace, InnerSpace, Point3, Vector3};
use image::Rgba;
use wgpu::util::DeviceExt;

use crate::{noise::OctavedNoise, shaders::skybox::SkyboxVertexPod, OrientedAxis, SkyboxData};

pub const SKYBOX_SIDE_DIMS: (usize, usize) = (512, 512);

pub struct SkyboxMesh {
	pub vertices: Vec<SkyboxVertexPod>,
	pub vertex_buffer: wgpu::Buffer,
}

impl SkyboxMesh {
	pub fn new(device: &wgpu::Device, center_coords: Point3<f32>) -> SkyboxMesh {
		let new_vertex =
			|position: Point3<f32>, transformations: &[Transformation]| -> SkyboxVertexPod {
				SkyboxVertexPod {
					position: (center_coords + position.to_vec()).into(),
					coords_in_skybox_cubemap: apply_transformations(transformations, position).into(),
				}
			};
		let new_face = |vertices: &mut Vec<SkyboxVertexPod>,
		                axis_a: usize,
		                axis_b: usize,
		                axis_c_and_value: (usize, f32),
		                transformations: &[Transformation]| {
			let ab_to_coords = |a: f32, b: f32| -> Point3<f32> {
				let (axis_c, c) = axis_c_and_value;
				let mut coords = point3(0.0, 0.0, 0.0);
				coords[axis_a] = a;
				coords[axis_b] = b;
				coords[axis_c] = c;
				coords
			};
			// One triangle.
			vertices.push(new_vertex(ab_to_coords(-1.0, -1.0), transformations));
			vertices.push(new_vertex(ab_to_coords(1.0, -1.0), transformations));
			vertices.push(new_vertex(ab_to_coords(-1.0, 1.0), transformations));
			// The other triangle.
			vertices.push(new_vertex(ab_to_coords(1.0, -1.0), transformations));
			vertices.push(new_vertex(ab_to_coords(-1.0, 1.0), transformations));
			vertices.push(new_vertex(ab_to_coords(1.0, 1.0), transformations));
		};

		/// Transform the mapping of the texture on a quad.
		/// Useful to adjust that mapping so that it looks right.
		enum Transformation {
			/// Flip along the given axis.
			Flip(usize),
			/// Rotate around the axis that is not given.
			Rot(usize, usize),
		}
		impl Transformation {
			fn apply(&self, coords: &mut Point3<f32>) {
				match self {
					Transformation::Flip(axis) => {
						coords[*axis] *= -1.0;
					},
					Transformation::Rot(axis_a, axis_b) => {
						let a = coords[*axis_a];
						coords[*axis_a] = coords[*axis_b] * -1.0;
						coords[*axis_b] = a;
					},
				}
			}
		}
		fn apply_transformations(
			transformations: &[Transformation],
			mut coords: Point3<f32>,
		) -> Point3<f32> {
			for transformation in transformations {
				transformation.apply(&mut coords);
			}
			coords
		}

		// Generate all the faces with the empirically discovered transformations
		// that make all the faces be oriented and turned so that they connect just right.
		let mut vertices = vec![];
		{
			use Transformation as Tr;
			new_face(&mut vertices, 0, 1, (2, -1.0), &[Tr::Flip(0), Tr::Flip(1)]);
			new_face(&mut vertices, 0, 1, (2, 1.0), &[Tr::Flip(1)]);
			new_face(&mut vertices, 0, 2, (1, -1.0), &[Tr::Flip(2)]);
			new_face(&mut vertices, 0, 2, (1, 1.0), &[]);
			new_face(&mut vertices, 1, 2, (0, -1.0), &[Tr::Rot(1, 2)]);
			new_face(&mut vertices, 1, 2, (0, 1.0), &[Tr::Rot(2, 1), Tr::Flip(1)]);
		}

		let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Skybox Vertex Buffer"),
			contents: bytemuck::cast_slice(&vertices),
			usage: wgpu::BufferUsages::VERTEX,
		});

		SkyboxMesh { vertices, vertex_buffer }
	}
}

pub fn default_skybox_painter(direction: Vector3<f32>) -> Rgba<u8> {
	Rgba([
		((direction.x + 1.0) / 2.0 * 255.0) as u8,
		((direction.y + 1.0) / 2.0 * 255.0) as u8,
		((direction.z + 1.0) / 2.0 * 255.0) as u8,
		255,
	])
}

pub fn _default_skybox_painter_2(
	number_of_octaves: u32,
	seed: i32,
) -> impl Fn(Vector3<f32>) -> Rgba<u8> {
	let noise = OctavedNoise::new(number_of_octaves, vec![seed]);
	move |mut direction: Vector3<f32>| -> Rgba<u8> {
		direction += (noise
			._sample_3d_3d(Point3::from_vec(direction) * 10.0, &[])
			.to_vec() - vec3(0.5, 0.5, 0.5))
			* 0.5;
		Rgba([
			((direction.x + 1.0) / 2.0 * 255.0) as u8,
			((direction.y + 1.0) / 2.0 * 255.0) as u8,
			((direction.z + 1.0) / 2.0 * 255.0) as u8,
			255,
		])
	}
}

pub fn default_skybox_painter_3(
	number_of_octaves: u32,
	seed: i32,
) -> impl Fn(Vector3<f32>) -> Rgba<u8> {
	let noise = OctavedNoise::new(number_of_octaves, vec![seed]);
	move |mut direction: Vector3<f32>| -> Rgba<u8> {
		let scale_m = 4.0;
		let scale_a = 10.0 * noise.sample_3d_1d(Point3::from_vec(direction * scale_m), &[1]);
		let nosie_value_a = noise.sample_3d_1d(Point3::from_vec(direction * scale_a), &[2]);
		let angle = nosie_value_a * TAU;
		let scale_d = 10.0;
		let distance = 0.4 * noise.sample_3d_1d(Point3::from_vec(direction * scale_d), &[4]);
		direction.x += f32::cos(angle) * distance;
		direction.y += f32::sin(angle) * distance;
		Rgba([
			((direction.x + 1.0) / 2.0 * 255.0) as u8,
			((direction.y + 1.0) / 2.0 * 255.0) as u8,
			((direction.z + 1.0) / 2.0 * 255.0) as u8,
			255,
		])
	}
}

fn generate_a_skybox_cubemap_face_image(
	face_direction: OrientedAxis,
	skybox_painter: &dyn Fn(Vector3<f32>) -> Rgba<u8>,
) -> image::RgbaImage {
	let mut image: image::RgbaImage =
		image::ImageBuffer::new(SKYBOX_SIDE_DIMS.0 as u32, SKYBOX_SIDE_DIMS.1 as u32);

	// The skybox shall appear as some shpere that is infinitely far away in every direction,
	// but we have to paint it on the faces of a cube that will be appear around the world.
	// It makes sense to paint each pixel of the faces of the cube with a color that depends
	// on the direction from the center of the cube to that pixel instead of say the coordinates
	// of the pixel on its face texture or something, because we should not think in terms of
	// cube because what we are really painting here is a sphere (that happens to be projected
	// on a cube).

	// First we get the directions from the center of the center of the cube to the four
	// vertices of the face we are currently painting.
	let fixed_axis = face_direction.axis.index();
	let fixed_axis_value = face_direction.orientation.sign() as f32;
	let axis_a = if fixed_axis == 0 { 1 } else { 0 };
	let axis_b = if fixed_axis.max(axis_a) == 1 {
		2
	} else if fixed_axis.min(axis_a) == 1 {
		0
	} else {
		1
	};
	let ab_to_coords = |a: f32, b: f32| -> Point3<f32> {
		let mut coords = point3(0.0, 0.0, 0.0);
		coords[axis_a] = a;
		coords[axis_b] = b;
		coords[fixed_axis] = fixed_axis_value;
		coords
	};
	let mm = ab_to_coords(-1.0, -1.0);
	let mp = ab_to_coords(-1.0, 1.0);
	let pm = ab_to_coords(1.0, -1.0);
	let pp = ab_to_coords(1.0, 1.0);

	/// Returns `inf` when `value` is 0.0 and `sup` when `value` is 1.0.
	fn point3_lerp(value: f32, inf: Point3<f32>, sup: Point3<f32>) -> Point3<f32> {
		inf + (sup - inf.to_vec()).to_vec() * value
	}

	for y in 0..SKYBOX_SIDE_DIMS.1 {
		for x in 0..SKYBOX_SIDE_DIMS.0 {
			// Here we are going to paint a pixel, but we want to rely on the direction
			// from the center of the cube to the pixel for that instead of its coords in its face,
			// so we interpolate a bit to get its direction.
			let mi = point3_lerp(y as f32 / SKYBOX_SIDE_DIMS.1 as f32, mm, mp);
			let pi = point3_lerp(y as f32 / SKYBOX_SIDE_DIMS.1 as f32, pm, pp);
			let ii = point3_lerp(x as f32 / SKYBOX_SIDE_DIMS.0 as f32, mi, pi);
			let direction = ii;
			let direction = direction.to_vec().normalize();

			let color = skybox_painter(direction);

			image.put_pixel(x as u32, y as u32, color);
		}
	}
	image
}

pub fn generate_skybox_cubemap_faces_images(
	skybox_painter: &dyn Fn(Vector3<f32>) -> Rgba<u8>,
	face_counter: Option<Arc<AtomicI32>>,
) -> SkyboxFaces {
	let mut faces = vec![];
	let mut face_directions = vec![];
	for face_direction in OrientedAxis::all_the_six_possible_directions() {
		faces.push(generate_a_skybox_cubemap_face_image(
			face_direction,
			skybox_painter,
		));
		face_directions.push(face_direction);
		if let Some(ref face_counter) = face_counter {
			face_counter.fetch_add(1, atomic::Ordering::Relaxed);
		}
	}
	SkyboxFaces {
		faces: faces.try_into().unwrap(),
		face_directions: face_directions.try_into().unwrap(),
	}
}

pub struct SkyboxFaces {
	pub faces: [image::RgbaImage; 6],
	pub face_directions: [OrientedAxis; 6],
}

impl SkyboxFaces {
	pub fn data(&self) -> SkyboxData {
		[
			self.faces[0].as_ref(),
			self.faces[1].as_ref(),
			self.faces[2].as_ref(),
			self.faces[3].as_ref(),
			self.faces[4].as_ref(),
			self.faces[5].as_ref(),
		]
	}
}
