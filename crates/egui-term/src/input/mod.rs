use crate::alacritty::{selection_point, BackendCommand, LinkAction, MouseButton};
use crate::view::TerminalViewState;
use crate::{BindingAction, InputKind, TerminalView};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::selection::SelectionType;
use alacritty_terminal::term::TermMode;
use egui::{Key, Modifiers, MouseWheelUnit, PointerButton, Pos2, Rect, Response, Vec2};
use std::cmp::min;

/// Minimum number of pixels at the bottom/top where selection scrolling is performed.
const MIN_SELECTION_SCROLLING_HEIGHT: f64 = 5.;

/// Number of pixels for increasing the selection scrolling speed factor by one.
const SELECTION_SCROLLING_STEP: f64 = 20.;

#[derive(Debug, Clone)]
pub enum InputAction {
    BackendCall(BackendCommand),
    WriteToClipboard(String),
}

impl TerminalView<'_> {
    pub fn text_input(&self, text: &str) -> InputAction {
        InputAction::BackendCall(BackendCommand::Write(text.as_bytes().to_vec()))
    }

    pub fn keyboard_input(
        &mut self,
        key: Key,
        modifiers: Modifiers,
        pressed: bool,
    ) -> Option<InputAction> {
        if !pressed {
            return None;
        }
        let terminal_mode = self.term_ctx.term_mode();
        match self
            .bindings_layout
            .get_action(InputKind::KeyCode(key), modifiers, terminal_mode)
        {
            Some(BindingAction::Char(c)) => {
                let mut buf = [0, 0, 0, 0];
                let str = c.encode_utf8(&mut buf);
                Some(InputAction::BackendCall(BackendCommand::Write(
                    str.as_bytes().to_vec(),
                )))
            }
            Some(BindingAction::Esc(seq)) => Some(InputAction::BackendCall(BackendCommand::Write(
                seq.as_bytes().to_vec(),
            ))),
            Some(BindingAction::Copy) => {
                let content = self.term_ctx.selection_content();
                Some(InputAction::WriteToClipboard(content))
            }
            Some(BindingAction::ResetFontSize) => {
                self.reset_font_size(self.options.default_font_size);
                None
            }
            Some(BindingAction::IncreaseFontSize) => {
                self.set_font_size(1.);
                None
            }
            Some(BindingAction::DecreaseFontSize) => {
                self.set_font_size(-1.);
                None
            }
            Some(BindingAction::SelectAll) => {
                Some(InputAction::BackendCall(BackendCommand::SelectAll))
            }
            _ => None,
        }
    }

    fn reset_font_size(&mut self, default_font_size: f32) {
        *self.options.font.font_size_mut() = default_font_size;
    }

    fn set_font_size(&mut self, size: f32) {
        let font_size = self.options.font.font_size() + size;
        if (5. ..=100.).contains(&font_size) {
            *self.options.font.font_size_mut() += size;
        }
    }

    pub fn mouse_wheel_input(
        &mut self,
        state: &mut TerminalViewState,
        unit: MouseWheelUnit,
        delta: Vec2,
        modifiers: Modifiers,
    ) -> Option<InputAction> {
        match (unit, modifiers.command_only()) {
            (MouseWheelUnit::Line | MouseWheelUnit::Point, true) => {
                let font_size = self.options.font.font_size() + delta.y;
                if font_size > 10. && font_size < 50. {
                    *self.options.font.font_size_mut() += delta.y;
                }
                None
            }
            (MouseWheelUnit::Line, _) => {
                let lines = delta.y.signum() * delta.y.abs().ceil();
                Some(InputAction::BackendCall(BackendCommand::Scroll(
                    lines as i32,
                )))
            }
            (MouseWheelUnit::Point, _) => {
                let font_size = self.options.font.font_size();
                state.scrollbar_state.scroll_pixels -= delta.y;
                let lines = (state.scrollbar_state.scroll_pixels / font_size).trunc();
                state.scrollbar_state.scroll_pixels %= font_size;
                if lines != 0.0 {
                    Some(InputAction::BackendCall(BackendCommand::Scroll(
                        -lines as i32,
                    )))
                } else {
                    None
                }
            }
            (MouseWheelUnit::Page, _) => None,
        }
    }

    pub fn button_click(
        &mut self,
        state: &mut TerminalViewState,
        layout: &Response,
        button: PointerButton,
        position: Pos2,
        modifiers: &Modifiers,
        pressed: bool,
    ) -> Option<InputAction> {
        match button {
            PointerButton::Primary => {
                self.left_button_click(state, layout, position, modifiers, pressed)
            }
            PointerButton::Secondary => {
                state.context_menu_position = Some(position);
                None
            }
            _ => None,
        }
    }

    pub fn left_button_click(
        &self,
        state: &mut TerminalViewState,
        layout: &Response,
        position: Pos2,
        modifiers: &Modifiers,
        pressed: bool,
    ) -> Option<InputAction> {
        if state.context_menu_position.is_some() {
            return None;
        }
        let terminal_mode = self.term_ctx.terminal.mode();
        if terminal_mode.intersects(TermMode::MOUSE_MODE) {
            Some(InputAction::BackendCall(BackendCommand::MouseReport(
                MouseButton::LeftButton,
                *modifiers,
                state.mouse_point,
                pressed,
            )))
        } else if pressed && is_in_terminal(position, layout.rect) {
            state.is_dragged = true;
            Some(InputAction::BackendCall(start_select_command(
                layout, position,
            )))
        } else {
            self.left_button_released(state, layout, position, modifiers)
        }
    }

    pub fn left_button_released(
        &self,
        state: &mut TerminalViewState,
        layout: &Response,
        position: Pos2,
        modifiers: &Modifiers,
    ) -> Option<InputAction> {
        state.is_dragged = false;
        if layout.double_clicked() || layout.triple_clicked() {
            Some(InputAction::BackendCall(start_select_command(
                layout, position,
            )))
        } else {
            match self.bindings_layout.get_action(
                InputKind::Mouse(PointerButton::Primary),
                *modifiers,
                *self.term_ctx.terminal.mode(),
            ) {
                Some(BindingAction::LinkOpen) => Some(InputAction::BackendCall(
                    BackendCommand::ProcessLink(LinkAction::Open, state.mouse_point),
                )),
                _ => None,
            }
        }
    }

    pub fn mouse_move(
        &self,
        state: &mut TerminalViewState,
        layout: &Response,
        position: Pos2,
        modifiers: &Modifiers,
    ) -> Vec<InputAction> {
        let mouse_x = position.x - layout.rect.min.x;
        let mouse_y = position.y - layout.rect.min.y;

        state.mouse_point = selection_point(
            mouse_x,
            mouse_y,
            self.term_ctx.size,
            self.term_ctx.terminal.grid().display_offset(),
        );
        state.mouse_position = Some(position);

        let mut actions = vec![];
        // Handle command or selection update based on terminal mode and modifiers
        if state.is_dragged {
            if !self.term_ctx.selection_is_empty() {
                if let Some(action) = self.update_selection_scrolling(mouse_y as i32) {
                    actions.push(action);
                }
            }

            let cmd = if self
                .term_ctx
                .terminal
                .mode()
                .intersects(TermMode::MOUSE_MOTION | TermMode::MOUSE_DRAG)
                && modifiers.is_none()
            {
                InputAction::BackendCall(BackendCommand::MouseReport(
                    MouseButton::LeftMove,
                    *modifiers,
                    state.mouse_point,
                    true,
                ))
            } else {
                InputAction::BackendCall(BackendCommand::SelectUpdate(mouse_x, mouse_y))
            };

            actions.push(cmd);
        }

        // Handle link hover if applicable
        actions.push(InputAction::BackendCall(BackendCommand::ProcessLink(
            LinkAction::Hover,
            state.mouse_point,
        )));

        actions
    }

    pub fn update_selection_scrolling(&self, cursor_y: i32) -> Option<InputAction> {
        let term_size = *self.term_ctx.size;

        let min_height = MIN_SELECTION_SCROLLING_HEIGHT as i32;
        let step = SELECTION_SCROLLING_STEP as i32;

        let end_top = min_height;
        let text_area_bottom = term_size.screen_lines() as f32 * term_size.cell_height as f32;
        let start_bottom = min(
            term_size.layout_size.height as i32 - min_height,
            text_area_bottom as i32,
        );

        let delta = if cursor_y < end_top {
            end_top - cursor_y + step
        } else if cursor_y >= start_bottom {
            start_bottom - cursor_y - step
        } else {
            return None;
        };

        Some(InputAction::BackendCall(BackendCommand::Scroll(
            delta / step,
        )))
    }
}

fn start_select_command(layout: &Response, cursor_position: Pos2) -> BackendCommand {
    let selection_type = if layout.double_clicked() {
        SelectionType::Semantic
    } else if layout.triple_clicked() {
        SelectionType::Lines
    } else {
        SelectionType::Simple
    };

    BackendCommand::SelectStart(
        selection_type,
        cursor_position.x - layout.rect.min.x,
        cursor_position.y - layout.rect.min.y,
    )
}

pub fn is_in_terminal(pos: Pos2, rect: Rect) -> bool {
    pos.x > rect.min.x && pos.x < rect.max.x && pos.y > rect.min.y && pos.y < rect.max.y
}
