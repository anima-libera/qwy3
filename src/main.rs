use std::f32::consts::TAU;

use wgpu::util::DeviceExt;
use winit::{
	event_loop::{ControlFlow, EventLoop},
	window::WindowBuilder,
};

fn main() {
	env_logger::init();
	let event_loop = EventLoop::new();
	let window = WindowBuilder::new()
		.with_title("Qwy3")
		.build(&event_loop)
		.unwrap();

	let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
		backends: wgpu::Backends::all(),
		dx12_shader_compiler: Default::default(),
	});

	let window_surface = unsafe { instance.create_surface(&window) }.unwrap();

	// Try to get a cool adapter first.
	let adapter = instance
		.enumerate_adapters(wgpu::Backends::all())
		.find(|adapter| {
			let info = adapter.get_info();
			info.device_type == wgpu::DeviceType::DiscreteGpu
				&& adapter.is_surface_supported(&window_surface)
		});
	// In case we didn't found any cool adapter, at least we can try to get a bad adapter.
	let adapter = adapter.or_else(|| {
		futures::executor::block_on(async {
			instance
				.request_adapter(&wgpu::RequestAdapterOptions {
					power_preference: wgpu::PowerPreference::HighPerformance,
					compatible_surface: Some(&window_surface),
					force_fallback_adapter: false,
				})
				.await
		})
	});
	let adapter = adapter.unwrap();

	println!("SELECTED ADAPTER:");
	dbg!(adapter.get_info());
	println!("AVAILABLE ADAPTERS:");
	for adapter in instance.enumerate_adapters(wgpu::Backends::all()) {
		dbg!(adapter.get_info());
	}

	let (device, queue) = futures::executor::block_on(async {
		adapter
			.request_device(
				&wgpu::DeviceDescriptor {
					features: wgpu::Features::empty(),
					limits: wgpu::Limits::default(),
					label: None,
				},
				None,
			)
			.await
			.unwrap()
	});

	let size = window.inner_size();

	let surface_caps = window_surface.get_capabilities(&adapter);
	let surface_format = surface_caps
		.formats
		.iter()
		.copied()
		.find(|f| f.is_srgb())
		.unwrap_or(surface_caps.formats[0]);
	assert!(surface_caps
		.present_modes
		.contains(&wgpu::PresentMode::Fifo));
	let mut config = wgpu::SurfaceConfiguration {
		usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
		format: surface_format,
		width: size.width,
		height: size.height,
		present_mode: wgpu::PresentMode::Fifo,
		alpha_mode: surface_caps.alpha_modes[0],
		view_formats: vec![],
	};
	window_surface.configure(&device, &config);

	struct CameraPerspective {
		position: cgmath::Point3<f32>,
		target: cgmath::Point3<f32>,
		up_direction: cgmath::Vector3<f32>,
		aspect_ratio: f32,
		field_of_view_y: f32,
		near_plane: f32,
		far_plane: f32,
	}

	let mut camera = CameraPerspective {
		position: (0.0, 0.0, 2.0).into(),
		target: (0.0, 0.0, 0.0).into(),
		up_direction: (0.0, 1.0, 0.0).into(),
		// Ugh, TODO: make Z+ the up direction instead of Y+
		aspect_ratio: config.width as f32 / config.height as f32,
		field_of_view_y: TAU / 4.0,
		near_plane: 0.001,
		far_plane: 10.0,
	};

	impl CameraPerspective {
		fn wgpu_matrix_pod(&self) -> Matrix4x4 {
			let view_matrix =
				cgmath::Matrix4::look_at_rh(self.position, self.target, self.up_direction);
			let projection_matrix = cgmath::perspective(
				cgmath::Rad(self.field_of_view_y),
				self.aspect_ratio,
				self.near_plane,
				self.far_plane,
			);

			#[rustfmt::skip]
			pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
				1.0, 0.0, 0.0, 0.0,
				0.0, 1.0, 0.0, 0.0,
				0.0, 0.0, 0.5, 0.0,
				0.0, 0.0, 0.5, 1.0,
			);
			let wgpu_matrix = OPENGL_TO_WGPU_MATRIX * projection_matrix * view_matrix;
			Matrix4x4 { values: wgpu_matrix.into() }
		}
	}

	#[repr(C)]
	#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
	struct Matrix4x4 {
		values: [[f32; 4]; 4],
	}

	let camera_wgpu_matrix_pod = camera.wgpu_matrix_pod();

	let camera_matrix_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Camera Buffer"),
		contents: bytemuck::cast_slice(&[camera_wgpu_matrix_pod]),
		usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
	});

	let camera_bind_group_layout =
		device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
			entries: &[wgpu::BindGroupLayoutEntry {
				binding: 0,
				visibility: wgpu::ShaderStages::VERTEX,
				ty: wgpu::BindingType::Buffer {
					ty: wgpu::BufferBindingType::Uniform,
					has_dynamic_offset: false,
					min_binding_size: None,
				},
				count: None,
			}],
			label: Some("Camera Bind Group Layout"),
		});
	let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
		layout: &camera_bind_group_layout,
		entries: &[wgpu::BindGroupEntry {
			binding: 0,
			resource: camera_matrix_buffer.as_entire_binding(),
		}],
		label: Some("Camera Bind Group"),
	});

	#[repr(C)]
	#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
	struct Vertex {
		position: [f32; 3],
		color: [f32; 3],
	}

	let vertices = [
		Vertex { position: [-0.5, -0.5, 0.0], color: [1.0, 0.0, 0.0] },
		Vertex { position: [0.5, -0.5, 0.0], color: [0.0, 1.0, 0.0] },
		Vertex { position: [0.0, 0.5, 0.0], color: [0.0, 0.0, 1.0] },
	];

	let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
		label: Some("Vertex Buffer"),
		contents: bytemuck::cast_slice(&vertices),
		usage: wgpu::BufferUsages::VERTEX,
	});

	let vertex_buffer_layout = wgpu::VertexBufferLayout {
		array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
		step_mode: wgpu::VertexStepMode::Vertex,
		attributes: &[
			wgpu::VertexAttribute {
				offset: 0,
				shader_location: 0,
				format: wgpu::VertexFormat::Float32x3,
			},
			wgpu::VertexAttribute {
				offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
				shader_location: 1,
				format: wgpu::VertexFormat::Float32x3,
			},
		],
	};

	let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
		label: Some("Shader"),
		source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/test_01.wgsl").into()),
	});

	let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
		label: Some("Render Pipeline Layout"),
		bind_group_layouts: &[&camera_bind_group_layout],
		push_constant_ranges: &[],
	});

	let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
		label: Some("Render Pipeline"),
		layout: Some(&render_pipeline_layout),
		vertex: wgpu::VertexState {
			module: &shader,
			entry_point: "vs_main",
			buffers: &[vertex_buffer_layout],
		},
		fragment: Some(wgpu::FragmentState {
			module: &shader,
			entry_point: "fs_main",
			targets: &[Some(wgpu::ColorTargetState {
				format: config.format,
				blend: Some(wgpu::BlendState::REPLACE),
				write_mask: wgpu::ColorWrites::ALL,
			})],
		}),
		primitive: wgpu::PrimitiveState {
			topology: wgpu::PrimitiveTopology::TriangleList,
			strip_index_format: None,
			front_face: wgpu::FrontFace::Ccw,
			cull_mode: Some(wgpu::Face::Back),
			polygon_mode: wgpu::PolygonMode::Fill,
			unclipped_depth: false,
			conservative: false,
		},
		depth_stencil: None,
		multisample: wgpu::MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false },
		multiview: None,
	});

	let time_beginning = std::time::Instant::now();

	use winit::event::*;
	event_loop.run(move |event, _, control_flow| match event {
		Event::WindowEvent { ref event, window_id } if window_id == window.id() => match event {
			WindowEvent::CloseRequested
			| WindowEvent::KeyboardInput {
				input:
					KeyboardInput {
						state: ElementState::Pressed,
						virtual_keycode: Some(VirtualKeyCode::Escape),
						..
					},
				..
			} => *control_flow = ControlFlow::Exit,
			WindowEvent::Resized(new_size) => {
				config.width = new_size.width;
				config.height = new_size.height;
				window_surface.configure(&device, &config);
				camera.aspect_ratio = config.width as f32 / config.height as f32;
			},
			_ => {},
		},
		Event::MainEventsCleared => {
			let time_since_beginning = time_beginning.elapsed();
			let ts = time_since_beginning.as_secs_f32();
			camera.position.x = f32::cos(ts * 5.0) * 3.0;
			camera.position.z = f32::sin(ts * 5.0) * 3.0;

			let camera_wgpu_matrix_pod = camera.wgpu_matrix_pod();
			let camera_matrix_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some("Camera Buffer"),
				contents: bytemuck::cast_slice(&[camera_wgpu_matrix_pod]),
				usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
			});
			let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
				layout: &camera_bind_group_layout,
				entries: &[wgpu::BindGroupEntry {
					binding: 0,
					resource: camera_matrix_buffer.as_entire_binding(),
				}],
				label: Some("Camera Bind Group"),
			});

			let window_texture = window_surface.get_current_texture().unwrap();
			let view = window_texture
				.texture
				.create_view(&wgpu::TextureViewDescriptor::default());
			let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
				label: Some("Render Encoder"),
			});

			{
				let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: Some("Render Pass"),
					color_attachments: &[Some(wgpu::RenderPassColorAttachment {
						view: &view,
						resolve_target: None,
						ops: wgpu::Operations {
							load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.01, g: 0.0, b: 0.05, a: 1.0 }),
							store: true,
						},
					})],
					depth_stencil_attachment: None,
				});
				render_pass.set_pipeline(&render_pipeline);
				render_pass.set_bind_group(0, &camera_bind_group, &[]);
				render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
				render_pass.draw(0..(vertices.len() as u32), 0..1);
			}
			queue.submit(std::iter::once(encoder.finish()));
			window_texture.present();
		},
		_ => {},
	});
}
