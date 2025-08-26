use crate::alacritty::{BackendCommand, TerminalContext};
use crate::bindings::Binding;
use crate::bindings::{BindingAction, Bindings, InputKind};
use crate::font::TerminalFont;
use crate::input::{is_in_terminal, InputAction};
use crate::scroll_bar::{InteractiveScrollbar, ScrollbarState};
use crate::theme::TerminalTheme;
use crate::types::Size;
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::Point;
use alacritty_terminal::vte::ansi::{Color, NamedColor};
use egui::output::IMEOutput;
use egui::Widget;
use egui::{Context, Event};
use egui::{CursorIcon, Key};
use egui::{Id, Pos2};
use egui::{ImeEvent, Rect};
use egui::{Response, Vec2};

#[derive(Clone, Default)]
pub struct TerminalViewState {
    pub is_dragged: bool,
    // for terminal
    pub mouse_point: Point,
    pub mouse_position: Option<Pos2>,
    pub cursor_position: Option<Pos2>,
    pub scrollbar_state: ScrollbarState,
}

impl TerminalViewState {
    pub fn load(ctx: &Context, id: Id) -> Self {
        ctx.data_mut(|d| d.get_temp::<Self>(id).unwrap_or_default())
    }

    pub fn store(self, ctx: &Context, id: Id) {
        ctx.data_mut(|d| d.insert_temp(id, self));
    }

    pub fn remove(self, ctx: &Context, id: Id) {
        ctx.data_mut(|d| d.remove_temp::<Self>(id));
    }
}

pub struct TerminalView<'a> {
    widget_id: Id,
    has_focus: bool,
    size: Vec2,
    pub options: TerminalOptions<'a>,
    pub term_ctx: TerminalContext<'a>,
    pub bindings_layout: Bindings,
}

pub struct TerminalOptions<'a> {
    pub default_font_size: f32,
    pub font: &'a mut TerminalFont,
    pub multi_exec: &'a mut bool,
    pub theme: &'a mut TerminalTheme,
    pub active_tab_id: &'a mut Option<Id>,
}

impl Widget for TerminalView<'_> {
    fn ui(mut self, ui: &mut egui::Ui) -> Response {
        let widget_id = self.widget_id;
        let mut state = TerminalViewState::load(ui.ctx(), widget_id);

        ui.horizontal(|ui| {
            let size_p = Vec2::new(self.size.x - InteractiveScrollbar::WIDTH, self.size.y);
            let (layout, painter) = ui.allocate_painter(size_p, egui::Sense::click());

            if layout.contains_pointer() {
                *self.options.active_tab_id = Some(self.widget_id);
                layout.ctx.set_cursor_icon(CursorIcon::Text);
            } else {
                layout.ctx.set_cursor_icon(CursorIcon::Default);
            }

            if self.options.active_tab_id.is_none() {
                self.has_focus = false;
            }

            self.context_menu(&layout);

            let background = self.theme().get_color(Color::Named(NamedColor::Background));

            let mut term = self
                .focus(&layout)
                .resize(&layout)
                .process_input(&mut state, &layout);

            if let Some(pos) = state.mouse_position {
                if is_in_terminal(pos, layout.rect) {
                    if let Some(cur_pos) = state.cursor_position {
                        ui.ctx().output_mut(|output| {
                            let vec = Vec2::new(15., 15.);
                            output.ime = Some(IMEOutput {
                                rect: Rect::from_min_size(cur_pos, vec),
                                cursor_rect: Rect::from_min_size(cur_pos, vec),
                            })
                        });
                    }
                }
            }

            let grid = term.term_ctx.terminal.grid_mut();
            let total_lines = grid.total_lines() as f32;
            let display_offset = grid.display_offset() as f32;
            let cell_height = term.term_ctx.size.cell_height as f32;
            let total_height = cell_height * total_lines;
            let display_offset_pos = display_offset * cell_height;

            let mut scrollbar = InteractiveScrollbar::new(background);
            scrollbar.set_first_row_pos(display_offset_pos);
            scrollbar.ui(total_height, ui);
            if let Some(new_first_row_pos) = scrollbar.new_first_row_pos {
                let total_row_pos = new_first_row_pos + state.scrollbar_state.scroll_pixels;
                let new_pos = total_row_pos / cell_height;
                state.scrollbar_state.scroll_pixels = total_row_pos % cell_height;
                let line_diff = new_pos - display_offset;
                let line_delta = Scroll::Delta(line_diff.ceil() as i32);
                grid.scroll_display(line_delta);
            }

            term.show(&mut state, &layout, &painter);

            state.store(ui.ctx(), widget_id);
            layout
        })
        .inner
    }
}

impl<'a> TerminalView<'a> {
    pub fn new(
        ui: &mut egui::Ui,
        term_ctx: TerminalContext<'a>,
        options: TerminalOptions<'a>,
    ) -> Self {
        let widget_id = ui.make_persistent_id(term_ctx.id);

        Self {
            widget_id,
            has_focus: false,
            size: ui.available_size(),
            term_ctx,
            options,
            bindings_layout: Bindings::new(),
        }
    }

    pub fn id(&self) -> Id {
        self.widget_id
    }

    pub fn theme(&self) -> &TerminalTheme {
        self.options.theme
    }

    #[inline]
    pub fn set_theme(self, theme: TerminalTheme) -> Self {
        *self.options.theme = theme;
        self
    }

    #[inline]
    pub fn set_focus(mut self, has_focus: bool) -> Self {
        self.has_focus = has_focus;
        self
    }

    #[inline]
    pub fn set_size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    #[inline]
    pub fn add_bindings(mut self, bindings: Vec<(Binding<InputKind>, BindingAction)>) -> Self {
        self.bindings_layout.add_bindings(bindings);
        self
    }

    fn focus(self, layout: &Response) -> Self {
        if self.has_focus {
            layout.request_focus();
        } else {
            layout.surrender_focus();
        }

        self
    }

    fn resize(mut self, layout: &Response) -> Self {
        self.term_ctx.process_command(BackendCommand::Resize(
            Size::from(layout.rect.size()),
            self.options.font.font_measure(&layout.ctx),
        ));

        self
    }

    fn process_input(mut self, state: &mut TerminalViewState, layout: &Response) -> Self {
        if !layout.has_focus() {
            return self;
        }
        if self.options.active_tab_id != &Some(self.widget_id) && !*self.options.multi_exec {
            return self;
        }

        let modifiers = layout.ctx.input(|i| i.modifiers);
        let events = layout.ctx.input(|i| i.events.clone());

        for event in events {
            let mut input_actions = vec![];
            match event {
                Event::Text(text) | Event::Paste(text) => {
                    input_actions.push(self.text_input(&text));
                }
                Event::Copy => {
                    if let Some(action) = self.keyboard_input(Key::C, modifiers, true) {
                        input_actions.push(action);
                    }
                }
                Event::Key {
                    key,
                    pressed,
                    modifiers,
                    ..
                } => {
                    if let Some(action) = self.keyboard_input(key, modifiers, pressed) {
                        input_actions.push(action);
                    }
                }
                Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                } => {
                    if let Some(action) = self.mouse_wheel_input(state, unit, delta, modifiers) {
                        input_actions.push(action);
                    }
                }
                Event::PointerButton {
                    button,
                    pressed,
                    modifiers,
                    pos,
                } => {
                    let out_of = !is_in_terminal(pos, layout.rect);
                    if out_of && pressed {
                        continue;
                    }

                    let new_pos = if out_of {
                        pos.clamp(layout.rect.min, layout.rect.max)
                    } else {
                        pos
                    };

                    if let Some(action) =
                        self.button_click(state, layout, button, new_pos, &modifiers, pressed)
                    {
                        input_actions.push(action);
                    }
                }
                Event::PointerMoved(pos) => {
                    input_actions = self.mouse_move(state, layout, pos, &modifiers)
                }
                Event::Ime(event) => match event {
                    ImeEvent::Preedit(text_mark) => {
                        if text_mark != "\n" && text_mark != "\r" {
                            // TODO: handle preedit
                        }
                    }
                    ImeEvent::Commit(prediction) => {
                        if prediction != "\n" && prediction != "\r" {
                            input_actions.push(self.text_input(&prediction));
                        }
                    }
                    _ => {}
                },
                _ => {}
            };

            for action in input_actions {
                match action {
                    InputAction::BackendCall(cmd) => {
                        self.term_ctx.process_command(cmd);
                    }
                    InputAction::WriteToClipboard(data) => {
                        layout.ctx.copy_text(data);
                    }
                }
            }
        }

        self
    }
}
