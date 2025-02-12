use copypasta::ClipboardContext;
use egui::{Id, Key, Modifiers, Vec2};
use egui_term::{
    generate_bindings, Binding, BindingAction, InputKind, KeyboardBinding, PtyEvent, TermMode,
    Terminal, TerminalContext, TerminalFont, TerminalOptions, TerminalTheme, TerminalView,
};
use std::sync::mpsc::Receiver;

pub struct App {
    terminal_backend: Terminal,
    terminal_font: TerminalFont,
    terminal_theme: TerminalTheme,
    multi_exec: bool,
    active_id: Option<Id>,
    clipboard: ClipboardContext,
    pty_proxy_receiver: Receiver<(u64, PtyEvent)>,
    custom_terminal_bindings: Vec<(Binding<InputKind>, BindingAction)>,
}

impl App {
    pub fn new(ctx: egui::Context) -> Self {
        let (pty_proxy_sender, pty_proxy_receiver) = std::sync::mpsc::channel();
        let terminal_backend = Terminal::new_regular(0, ctx, None, pty_proxy_sender).unwrap();

        let mut custom_terminal_bindings = vec![
            (
                Binding {
                    target: InputKind::KeyCode(Key::C),
                    modifiers: Modifiers::SHIFT,
                    term_mode_include: TermMode::ALT_SCREEN,
                    term_mode_exclude: TermMode::empty(),
                },
                BindingAction::Paste,
            ),
            (
                Binding {
                    target: InputKind::KeyCode(Key::A),
                    modifiers: Modifiers::SHIFT | Modifiers::CTRL,
                    term_mode_include: TermMode::empty(),
                    term_mode_exclude: TermMode::empty(),
                },
                BindingAction::Char('B'),
            ),
            (
                Binding {
                    target: InputKind::KeyCode(Key::B),
                    modifiers: Modifiers::SHIFT | Modifiers::CTRL,
                    term_mode_include: TermMode::empty(),
                    term_mode_exclude: TermMode::empty(),
                },
                BindingAction::Esc("\x1b[5~".into()),
            ),
        ];

        custom_terminal_bindings = [
            custom_terminal_bindings,
            // You can also use generate_bindings macros
            generate_bindings!(
                KeyboardBinding;
                L, Modifiers::SHIFT; BindingAction::Char('K');
            ),
        ]
        .concat();

        Self {
            terminal_backend,
            terminal_theme: TerminalTheme::default(),
            terminal_font: TerminalFont::default(),
            multi_exec: false,
            active_id: None,
            clipboard: ClipboardContext::new().unwrap(),
            pty_proxy_receiver,
            custom_terminal_bindings,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Ok((_, PtyEvent::Exit)) = self.pty_proxy_receiver.try_recv() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let term_ctx = TerminalContext::new(&mut self.terminal_backend, &mut self.clipboard);
            let term_opt = TerminalOptions {
                font: &mut self.terminal_font,
                multi_exec: &mut self.multi_exec,
                theme: &mut self.terminal_theme,
                default_font_size: 14.,
                active_tab_id: &mut self.active_id,
            };
            let terminal = TerminalView::new(ui, term_ctx, term_opt)
                .set_focus(true)
                .add_bindings(self.custom_terminal_bindings.clone())
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
        "custom_bindings_example",
        native_options,
        Box::new(|cc| Ok(Box::new(App::new(cc.egui_ctx.clone())))),
    )
}
