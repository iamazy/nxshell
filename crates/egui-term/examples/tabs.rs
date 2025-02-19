use copypasta::ClipboardContext;
use eframe::glow;
use egui_term::{
    PtyEvent, Terminal, TerminalContext, TerminalFont, TerminalOptions, TerminalTheme, TerminalView,
};
use std::{
    collections::BTreeMap,
    sync::mpsc::{self, Receiver, Sender},
};

pub struct App {
    command_sender: Sender<(u64, PtyEvent)>,
    command_receiver: Receiver<(u64, PtyEvent)>,
    tab_manager: TabManager,
    multi_exec: bool,
    clipboard: ClipboardContext,
}

impl App {
    pub fn new(_: &eframe::CreationContext<'_>) -> Self {
        let (command_sender, command_receiver) = mpsc::channel();
        Self {
            command_sender,
            command_receiver,
            tab_manager: TabManager::new(),
            multi_exec: false,
            clipboard: ClipboardContext::new().unwrap(),
        }
    }
}

impl eframe::App for App {
    fn on_exit(&mut self, _gl: Option<&glow::Context>) {
        self.tab_manager.clear();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Ok((tab_id, event)) = self.command_receiver.try_recv() {
            match event {
                PtyEvent::Exit => {
                    self.tab_manager.remove(tab_id);
                }
                PtyEvent::Title(title) => {
                    self.tab_manager.set_title(tab_id, title);
                }
                _ => {}
            }
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let tab_ids = self.tab_manager.get_tab_ids();
                for id in tab_ids {
                    let tab_title = if let Some(title) = self.tab_manager.get_title(id) {
                        title
                    } else {
                        String::from("unknown")
                    };
                    if ui.button(tab_title).clicked() {
                        self.tab_manager.set_active(id);
                    }
                }

                if ui.button("[+]").clicked() {
                    self.tab_manager
                        .add(ctx.clone(), self.command_sender.clone());
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(tab) = self.tab_manager.get_active() {
                let term_ctx = TerminalContext::new(&mut tab.backend, &mut self.clipboard);
                let term_opt = TerminalOptions {
                    font: &mut tab.font,
                    multi_exec: &mut self.multi_exec,
                    theme: &mut tab.theme,
                    default_font_size: 14.,
                    active_tab_id: None,
                };
                let terminal = TerminalView::new(ui, term_ctx, term_opt)
                    .set_focus(true)
                    .set_size(ui.available_size());

                ui.add(terminal);
            }
        });
    }
}

struct TabManager {
    active_tab_id: Option<u64>,
    tabs: BTreeMap<u64, Tab>,
}

impl TabManager {
    fn new() -> Self {
        Self {
            active_tab_id: None,
            tabs: BTreeMap::new(),
        }
    }

    fn add(&mut self, ctx: egui::Context, command_sender: Sender<(u64, PtyEvent)>) {
        let id = self.tabs.len() as u64;
        let tab = Tab::new(ctx, command_sender, id);
        self.tabs.insert(id, tab);
        self.active_tab_id = Some(id)
    }

    fn remove(&mut self, id: u64) {
        if self.tabs.is_empty() {
            return;
        }

        self.tabs.remove(&id).unwrap();
        self.active_tab_id = if let Some(next_tab) = self.tabs.iter().find(|t| t.0 <= &id) {
            Some(*next_tab.0)
        } else {
            self.tabs.last_key_value().map(|last_tab| *last_tab.0)
        };
    }

    fn clear(&mut self) {
        self.tabs.clear();
    }

    fn set_title(&mut self, id: u64, title: String) {
        if let Some(tab) = self.tabs.get_mut(&id) {
            tab.set_title(title);
        }
    }

    fn get_title(&mut self, id: u64) -> Option<String> {
        self.tabs.get(&id).map(|tab| tab.title.clone())
    }

    fn get_active(&mut self) -> Option<&mut Tab> {
        self.active_tab_id?;

        if let Some(tab) = self.tabs.get_mut(&self.active_tab_id.unwrap()) {
            return Some(tab);
        }

        None
    }

    fn get_tab_ids(&self) -> Vec<u64> {
        self.tabs.keys().copied().collect()
    }

    fn set_active(&mut self, id: u64) {
        if id as usize > self.tabs.len() {
            return;
        }

        self.active_tab_id = Some(id);
    }
}

struct Tab {
    backend: Terminal,
    theme: TerminalTheme,
    font: TerminalFont,
    title: String,
}

impl Tab {
    fn new(ctx: egui::Context, command_sender: Sender<(u64, PtyEvent)>, id: u64) -> Self {
        let backend = Terminal::new_regular(id, ctx, None, command_sender).unwrap();

        Self {
            backend,
            theme: TerminalTheme::default(),
            font: TerminalFont::default(),
            title: format!("tab: {}", id),
        }
    }

    fn set_title(&mut self, title: String) {
        self.title = title;
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
        "tabs_example",
        native_options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
