use std::f32::consts::TAU;

use crate::{
	camera::{aspect_ratio, CameraSettings},
	chunks::{Block, BlockData},
	coords::{iter_3d_cube_center_radius, AlignedBox, BlockCoords, ChunkCoordsSpan},
	entities::Entity,
	font,
	game_init::{init_game, save_savable_state},
	lang::{self, LogItem},
	line_meshes::SimpleLineMesh,
	rendering,
	rendering_init::{make_z_buffer_texture_view, update_atlas_texture, update_skybox_texture},
	shaders::{Vector2Pod, Vector3Pod},
	skybox::SkyboxMesh,
	unsorted::{Action, Control, ControlEvent, SimpleTextureMesh, WhichCameraToUse, WorkerTask},
	widgets::{InterfaceMeshesVertices, Widget, WidgetLabel},
};

use cgmath::{point3, InnerSpace, MetricSpace};
use rand::Rng;
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;

/// Initializes the game and run the main game loop.
pub fn init_and_run_game_loop() {
	let (mut game, event_loop) = init_game();

	use winit::event::*;
	use winit::keyboard::*;
	let res = event_loop.run(move |event, elwt| match event {
		Event::WindowEvent { ref event, window_id } if window_id == game.window.id() => match event {
			WindowEvent::CloseRequested
			| WindowEvent::KeyboardInput {
				event:
					KeyEvent {
						logical_key: Key::Named(NamedKey::Escape),
						state: ElementState::Pressed,
						..
					},
				..
			} => elwt.exit(),

			WindowEvent::Resized(new_size) => {
				let winit::dpi::PhysicalSize { width, height } = *new_size;
				game.window_surface_config.width = width;
				game.window_surface_config.height = height;
				game.window_surface.configure(&game.device, &game.window_surface_config);
				game.z_buffer_view =
					make_z_buffer_texture_view(&game.device, game.z_buffer_format, width, height);
				game.camera_settings.aspect_ratio = aspect_ratio(width, height);

				game.queue.write_buffer(
					&game.aspect_ratio_thingy.resource,
					0,
					bytemuck::cast_slice(&[game.camera_settings.aspect_ratio]),
				);
			},

			WindowEvent::MouseInput {
				state: winit::event::ElementState::Pressed,
				button: winit::event::MouseButton::Left,
				..
			} if !game.cursor_is_captured => {
				game.cursor_is_captured = true;
				game.window.set_cursor_grab(winit::window::CursorGrabMode::Confined).unwrap();
				game.window.set_cursor_visible(false);
			},

			WindowEvent::KeyboardInput {
				event: event @ KeyEvent { logical_key, state, repeat, .. },
				..
			} => {
				if game.typing_in_command_line && *state == ElementState::Pressed {
					if matches!(logical_key, Key::Named(NamedKey::Enter)) {
						game.command_confirmed = true;
						game.typing_in_command_line = false;
						game.last_command_line_interaction = Some(std::time::Instant::now());
					} else if matches!(logical_key, Key::Named(NamedKey::Backspace)) {
						game.command_line_content.pop();
						game.last_command_line_interaction = Some(std::time::Instant::now());
					} else if let Key::Character(string) = logical_key {
						game.command_line_content += string;
						game.last_command_line_interaction = Some(std::time::Instant::now());
					}
				} else if !repeat {
					game.controls_to_trigger.push(ControlEvent {
						control: Control::KeyboardKey(event.key_without_modifiers()),
						pressed: *state == ElementState::Pressed,
					});
				}
			},

			WindowEvent::MouseInput { state, button, .. } if game.cursor_is_captured => {
				game.controls_to_trigger.push(ControlEvent {
					control: Control::MouseButton(*button),
					pressed: *state == ElementState::Pressed,
				});
			},

			_ => {},
		},

		Event::DeviceEvent { event: winit::event::DeviceEvent::MouseMotion { delta }, .. }
			if game.cursor_is_captured =>
		{
			// Move camera.
			let sensitivity = 0.0025;
			game.camera_direction.angle_horizontal += -1.0 * delta.0 as f32 * sensitivity;
			game.camera_direction.angle_vertical += delta.1 as f32 * sensitivity;
			if game.camera_direction.angle_vertical < 0.0 {
				game.camera_direction.angle_vertical = 0.0;
			}
			if TAU / 2.0 < game.camera_direction.angle_vertical {
				game.camera_direction.angle_vertical = TAU / 2.0;
			}
		},

		Event::DeviceEvent { event: winit::event::DeviceEvent::MouseWheel { delta }, .. } => {
			// Wheel moves the player along the vertical axis.
			// Useful when physics are disabled.
			let (dx, dy) = match delta {
				MouseScrollDelta::LineDelta(horizontal, vertical) => (horizontal, vertical),
				MouseScrollDelta::PixelDelta(position) => (position.x as f32, position.y as f32),
			};
			let sensitivity = 0.01;
			let direction_left_or_right =
				game.camera_direction.to_horizontal().add_to_horizontal_angle(TAU / 4.0 * dx.signum());
			let mut pos = game.player_phys.aligned_box().pos;
			pos.z -= dy * sensitivity;
			pos += direction_left_or_right.to_vec3() * f32::abs(dx) * sensitivity;
			game.player_phys.impose_new_pos(pos);
		},

		Event::AboutToWait => {
			// Here shall begin the body of the gameloop.

			let _time_since_beginning = game.time_beginning.elapsed();
			let now = std::time::Instant::now();
			let dt = now - game.time_from_last_iteration;
			game.time_from_last_iteration = now;

			// Perform actions triggered by controls.
			for control_event in game.controls_to_trigger.iter() {
				let pressed = control_event.pressed;
				if let Some(action) = game.control_bindings.get(&control_event.control) {
					match (action, pressed) {
						(Action::WalkForward, pressed) => {
							game.walking_forward = pressed;
						},
						(Action::WalkBackward, pressed) => {
							game.walking_backward = pressed;
						},
						(Action::WalkLeftward, pressed) => {
							game.walking_leftward = pressed;
						},
						(Action::WalkRightward, pressed) => {
							game.walking_rightward = pressed;
						},
						(Action::Jump, true) => {
							game.player_phys.jump();
						},
						(Action::TogglePhysics, true) => {
							game.enable_physics = !game.enable_physics;
						},
						(Action::ToggleWorldGeneration, true) => {
							game.enable_world_generation = !game.enable_world_generation;
						},
						(Action::CycleFirstAndThirdPersonViews, true) => {
							game.selected_camera = match game.selected_camera {
								WhichCameraToUse::FirstPerson => WhichCameraToUse::ThirdPersonNear,
								WhichCameraToUse::ThirdPersonNear => WhichCameraToUse::ThirdPersonFar,
								WhichCameraToUse::ThirdPersonFar => WhichCameraToUse::ThirdPersonVeryFar,
								WhichCameraToUse::ThirdPersonVeryFar => WhichCameraToUse::ThirdPersonTooFar,
								WhichCameraToUse::ThirdPersonTooFar => WhichCameraToUse::FirstPerson,
								WhichCameraToUse::Sun => WhichCameraToUse::FirstPerson,
							};
						},
						(Action::ToggleDisplayPlayerBox, true) => {
							game.enable_display_phys_box = !game.enable_display_phys_box;
						},
						(Action::ToggleSunView, true) => {
							game.selected_camera = match game.selected_camera {
								WhichCameraToUse::Sun => WhichCameraToUse::FirstPerson,
								_ => WhichCameraToUse::Sun,
							};
						},
						(Action::ToggleCursorCaptured, true) => {
							game.cursor_is_captured = !game.cursor_is_captured;
							if game.cursor_is_captured {
								game
									.window
									.set_cursor_grab(winit::window::CursorGrabMode::Confined)
									.unwrap();
								game.window.set_cursor_visible(false);
							} else {
								game.window.set_cursor_grab(winit::window::CursorGrabMode::None).unwrap();
								game.window.set_cursor_visible(true);
							}
						},
						(Action::PrintCoords, true) => {
							dbg!(game.player_phys.aligned_box().pos);
							let player_bottom = game.player_phys.aligned_box().pos
								- cgmath::Vector3::<f32>::from((
									0.0,
									0.0,
									game.player_phys.aligned_box().dims.z / 2.0,
								));
							dbg!(player_bottom);
						},
						(Action::PlaceOrRemoveBlockUnderPlayer, true) => {
							let player_bottom = game.player_phys.aligned_box().pos
								- cgmath::Vector3::<f32>::unit_z()
									* (game.player_phys.aligned_box().dims.z / 2.0 + 0.1);
							let player_bottom_block_coords = player_bottom.map(|x| x.round() as i32);
							let player_bottom_block_opt =
								game.chunk_grid.get_block(player_bottom_block_coords);
							if let Some(block) = player_bottom_block_opt {
								game.chunk_grid.set_block_and_request_updates_to_meshes(
									player_bottom_block_coords,
									if game.block_type_table.get(block.type_id).unwrap().is_opaque() {
										game.block_type_table.air_id().into()
									} else {
										game.block_type_table.ground_id().into()
									},
								);
							}
						},
						(Action::PlaceBlockAtTarget, true) => {
							if let Some((_, coords)) = game.targeted_block_coords {
								game.chunk_grid.set_block_and_request_updates_to_meshes(
									coords,
									//game.block_type_table.ground_id().into(),
									Block {
										type_id: game.block_type_table.text_id(),
										data: Some(BlockData::Text("Jaaj".to_string())),
									},
								);
							}
						},
						(Action::RemoveBlockAtTarget, true) => {
							if let Some((coords, _)) = game.targeted_block_coords {
								game.chunk_grid.set_block_and_request_updates_to_meshes(
									coords,
									game.block_type_table.air_id().into(),
								);
							}
						},
						(Action::ToggleDisplayInterface, true) => {
							game.enable_display_interface = !game.enable_display_interface;
						},
						(Action::OpenCommandLine, true) => {
							game.typing_in_command_line = true;
							game.last_command_line_interaction = Some(std::time::Instant::now());
						},
						(Action::ToggleDisplayNotSurroundedChunksAsBoxes, true) => {
							game.enable_display_not_surrounded_chunks_as_boxes =
								!game.enable_display_not_surrounded_chunks_as_boxes;
						},
						(Action::ToggleDisplayInterfaceDebugBoxes, true) => {
							game.enable_interface_draw_debug_boxes =
								!game.enable_interface_draw_debug_boxes;
						},
						(Action::ToggleFog, true) => {
							game.enable_fog = !game.enable_fog;
							let (inf, sup) = if game.enable_fog {
								game.fog_inf_sup_radiuses
							} else {
								(10000.0, 10000.0)
							};
							game.queue.write_buffer(
								&game.fog_inf_sup_radiuses_thingy.resource,
								0,
								bytemuck::cast_slice(&[Vector2Pod { values: [inf, sup] }]),
							);
						},
						(Action::ToggleFullscreen, true) => {
							game.enable_fullscreen = !game.enable_fullscreen;
							game.window.set_fullscreen(
								game
									.enable_fullscreen
									.then_some(winit::window::Fullscreen::Borderless(None)),
							);
						},
						(Action::ThrowBlock, true) => {
							for _ in 0..30 {
								let block =
									Block { type_id: game.block_type_table.kinda_leaf_id(), data: None };

								let mut motion = game.camera_direction.to_vec3();
								motion.x +=
									rand::thread_rng().gen_range(-1.0..1.0) * 0.1 * 144.0 * dt.as_secs_f32();
								motion.y +=
									rand::thread_rng().gen_range(-1.0..1.0) * 0.1 * 144.0 * dt.as_secs_f32();
								motion.z +=
									rand::thread_rng().gen_range(-1.0..1.0) * 0.1 * 144.0 * dt.as_secs_f32();

								game.chunk_grid.spawn_entity(Entity::new_block(
									block,
									game.player_phys.aligned_box().pos,
									motion,
								))
							}
						},
						(Action::ToggleDisplayChunksWithEntitiesAsBoxes, true) => {
							game.enable_display_chunks_with_entities_as_boxes =
								!game.enable_display_chunks_with_entities_as_boxes;
						},
						(_, false) => {},
					}
				}
			}
			game.controls_to_trigger.clear();

			let mut interface_meshes_vertices = InterfaceMeshesVertices::new();

			// Top left info.
			if let Some(general_debug_info_widget) =
				game.widget_tree_root.find_label_content(WidgetLabel::GeneralDebugInfo)
			{
				let fps = 1.0 / dt.as_secs_f32();
				let chunk_count = game.chunk_grid.count_chunks_that_have_blocks();
				let block_count = chunk_count * game.cd.number_of_blocks();
				let chunk_meshed_count = game.chunk_grid.count_chunks_that_have_meshes();
				let player_block_coords = (game.player_phys.aligned_box().pos
					- cgmath::Vector3::<f32>::unit_z()
						* (game.player_phys.aligned_box().dims.z / 2.0 + 0.1))
					.map(|x| x.round() as i32);
				let player_block_coords_str = {
					let cgmath::Point3 { x, y, z } = player_block_coords;
					format!("{x},{y},{z}")
				};
				let (entity_count, chunk_entity_count) =
					game.chunk_grid.count_entities_and_chunks_that_have_entities();
				let seed = game.world_gen_seed;
				let random_message = game.random_message;
				let settings = font::TextRenderingSettings::with_scale(3.0);
				let text = format!(
					"fps: {fps}\n\
					chunks loaded: {chunk_count}\n\
					blocks loaded: {block_count}\n\
					chunks meshed: {chunk_meshed_count}\n\
					entities: {entity_count}\n\
					chunk with entities: {chunk_entity_count}\n\
					player coords: {player_block_coords_str}\n\
					seed: {seed}\n\
					{random_message}"
				);
				*general_debug_info_widget = Widget::new_simple_text(text, settings);
			}

			// Command line handling.
			if game.command_confirmed {
				let text = game.command_line_content.clone();

				let mut log = lang::Log::new();
				let res = lang::run(&text, &mut lang::Context::with_builtins(), &mut log);

				let text = if let Err(error) = res {
					format!("{error:?}")
				} else {
					let lines: Vec<_> = log
						.log_items
						.into_iter()
						.map(|item| match item {
							LogItem::Text(text) => text,
						})
						.collect();
					lines.join("\n")
				};

				let widget = if text.is_empty() {
					let scale = rand::thread_rng().gen_range(1..=3) as f32;
					let settings = font::TextRenderingSettings::with_scale(scale);
					let text = "uwu test".to_string();
					Widget::new_simple_text(text, settings)
				} else {
					let settings = font::TextRenderingSettings::with_scale(3.0);
					Widget::new_simple_text(text, settings)
				};

				if let Some(Widget::List { sub_widgets, .. }) =
					game.widget_tree_root.find_label_content(WidgetLabel::LogLineList)
				{
					sub_widgets.push(Widget::new_smoothly_incoming(
						cgmath::point2(0.0, 0.0),
						std::time::Instant::now(),
						std::time::Duration::from_secs_f32(1.0),
						Box::new(widget),
					));

					if sub_widgets.iter().filter(|widget| !widget.is_diappearing()).count() > 25 {
						let window_width = game.window_surface_config.width as f32;
						sub_widgets
							.iter_mut()
							.find(|widget| !widget.is_diappearing())
							.expect("we just checked that there are at least some amout of them")
							.pop_while_smoothly_closing_space(
								std::time::Instant::now(),
								std::time::Duration::from_secs_f32(1.0),
								&game.font,
								window_width,
							);
					}
				}

				game.command_line_content.clear();
				game.command_confirmed = false;
			}
			{
				let carret_blinking_speed = 1.5;
				let carret_blinking_visibility_ratio = 0.5;
				let carret_text_representation = "█";
				let carret_visible = game.typing_in_command_line
					&& game.last_command_line_interaction.is_some_and(|time| {
						(time.elapsed().as_secs_f32() * carret_blinking_speed).fract()
							< carret_blinking_visibility_ratio
					});
				let window_width = game.window_surface_config.width as f32;
				let command_line_content = game.command_line_content.as_str();
				let command_line_content_with_carret =
					command_line_content.to_string() + carret_text_representation;
				let settings = font::TextRenderingSettings::with_scale(4.0);
				let dimensions = game.font.dimensions_of_text(
					window_width,
					settings.clone(),
					command_line_content_with_carret.as_str(),
				);
				let y = 0.0 + dimensions.y / 2.0;
				let x = 0.0 - dimensions.x / 2.0;
				// Somehow this makes it pixel perfect, somehow?
				let x = (x * (window_width * 8.0) - 0.5).floor() / (window_width * 8.0);
				let text_displayed = if carret_visible {
					command_line_content_with_carret.as_str()
				} else {
					command_line_content
				};
				let simple_texture_vertices = game.font.simple_texture_vertices_from_text(
					window_width,
					cgmath::point3(x, y, 0.5),
					settings,
					text_displayed,
				);
				interface_meshes_vertices.add_simple_texture_vertices(simple_texture_vertices);
			}

			// Interface widget tree.
			{
				let window_width = game.window_surface_config.width as f32;
				let window_height = game.window_surface_config.height as f32;

				game.widget_tree_root.for_each_rec(&mut |widget| {
					if let Widget::DisappearWhenComplete {
						sub_widget,
						completed_time,
						delay_before_disappearing,
					} = widget
					{
						if sub_widget.is_completed() && completed_time.is_none() {
							*completed_time = Some(std::time::Instant::now());
						} else if completed_time.is_some_and(|completed_time| {
							completed_time.elapsed() > *delay_before_disappearing
						}) {
							widget.pop_while_smoothly_closing_space(
								std::time::Instant::now(),
								std::time::Duration::from_secs_f32(0.5),
								&game.font,
								window_width,
							);
						}
					}
				});

				game.widget_tree_root.generate_mesh_vertices(
					cgmath::point3(-1.0, window_height / window_width, 0.5),
					&mut interface_meshes_vertices,
					&game.font,
					window_width,
					game.enable_interface_draw_debug_boxes,
				);
			}

			// Recieve task results from workers.
			game.worker_tasks.tasks.retain_mut(|worker_task| {
				let is_not_done_yet = match worker_task {
					WorkerTask::LoadChunkBlocksAndEntities(chunk_coords, receiver) => {
						let chunk_coords_and_result_opt = receiver.try_recv().ok().map(
							|(chunk_blocks, chunk_culling_info, chunk_entities)| {
								(
									*chunk_coords,
									chunk_blocks,
									chunk_culling_info,
									chunk_entities,
								)
							},
						);
						let is_not_done_yet = chunk_coords_and_result_opt.is_none();
						if let Some((chunk_coords, chunk_blocks, chunk_culling_info, chunk_entities)) =
							chunk_coords_and_result_opt
						{
							game.loading_manager.handle_chunk_loading_results(
								chunk_coords,
								chunk_blocks,
								chunk_culling_info,
								chunk_entities,
								&mut game.chunk_grid,
							);
						}
						is_not_done_yet
					},
					WorkerTask::MeshChunk(chunk_coords, receiver) => {
						let chunk_coords_and_result_opt =
							receiver.try_recv().ok().map(|chunk_mesh| (*chunk_coords, chunk_mesh));
						let is_not_done_yet = chunk_coords_and_result_opt.is_none();
						if let Some((chunk_coords, chunk_mesh)) = chunk_coords_and_result_opt {
							game.chunk_grid.add_chunk_meshing_results(chunk_coords, chunk_mesh);
						}
						is_not_done_yet
					},
					WorkerTask::PaintNewSkybox(receiver, _face_counter) => {
						let result_opt = receiver.try_recv().ok();
						let is_not_done_yet = result_opt.is_none();
						if let Some(skybox_faces) = result_opt {
							if let Some(save) = game.save.as_ref() {
								skybox_faces.save(save);
							}
							update_skybox_texture(
								&game.queue,
								&game.skybox_cubemap_texture,
								&skybox_faces.data(),
							);
						}
						is_not_done_yet
					},
					WorkerTask::GenerateAtlas(receiver) => {
						let result_opt = receiver.try_recv().ok();
						let is_not_done_yet = result_opt.is_none();
						if let Some(completed_atlas) = result_opt {
							if game.output_atlas_when_generated {
								let path = "atlas.png";
								println!("Outputting atlas to \"{path}\"");
								completed_atlas
									.image
									.save_with_format(path, image::ImageFormat::Png)
									.unwrap();
							}
							if let Some(save) = game.save.as_ref() {
								completed_atlas.save(save);
							}
							update_atlas_texture(
								&game.queue,
								&game.atlas_texture,
								&completed_atlas.image.as_ref(),
							);
						}
						is_not_done_yet
					},
				};
				is_not_done_yet
			});

			// Request meshing for chunks that can be meshed or should be re-meshed.
			game.chunk_grid.run_some_required_remeshing_tasks(
				&mut game.worker_tasks,
				&mut game.pool,
				&game.block_type_table,
				&game.font,
				&game.device,
			);

			// Handle fog adjustment.
			// Current fog fix,
			// works fine when the loading of chunks is finished or almost finished.
			let sqrt_3 = 3.0_f32.sqrt();
			let distance = game.loading_manager.loading_distance - game.cd.edge as f32 * sqrt_3 / 2.0;
			game.fog_inf_sup_radiuses.1 = distance.max(game.fog_margin);
			game.fog_inf_sup_radiuses.0 = game.fog_inf_sup_radiuses.1 - game.fog_margin;
			if game.enable_fog {
				game.queue.write_buffer(
					&game.fog_inf_sup_radiuses_thingy.resource,
					0,
					bytemuck::cast_slice(&[Vector2Pod {
						values: [game.fog_inf_sup_radiuses.0, game.fog_inf_sup_radiuses.1],
					}]),
				);
			}

			// Request generation of chunk blocks for not-generated not-being-generated close chunks.
			let player_chunk = game.player_chunk();
			game.loading_manager.handle_loading(
				&mut game.chunk_grid,
				&mut game.worker_tasks,
				&mut game.pool,
				player_chunk,
				&game.world_generator,
				&game.block_type_table,
				game.save.as_ref(),
			);

			// Unload chunks that are a bit too far.
			let unloading_distance =
				game.loading_manager.loading_distance + game.loading_manager.margin_before_unloading;
			game.chunk_grid.unload_chunks_too_far(
				game.player_chunk(),
				unloading_distance,
				game.save.as_ref(),
				game.only_save_modified_chunks,
			);

			// Walking.
			let walking_vector = {
				let walking_factor = if game.enable_physics { 12.0 } else { 35.0 } * dt.as_secs_f32();
				let walking_forward_factor = if game.walking_forward { 1 } else { 0 }
					+ if game.walking_backward { -1 } else { 0 };
				let walking_rightward_factor = if game.walking_rightward { 1 } else { 0 }
					+ if game.walking_leftward { -1 } else { 0 };
				let walking_forward_direction =
					game.camera_direction.to_horizontal().to_vec3() * walking_forward_factor as f32;
				let walking_rightward_direction =
					game.camera_direction.to_horizontal().add_to_horizontal_angle(-TAU / 4.0).to_vec3()
						* walking_rightward_factor as f32;
				let walking_vector_direction = walking_forward_direction + walking_rightward_direction;
				(if walking_vector_direction.magnitude() == 0.0 {
					walking_vector_direction
				} else {
					walking_vector_direction.normalize()
				} * walking_factor)
			};
			game.player_phys.walk(walking_vector, !game.enable_physics);

			if game.enable_physics {
				game.player_phys.apply_one_physics_step(&game.chunk_grid, &game.block_type_table, dt);
			}

			game.chunk_grid.apply_one_physics_step(&game.block_type_table, dt);

			game.queue.write_buffer(
				&game.fog_center_position_thingy.resource,
				0,
				bytemuck::cast_slice(&[Vector3Pod {
					values: game.player_phys.aligned_box().pos.into(),
				}]),
			);

			let player_box_mesh =
				SimpleLineMesh::from_aligned_box(&game.device, game.player_phys.aligned_box());

			let player_blocks_box_mesh = SimpleLineMesh::from_aligned_box(
				&game.device,
				&game.player_phys.aligned_box().overlapping_block_coords_span().to_aligned_box(),
			);

			let mut entities_box_meshes = vec![];
			for entity in game.chunk_grid.iter_entities() {
				if let Some(aligned_box) = entity.aligned_box() {
					entities_box_meshes
						.push(SimpleLineMesh::from_aligned_box(&game.device, &aligned_box));
				}
			}

			let first_person_camera_position = game.player_phys.aligned_box().pos
				+ cgmath::Vector3::<f32>::from((0.0, 0.0, game.player_phys.aligned_box().dims.z / 2.0))
					* 0.7;

			// Targeted block coords update.
			let direction = game.camera_direction.to_vec3();
			let mut position = first_person_camera_position;
			let mut last_position_int: Option<BlockCoords> = None;
			game.targeted_block_coords = loop {
				if first_person_camera_position.distance(position) > 6.0 {
					break None;
				}
				let position_int = position.map(|x| x.round() as i32);
				if game
					.chunk_grid
					.get_block(position_int)
					.is_some_and(|block| !game.block_type_table.get(block.type_id).unwrap().is_air())
				{
					if let Some(last_position_int) = last_position_int {
						break Some((position_int, last_position_int));
					} else {
						break None;
					}
				}
				if last_position_int != Some(position_int) {
					last_position_int = Some(position_int);
				}
				// TODO: Advance directly to the next block with exactly the right step distance,
				// also do not skip blocks (even a small arbitrary step can be too big sometimes).
				position += direction * 0.01;
			};

			let targeted_block_box_mesh_opt = game.targeted_block_coords.map(|(coords, _)| {
				SimpleLineMesh::from_aligned_box(
					&game.device,
					&AlignedBox {
						pos: coords.map(|x| x as f32),
						dims: cgmath::vec3(1.01, 1.01, 1.01),
					},
				)
			});

			let mut chunk_box_meshes = vec![];
			if game.enable_display_not_surrounded_chunks_as_boxes {
				for chunk_coords in game.chunk_grid.iter_loaded_chunk_coords() {
					let is_surrounded = 'is_surrounded: {
						for neighbor_chunk_coords in iter_3d_cube_center_radius(chunk_coords, 2) {
							let blocks_was_generated = game.chunk_grid.is_loaded(neighbor_chunk_coords);
							if !blocks_was_generated {
								break 'is_surrounded false;
							}
						}
						true
					};
					if !is_surrounded {
						let coords_span = ChunkCoordsSpan { cd: game.cd, chunk_coords };
						let inf = coords_span.block_coords_inf().map(|x| x as f32);
						let dims = coords_span.cd._dimensions().map(|x| x as f32 - 1.0);
						let pos = inf + dims / 2.0;
						chunk_box_meshes.push(SimpleLineMesh::from_aligned_box(
							&game.device,
							&AlignedBox { pos, dims },
						));
					}
				}
			}

			let mut chunk_with_entities_box_meshes = vec![];
			if game.enable_display_chunks_with_entities_as_boxes {
				for chunk_coords in game.chunk_grid.iter_chunk_whith_entities_coords() {
					let coords_span = ChunkCoordsSpan { cd: game.cd, chunk_coords };
					let inf = coords_span.block_coords_inf().map(|x| x as f32);
					let dims = coords_span.cd._dimensions().map(|x| x as f32 - 1.0);
					let pos = inf + dims / 2.0;
					chunk_with_entities_box_meshes.push(SimpleLineMesh::from_aligned_box(
						&game.device,
						&AlignedBox { pos, dims },
					));
				}
			}

			game.sun_position_in_sky.angle_horizontal += (TAU / 150.0) * dt.as_secs_f32();

			let sun_camera_view_projection_matrices: Vec<_> = game
				.sun_cameras
				.iter()
				.map(|camera| {
					let camera_position = first_person_camera_position;
					let camera_direction_vector = -game.sun_position_in_sky.to_vec3();
					let camera_up_vector = (0.0, 0.0, 1.0).into();
					camera.view_projection_matrix(
						camera_position,
						camera_direction_vector,
						camera_up_vector,
					)
				})
				.collect();
			game.queue.write_buffer(
				&game.sun_camera_matrices_thingy.resource,
				0,
				bytemuck::cast_slice(&sun_camera_view_projection_matrices),
			);

			let (camera_view_projection_matrix, camera_position_ifany) = {
				if matches!(game.selected_camera, WhichCameraToUse::Sun) {
					(sun_camera_view_projection_matrices[0], None)
				} else {
					let mut camera_position = first_person_camera_position;
					let camera_direction_vector = game.camera_direction.to_vec3();
					match game.selected_camera {
						WhichCameraToUse::FirstPerson | WhichCameraToUse::Sun => {},
						WhichCameraToUse::ThirdPersonNear => {
							camera_position -= camera_direction_vector * 5.0;
						},
						WhichCameraToUse::ThirdPersonFar => {
							camera_position -= camera_direction_vector * 40.0;
						},
						WhichCameraToUse::ThirdPersonVeryFar => {
							camera_position -= camera_direction_vector * 200.0;
						},
						WhichCameraToUse::ThirdPersonTooFar => {
							camera_position -= camera_direction_vector
								* (game.loading_manager.loading_distance + 250.0).max(300.0);
						},
					}
					let camera_up_vector =
						game.camera_direction.add_to_vertical_angle(-TAU / 4.0).to_vec3();
					let camera_view_projection_matrix = game.camera_settings.view_projection_matrix(
						camera_position,
						camera_direction_vector,
						camera_up_vector,
					);
					(camera_view_projection_matrix, Some(camera_position))
				}
			};
			game.queue.write_buffer(
				&game.camera_matrix_thingy.resource,
				0,
				bytemuck::cast_slice(&[camera_view_projection_matrix]),
			);

			let skybox_mesh = SkyboxMesh::new(
				&game.device,
				camera_position_ifany.unwrap_or(point3(0.0, 0.0, 0.0)),
			);

			let sun_light_direction =
				Vector3Pod { values: (-game.sun_position_in_sky.to_vec3()).into() };
			game.queue.write_buffer(
				&game.sun_light_direction_thingy.resource,
				0,
				bytemuck::cast_slice(&[sun_light_direction]),
			);

			let interface_simple_texture_mesh = SimpleTextureMesh::from_vertices(
				&game.device,
				interface_meshes_vertices.simple_texture_vertices,
			);
			let interface_simple_line_mesh = SimpleLineMesh::from_vertices(
				&game.device,
				interface_meshes_vertices.simple_line_vertices,
			);

			let data_for_rendering = rendering::DataForRendering {
				device: &game.device,
				queue: &game.queue,
				window_surface: &game.window_surface,
				window_surface_config: &game.window_surface_config,
				rendering: &game.rendering,
				sun_cameras: &game.sun_cameras,
				sun_camera_matrices_thingy: &game.sun_camera_matrices_thingy,
				sun_camera_single_matrix_thingy: &game.sun_camera_single_matrix_thingy,
				shadow_map_cascade_view_thingies: &game.shadow_map_cascade_view_thingies,
				chunk_grid: &game.chunk_grid,
				z_buffer_view: &game.z_buffer_view,
				selected_camera: game.selected_camera,
				enable_display_phys_box: game.enable_display_phys_box,
				player_box_mesh: &player_box_mesh,
				player_blocks_box_mesh: &player_blocks_box_mesh,
				entities_box_meshes: &entities_box_meshes,
				chunk_with_entities_box_meshes: &chunk_with_entities_box_meshes,
				targeted_block_box_mesh_opt: &targeted_block_box_mesh_opt,
				enable_display_interface: game.enable_display_interface,
				chunk_box_meshes: &chunk_box_meshes,
				skybox_mesh: &skybox_mesh,
				typing_in_command_line: game.typing_in_command_line,
				cursor_mesh: &game.cursor_mesh,
				interface_simple_texture_mesh: &interface_simple_texture_mesh,
				interface_simple_line_mesh: &interface_simple_line_mesh,
			};
			data_for_rendering.render();

			// Limit FPS if asked for and needed.
			if let Some(max_fps) = game.max_fps {
				let time_at_start_of_iteration = game.time_from_last_iteration;
				let iteration_duration = time_at_start_of_iteration.elapsed();
				let min_iteration_duration = std::time::Duration::from_secs_f32(1.0 / max_fps as f32);
				let sleep_time_if_any = min_iteration_duration.checked_sub(iteration_duration);
				if let Some(sleep_time) = sleep_time_if_any {
					std::thread::sleep(sleep_time);
				}
			}

			if game.close_after_one_frame {
				println!("Closing after one frame, as asked via command line arguments");
				elwt.exit();
			}
		},

		Event::LoopExiting => {
			if game.save.is_some() {
				save_savable_state(&game);
				game.chunk_grid.unload_all_chunks(game.save.as_ref(), game.only_save_modified_chunks);
			}

			//game.window.set_visible(false);
			//game.pool._end_blocking();
		},

		_ => {},
	});
	res.unwrap();
}
