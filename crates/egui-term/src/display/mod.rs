#![allow(dead_code)]
mod color;

use crate::display::color::HOVERED_HYPERLINK_COLOR;
use crate::view::TerminalViewState;
use crate::TerminalView;
use alacritty_terminal::grid::GridCell;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::{Color, NamedColor};
use egui::epaint::RectShape;
use egui::{Align2, CornerRadius, CursorIcon, Painter, Pos2, Rect, Response, Vec2};
use egui::{Shape, Stroke};

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
            let is_hovered_hyperlink = self
                .term_ctx
                .hovered_hyperlink
                .as_ref()
                .is_some_and(|r| r.contains(&indexed.point) && r.contains(&state.mouse_point));
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

                state.cursor_position = Some(Pos2::new(x, y));
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
