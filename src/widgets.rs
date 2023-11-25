use cgmath::EuclideanSpace;

use crate::font;
use crate::{SimpleLineVertexPod, SimpleTextureVertexPod};

fn simple_line_vertices_for_rect(
	top_left: cgmath::Point3<f32>,
	dimensions: cgmath::Vector2<f32>,
	color: [f32; 3],
) -> Vec<SimpleLineVertexPod> {
	let mut vertices = vec![];
	let a = top_left + cgmath::vec3(0.0, 0.0, 0.0);
	let b = top_left + cgmath::vec3(dimensions.x, 0.0, 0.0);
	let c = top_left + cgmath::vec3(0.0, -dimensions.y, 0.0);
	let d = top_left + cgmath::vec3(dimensions.x, -dimensions.y, 0.0);
	vertices.push(SimpleLineVertexPod { position: a.into(), color });
	vertices.push(SimpleLineVertexPod { position: b.into(), color });
	vertices.push(SimpleLineVertexPod { position: b.into(), color });
	vertices.push(SimpleLineVertexPod { position: d.into(), color });
	vertices.push(SimpleLineVertexPod { position: d.into(), color });
	vertices.push(SimpleLineVertexPod { position: c.into(), color });
	vertices.push(SimpleLineVertexPod { position: c.into(), color });
	vertices.push(SimpleLineVertexPod { position: a.into(), color });
	vertices
}

fn simple_line_vertices_for_diamond(
	center: cgmath::Point3<f32>,
	dimensions: cgmath::Vector2<f32>,
	color: [f32; 3],
) -> Vec<SimpleLineVertexPod> {
	let mut vertices = vec![];
	let a = center + cgmath::vec3(0.0, dimensions.y, 0.0) / 2.0;
	let b = center + cgmath::vec3(dimensions.x, 0.0, 0.0) / 2.0;
	let c = center + cgmath::vec3(0.0, -dimensions.y, 0.0) / 2.0;
	let d = center + cgmath::vec3(-dimensions.x, 0.0, 0.0) / 2.0;
	vertices.push(SimpleLineVertexPod { position: a.into(), color });
	vertices.push(SimpleLineVertexPod { position: b.into(), color });
	vertices.push(SimpleLineVertexPod { position: b.into(), color });
	vertices.push(SimpleLineVertexPod { position: c.into(), color });
	vertices.push(SimpleLineVertexPod { position: c.into(), color });
	vertices.push(SimpleLineVertexPod { position: d.into(), color });
	vertices.push(SimpleLineVertexPod { position: d.into(), color });
	vertices.push(SimpleLineVertexPod { position: a.into(), color });
	vertices
}

pub struct InterfaceMeshesVertices {
	pub simple_texture_vertices: Vec<SimpleTextureVertexPod>,
	pub simple_line_vertices: Vec<SimpleLineVertexPod>,
}

impl InterfaceMeshesVertices {
	pub fn new() -> InterfaceMeshesVertices {
		InterfaceMeshesVertices { simple_texture_vertices: vec![], simple_line_vertices: vec![] }
	}

	pub fn add_simple_texture_vertices(&mut self, mut vertices: Vec<SimpleTextureVertexPod>) {
		self.simple_texture_vertices.append(&mut vertices);
	}

	fn add_simple_line_vertices(&mut self, mut vertices: Vec<SimpleLineVertexPod>) {
		self.simple_line_vertices.append(&mut vertices);
	}
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WidgetLabel {
	GeneralDebugInfo,
	LogLineList,
}

pub enum Widget {
	Nothing,
	SimpleText {
		text: String,
		settings: font::TextRenderingSettings,
	},
	Label {
		sub_widget: Box<Widget>,
		label: WidgetLabel,
	},
	Margins {
		sub_widget: Box<Widget>,
		margin_left: f32,
		margin_top: f32,
		margin_right: f32,
		margin_bottom: f32,
	},
	SmoothlyIncoming {
		sub_widget: Box<Widget>,
		start_top_left: cgmath::Point2<f32>,
		animation_start_time: std::time::Instant,
		animation_duration: std::time::Duration,
	},
	List {
		sub_widgets: Vec<Widget>,
		interspace: f32,
	},
}

impl Widget {
	fn new_nothing() -> Widget {
		Widget::Nothing
	}

	pub fn new_simple_text(text: String, settings: font::TextRenderingSettings) -> Widget {
		Widget::SimpleText { text, settings }
	}

	pub fn new_labeled_nothing(label: WidgetLabel) -> Widget {
		Widget::Label { sub_widget: Box::new(Widget::new_nothing()), label }
	}

	pub fn new_label(label: WidgetLabel, sub_widget: Box<Widget>) -> Widget {
		Widget::Label { sub_widget, label }
	}

	pub fn new_margins(
		(margin_left, margin_top, margin_right, margin_bottom): (f32, f32, f32, f32),
		sub_widget: Box<Widget>,
	) -> Widget {
		Widget::Margins { sub_widget, margin_left, margin_top, margin_right, margin_bottom }
	}

	pub fn new_smoothly_incoming(
		start_top_left: cgmath::Point2<f32>,
		animation_start_time: std::time::Instant,
		animation_duration: std::time::Duration,
		sub_widget: Box<Widget>,
	) -> Widget {
		Widget::SmoothlyIncoming {
			sub_widget,
			start_top_left,
			animation_start_time,
			animation_duration,
		}
	}

	pub fn new_list(sub_widgets: Vec<Widget>, interspace: f32) -> Widget {
		Widget::List { sub_widgets, interspace }
	}

	fn find_label(&mut self, label_to_find: WidgetLabel) -> Option<&mut Widget> {
		match self {
			Widget::Nothing => None,
			Widget::SimpleText { .. } => None,
			Widget::Label { label, .. } if *label == label_to_find => Some(self),
			Widget::Label { sub_widget, .. } => sub_widget.find_label(label_to_find),
			Widget::Margins { sub_widget, .. } => sub_widget.find_label(label_to_find),
			Widget::SmoothlyIncoming { sub_widget, .. } => sub_widget.find_label(label_to_find),
			Widget::List { sub_widgets, .. } => sub_widgets
				.iter_mut()
				.find_map(|sub_widget| sub_widget.find_label(label_to_find)),
		}
	}

	pub fn find_label_content(&mut self, label_to_find: WidgetLabel) -> Option<&mut Widget> {
		self.find_label(label_to_find).map(|label_widget| {
			if let Widget::Label { sub_widget, .. } = label_widget {
				Box::as_mut(sub_widget)
			} else {
				unreachable!("`find_label` returns a label");
			}
		})
	}

	fn smoothed_animation_progression_ratio(&self) -> f32 {
		match self {
			Widget::SmoothlyIncoming { animation_start_time, animation_duration, .. } => {
				let ratio = (animation_start_time.elapsed().as_secs_f32()
					/ animation_duration.as_secs_f32())
				.min(1.0);
				// Smoothing the end of the animation a bit (arount when the ratio is 1.0).
				1.0 - (1.0 - ratio).powi(3)
			},
			_ => 1.0,
		}
	}

	fn dimensions(&self, font: &font::Font, window_width: f32) -> cgmath::Vector2<f32> {
		match self {
			Widget::Nothing => cgmath::vec2(0.0, 0.0),
			Widget::SimpleText { text, settings } => {
				font.dimensions_of_text(window_width, settings.clone(), text.as_str())
			},
			Widget::Label { sub_widget, .. } => sub_widget.dimensions(font, window_width),
			Widget::Margins { sub_widget, margin_left, margin_top, margin_right, margin_bottom } => {
				let sub_dimensions = sub_widget.dimensions(font, window_width);
				sub_dimensions
					+ cgmath::vec2(margin_left + margin_right, margin_top + margin_bottom)
						* (2.0 / window_width)
			},
			Widget::SmoothlyIncoming { sub_widget, .. } => {
				let progression = self.smoothed_animation_progression_ratio();
				let sub_dimensions = sub_widget.dimensions(font, window_width);
				sub_dimensions * progression
			},
			Widget::List { sub_widgets, interspace } => {
				let mut dimensions = cgmath::vec2(0.0f32, 0.0f32);
				for i in 0..sub_widgets.len() {
					let sub_dimensions = sub_widgets[i].dimensions(font, window_width);
					dimensions.y += sub_dimensions.y;
					dimensions.x = dimensions.x.max(sub_dimensions.x);
					// Now we add the interspaces between the current sub widget and the
					// next sub widget.
					// If the current or next (or both) sub widgets have not fully arrived
					// then the interspace should also not be fully developped (so that everything
					// in the list make space in a smooth manner, even the interspaces).
					if i != sub_widgets.len() - 1 {
						let current_sub_progression =
							sub_widgets[i].smoothed_animation_progression_ratio();
						let next_sub_progression =
							sub_widgets[i + 1].smoothed_animation_progression_ratio();
						let mean_progression = (current_sub_progression + next_sub_progression) / 2.0;
						dimensions.y += interspace * mean_progression * (2.0 / window_width);
					}
				}
				dimensions
			},
		}
	}

	pub fn generate_meshes(
		&self,
		top_left: cgmath::Point3<f32>,
		meshes: &mut InterfaceMeshesVertices,
		font: &font::Font,
		window_width: f32,
		draw_debug_boxes: bool,
	) {
		match self {
			Widget::Nothing => {},
			Widget::SimpleText { settings, text, .. } => {
				let simple_texture_vertices = font.simple_texture_vertices_from_text(
					window_width,
					top_left,
					settings.clone(),
					text,
				);
				meshes.add_simple_texture_vertices(simple_texture_vertices);
			},
			Widget::Label { sub_widget, .. } => {
				sub_widget.generate_meshes(top_left, meshes, font, window_width, draw_debug_boxes);
			},
			Widget::Margins { sub_widget, margin_left, margin_top, .. } => {
				let sub_top_left =
					top_left + cgmath::vec3(*margin_left, -*margin_top, 0.0) * (2.0 / window_width);
				sub_widget.generate_meshes(sub_top_left, meshes, font, window_width, draw_debug_boxes);
			},
			Widget::SmoothlyIncoming { sub_widget, start_top_left, .. } => {
				let progression = self.smoothed_animation_progression_ratio();
				let current_top_left = top_left.to_vec() * progression
					+ start_top_left.to_vec().extend(top_left.z) * (1.0 - progression);
				sub_widget.generate_meshes(
					cgmath::Point3::<f32>::from_vec(current_top_left),
					meshes,
					font,
					window_width,
					draw_debug_boxes,
				);
			},
			Widget::List { sub_widgets, interspace } => {
				let mut top_left = top_left;
				for i in 0..sub_widgets.len() {
					sub_widgets[i].generate_meshes(
						top_left,
						meshes,
						font,
						window_width,
						draw_debug_boxes,
					);

					let sub_dimensions = sub_widgets[i].dimensions(font, window_width);
					top_left.y -= sub_dimensions.y;

					// Now we add the interspaces between the current sub widget and the
					// next sub widget.
					// If the current or next (or both) sub widgets have not fully arrived
					// then the interspace should also not be fully developped (so that everything
					// in the list make space in a smooth manner, even the interspaces).
					if i != sub_widgets.len() - 1 {
						let current_sub_progression =
							sub_widgets[i].smoothed_animation_progression_ratio();
						let next_sub_progression =
							sub_widgets[i + 1].smoothed_animation_progression_ratio();
						let mean_progression = (current_sub_progression + next_sub_progression) / 2.0;
						top_left.y -= interspace * mean_progression * (2.0 / window_width);
					}
				}
			},
		}
		if draw_debug_boxes {
			const DEBUG_HITBOXES_COLOR: [f32; 3] = [1.0, 0.0, 0.0];
			const DEBUG_HITBOXES_DIAMOND_COLOR: [f32; 3] = [0.0, 0.0, 1.0];

			let dimensions = self.dimensions(font, window_width);
			let mut top_left = top_left;
			top_left.z = 0.0;
			meshes.add_simple_line_vertices(simple_line_vertices_for_rect(
				top_left,
				dimensions,
				DEBUG_HITBOXES_COLOR,
			));

			meshes.add_simple_line_vertices(simple_line_vertices_for_diamond(
				top_left,
				cgmath::vec2(6.0, 6.0) * (2.0 / window_width),
				DEBUG_HITBOXES_DIAMOND_COLOR,
			));
		}
	}
}
