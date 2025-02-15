use crate::app::NxShell;
use crate::consts::{REPOSITORY_URL, SHOW_DOCK_PANEL_ONCE};
use crate::db::Session;
use crate::errors::NxError;
use crate::ui::tab_view::Tab;
use egui::{Button, Checkbox, Modifiers};
use egui_dock::DockState;
use egui_term::{Authentication, SshOptions, TermType};
use orion::aead::{open as orion_open, SecretKey};
use std::env;
use std::process::Command;
use tracing::error;

use super::form::AuthType;

const BTN_WIDTH: f32 = 200.0;

impl NxShell {
    pub fn menubar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            // Session
            self.session_menu(ui);
            // Window
            window_menu(ui);
            // Tools
            self.tools_menu(ui);
            // Help
            help_menu(ui);
        });
    }

    fn session_menu(&mut self, ui: &mut egui::Ui) {
        let new_term_shortcut = egui::KeyboardShortcut::new(Modifiers::CTRL, egui::Key::N);
        if ui.input_mut(|i| i.consume_shortcut(&new_term_shortcut)) {
            let _ = self.add_shell_tab(
                ui.ctx().clone(),
                TermType::Regular {
                    working_directory: None,
                },
            );
        }
        ui.menu_button("Session", |ui| {
            let new_session_btn = Button::new("New Session").min_size((BTN_WIDTH, 0.).into());
            if ui.add(new_session_btn).clicked() {
                *self.opts.show_add_session_modal.borrow_mut() = true;
                ui.close_menu();
            }
            let new_term_shortcut = ui.ctx().format_shortcut(&new_term_shortcut);
            let new_term_btn = Button::new("New Terminal")
                .min_size((BTN_WIDTH, 0.).into())
                .shortcut_text(new_term_shortcut);
            if ui.add(new_term_btn).clicked() {
                let _ = self.add_shell_tab(
                    ui.ctx().clone(),
                    TermType::Regular {
                        working_directory: None,
                    },
                );
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Quit").clicked() {
                std::process::exit(0);
            }
        });
    }

    fn tools_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Tools", |ui| {
            ui.add(Checkbox::new(&mut self.opts.multi_exec, "Multi Exec"));
        });
    }
}

impl NxShell {
    pub fn add_shell_tab(&mut self, ctx: egui::Context, typ: TermType) -> Result<(), NxError> {
        if self.dock_state.surfaces_count() == 0 {
            self.dock_state = DockState::new(vec![]);
        }
        SHOW_DOCK_PANEL_ONCE.call_once(|| {
            self.opts.show_dock_panel = true;
        });
        match Tab::term(ctx, typ, self.command_sender.clone()) {
            Ok(tab) => {
                self.dock_state.push_to_focused_leaf(tab);
                Ok(())
            }
            Err(err) => {
                error!("add session error: {err}");
                Err(NxError::Plain(err.to_string()))
            }
        }
    }

    pub fn add_shell_tab_with_secret(
        &mut self,
        ctx: &egui::Context,
        session: Session,
    ) -> Result<(), NxError> {
        let auth = match AuthType::from(session.auth_type) {
            AuthType::Password => {
                let key = SecretKey::from_slice(&session.secret_key)?;
                let auth_data = orion_open(&key, &session.secret_data)?;
                let auth_data = String::from_utf8(auth_data)?;

                Authentication::Password(session.username, auth_data)
            }
            AuthType::PublicKey => Authentication::PublicKey,
        };

        self.add_shell_tab(
            ctx.clone(),
            TermType::Ssh {
                options: SshOptions {
                    group: session.group,
                    name: session.name,
                    host: session.host,
                    port: Some(session.port),
                    auth,
                },
            },
        )
    }

    pub fn add_sessions_tab(&mut self) {
        if self.dock_state.surfaces_count() == 0 {
            self.dock_state = DockState::new(vec![]);
        }
        SHOW_DOCK_PANEL_ONCE.call_once(|| {
            self.opts.show_dock_panel = true;
        });
        self.dock_state.push_to_focused_leaf(Tab::session_list());
    }
}

fn window_menu(ui: &mut egui::Ui) {
    ui.menu_button("Window", |ui| {
        let new_window_btn = Button::new("New Window").min_size((BTN_WIDTH, 0.).into());
        if ui.add(new_window_btn).clicked() {
            match env::current_exe() {
                Ok(path) => {
                    let mut child = Command::new(path);

                    #[cfg(windows)]
                    {
                        use std::os::windows::process::CommandExt;
                        use windows::Win32::System::Threading::CREATE_NEW_PROCESS_GROUP;

                        child.creation_flags(CREATE_NEW_PROCESS_GROUP.0 as u32);
                    }

                    #[cfg(unix)]
                    {
                        use std::os::unix::prelude::CommandExt;
                        unsafe {
                            child.pre_exec(|| {
                                let _ = rustix::process::setsid();
                                Ok(())
                            });
                        }
                    }

                    if let Err(err) = child.spawn() {
                        error!("failed to launch new window: {err}");
                    }
                }
                Err(err) => error!("failed to get current exe path: {err}"),
            }
            ui.close_menu();
        }
    });
}

fn help_menu(ui: &mut egui::Ui) {
    ui.menu_button("Help", |ui| {
        let about_btn = Button::new("About").min_size((BTN_WIDTH, 0.).into());
        if ui.add(about_btn).clicked() {
            if let Err(err) = open::that(REPOSITORY_URL) {
                error!("opening page {REPOSITORY_URL} error: {err}");
            }
        }
    });
}
