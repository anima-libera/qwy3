use std::{collections::HashMap, io::Write};

use crate::unsorted::{Action, Control};

pub(crate) fn parse_control_binding_file() -> HashMap<Control, Action> {
	let mut control_bindings: HashMap<Control, Action> = HashMap::new();

	let command_file_path = "controls.qwy3_controls";
	if !std::path::Path::new(command_file_path).is_file() {
		let mut file =
			std::fs::File::create(command_file_path).expect("count not create config file");
		file
			.write_all(include_str!("default_controls.qwy3_controls").as_bytes())
			.expect("could not fill the default config in the new config file");
	}

	use winit::event::*;
	use winit::keyboard::*;
	if let Ok(controls_config_string) = std::fs::read_to_string(command_file_path) {
		for (line_index, line) in controls_config_string.lines().enumerate() {
			let line_number = line_index + 1;
			let mut words = line.split_whitespace();
			let command_name = words.next();
			if command_name == Some("bind_control") {
				let control_name = words.next().expect("expected control name");
				let action_name = words.next().expect("expected action name");

				let control = if let Some(key_name) = control_name.strip_prefix("key:") {
					if key_name.chars().count() == 1 {
						let signle_char_key_name = key_name.chars().next().unwrap();
						if signle_char_key_name.is_alphabetic() || signle_char_key_name.is_ascii_digit() {
							let string = signle_char_key_name.to_lowercase().to_string();
							Control::KeyboardKey(Key::Character(SmolStr::new(string)))
						} else {
							panic!("unknown signle character key name \"{signle_char_key_name}\"")
						}
					} else if let Some(f_key_keycode) = try_paring_f_key(key_name) {
						Control::KeyboardKey(f_key_keycode)
					} else {
						match key_name {
							"up" => Control::KeyboardKey(Key::Named(NamedKey::ArrowUp)),
							"down" => Control::KeyboardKey(Key::Named(NamedKey::ArrowDown)),
							"left" => Control::KeyboardKey(Key::Named(NamedKey::ArrowLeft)),
							"right" => Control::KeyboardKey(Key::Named(NamedKey::ArrowRight)),
							"space" => Control::KeyboardKey(Key::Named(NamedKey::Space)),
							"left_shift" | "right_shift" => {
								// TODO: Add a `winit::keyboardKeyLocation` to `Control::KeyboardKey`
								// to reintroduce the difference between these two keys.
								println!(
									"\x1b[33mWarning in file \"{command_file_path}\" at line {line_number}: \
									The \"left_shift\" and \"right_shift\" key names both refer to both keys
									for now (this will be fixed at some point)\x1b[39m"
								);
								Control::KeyboardKey(Key::Named(NamedKey::Shift))
							},
							"tab" => Control::KeyboardKey(Key::Named(NamedKey::Tab)),
							"return" | "enter" => Control::KeyboardKey(Key::Named(NamedKey::Enter)),
							unknown_key_name => panic!("unknown key name \"{unknown_key_name}\""),
						}
					}
				} else if let Some(button_name) = control_name.strip_prefix("mouse_button:") {
					if button_name == "left" {
						Control::MouseButton(MouseButton::Left)
					} else if button_name == "right" {
						Control::MouseButton(MouseButton::Right)
					} else if button_name == "middle" {
						Control::MouseButton(MouseButton::Middle)
					} else if let Ok(number) = button_name.parse() {
						Control::MouseButton(MouseButton::Other(number))
					} else {
						panic!("unknown mouse button name \"{button_name}\"")
					}
				} else {
					panic!(
						"unknown control \"{control_name}\" \
						(it must start with \"key:\" or \"mouse_button:\")"
					)
				};

				let action = match action_name {
					"walk_forward" => Action::WalkForward,
					"walk_backward" => Action::WalkBackward,
					"walk_leftward" => Action::WalkLeftward,
					"walk_rightward" => Action::WalkRightward,
					"jump" => Action::Jump,
					"toggle_physics" => Action::TogglePhysics,
					"toggle_world_generation" => Action::ToggleWorldGeneration,
					"cycle_first_and_third_person_views" => Action::CycleFirstAndThirdPersonViews,
					"toggle_display_player_box" => Action::ToggleDisplayPlayerBox,
					"toggle_sun_view" => Action::ToggleSunView,
					"toggle_cursor_captured" => Action::ToggleCursorCaptured,
					"print_coords" => Action::PrintCoords,
					"place_or_remove_block_under_player" => Action::PlaceOrRemoveBlockUnderPlayer,
					"place_block_at_target" => Action::PlaceBlockAtTarget,
					"remove_block_at_target" => Action::RemoveBlockAtTarget,
					"toggle_display_interface" => Action::ToggleDisplayInterface,
					"open_command_line" => Action::OpenCommandLine,
					"toggle_display_not_surrounded_chunks_as_boxes" => {
						Action::ToggleDisplayNotSurroundedChunksAsBoxes
					},
					"toggle_display_interfaces_debug_boxes" => Action::ToggleDisplayInterfaceDebugBoxes,
					"toggle_fog" => Action::ToggleFog,
					"toggle_fullscreen" => Action::ToggleFullscreen,
					"throw_block" => Action::ThrowBlock,
					"toggle_display_chunks_with_entities_as_boxes" => {
						Action::ToggleDisplayChunksWithEntitiesAsBoxes
					},
					"toggle_third_person_view" => {
						println!(
							"\x1b[33mWarning in file \"{command_file_path}\" at line {line_number}: \
							The \"toggle_third_person_view\" action name is deprecated \
							and should be replaced by \"cycle_first_and_third_person_views\" to better \
							express the new behavior of this action\x1b[39m"
						);
						Action::CycleFirstAndThirdPersonViews
					},
					unknown_action_name => panic!("unknown action name \"{unknown_action_name}\""),
				};
				control_bindings.insert(control, action);
			} else if let Some(unknown_command_name) = command_name {
				println!(
					"Error in file \"{command_file_path}\" at line {line_number}: \
					Command name \"{unknown_command_name}\" is unknown"
				);
			}
		}
	} else {
		println!("Couldn't read file \"{command_file_path}\"");
	}

	control_bindings
}

/// Parsing key names like "F11" to its proper key code.
fn try_paring_f_key(key_name: &str) -> Option<winit::keyboard::Key> {
	let f_number = key_name.strip_prefix('F').or_else(|| key_name.strip_prefix('f'))?;
	let number: u32 = f_number.parse().ok()?;
	use winit::keyboard::NamedKey as K;
	use Some as S;
	#[rustfmt::skip]
	let keycode = match number {
		1 => S(K::F1), 2 => S(K::F2), 3 => S(K::F3), 4 => S(K::F4),
		5 => S(K::F5), 6 => S(K::F6), 7 => S(K::F7), 8 => S(K::F8),
		9 => S(K::F9), 10 => S(K::F10), 11 => S(K::F11), 12 => S(K::F12),
		_ => None,
	};
	keycode.map(winit::keyboard::Key::Named)
}
