#![allow(dead_code)]
mod color;
mod sftp;
pub use sftp::SftpExplorer;

use crate::display::color::HOVERED_HYPERLINK_COLOR;
use crate::view::TerminalViewState;
use crate::TerminalView;
use alacritty_terminal::grid::GridCell;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::{Color, NamedColor};
use copypasta::ClipboardProvider;
use egui::epaint::RectShape;
use egui::{
    Align2, Area, Button, Color32, CornerRadius, CursorIcon, Id, Key, KeyboardShortcut, Modifiers,
    Painter, Pos2, Rect, Response, Vec2, WidgetText,
};
use egui::{Shape, Stroke};
use tracing::error;
use wezterm_ssh::Session;

impl TerminalView<'_> {
    pub fn show(self, state: &mut TerminalViewState, layout: &Response, painter: &Painter) {
        let layout_min = layout.rect.min;
        let layout_max = layout.rect.max;
        let cell_height = self.term_ctx.size.cell_height as f32;
        let cell_width = self.term_ctx.size.cell_width as f32;

        let global_bg = self.theme().get_color(Color::Named(NamedColor::Background));

        let mut shapes = vec![Shape::Rect(RectShape::filled(
            Rect::from_min_max(layout_min, layout_max),
            CornerRadius::ZERO,
            global_bg,
        ))];

        let grid = self.term_ctx.terminal.grid();

        for indexed in grid.display_iter() {
            let is_wide_char_spacer = indexed.flags().contains(Flags::WIDE_CHAR_SPACER);
            if is_wide_char_spacer {
                continue;
            }
            let is_app_cursor_mode = self.term_ctx.term_mode().contains(TermMode::APP_CURSOR);
            let is_inverse = indexed.flags().contains(Flags::INVERSE);
            let is_dim = indexed.flags().intersects(Flags::DIM | Flags::DIM_BOLD);
            let is_wide_char = indexed.flags().contains(Flags::WIDE_CHAR);
            let is_selected = self
                .term_ctx
                .to_range()
                .is_some_and(|r| r.contains(indexed.point));
            let is_hovered_hyperlink =
                self.term_ctx.hovered_hyperlink.as_ref().is_some_and(|r| {
                    r.contains(&indexed.point) && r.contains(&state.mouse_position)
                });
            let is_text_cell = indexed.c != ' ' && indexed.c != '\t';

            let x = layout_min.x + indexed.point.column.saturating_mul(cell_width as usize) as f32;
            let y = layout_min.y
                + indexed
                    .point
                    .line
                    .saturating_add(grid.display_offset() as i32)
                    .saturating_mul(cell_height as i32) as f32;

            let mut fg = self.theme().get_color(indexed.fg);
            let mut bg = self.theme().get_color(indexed.bg);

            let cell_width = if is_wide_char {
                cell_width * 2.0
            } else {
                cell_width
            };

            if is_dim {
                fg = fg.linear_multiply(0.7);
            }

            if is_inverse {
                std::mem::swap(&mut fg, &mut bg);
            }

            if is_selected {
                bg = self.theme().get_selection_color()
            }

            if global_bg != bg {
                shapes.push(Shape::Rect(RectShape::filled(
                    Rect::from_min_size(Pos2::new(x, y), Vec2::new(cell_width, cell_height)),
                    CornerRadius::ZERO,
                    bg,
                )));
            }

            // Handle hovered hyperlink underline
            if is_hovered_hyperlink {
                layout.ctx.set_cursor_icon(CursorIcon::PointingHand);
                let underline_height = y + cell_height;
                shapes.push(Shape::LineSegment {
                    points: [
                        Pos2::new(x, underline_height),
                        Pos2::new(x + cell_width, underline_height),
                    ],
                    stroke: Stroke::new(cell_height * 0.08, fg),
                });
            }

            // Handle cursor rendering
            if grid.cursor.point == indexed.point {
                let cursor_color = self.theme().get_color(self.term_ctx.cursor_cell().fg);

                let cursor_width = if is_text_cell {
                    cell_width
                } else {
                    cell_width / 2.
                };

                shapes.push(Shape::Rect(RectShape::filled(
                    Rect::from_min_size(Pos2::new(x, y), Vec2::new(cursor_width, cell_height)),
                    CornerRadius::default(),
                    cursor_color,
                )));
            }

            // Draw text content
            if is_text_cell {
                if is_hovered_hyperlink {
                    fg = HOVERED_HYPERLINK_COLOR;
                } else if grid.cursor.point == indexed.point && is_app_cursor_mode {
                    std::mem::swap(&mut fg, &mut bg);
                }

                shapes.push(Shape::text(
                    &painter.fonts(|c| c.clone()),
                    Pos2 {
                        x: x + (cell_width / 2.0),
                        y,
                    },
                    Align2::CENTER_TOP,
                    indexed.c,
                    self.options.font.font_type(),
                    fg,
                ));
            }
        }

        painter.extend(shapes);
    }
}

impl TerminalView<'_> {
    pub fn context_menu(&mut self, pos: Pos2, layout: &Response, ui: &mut egui::Ui) {
        Area::new(Id::new(format!("context_menu_{:?}", self.id())))
            .fixed_pos(pos)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let width = 200.;
                    ui.set_width(width);
                    // copy btn
                    self.copy_btn(ui, layout, width);
                    // paste btn
                    self.paste_btn(ui, width);

                    ui.separator();
                    // select all btn
                    self.select_all_btn(ui, width);

                    if let Some(session) = self.term_ctx.session {
                        ui.separator();

                        // sftp
                        self.sftp_btn(session, ui, width);
                    }
                });
            });
    }

    fn copy_btn(&mut self, ui: &mut egui::Ui, layout: &Response, btn_width: f32) {
        #[cfg(not(target_os = "macos"))]
        let copy_shortcut = KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, Key::C);
        #[cfg(target_os = "macos")]
        let copy_shortcut = KeyboardShortcut::new(Modifiers::MAC_CMD, Key::C);
        let copy_shortcut = ui.ctx().format_shortcut(&copy_shortcut);
        let copy_btn = context_btn("Copy", btn_width, Some(copy_shortcut));
        if ui.add(copy_btn).clicked() {
            let data = self.term_ctx.selection_content();
            layout.ctx.copy_text(data);
            ui.close_menu();
        }
    }

    fn paste_btn(&mut self, ui: &mut egui::Ui, btn_width: f32) {
        #[cfg(not(target_os = "macos"))]
        let paste_shortcut = KeyboardShortcut::new(Modifiers::CTRL, Key::V);
        #[cfg(target_os = "macos")]
        let paste_shortcut = KeyboardShortcut::new(Modifiers::MAC_CMD, Key::V);
        let paste_shortcut = ui.ctx().format_shortcut(&paste_shortcut);
        let paste_btn = context_btn("Paste", btn_width, Some(paste_shortcut));
        if ui.add(paste_btn).clicked() {
            if let Ok(data) = self.term_ctx.clipboard.get_contents() {
                self.term_ctx.write_data(data.into_bytes());
                self.term_ctx.terminal.selection = None;
            }
            ui.close_menu();
        }
    }

    fn select_all_btn(&mut self, ui: &mut egui::Ui, btn_width: f32) {
        #[cfg(not(target_os = "macos"))]
        let select_all_shortcut = KeyboardShortcut::new(Modifiers::CTRL, Key::A);
        #[cfg(target_os = "macos")]
        let select_all_shortcut = KeyboardShortcut::new(Modifiers::MAC_CMD, Key::A);
        let select_all_shortcut = ui.ctx().format_shortcut(&select_all_shortcut);
        let select_all_btn = context_btn("Select All", btn_width, Some(select_all_shortcut));
        if ui.add(select_all_btn).clicked() {
            self.term_ctx.select_all();
            ui.close_menu();
        }
    }

    fn sftp_btn(&mut self, session: &Session, ui: &mut egui::Ui, btn_width: f32) {
        let sftp_btn = context_btn("Sftp", btn_width, None);
        if ui.add(sftp_btn).clicked() {
            match self.term_ctx.open_sftp(session) {
                Ok(_) => {
                    *self.term_ctx.show_sftp_window = true;
                }
                Err(err) => error!("opening sftp error: {err}"),
            }
            ui.close_menu();
        }
    }
}

fn context_btn<'a>(
    text: impl Into<WidgetText>,
    width: f32,
    shortcut: Option<String>,
) -> Button<'a> {
    let mut btn = Button::new(text)
        .fill(Color32::TRANSPARENT)
        .min_size((width, 0.).into());
    if let Some(shortcut) = shortcut {
        btn = btn.shortcut_text(shortcut);
    }
    btn
}
