use crate::db::{DbConn, Session};
use crate::errors::{error_toast, NxError};
use crate::ui::form::NxStateManager;
use crate::ui::tab_view::Tab;
use copypasta::ClipboardContext;
use eframe::{egui, NativeOptions};
use egui::{Align2, CollapsingHeader, FontData, FontId, Id};
use egui_dock::{DockState, NodeIndex, SurfaceIndex, TabIndex};
use egui_term::{FontSettings, PtyEvent, SshOptions, TermType, TerminalFont};
use egui_theme_switch::global_theme_switch;
use egui_toast::Toasts;
use orion::aead::{open, SecretKey};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct NxShellOptions {
    pub show_add_session_modal: Rc<RefCell<bool>>,
    pub show_dock_panel: bool,
    pub multi_exec: bool,
    pub active_tab_id: Option<Id>,
    pub term_font: TerminalFont,
    pub term_font_size: f32,
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
        egui::SidePanel::right("main_right_panel")
            .resizable(true)
            .width_range(200.0..=300.0)
            .show(ctx, |ui| {
                ui.label("Sessions");
                ui.separator();

                if let Some(sessions) = self.state_manager.sessions.take() {
                    for (group, sessions) in sessions.iter() {
                        CollapsingHeader::new(group)
                            .default_open(true)
                            .show(ui, |ui| {
                                for session in sessions {
                                    let response = ui.button(&session.name);
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
            });
        egui::TopBottomPanel::bottom("main_bottom_panel").show(ctx, |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                global_theme_switch(ui);
            });
        });

        if *self.opts.show_add_session_modal.borrow() {
            self.show_add_session_window(ctx, &mut toasts);
        } else {
            egui::CentralPanel::default().show(ctx, |_ui| {
                self.tab_view(ctx);
            });
        }

        toasts.show(ctx);
    }
}

impl NxShell {
    fn recv_event(&mut self) {
        if let Ok((tab_id, event)) = self.command_receiver.try_recv() {
            match event {
                PtyEvent::Exit => {
                    let mut index: Option<(SurfaceIndex, NodeIndex, TabIndex)> = None;
                    for ((surface, node), tab) in self.dock_state.iter_all_tabs() {
                        if tab.id() == tab_id {
                            index = Some((surface, node, TabIndex(tab.id() as usize)));
                            break;
                        }
                    }
                    if let Some(index) = index {
                        self.dock_state.remove_tab(index);
                    }
                }
                PtyEvent::Title(_title) => {
                    // change tab title
                }
                _ => {}
            }
        }
    }

    fn add_shell_tab_with_secret(
        &mut self,
        ctx: &egui::Context,
        session: Session,
    ) -> Result<(), NxError> {
        let key = SecretKey::from_slice(&session.secret_key)?;
        let password = open(&key, &session.secret_data)?;
        let password = String::from_utf8(password)?;
        self.add_shell_tab(
            ctx.clone(),
            TermType::Ssh {
                options: SshOptions {
                    host: session.host,
                    port: Some(session.port),
                    user: Some(session.username),
                    password: Some(password),
                },
            },
        )
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
    ctx.set_fonts(fonts);
}
