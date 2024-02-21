use std::{mem::size_of, sync::Arc};

use crate::{
	camera::{CameraOrthographicSettings, Matrix4x4Pod},
	skybox::SkyboxMesh,
	BindingThingy, ChunkGrid, RenderPipelinesAndBindGroups, SimpleLineMesh, SimpleTextureMesh,
	WhichCameraToUse,
};

pub(crate) struct DataForRendering<'a> {
	pub(crate) device: &'a Arc<wgpu::Device>,
	pub(crate) queue: &'a wgpu::Queue,
	pub(crate) window_surface: &'a wgpu::Surface<'static>,
	pub(crate) window_surface_config: &'a wgpu::SurfaceConfiguration,
	pub(crate) rendering: &'a RenderPipelinesAndBindGroups,
	pub(crate) sun_cameras: &'a [CameraOrthographicSettings],
	pub(crate) sun_camera_matrices_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) sun_camera_single_matrix_thingy: &'a BindingThingy<wgpu::Buffer>,
	pub(crate) shadow_map_cascade_view_thingies: &'a [BindingThingy<wgpu::TextureView>],
	pub(crate) chunk_grid: &'a ChunkGrid,
	pub(crate) z_buffer_view: &'a wgpu::TextureView,
	pub(crate) selected_camera: WhichCameraToUse,
	pub(crate) enable_display_phys_box: bool,
	pub(crate) player_box_mesh: &'a SimpleLineMesh,
	pub(crate) targeted_block_box_mesh_opt: &'a Option<SimpleLineMesh>,
	pub(crate) enable_display_interface: bool,
	pub(crate) chunk_box_meshes: &'a [SimpleLineMesh],
	pub(crate) skybox_mesh: &'a SkyboxMesh,
	pub(crate) typing_in_command_line: bool,
	pub(crate) cursor_mesh: &'a SimpleLineMesh,
	pub(crate) interface_simple_texture_mesh: &'a SimpleTextureMesh,
	pub(crate) interface_simple_line_mesh: &'a SimpleLineMesh,
}

impl<'a> DataForRendering<'a> {
	pub(crate) fn render(&self) {
		let mut encoder = self
			.device
			.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Render Encoder") });

		// Render pass to generate the shadow map cascades.
		for cascade_index in 0..self.sun_cameras.len() {
			encoder.copy_buffer_to_buffer(
				&self.sun_camera_matrices_thingy.resource,
				size_of::<Matrix4x4Pod>() as u64 * cascade_index as u64,
				&self.sun_camera_single_matrix_thingy.resource,
				0,
				size_of::<Matrix4x4Pod>() as u64,
			);

			let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("Render Pass for Shadow Map"),
				color_attachments: &[],
				depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
					view: &self.shadow_map_cascade_view_thingies[cascade_index].resource,
					depth_ops: Some(wgpu::Operations {
						load: wgpu::LoadOp::Clear(1.0),
						store: wgpu::StoreOp::Store,
					}),
					stencil_ops: None,
				}),
				timestamp_writes: None,
				occlusion_query_set: None,
			});

			render_pass.set_pipeline(&self.rendering.block_shadow_render_pipeline);
			render_pass.set_bind_group(0, &self.rendering.block_shadow_bind_group, &[]);
			for mesh in self.chunk_grid.iter_chunk_meshes() {
				render_pass.set_vertex_buffer(0, mesh.block_vertex_buffer.as_ref().unwrap().slice(..));
				render_pass.draw(0..(mesh.block_vertices.len() as u32), 0..1);
			}
		}

		// Render pass to render the world to the screen.
		let window_texture = self.window_surface.get_current_texture().unwrap();
		{
			let window_texture_view =
				window_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
			let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("Render Pass to render the world"),
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &window_texture_view,
					resolve_target: None,
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.7, b: 1.0, a: 0.0 }),
						store: wgpu::StoreOp::Store,
					},
				})],
				depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
					view: self.z_buffer_view,
					depth_ops: Some(wgpu::Operations {
						load: wgpu::LoadOp::Clear(1.0),
						store: wgpu::StoreOp::Store,
					}),
					stencil_ops: None,
				}),
				timestamp_writes: None,
				occlusion_query_set: None,
			});

			if matches!(self.selected_camera, WhichCameraToUse::Sun) {
				let scale = self.window_surface_config.height as f32 / self.sun_cameras[0].height;
				let w = self.sun_cameras[0].width * scale;
				let h = self.sun_cameras[0].height * scale;
				let x = self.window_surface_config.width as f32 / 2.0 - w / 2.0;
				let y = self.window_surface_config.height as f32 / 2.0 - h / 2.0;
				render_pass.set_viewport(x, y, w, h, 0.0, 1.0);
			}

			render_pass.set_pipeline(&self.rendering.block_render_pipeline);
			render_pass.set_bind_group(0, &self.rendering.block_bind_group, &[]);
			for mesh in self.chunk_grid.iter_chunk_meshes() {
				render_pass.set_vertex_buffer(0, mesh.block_vertex_buffer.as_ref().unwrap().slice(..));
				render_pass.draw(0..(mesh.block_vertices.len() as u32), 0..1);
			}

			if self.enable_display_phys_box {
				render_pass.set_pipeline(&self.rendering.simple_line_render_pipeline);
				render_pass.set_bind_group(0, &self.rendering.simple_line_bind_group, &[]);
				render_pass.set_vertex_buffer(0, self.player_box_mesh.vertex_buffer.slice(..));
				render_pass.draw(0..(self.player_box_mesh.vertices.len() as u32), 0..1);
			}

			if let Some(targeted_block_box_mesh) = &self.targeted_block_box_mesh_opt {
				if self.enable_display_interface {
					render_pass.set_pipeline(&self.rendering.simple_line_render_pipeline);
					render_pass.set_bind_group(0, &self.rendering.simple_line_bind_group, &[]);
					render_pass.set_vertex_buffer(0, targeted_block_box_mesh.vertex_buffer.slice(..));
					render_pass.draw(0..(targeted_block_box_mesh.vertices.len() as u32), 0..1);
				}
			}

			for chunk_box_mesh in self.chunk_box_meshes.iter() {
				render_pass.set_pipeline(&self.rendering.simple_line_render_pipeline);
				render_pass.set_bind_group(0, &self.rendering.simple_line_bind_group, &[]);
				render_pass.set_vertex_buffer(0, chunk_box_mesh.vertex_buffer.slice(..));
				render_pass.draw(0..(chunk_box_mesh.vertices.len() as u32), 0..1);
			}
		}

		// Render pass to render the skybox to the screen.
		{
			let window_texture_view =
				window_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
			let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("Render Pass to render the skybox"),
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &window_texture_view,
					resolve_target: None,
					ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
				})],
				depth_stencil_attachment: None,
				timestamp_writes: None,
				occlusion_query_set: None,
			});

			if matches!(self.selected_camera, WhichCameraToUse::Sun) {
				let scale = self.window_surface_config.height as f32 / self.sun_cameras[0].height;
				let w = self.sun_cameras[0].width * scale;
				let h = self.sun_cameras[0].height * scale;
				let x = self.window_surface_config.width as f32 / 2.0 - w / 2.0;
				let y = self.window_surface_config.height as f32 / 2.0 - h / 2.0;
				render_pass.set_viewport(x, y, w, h, 0.0, 1.0);
			}

			render_pass.set_pipeline(&self.rendering.skybox_render_pipeline);
			render_pass.set_bind_group(0, &self.rendering.skybox_bind_group, &[]);
			render_pass.set_vertex_buffer(0, self.skybox_mesh.vertex_buffer.slice(..));
			render_pass.draw(0..(self.skybox_mesh.vertices.len() as u32), 0..1);
		}

		// Render pass to draw the interface.
		{
			let window_texture_view =
				window_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
			let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("Render Pass to render the interface"),
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view: &window_texture_view,
					resolve_target: None,
					ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
				})],
				depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
					view: self.z_buffer_view,
					depth_ops: Some(wgpu::Operations {
						load: wgpu::LoadOp::Clear(1.0),
						store: wgpu::StoreOp::Store,
					}),
					stencil_ops: None,
				}),
				timestamp_writes: None,
				occlusion_query_set: None,
			});

			if self.enable_display_interface
				&& !matches!(self.selected_camera, WhichCameraToUse::Sun)
				&& !self.typing_in_command_line
			{
				render_pass.set_pipeline(&self.rendering.simple_line_2d_render_pipeline);
				render_pass.set_bind_group(0, &self.rendering.simple_line_2d_bind_group, &[]);
				render_pass.set_vertex_buffer(0, self.cursor_mesh.vertex_buffer.slice(..));
				render_pass.draw(0..(self.cursor_mesh.vertices.len() as u32), 0..1);
			}

			if self.enable_display_interface && !matches!(self.selected_camera, WhichCameraToUse::Sun)
			{
				render_pass.set_pipeline(&self.rendering.simple_texture_2d_render_pipeline);
				render_pass.set_bind_group(0, &self.rendering.simple_texture_2d_bind_group, &[]);
				let mesh = &self.interface_simple_texture_mesh;
				render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
				render_pass.draw(0..(mesh.vertices.len() as u32), 0..1);

				render_pass.set_pipeline(&self.rendering.simple_line_2d_render_pipeline);
				render_pass.set_bind_group(0, &self.rendering.simple_line_2d_bind_group, &[]);
				let mesh = &self.interface_simple_line_mesh;
				render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
				render_pass.draw(0..(mesh.vertices.len() as u32), 0..1);
			}
		}

		self.queue.submit(std::iter::once(encoder.finish()));

		window_texture.present();
	}
}
