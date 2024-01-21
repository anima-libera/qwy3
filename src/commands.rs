use std::{collections::HashMap, io::Write};

pub(crate) use crate::{Action, Control};

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
						if signle_char_key_name.is_ascii_alphabetic() {
							Control::KeyboardKey(letter_to_keycode(signle_char_key_name))
						} else if signle_char_key_name.is_ascii_digit() {
							Control::KeyboardKey(digit_to_keycode(signle_char_key_name))
						} else {
							panic!("unknown signle character key name \"{signle_char_key_name}\"")
						}
					} else {
						match key_name {
							"up" => Control::KeyboardKey(VirtualKeyCode::Up),
							"down" => Control::KeyboardKey(VirtualKeyCode::Down),
							"left" => Control::KeyboardKey(VirtualKeyCode::Left),
							"right" => Control::KeyboardKey(VirtualKeyCode::Right),
							"space" => Control::KeyboardKey(VirtualKeyCode::Space),
							"left_shift" => Control::KeyboardKey(VirtualKeyCode::LShift),
							"right_shift" => Control::KeyboardKey(VirtualKeyCode::RShift),
							"tab" => Control::KeyboardKey(VirtualKeyCode::Tab),
							"return" | "enter" => Control::KeyboardKey(VirtualKeyCode::Return),
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

fn letter_to_keycode(letter: char) -> winit::event::VirtualKeyCode {
	use winit::event::VirtualKeyCode as K;
	#[rustfmt::skip]
	let keycode = match letter.to_ascii_uppercase() {
		'A' => K::A, 'B' => K::B, 'C' => K::C, 'D' => K::D, 'E' => K::E, 'F' => K::F, 'G' => K::G,
		'H' => K::H, 'I' => K::I, 'J' => K::J, 'K' => K::K, 'L' => K::L, 'M' => K::M, 'N' => K::N,
		'O' => K::O, 'P' => K::P, 'Q' => K::Q, 'R' => K::R, 'S' => K::S, 'T' => K::T, 'U' => K::U,
		'V' => K::V, 'W' => K::W, 'X' => K::X, 'Y' => K::Y, 'Z' => K::Z,
		not_a_letter => panic!("can't convert \"{not_a_letter}\" to an ascii letter keycode"),
	};
	keycode
}

fn digit_to_keycode(digit: char) -> winit::event::VirtualKeyCode {
	use winit::event::VirtualKeyCode as K;
	#[rustfmt::skip]
	let keycode = match digit {
		'0' => K::Key0, '1' => K::Key1, '2' => K::Key2, '3' => K::Key3, '4' => K::Key4,
		'5' => K::Key5, '6' => K::Key6, '7' => K::Key7, '8' => K::Key8, '9' => K::Key9,
		not_a_digit => panic!("can't convert \"{not_a_digit}\" to an digit keycode"),
	};
	keycode
}
