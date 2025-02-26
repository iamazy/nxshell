use copypasta::ClipboardContext;
use egui::Vec2;
use egui_term::{
    ColorPalette, PtyEvent, Terminal, TerminalContext, TerminalFont, TerminalOptions,
    TerminalTheme, TerminalView,
};
use std::sync::mpsc::Receiver;

pub struct App {
    terminal_backend: Terminal,
    terminal_font: TerminalFont,
    terminal_theme: TerminalTheme,
    multi_exec: bool,
    clipboard: ClipboardContext,
    pty_proxy_receiver: Receiver<(u64, PtyEvent)>,
}

impl App {
    pub fn new(ctx: egui::Context) -> Self {
        let (pty_proxy_sender, pty_proxy_receiver) = std::sync::mpsc::channel();
        let terminal_backend =
            Terminal::new_regular(0, ctx, None, pty_proxy_sender.clone()).unwrap();

        Self {
            terminal_backend,
            multi_exec: false,
            clipboard: ClipboardContext::new().unwrap(),
            terminal_font: TerminalFont::default(),
            terminal_theme: TerminalTheme::default(),
            pty_proxy_receiver,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Ok((_, PtyEvent::Exit)) = self.pty_proxy_receiver.try_recv() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("ubuntu").clicked() {
                    self.terminal_theme = TerminalTheme::default();
                }

                if ui.button("3024 Day").clicked() {
                    self.terminal_theme = TerminalTheme::new(Box::new(ColorPalette {
                        background: String::from("#F7F7F7"),
                        foreground: String::from("#4A4543"),
                        black: String::from("#090300"),
                        red: String::from("#DB2D20"),
                        green: String::from("#01A252"),
                        yellow: String::from("#FDED02"),
                        blue: String::from("#01A0E4"),
                        magenta: String::from("#A16A94"),
                        cyan: String::from("#B5E4F4"),
                        white: String::from("#A5A2A2"),
                        bright_black: String::from("#5C5855"),
                        bright_red: String::from("#E8BBD0"),
                        bright_green: String::from("#3A3432"),
                        bright_yellow: String::from("#4A4543"),
                        bright_blue: String::from("#807D7C"),
                        bright_magenta: String::from("#D6D5D4"),
                        bright_cyan: String::from("#CDAB53"),
                        bright_white: String::from("#F7F7F7"),
                        ..Default::default()
                    }));
                }

                if ui.button("ubuntu").clicked() {
                    self.terminal_theme = TerminalTheme::new(Box::new(ColorPalette {
                        background: String::from("#300A24"),
                        foreground: String::from("#FFFFFF"),
                        black: String::from("#2E3436"),
                        red: String::from("#CC0000"),
                        green: String::from("#4E9A06"),
                        yellow: String::from("#C4A000"),
                        blue: String::from("#3465A4"),
                        magenta: String::from("#75507B"),
                        cyan: String::from("#06989A"),
                        white: String::from("#D3D7CF"),
                        bright_black: String::from("#555753"),
                        bright_red: String::from("#EF2929"),
                        bright_green: String::from("#8AE234"),
                        bright_yellow: String::from("#FCE94F"),
                        bright_blue: String::from("#729FCF"),
                        bright_magenta: String::from("#AD7FA8"),
                        bright_cyan: String::from("#34E2E2"),
                        bright_white: String::from("#EEEEEC"),
                        ..Default::default()
                    }));
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let term_ctx =
                TerminalContext::new(&mut self.terminal_backend, &mut self.clipboard, &mut false);
            let term_opt = TerminalOptions {
                font: &mut self.terminal_font,
                multi_exec: &mut self.multi_exec,
                theme: &mut self.terminal_theme,
                default_font_size: 14.,
                active_tab_id: None,
            };
            let terminal = TerminalView::new(ui, term_ctx, term_opt)
                .set_focus(true)
                .set_size(Vec2::new(ui.available_width(), ui.available_height()));

            ui.add(terminal);
        });
    }
}

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0]),
        ..Default::default()
    };

    eframe::run_native(
        "themes_example",
        native_options,
        Box::new(|cc| Ok(Box::new(App::new(cc.egui_ctx.clone())))),
    )
}
