use crate::TerminalView;
use copypasta::ClipboardProvider;
use egui::{Button, Key, KeyboardShortcut, Modifiers, Response, WidgetText};

impl TerminalView<'_> {
    pub fn context_menu(&mut self, layout: &Response) {
        layout.context_menu(|ui| {
            let width = 200.;
            ui.set_width(width);
            // copy btn
            self.copy_btn(ui, layout, width);
            // paste btn
            self.paste_btn(ui, width);

            ui.separator();
            // select all btn
            self.select_all_btn(ui, width);
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
            ui.close();
        }
    }

    fn paste_btn(&mut self, ui: &mut egui::Ui, btn_width: f32) {
        #[cfg(not(target_os = "macos"))]
        let paste_shortcut = KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, Key::V);
        #[cfg(target_os = "macos")]
        let paste_shortcut = KeyboardShortcut::new(Modifiers::MAC_CMD, Key::V);
        let paste_shortcut = ui.ctx().format_shortcut(&paste_shortcut);
        let paste_btn = context_btn("Paste", btn_width, Some(paste_shortcut));
        if ui.add(paste_btn).clicked() {
            if let Ok(data) = self.term_ctx.clipboard.get_contents() {
                self.term_ctx.write_data(data.into_bytes());
                self.term_ctx.terminal.selection = None;
            }
            ui.close();
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
            ui.close();
        }
    }
}

fn context_btn<'a>(
    text: impl Into<WidgetText>,
    width: f32,
    shortcut: Option<String>,
) -> Button<'a> {
    let mut btn = Button::new(text).min_size((width, 0.).into());
    if let Some(shortcut) = shortcut {
        btn = btn.shortcut_text(shortcut);
    }
    btn
}
