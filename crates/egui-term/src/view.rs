use crate::alacritty::{BackendCommand, TerminalContext};
use crate::bindings::Binding;
use crate::bindings::{BindingAction, Bindings, InputKind};
use crate::font::TerminalFont;
use crate::input::InputAction;
use crate::theme::TerminalTheme;
use crate::types::Size;
use alacritty_terminal::index::Point;
use egui::ImeEvent;
use egui::Widget;
use egui::{Context, Event};
use egui::{CursorIcon, Key};
use egui::{Id, Pos2};
use egui::{Response, Vec2};

#[derive(Clone, Default)]
pub struct TerminalViewState {
    pub is_dragged: bool,
    pub scroll_pixels: f32,
    // for terminal
    pub mouse_position: Point,
    pub cursor_position: Option<Pos2>,
    // ime_enabled: bool,
    // ime_cursor_range: CursorRange,
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
    pub active_tab_id: Option<&'a mut Id>,
}

impl TerminalOptions<'_> {
    pub fn surrender_focus(&mut self) {
        if let Some(active_tab_id) = self.active_tab_id.as_mut() {
            **active_tab_id = Id::NULL;
        }
    }
}

impl Widget for TerminalView<'_> {
    fn ui(mut self, ui: &mut egui::Ui) -> Response {
        let (layout, painter) = ui.allocate_painter(self.size, egui::Sense::click());

        let widget_id = self.widget_id;
        let mut state = TerminalViewState::load(ui.ctx(), widget_id);

        if layout.contains_pointer() {
            if let Some(tab_id) = self.options.active_tab_id.as_mut() {
                **tab_id = self.widget_id;
            }
            layout.ctx.set_cursor_icon(CursorIcon::Text);
        } else {
            layout.ctx.set_cursor_icon(CursorIcon::Default);
        }

        if self
            .options
            .active_tab_id
            .as_ref()
            .is_some_and(|id| **id == Id::NULL)
        {
            self.has_focus = false;
        }

        // context menu
        if let Some(pos) = state.cursor_position {
            self.context_menu(pos, &layout, ui);
        }
        if ui.input(|input_state| input_state.pointer.primary_clicked()) {
            state.cursor_position = None;
            ui.close_menu();
        }

        self.show_sftp_window(ui.ctx());

        self.focus(&layout)
            .resize(&layout)
            .process_input(&mut state, &layout)
            .show(&mut state, &layout, &painter);

        state.store(ui.ctx(), widget_id);
        layout
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

        if let Some(tab_id) = self.options.active_tab_id.as_ref() {
            if **tab_id != self.widget_id && !*self.options.multi_exec {
                return self;
            }
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
                    if let Some(action) =
                        self.button_click(state, layout, button, pos, &modifiers, pressed)
                    {
                        input_actions.push(action);
                    }
                }
                Event::PointerMoved(pos) => {
                    input_actions = self.mouse_move(state, layout, pos, &modifiers)
                }
                Event::Ime(event) => match event {
                    ImeEvent::Disabled => {
                        // state.ime_enabled = false;
                    }
                    ImeEvent::Enabled | ImeEvent::Preedit(_) => {
                        // state.ime_enabled = true;
                    }
                    ImeEvent::Commit(text) => {
                        input_actions.push(self.text_input(&text));
                    }
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
