//! Font handling and text rendering.

use std::collections::HashMap;

use crate::{
	shaders::simple_texture_2d::SimpleTextureVertexPod, unsorted::RectInAtlas,
	unsorted::SimpleTextureMesh,
};

#[derive(Clone)]
struct CharacterDetails {
	rect_in_atlas: RectInAtlas,
	dimensions_in_pixels: cgmath::Vector2<i32>,
}

pub(crate) struct Font {
	character_details_map: HashMap<char, CharacterDetails>,
	/// Character details for the special character that is used to represent
	/// erroneous or unsupported characters.
	error_character_detials: CharacterDetails,
	max_character_height_in_pixels: i32,
}

impl Font {
	pub(crate) fn _font_01() -> Font {
		let mut character_details_map = HashMap::new();

		let coords_asset_to_details = |x: i32, y: i32, w: i32, h: i32| -> CharacterDetails {
			let y = y + 32; // The assert image is loaded into y=32+ area of the atlas.
			CharacterDetails {
				rect_in_atlas: RectInAtlas {
					texture_rect_in_atlas_xy: cgmath::point2(x as f32, y as f32) * (1.0 / 512.0),
					texture_rect_in_atlas_wh: cgmath::vec2(w as f32, h as f32) * (1.0 / 512.0),
				},
				dimensions_in_pixels: cgmath::vec2(w, h),
			}
		};

		const PUNCT_1: &str = "|.:!";
		const PUNCT_2: &str = ",;'[]()`";
		const PUNCT_3: &str = "_/\\%#\"^{}?*+-=@<>¨~°";
		let row_height = 5 + 1;

		for letter in "abcdefghijklmnopqrstuvwxyz".chars() {
			// First row from the bottom in the spritesheet, case insensitive, a few letters are wider.
			let mut x = (letter as i32 - 'a' as i32) * 4;
			let mut w = 3;
			for (wider_letter, how_much_wider) in [('m', 2), ('n', 1), ('q', 1), ('w', 2)] {
				use std::cmp::Ordering;
				match Ord::cmp(&letter, &wider_letter) {
					Ordering::Less => {},
					Ordering::Equal => w += how_much_wider,
					Ordering::Greater => x += how_much_wider,
				}
			}
			let details = coords_asset_to_details(x, row_height * 3, w, 5);
			character_details_map.insert(letter, details.clone());
			character_details_map.insert(letter.to_ascii_uppercase(), details);
		}

		for digit in "0123456789".chars() {
			// Second row from the bottom.
			let x = (digit as i32 - '0' as i32) * 4;
			let details = coords_asset_to_details(x, row_height * 2, 3, 5);
			character_details_map.insert(digit, details);
		}

		for (index, punctuation) in PUNCT_1.chars().enumerate() {
			// Beginning of the forth row from the bottom, for 1-pixel-wide special characters.
			let details = coords_asset_to_details(index as i32 * 2, 0, 1, 5);
			character_details_map.insert(punctuation, details);
		}
		for (index, punctuation) in PUNCT_2.chars().enumerate() {
			// End of the forth row from the bottom, for 2-pixel-wide special characters.
			let x = PUNCT_1.len() as i32 * 2 + index as i32 * 3;
			let details = coords_asset_to_details(x, 0, 2, 5);
			character_details_map.insert(punctuation, details);
		}

		for (index, punctuation) in PUNCT_3.chars().enumerate() {
			// Third row from the bottom, reserved for 3-pixel-wide special characters.
			let details = coords_asset_to_details(index as i32 * 4, row_height, 3, 5);
			character_details_map.insert(punctuation, details);
		}

		let block_character_details = coords_asset_to_details(109, 0, 3, 5);
		character_details_map.insert('█', block_character_details.clone());
		let error_character_detials = block_character_details;

		let max_character_height_in_pixels = 5;

		Font {
			character_details_map,
			error_character_detials,
			max_character_height_in_pixels,
		}
	}

	pub(crate) fn font_02() -> Font {
		let mut character_details_map = HashMap::new();

		let coords_asset_to_details = |x: i32, y: i32, w: i32, h: i32| -> CharacterDetails {
			let y = y + 32; // The assert image is loaded into y=32+ area of the atlas.
			CharacterDetails {
				rect_in_atlas: RectInAtlas {
					texture_rect_in_atlas_xy: cgmath::point2(x as f32, y as f32) * (1.0 / 512.0),
					texture_rect_in_atlas_wh: cgmath::vec2(w as f32, h as f32) * (1.0 / 512.0),
				},
				dimensions_in_pixels: cgmath::vec2(w, h),
			}
		};

		{
			let upper_y = 0;
			let lower_y = 8;
			let mut x = 0;
			for letter_lower in "abcdefghijklmnopqrstuvwxyz".chars() {
				let letter_upper = letter_lower.to_ascii_uppercase();

				let upper_w = match letter_upper {
					'I' => 1,
					'J' => 3,
					'M' | 'T' | 'W' | 'X' => 5,
					letter if letter.is_ascii_uppercase() => 4,
					other => panic!("Only uppercase ascii letters are handled here, not '{other}'"),
				};
				let lower_w = match letter_lower {
					'i' => 1,
					'l' => 2,
					'f' | 'j' | 'k' | 's' | 't' | 'z' => 3,
					'm' | 'w' => 5,
					letter if letter.is_ascii_lowercase() => 4,
					other => panic!("Only lowercase ascii letters are handled here, not '{other}'"),
				};

				character_details_map.insert(
					letter_upper,
					coords_asset_to_details(x, upper_y, upper_w, 7),
				);
				character_details_map.insert(
					letter_lower,
					coords_asset_to_details(x, lower_y, lower_w, 7),
				);

				x += upper_w + 1;
			}
		}

		{
			let chars_and_widths = [
				('0', 4),
				('1', 3),
				('2', 4),
				('3', 4),
				('4', 4),
				('5', 4),
				('6', 4),
				('7', 4),
				('8', 4),
				('9', 4),
				('.', 1),
				(':', 1),
				('!', 1),
				('|', 1),
				(',', 2),
				(';', 2),
				('`', 2),
				('\'', 2),
				('[', 2),
				(']', 2),
				('(', 2),
				(')', 2),
				('_', 4),
				('/', 4),
				('\\', 4),
				('{', 3),
				('}', 3),
				('%', 4),
				('#', 5),
				('\"', 3),
				('^', 3),
				('?', 4),
				('+', 3),
				('-', 3),
				('*', 3),
				('=', 3),
				('@', 4),
				('<', 3),
				('>', 3),
				('¨', 3),
				('~', 4),
				('°', 3),
				('█', 4),
				('�', 4),
			];
			let y = 16;
			let mut x = 0;
			for (character, width) in chars_and_widths {
				character_details_map.insert(character, coords_asset_to_details(x, y, width, 7));
				x += width + 1;
			}
		}

		let error_character_detials = character_details_map[&'�'].clone();

		let max_character_height_in_pixels = 7;

		Font {
			character_details_map,
			error_character_detials,
			max_character_height_in_pixels,
		}
	}

	fn character_details(&self, character: char) -> Option<CharacterDetails> {
		self.character_details_map.get(&character).cloned()
	}

	/// Returns (width, height) in screen pixels of the smallest rect
	/// that would contain the rendering of the given text.
	pub(crate) fn dimensions_of_text(
		&self,
		window_width: f32,
		settings: TextRenderingSettings,
		text: &str,
	) -> cgmath::Vector2<f32> {
		// Size of a screen pixel in Wgpu/Vulkan XY-plane coordinate space.
		// It would be `1.0 / window_width` if the coord space would go from 0.0 to 1.0,
		// but since it goes from -1.0 to 1.0 then it is twice as big so we account for that.
		let screen_pixel_size = 2.0 / window_width;

		let mut max_width = 0.0f32;
		let mut max_height = 0.0f32;

		let mut coords = cgmath::point2(0.0, 0.0);
		for character in text.chars() {
			if character == ' ' {
				coords.x += settings.space_character_scaled_width * screen_pixel_size * settings.scale;
				max_width = max_width.max(coords.x);
			} else if character == '\n' {
				coords.x = 0.0;
				coords.y -=
					self.max_character_height_in_pixels as f32 * screen_pixel_size * settings.scale
						+ settings.inbetween_lines_space_height * screen_pixel_size;
			} else {
				let character_details =
					self.character_details(character).unwrap_or(self.error_character_detials.clone());
				let dimensions = character_details.dimensions_in_pixels.map(|x| x as f32)
					* screen_pixel_size
					* settings.scale;
				max_width = max_width.max(coords.x + dimensions.x);
				max_height = max_height.max(-coords.y + dimensions.y);
				coords.x +=
					character_details.dimensions_in_pixels.x as f32 * screen_pixel_size * settings.scale
						+ settings.inbetween_characters_space_width * screen_pixel_size;
			}
		}

		cgmath::vec2(max_width, max_height)
	}

	pub(crate) fn simple_texture_vertices_from_text(
		&self,
		window_width: f32,
		mut coords: cgmath::Point3<f32>,
		settings: TextRenderingSettings,
		text: &str,
	) -> Vec<SimpleTextureVertexPod> {
		// Size of a screen pixel in Wgpu/Vulkan XY-plane coordinate space.
		// It would be `1.0 / window_width` if the coord space would go from 0.0 to 1.0,
		// but since it goes from -1.0 to 1.0 then it is twice as big so we account for that.
		let screen_pixel_size = 2.0 / window_width;

		let initial_coords = coords;
		let mut vertices = vec![];
		for character in text.chars() {
			if character == ' ' {
				coords.x += settings.space_character_scaled_width * screen_pixel_size * settings.scale;
			} else if character == '\n' {
				coords.x = initial_coords.x;
				coords.y -=
					self.max_character_height_in_pixels as f32 * screen_pixel_size * settings.scale
						+ settings.inbetween_lines_space_height * screen_pixel_size;
			} else {
				let character_details =
					self.character_details(character).unwrap_or(self.error_character_detials.clone());
				let dimensions = character_details.dimensions_in_pixels.map(|x| x as f32)
					* screen_pixel_size
					* settings.scale;
				vertices.extend(SimpleTextureMesh::vertices_for_rect(
					coords,
					dimensions,
					character_details.rect_in_atlas.texture_rect_in_atlas_xy,
					character_details.rect_in_atlas.texture_rect_in_atlas_wh,
					settings.color,
				));
				coords.x +=
					character_details.dimensions_in_pixels.x as f32 * screen_pixel_size * settings.scale
						+ settings.inbetween_characters_space_width * screen_pixel_size;
			}
		}

		vertices
	}

	pub(crate) fn _simple_texture_mesh_from_text(
		&self,
		device: &wgpu::Device,
		window_width: f32,
		coords: cgmath::Point3<f32>,
		settings: TextRenderingSettings,
		text: &str,
	) -> SimpleTextureMesh {
		let vertices = self.simple_texture_vertices_from_text(window_width, coords, settings, text);
		SimpleTextureMesh::from_vertices(device, vertices)
	}
}

#[derive(Clone)]
pub(crate) struct TextRenderingSettings {
	/// Factor by which are stretched the character textures.
	/// Should be integer values or else it won't render pixel-perfect ><.
	pub(crate) scale: f32,
	/// In screen pixels times `scale`.
	pub(crate) space_character_scaled_width: f32,
	/// In screen pixels.
	pub(crate) inbetween_characters_space_width: f32,
	/// In screen pixels.
	pub(crate) inbetween_lines_space_height: f32,
	pub(crate) color: [f32; 3],
}

impl TextRenderingSettings {
	pub(crate) fn with_scale(scale: f32) -> TextRenderingSettings {
		TextRenderingSettings {
			scale,
			space_character_scaled_width: 3.0,
			inbetween_characters_space_width: scale,
			inbetween_lines_space_height: scale,
			color: [0.0, 0.0, 0.0],
		}
	}
}
