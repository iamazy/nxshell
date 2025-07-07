use egui::{Color32, NumExt, Pos2, Rect, Sense, Ui, Vec2};

#[derive(Clone)]
pub struct ScrollbarState {
    pub scroll_pixels: f32,
}

impl Default for ScrollbarState {
    fn default() -> Self {
        Self { scroll_pixels: 0.0 }
    }
}

pub struct InteractiveScrollbar {
    pub first_row_pos: f32,
    pub new_first_row_pos: Option<f32>,
}

impl InteractiveScrollbar {
    pub fn new() -> Self {
        Self {
            first_row_pos: 0.0,
            new_first_row_pos: None,
        }
    }

    pub fn set_first_row_pos(&mut self, row: f32) {
        self.first_row_pos = row;
    }

    pub const WIDTH: f32 = 16.0;
    pub const MARGIN: f32 = 0.0;
}

impl InteractiveScrollbar {
    pub fn ui(&mut self, total_height: f32, ui: &mut Ui) {
        let mut position: f32;
        let scrollbar_width = InteractiveScrollbar::WIDTH;
        let margin = InteractiveScrollbar::MARGIN;

        let available_rect = ui.available_rect_before_wrap();
        let height = available_rect.bottom() - available_rect.top();
        let y_min = available_rect.top() + margin;
        let scrollbar_rect = Rect::from_min_size(
            Pos2::new(available_rect.right() - scrollbar_width - margin, y_min),
            Vec2::new(scrollbar_width, height),
        );

        let ratio = (height / total_height).min(1.0);
        let slider_height = (height * ratio).at_least(64.0);
        let max_value = total_height - height;
        let max_scroll_top = height - slider_height;
        let scroll_pos = max_scroll_top - self.first_row_pos * max_scroll_top / max_value;
        let slider_rect = Rect::from_min_size(
            scrollbar_rect.min + Vec2::new(0.0, scroll_pos),
            Vec2::new(scrollbar_width, slider_height),
        );

        ui.painter().rect_filled(
            scrollbar_rect,
            0.0,
            Color32::BLACK, //from_gray(100)
        );

        ui.painter().rect_filled(
            slider_rect,
            0.0,
            Color32::DARK_GRAY, //from_gray(200)
        );

        let response = ui.allocate_rect(slider_rect, Sense::click_and_drag());

        let scrollbar_response = ui.allocate_rect(scrollbar_rect, Sense::click());

        if response.dragged() {
            if let Some(pos) = response.hover_pos() {
                let new_position = pos.y - scrollbar_rect.top();
                position = new_position.clamp(0.0, height);
                let new_first_row_pos = max_value - position * max_value / max_scroll_top;
                self.new_first_row_pos = Some(new_first_row_pos);
            }
        }

        if scrollbar_response.clicked() {
            if let Some(click_pos) = scrollbar_response.interact_pointer_pos() {
                let click_y = click_pos.y - scrollbar_rect.top();
                position = click_y.clamp(0.0, height);
                let new_first_row_pos = max_value - position * max_value / max_scroll_top;
                self.new_first_row_pos = Some(new_first_row_pos);
            }
        }

        // mouse wheel
        /*
        let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll_delta != 0.0 {
            self.state.position += scroll_delta * 1.0;
            self.state.position = self.state.position.clamp(0.0, height);
        }
        */

        ui.ctx().request_repaint();
    }
}
