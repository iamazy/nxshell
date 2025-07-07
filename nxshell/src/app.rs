use crate::db::DbConn;
use crate::errors::{error_toast, NxError};
use crate::ui::form::{AuthType, NxStateManager};
use crate::ui::side_panel::SidePanel;
use crate::ui::tab_view::{Tab, TabEvent};
use copypasta::ClipboardContext;
use eframe::{egui, NativeOptions};
use egui::{Align2, CollapsingHeader, FontData, FontId, Id, TextEdit};
use egui_dock::{DockState, NodeIndex, SurfaceIndex, TabIndex};
use egui_phosphor::regular::{DRONE, NUMPAD};
use egui_term::{FontSettings, PtyEvent, TerminalFont};
use egui_theme_switch::global_theme_switch;
use egui_toast::Toasts;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct NxShellOptions {
    pub show_add_session_modal: Rc<RefCell<bool>>,
    pub show_dock_panel: bool,
    pub multi_exec: bool,
    /// Id of active tab
    ///
    /// Its main purpose is to preserve the state of egui::Response::contains_pointer().
    /// Its functions :
    ///
    /// 1. When the mouse cursor leaves the terminal, it still influences the state of the current
    ///    terminal's selection.
    /// 2. When it is None, all tabs lose focus, and you can iteract with the other UI components.
    pub active_tab_id: Option<Id>,
    pub term_font: TerminalFont,
    pub term_font_size: f32,
    pub session_filter: String,

    pub side_panel: SidePanel,

    pub show_rename_view: Rc<RefCell<bool>>,
    pub renaming_tab_id: Option<u64>,
    pub tab_events: Vec<TabEvent>,

    pub search_start: bool,
    pub search_regex: String,
}

impl NxShellOptions {
    pub fn surrender_focus(&mut self) {
        self.active_tab_id = None;
    }
}

impl Default for NxShellOptions {
    fn default() -> Self {
        let term_font_size = 14.;
        let font_setting = FontSettings {
            font_type: FontId::monospace(term_font_size),
        };
        Self {
            show_add_session_modal: Rc::new(RefCell::new(false)),
            show_dock_panel: false,
            active_tab_id: None,
            multi_exec: false,
            term_font: TerminalFont::new(font_setting),
            term_font_size,
            session_filter: String::default(),
            side_panel: SidePanel::new(true),
            show_rename_view: Rc::new(RefCell::new(false)),
            renaming_tab_id: None,
            tab_events: Vec::new(),
            search_start: false,
            search_regex: String::default(),
        }
    }
}

pub struct NxShell {
    pub state_manager: NxStateManager,
    pub dock_state: DockState<Tab>,
    pub command_sender: Sender<(u64, PtyEvent)>,
    pub command_receiver: Receiver<(u64, PtyEvent)>,
    pub clipboard: ClipboardContext,
    pub db: DbConn,
    pub opts: NxShellOptions,
}

impl NxShell {
    fn new() -> Result<Self, NxError> {
        let (command_sender, command_receiver) = std::sync::mpsc::channel();
        let dock_state = DockState::new(vec![]);
        let db = DbConn::open()?;
        let state_manager = NxStateManager {
            sessions: Some(db.find_all_sessions()?),
        };
        Ok(Self {
            command_sender,
            command_receiver,
            dock_state,
            clipboard: ClipboardContext::new()?,
            db,
            opts: NxShellOptions {
                term_font: TerminalFont::new(FontSettings {
                    font_type: FontId::monospace(14.),
                }),
                ..Default::default()
            },
            state_manager,
        })
    }
    pub fn start(options: NativeOptions) -> eframe::Result<()> {
        eframe::run_native(
            "NxShell",
            options,
            Box::new(|cc| {
                catppuccin_egui::set_theme(&cc.egui_ctx, catppuccin_egui::FRAPPE);
                egui_extras::install_image_loaders(&cc.egui_ctx);
                set_font(&cc.egui_ctx);
                cc.egui_ctx
                    .options_mut(|opt| opt.zoom_with_keyboard = false);
                Ok(Box::new(NxShell::new()?))
            }),
        )
    }
}

impl eframe::App for NxShell {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.recv_event();

        let mut toasts = Toasts::new()
            .anchor(Align2::CENTER_CENTER, (10.0, 10.0))
            .direction(egui::Direction::TopDown);

        egui::TopBottomPanel::top("main_top_panel").show(ctx, |ui| {
            self.menubar(ui);
        });

        if self.opts.side_panel.show_right_panel {
            let side_panel_response = egui::SidePanel::right("main_right_panel")
                .resizable(true)
                .width_range(self.opts.side_panel.min_panel_width..=SidePanel::MAX_WIDTH)
                .default_width(SidePanel::DEFAULT_WIDTH)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                            ui.label("Sessions");
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Max), |ui| {
                            if ui.button("X").clicked() {
                                self.opts.side_panel.show_right_panel = false;
                            }
                        });
                    });

                    self.search_sessions(ui);
                    ui.separator();
                    self.list_sessions(ctx, ui, &mut toasts);
                });

            if side_panel_response.response.rect.width() <= SidePanel::CLOSE_WIDTH {
                self.opts.side_panel.show_right_panel = false;
                self.opts.side_panel.min_panel_width = SidePanel::DEFAULT_WIDTH;
            } else {
                self.opts.side_panel.min_panel_width = SidePanel::MIN_WIDTH;
            }
        }

        egui::TopBottomPanel::bottom("main_bottom_panel").show(ctx, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                global_theme_switch(ui);
            });
        });

        if *self.opts.show_add_session_modal.borrow() {
            self.opts.surrender_focus();
            self.show_add_session_window(ctx, &mut toasts);
        }

        egui::CentralPanel::default().show(ctx, |_ui| {
            self.tab_view(ctx);
        });

        self.rename_tab_view(ctx);

        toasts.show(ctx);
    }
}

impl NxShell {
    fn search_sessions(&mut self, ui: &mut egui::Ui) {
        let text_edit = TextEdit::singleline(&mut self.opts.session_filter);
        let response = ui.add(text_edit);
        if response.clicked() {
            self.opts.surrender_focus();
        } else if response.changed() {
            if let Ok(sessions) = self.db.find_sessions(&self.opts.session_filter) {
                self.state_manager.sessions = Some(sessions);
            }
        }
    }

    fn list_sessions(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, toasts: &mut Toasts) {
        if let Some(sessions) = self.state_manager.sessions.take() {
            for (group, sessions) in sessions.iter() {
                CollapsingHeader::new(group)
                    .default_open(true)
                    .show(ui, |ui| {
                        for session in sessions {
                            let icon = match AuthType::from(session.auth_type) {
                                AuthType::Password => NUMPAD,
                                AuthType::Config => DRONE,
                            };
                            let response = ui.button(format!("{icon} {}", session.name));
                            if response.double_clicked() {
                                match self.db.find_session(&session.group, &session.name) {
                                    Ok(Some(session)) => {
                                        if let Err(err) =
                                            self.add_shell_tab_with_secret(ctx, session)
                                        {
                                            toasts.add(error_toast(err.to_string()));
                                        }
                                    }
                                    Ok(None) => {}
                                    Err(err) => {
                                        toasts.add(error_toast(err.to_string()));
                                    }
                                }
                            } else if response.secondary_clicked() {
                            }
                        }
                    });
            }
            self.state_manager.sessions = Some(sessions);
        }
    }
}

impl NxShell {
    fn recv_event(&mut self) {
        if let Ok((tab_id, PtyEvent::Exit)) = self.command_receiver.try_recv() {
            let mut index: Option<(SurfaceIndex, NodeIndex, TabIndex)> = None;
            for (_, tab) in self.dock_state.iter_all_tabs() {
                if tab.id() == tab_id {
                    index = self.dock_state.find_tab(tab);
                    break;
                }
            }
            if let Some(index) = index {
                self.dock_state.remove_tab(index);
            }
        }
    }
}

fn set_font(ctx: &egui::Context) {
    let name = "仓耳舒圆体";
    let font = include_bytes!("../assets/fonts/仓耳舒圆体W01.ttf");
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert(name.to_owned(), Arc::new(FontData::from_static(font)));
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push(name.to_owned());

    // add egui icon
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    ctx.set_fonts(fonts);
}
