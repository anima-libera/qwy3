use std::collections::HashMap;
use std::sync::atomic::{self, AtomicI32};
use std::sync::Arc;

use cgmath::EuclideanSpace;
use rustc_hash::FxHashMap;

use crate::unsorted::{RectInAtlas, SimpleTextureMesh};
use crate::{
	font,
	shaders::{simple_line::SimpleLineVertexPod, simple_texture_2d::SimpleTextureVertexPod},
};

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

/// Vertices for mutliple meshes used to render the interface.
/// Widgets can draw themselves by adding vertices in here.
pub(crate) struct InterfaceMeshesVertices {
	pub(crate) simple_texture_vertices: Vec<SimpleTextureVertexPod>,
	pub(crate) simple_line_vertices: Vec<SimpleLineVertexPod>,
}

impl InterfaceMeshesVertices {
	pub(crate) fn new() -> InterfaceMeshesVertices {
		InterfaceMeshesVertices { simple_texture_vertices: vec![], simple_line_vertices: vec![] }
	}

	pub(crate) fn add_simple_texture_vertices(&mut self, mut vertices: Vec<SimpleTextureVertexPod>) {
		self.simple_texture_vertices.append(&mut vertices);
	}

	fn add_simple_line_vertices(&mut self, mut vertices: Vec<SimpleLineVertexPod>) {
		self.simple_line_vertices.append(&mut vertices);
	}
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WidgetLabel {
	GeneralDebugInfo,
	LogLineList,
	ItemHeld,
	HealthBar,
}

pub(crate) enum Widget {
	Nothing,
	SimpleText {
		text: String,
		settings: font::TextRenderingSettings,
	},
	SimpleTexture {
		rect_in_atlas: RectInAtlas,
		scale: f32,
	},
	/// Loading bar for the face counter of some skybox generation.
	FaceCounter {
		settings: font::TextRenderingSettings,
		counter: Arc<AtomicI32>,
	},
	/// A wrapper around a widget that tags it with a label.
	/// It allows for some code to find the contained widget via the label easily.
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
	/// A wrapper around a widget that has to arrive at the wrapper's location over a period
	/// during an animation that takes some time.
	/// The wrapper begins as almost non-existant in the layout and progressively takes
	/// the space that will take the contained widget when it finally arrives.
	SmoothlyIncoming {
		sub_widget: Box<Widget>,
		start_top_left: cgmath::Point2<f32>,
		animation_start_time: std::time::Instant,
		animation_duration: std::time::Duration,
	},
	SmoothlyDisappearingEmptySpace {
		start_dimensions: cgmath::Vector2<f32>,
		animation_start_time: std::time::Instant,
		animation_duration: std::time::Duration,
	},
	/// A wrapper around a widget that can be "completed" (like a loading bar)
	/// and that disappears after a delay after the completion of the sub widget.
	DisappearWhenComplete {
		sub_widget: Box<Widget>,
		completed_time: Option<std::time::Instant>,
		delay_before_disappearing: std::time::Duration,
	},
	List {
		sub_widgets: Vec<Widget>,
		interspace: f32,
		orientation: WidgetListOrientation,
	},
	Box {
		dimensions: BoxDimensions,
		sub_widgets: FxHashMap<BoxContentPlacement, Widget>,
	},
}

#[derive(PartialEq, Eq)]
pub(crate) enum WidgetListOrientation {
	Bottomward,
	Rightward,
}

pub(crate) enum BoxDimensions {
	/// The box has the size of the whole window/screen.
	Screen,
}

#[derive(PartialEq, Eq, Hash)]
pub(crate) enum BoxContentPlacement {
	TopLeft,
	BottomRight,
}

impl Widget {
	fn new_nothing() -> Widget {
		Widget::Nothing
	}

	pub(crate) fn new_simple_text(text: String, settings: font::TextRenderingSettings) -> Widget {
		Widget::SimpleText { text, settings }
	}

	pub(crate) fn new_simple_texture(rect_in_atlas: RectInAtlas, scale: f32) -> Widget {
		Widget::SimpleTexture { rect_in_atlas, scale }
	}

	pub(crate) fn new_face_counter(
		settings: font::TextRenderingSettings,
		counter: Arc<AtomicI32>,
	) -> Widget {
		Widget::FaceCounter { settings, counter }
	}

	pub(crate) fn new_labeled_nothing(label: WidgetLabel) -> Widget {
		Widget::Label { sub_widget: Box::new(Widget::new_nothing()), label }
	}

	pub(crate) fn new_label(label: WidgetLabel, sub_widget: Box<Widget>) -> Widget {
		Widget::Label { sub_widget, label }
	}

	pub(crate) fn new_margins(
		(margin_left, margin_top, margin_right, margin_bottom): (f32, f32, f32, f32),
		sub_widget: Box<Widget>,
	) -> Widget {
		Widget::Margins { sub_widget, margin_left, margin_top, margin_right, margin_bottom }
	}

	pub(crate) fn new_smoothly_incoming(
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

	pub(crate) fn new_disappear_when_complete(
		delay_before_disappearing: std::time::Duration,
		sub_widget: Box<Widget>,
	) -> Widget {
		Widget::DisappearWhenComplete { sub_widget, completed_time: None, delay_before_disappearing }
	}

	pub(crate) fn new_list(
		sub_widgets: Vec<Widget>,
		interspace: f32,
		orientation: WidgetListOrientation,
	) -> Widget {
		Widget::List { sub_widgets, interspace, orientation }
	}

	pub(crate) fn new_box(dimensions: BoxDimensions) -> Widget {
		Widget::Box { dimensions, sub_widgets: HashMap::default() }
	}
	pub(crate) fn set_a_box_sub_widget(
		self,
		placement: BoxContentPlacement,
		sub_widget: Widget,
	) -> Widget {
		if let Widget::Box { dimensions, mut sub_widgets } = self {
			sub_widgets.insert(placement, sub_widget);
			Widget::Box { dimensions, sub_widgets }
		} else {
			panic!("Expected a box widget");
		}
	}

	pub(crate) fn pop_while_smoothly_closing_space(
		&mut self,
		animation_start_time: std::time::Instant,
		animation_duration: std::time::Duration,
		font: &font::Font,
		window_dimensions: cgmath::Vector2<f32>,
	) -> Widget {
		let mut widget = Widget::SmoothlyDisappearingEmptySpace {
			start_dimensions: self.dimensions(font, window_dimensions),
			animation_start_time,
			animation_duration,
		};
		std::mem::swap(self, &mut widget);
		widget
	}

	pub(crate) fn is_diappearing(&self) -> bool {
		matches!(self, Widget::SmoothlyDisappearingEmptySpace { .. })
	}

	pub(crate) fn is_completed(&self) -> bool {
		if let Widget::FaceCounter { counter, .. } = self {
			counter.load(atomic::Ordering::Relaxed) >= 6
		} else {
			false
		}
	}

	pub(crate) fn for_each_rec(&mut self, f: &mut dyn FnMut(&mut Widget)) {
		f(self);
		match self {
			Widget::Nothing => {},
			Widget::SimpleText { .. } => {},
			Widget::SimpleTexture { .. } => {},
			Widget::FaceCounter { .. } => {},
			Widget::Label { sub_widget, .. } => sub_widget.for_each_rec(f),
			Widget::Margins { sub_widget, .. } => sub_widget.for_each_rec(f),
			Widget::SmoothlyIncoming { sub_widget, .. } => sub_widget.for_each_rec(f),
			Widget::SmoothlyDisappearingEmptySpace { .. } => {},
			Widget::DisappearWhenComplete { sub_widget, .. } => sub_widget.for_each_rec(f),
			Widget::List { sub_widgets, .. } => {
				sub_widgets.iter_mut().for_each(|sub_widget| sub_widget.for_each_rec(f))
			},
			Widget::Box { sub_widgets, .. } => {
				sub_widgets.values_mut().for_each(|sub_widget| sub_widget.for_each_rec(f))
			},
		};
	}

	/// Returns the first found label widget that matches with the given label.
	fn find_label(&mut self, label_to_find: WidgetLabel) -> Option<&mut Widget> {
		match self {
			Widget::Nothing => None,
			Widget::SimpleText { .. } => None,
			Widget::SimpleTexture { .. } => None,
			Widget::FaceCounter { .. } => None,
			Widget::Label { label, .. } if *label == label_to_find => Some(self),
			Widget::Label { sub_widget, .. } => sub_widget.find_label(label_to_find),
			Widget::Margins { sub_widget, .. } => sub_widget.find_label(label_to_find),
			Widget::SmoothlyIncoming { sub_widget, .. } => sub_widget.find_label(label_to_find),
			Widget::SmoothlyDisappearingEmptySpace { .. } => None,
			Widget::DisappearWhenComplete { sub_widget, .. } => sub_widget.find_label(label_to_find),
			Widget::List { sub_widgets, .. } => {
				sub_widgets.iter_mut().find_map(|sub_widget| sub_widget.find_label(label_to_find))
			},
			Widget::Box { sub_widgets, .. } => {
				sub_widgets.values_mut().find_map(|sub_widget| sub_widget.find_label(label_to_find))
			},
		}
	}

	/// Returns the content of the first found label widget that matches with the given label.
	pub(crate) fn find_label_content(&mut self, label_to_find: WidgetLabel) -> Option<&mut Widget> {
		self.find_label(label_to_find).map(|label_widget| {
			if let Widget::Label { sub_widget, .. } = label_widget {
				Box::as_mut(sub_widget)
			} else {
				unreachable!("`find_label` returns a label");
			}
		})
	}

	/// Returns a value between 0.0 and 1.0 that represents how much the widget "exists".
	/// For example, a widget that is a wrapper with an apparition animation will
	/// have an existence ratio that slowly goes from 0.0 to 1.0, and a wrapper that
	/// has a disappearing animation will have a ratio that goes from 1.0 to 0.0.
	fn existence_ratio(&self) -> f32 {
		match self {
			Widget::SmoothlyIncoming { animation_start_time, animation_duration, .. } => {
				let ratio = (animation_start_time.elapsed().as_secs_f32()
					/ animation_duration.as_secs_f32())
				.min(1.0);
				// Smoothing the end of the animation a bit (arount when the ratio is 1.0).
				1.0 - (1.0 - ratio).powi(3)
			},
			Widget::SmoothlyDisappearingEmptySpace {
				animation_start_time,
				animation_duration,
				..
			} => {
				let ratio = (animation_start_time.elapsed().as_secs_f32()
					/ animation_duration.as_secs_f32())
				.min(1.0);
				let ratio = 1.0 - ratio;
				// Smoothing the end of the animation a bit (arount when the ratio is 0.0).
				ratio.powi(3)
			},
			_ => 1.0,
		}
	}

	/// Returns the dimensions of the widget, already corrected to wgsl coords space.
	fn dimensions(
		&self,
		font: &font::Font,
		window_dimensions: cgmath::Vector2<f32>,
	) -> cgmath::Vector2<f32> {
		match self {
			Widget::Nothing => cgmath::vec2(0.0, 0.0),
			Widget::SimpleText { text, settings } => {
				font.dimensions_of_text(window_dimensions.x, settings.clone(), text.as_str())
			},
			Widget::SimpleTexture { rect_in_atlas, scale } => {
				rect_in_atlas.texture_rect_in_atlas_wh * *scale
			},
			Widget::FaceCounter { settings, .. } => font.dimensions_of_text(
				window_dimensions.x,
				settings.clone(),
				"skybox generation: [██████] 6/6",
			),
			Widget::Label { sub_widget, .. } => sub_widget.dimensions(font, window_dimensions),
			Widget::Margins { sub_widget, margin_left, margin_top, margin_right, margin_bottom } => {
				let sub_dimensions = sub_widget.dimensions(font, window_dimensions);
				sub_dimensions
					+ cgmath::vec2(margin_left + margin_right, margin_top + margin_bottom)
						* (2.0 / window_dimensions.x)
			},
			Widget::SmoothlyIncoming { sub_widget, .. } => {
				let ratio = self.existence_ratio();
				let sub_dimensions = sub_widget.dimensions(font, window_dimensions);
				sub_dimensions * ratio
			},
			Widget::SmoothlyDisappearingEmptySpace { start_dimensions, .. } => {
				let ratio = self.existence_ratio();
				start_dimensions * ratio
			},
			Widget::DisappearWhenComplete { sub_widget, .. } => {
				sub_widget.dimensions(font, window_dimensions)
			},
			Widget::List { sub_widgets, interspace, orientation } => {
				let mut dimensions = cgmath::vec2(0.0f32, 0.0f32);
				for i in 0..sub_widgets.len() {
					let sub_dimensions = sub_widgets[i].dimensions(font, window_dimensions);
					if *orientation == WidgetListOrientation::Bottomward {
						dimensions.y += sub_dimensions.y;
						dimensions.x = dimensions.x.max(sub_dimensions.x);
					} else if *orientation == WidgetListOrientation::Rightward {
						dimensions.x += sub_dimensions.x;
						dimensions.y = dimensions.y.max(sub_dimensions.y);
					}

					// Now we add the interspaces between the current sub widget and the
					// next sub widget.
					// If the current or next (or both) sub widgets have not fully arrived
					// then the interspace should also not be fully developped (so that everything
					// in the list make space in a smooth manner, even the interspaces).
					if i != sub_widgets.len() - 1 {
						let current_sub_ratio = sub_widgets[i].existence_ratio();
						let next_sub_ratio = sub_widgets[i + 1].existence_ratio();
						let ratio = current_sub_ratio * next_sub_ratio;
						if *orientation == WidgetListOrientation::Bottomward {
							dimensions.y += interspace * ratio * (2.0 / window_dimensions.x);
						} else if *orientation == WidgetListOrientation::Rightward {
							dimensions.x += interspace * ratio * (2.0 / window_dimensions.x);
						}
					}
				}
				dimensions
			},
			Widget::Box { dimensions, .. } => match dimensions {
				BoxDimensions::Screen => window_dimensions * (2.0 / window_dimensions.x),
			},
		}
	}

	/// Generates the mesh vertices in the given `meshes` that draw the widget.
	pub(crate) fn generate_mesh_vertices(
		&self,
		top_left: cgmath::Point3<f32>,
		meshes: &mut InterfaceMeshesVertices,
		font: &font::Font,
		window_dimensions: cgmath::Vector2<f32>,
		draw_debug_boxes: bool,
	) {
		match self {
			Widget::Nothing => {},
			Widget::SimpleText { settings, text, .. } => {
				let simple_texture_vertices = font.simple_texture_vertices_from_text(
					window_dimensions.x,
					top_left,
					settings.clone(),
					text,
				);
				meshes.add_simple_texture_vertices(simple_texture_vertices);
			},
			Widget::SimpleTexture { rect_in_atlas, scale } => {
				let dimensions = rect_in_atlas.texture_rect_in_atlas_wh * *scale;
				// Add small inwards margins to the atlas rect to prevent the sampling from
				// going outside the rect it was assigned (which would sometimes happen otherwise).
				let fixed_rect_in_atlas = RectInAtlas {
					texture_rect_in_atlas_xy: rect_in_atlas
						.texture_rect_in_atlas_xy
						.map(|x| x + 0.00001),
					texture_rect_in_atlas_wh: rect_in_atlas
						.texture_rect_in_atlas_wh
						.map(|x| x - 0.00002),
				};
				let simple_texture_vertices = SimpleTextureMesh::vertices_for_rect(
					top_left,
					dimensions,
					fixed_rect_in_atlas.texture_rect_in_atlas_xy,
					fixed_rect_in_atlas.texture_rect_in_atlas_wh,
					[1.0, 1.0, 1.0],
				);
				meshes.add_simple_texture_vertices(simple_texture_vertices);
			},
			Widget::FaceCounter { settings, counter } => {
				let counter_value = counter.load(atomic::Ordering::Relaxed);
				let mut text = String::new();
				text += &"skybox generation: ";
				text.push('[');
				for _ in 0..counter_value {
					text.push('█');
				}
				for _ in 0..(6 - counter_value) {
					text.push('_');
				}
				text.push(']');
				text.push(' ');
				text += &format!("{counter_value}/6");
				let simple_texture_vertices = font.simple_texture_vertices_from_text(
					window_dimensions.x,
					top_left,
					settings.clone(),
					&text,
				);
				meshes.add_simple_texture_vertices(simple_texture_vertices);
			},
			Widget::Label { sub_widget, .. } => {
				sub_widget.generate_mesh_vertices(
					top_left,
					meshes,
					font,
					window_dimensions,
					draw_debug_boxes,
				);
			},
			Widget::Margins { sub_widget, margin_left, margin_top, .. } => {
				let sub_top_left = top_left
					+ cgmath::vec3(*margin_left, -*margin_top, 0.0) * (2.0 / window_dimensions.x);
				sub_widget.generate_mesh_vertices(
					sub_top_left,
					meshes,
					font,
					window_dimensions,
					draw_debug_boxes,
				);
			},
			Widget::SmoothlyIncoming { sub_widget, start_top_left, .. } => {
				let progression = self.existence_ratio();
				let current_top_left = top_left.to_vec() * progression
					+ start_top_left.to_vec().extend(top_left.z) * (1.0 - progression);
				sub_widget.generate_mesh_vertices(
					cgmath::Point3::<f32>::from_vec(current_top_left),
					meshes,
					font,
					window_dimensions,
					draw_debug_boxes,
				);
			},
			Widget::SmoothlyDisappearingEmptySpace { .. } => {},
			Widget::DisappearWhenComplete { sub_widget, .. } => {
				sub_widget.generate_mesh_vertices(
					top_left,
					meshes,
					font,
					window_dimensions,
					draw_debug_boxes,
				);
			},
			Widget::List { sub_widgets, interspace, orientation } => {
				let mut top_left = top_left;
				for i in 0..sub_widgets.len() {
					sub_widgets[i].generate_mesh_vertices(
						top_left,
						meshes,
						font,
						window_dimensions,
						draw_debug_boxes,
					);

					let sub_dimensions = sub_widgets[i].dimensions(font, window_dimensions);
					if *orientation == WidgetListOrientation::Bottomward {
						top_left.y -= sub_dimensions.y;
					} else if *orientation == WidgetListOrientation::Rightward {
						top_left.x += sub_dimensions.x;
					}

					// Now we add the interspaces between the current sub widget and the
					// next sub widget.
					// If the current or next (or both) sub widgets have not fully arrived
					// then the interspace should also not be fully developped (so that everything
					// in the list make space in a smooth manner, even the interspaces).
					if i != sub_widgets.len() - 1 {
						let current_sub_ratio = sub_widgets[i].existence_ratio();
						let next_sub_ratio = sub_widgets[i + 1].existence_ratio();
						let ratio = current_sub_ratio * next_sub_ratio;
						if *orientation == WidgetListOrientation::Bottomward {
							top_left.y -= interspace * ratio * (2.0 / window_dimensions.x);
						} else if *orientation == WidgetListOrientation::Rightward {
							top_left.x += interspace * ratio * (2.0 / window_dimensions.x);
						}
					}
				}
			},
			Widget::Box { sub_widgets, .. } => {
				let dimensions = self.dimensions(font, window_dimensions);
				for (position, sub_widget) in sub_widgets.iter() {
					let sub_dimensions = sub_widget.dimensions(font, window_dimensions);
					let sub_offset = match position {
						BoxContentPlacement::TopLeft => cgmath::vec2(0.0, 0.0),
						BoxContentPlacement::BottomRight => dimensions - sub_dimensions,
					};
					let sub_top_left = top_left + cgmath::vec3(sub_offset.x, -sub_offset.y, 0.0);
					sub_widget.generate_mesh_vertices(
						sub_top_left,
						meshes,
						font,
						window_dimensions,
						draw_debug_boxes,
					);
				}
			},
		}

		// If asked for, we can draw boxes around widgets to help debugging widget tree layout.
		if draw_debug_boxes {
			const DEBUG_HITBOXES_COLOR: [f32; 3] = [1.0, 0.0, 0.0];
			const DEBUG_HITBOXES_DIAMOND_COLOR: [f32; 3] = [0.0, 0.0, 1.0];

			let dimensions = self.dimensions(font, window_dimensions);
			let mut top_left = top_left;
			top_left.z = 0.0;
			meshes.add_simple_line_vertices(simple_line_vertices_for_rect(
				top_left,
				dimensions,
				DEBUG_HITBOXES_COLOR,
			));

			meshes.add_simple_line_vertices(simple_line_vertices_for_diamond(
				top_left,
				cgmath::vec2(6.0, 6.0) * (2.0 / window_dimensions.x),
				DEBUG_HITBOXES_DIAMOND_COLOR,
			));
		}
	}
}
