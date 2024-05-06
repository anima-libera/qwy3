use crate::{
	atlas::RectInAtlas,
	font,
	widgets::{
		BoxContentPlacement, BoxDimensions, ListAlignmentHorizontal, ListAlignmentVertical,
		ListOrientationAndAlignment, ListOrientationHorizontal, ListOrientationVertical, Widget,
		WidgetLabel,
	},
};

pub(crate) struct Interface {
	pub(crate) widget_tree_root: Widget,
}

impl Interface {
	pub(crate) fn new() -> Interface {
		let widget_tree_root = Widget::new_box(BoxDimensions::Screen)
			.set_a_box_sub_widget(
				BoxContentPlacement::TopLeft,
				Widget::new_margins(
					(5.0, 5.0, 0.0, 0.0),
					Box::new(Widget::new_list(
						vec![
							Widget::new_labeled_nothing(WidgetLabel::GeneralDebugInfo),
							Widget::new_smoothly_incoming(
								cgmath::point2(1.0, 0.0),
								std::time::Instant::now(),
								std::time::Duration::from_secs_f32(1.0),
								Box::new(Widget::new_simple_text(
									"nyoom >w<".to_string(),
									font::TextRenderingSettings::with_scale(3.0),
								)),
							),
							Widget::new_label(
								WidgetLabel::LogLineList,
								Box::new(Widget::new_list(
									vec![],
									5.0,
									ListOrientationAndAlignment::Vertical(
										ListOrientationVertical::TopToBottom,
										ListAlignmentVertical::Left,
									),
								)),
							),
						],
						5.0,
						ListOrientationAndAlignment::Vertical(
							ListOrientationVertical::TopToBottom,
							ListAlignmentVertical::Left,
						),
					)),
				),
			)
			.set_a_box_sub_widget(
				BoxContentPlacement::BottomRight,
				Widget::new_margins(
					(0.0, 0.0, 5.0, 5.0),
					Box::new(Widget::new_list(
						vec![
							Widget::new_labeled_nothing(WidgetLabel::HealthBar),
							Widget::new_labeled_nothing(WidgetLabel::ItemHeld),
						],
						5.0,
						ListOrientationAndAlignment::Vertical(
							ListOrientationVertical::BottomToTop,
							ListAlignmentVertical::Right,
						),
					)),
				),
			);

		Interface { widget_tree_root }
	}

	pub(crate) fn log_widget(&mut self, widget_to_log: Widget) {
		if let Some(Widget::List { sub_widgets, .. }) =
			self.widget_tree_root.find_label_content(WidgetLabel::LogLineList)
		{
			sub_widgets.push(widget_to_log);
		}
	}

	pub(crate) fn update_health_bar(&mut self, health: Option<u32>) {
		if let Some(health_bar_widget) =
			self.widget_tree_root.find_label_content(WidgetLabel::HealthBar)
		{
			if let Some(health) = &health {
				let mut hearts = vec![];
				for _i in 0..*health {
					hearts.push(Widget::SimpleTexture {
						// Heart sprite.
						rect_in_atlas: RectInAtlas {
							texture_rect_in_atlas_xy: cgmath::point2(256.0, 32.0) / 512.0,
							texture_rect_in_atlas_wh: cgmath::vec2(7.0, 7.0) / 512.0,
						},
						scale: 5.0,
					});
				}
				*health_bar_widget = Widget::new_list(
					hearts,
					6.0,
					ListOrientationAndAlignment::Horizontal(
						ListOrientationHorizontal::RightToLeft,
						ListAlignmentHorizontal::Center,
					),
				);
			} else {
				*health_bar_widget = Widget::Nothing;
			}
		}
	}
}
