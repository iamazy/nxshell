use crate::app::NxShell;
use crate::db::Session;
use crate::errors::{error_toast, NxError};
use egui::{
    Align2, CentralPanel, ComboBox, Context, Grid, Id, Layout, Order, TextEdit, TopBottomPanel,
    Window,
};
use egui_form::garde::GardeReport;
use egui_form::{Form, FormField};
use egui_term::{Authentication, SshOptions, TermType};
use egui_toast::Toasts;
use garde::Validate;
use orion::aead::{seal, SecretKey};
use std::fmt::Display;
use tracing::error;

#[derive(Debug, Clone, Validate)]
pub struct SessionState {
    #[garde(length(min = 0, max = 256))]
    pub group: String,
    #[garde(length(min = 0, max = 256))]
    pub name: String,
    #[garde(length(min = 1))]
    pub host: String,
    #[garde(range(min = 1, max = 65535))]
    pub port: u16,
    #[garde(skip)]
    pub auth_type: AuthType,
    #[garde(skip)]
    pub username: String,
    #[garde(skip)]
    pub auth_data: String,
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, Default, Hash, PartialEq)]
pub enum AuthType {
    #[default]
    Password = 0,
    Config = 1,
}

impl Display for AuthType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthType::Password => write!(f, "Password"),
            AuthType::Config => write!(f, "SSH Config"),
        }
    }
}

impl From<u16> for AuthType {
    fn from(value: u16) -> Self {
        match value {
            0 => AuthType::Password,
            _ => AuthType::Config,
        }
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            group: String::default(),
            name: String::default(),
            host: String::default(),
            port: 22,
            auth_type: AuthType::Password,
            username: String::default(),
            auth_data: String::default(),
        }
    }
}

impl SessionState {
    pub fn id() -> &'static str {
        "ssh-session"
    }

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

impl NxShell {
    pub fn show_add_session_window(&mut self, ctx: &Context, toasts: &mut Toasts) {
        let session_id = Id::new(SessionState::id());
        let mut session_state = SessionState::load(ctx, session_id);

        let show_add_session_modal = self.opts.show_add_session_modal.clone();
        let mut should_close = false;

        Window::new("New Session")
            .order(Order::Middle)
            .open(&mut show_add_session_modal.borrow_mut())
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([450., 400.])
            .show(ctx, |ui| {
                let validator = session_state.validate();
                let mut form = Form::new().add_report(GardeReport::new(validator));

                TopBottomPanel::bottom("session_modal_bottom_panel").show_inside(ui, |ui| {
                    ui.with_layout(Layout::right_to_left(egui::Align::TOP), |ui| {
                        if let Some(Ok(())) = form.handle_submit(&ui.button("Submit"), ui) {
                            match self.submit_session(ctx, &mut session_state) {
                                Ok(_) => should_close = true,
                                Err(err) => {
                                    error!("failed to add session: {err}");
                                    toasts.add(error_toast(err.to_string()));
                                }
                            }
                        }
                    });
                });

                CentralPanel::default().show_inside(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.add_space(20.);
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(20.);

                        self.ssh_form(ui, &mut form, &mut session_state);
                    });
                });
            });

        if should_close {
            *self.opts.show_add_session_modal.borrow_mut() = false;
            session_state.remove(ctx, session_id);
        } else {
            session_state.store(ctx, session_id);
        }
    }

    fn submit_session(&mut self, ctx: &Context, session: &mut SessionState) -> Result<(), NxError> {
        let (auth, secret_key, secret_data) = match session.auth_type {
            AuthType::Password => {
                if session.username.trim().is_empty() || session.auth_data.trim().is_empty() {
                    return Err(NxError::Plain(
                        "`username` and `password` cannot be empty in `Password` mode".to_string(),
                    ));
                }

                let secret_key = SecretKey::generate(32)?;
                let secret_data = seal(&secret_key, session.auth_data.as_bytes())?;
                let secret_key = secret_key.unprotected_as_bytes().to_vec();

                (
                    Authentication::Password(
                        session.username.to_string(),
                        session.auth_data.to_string(),
                    ),
                    secret_key,
                    secret_data,
                )
            }
            AuthType::Config => (Authentication::Config, vec![], vec![]),
        };
        let typ = TermType::Ssh {
            options: SshOptions {
                group: session.group.to_string(),
                name: session.name.to_string(),
                host: session.host.to_string(),
                port: Some(session.port),
                auth,
            },
        };

        if self
            .db
            .find_session(&session.group, &session.name)?
            .is_some()
        {
            return Err(NxError::Plain(
                "`group` and `name` already exist, please choose another name.".to_string(),
            ));
        }

        self.add_shell_tab(ctx.clone(), typ)?;

        self.db.insert_session(Session {
            group: session.group.to_string(),
            name: session.name.to_string(),
            host: session.host.to_string(),
            port: session.port,
            auth_type: session.auth_type as u16,
            username: session.username.to_string(),
            secret_data,
            secret_key,
            ..Default::default()
        })?;

        if let Ok(sessions) = self.db.find_all_sessions() {
            self.state_manager.sessions = Some(sessions);
        }
        Ok(())
    }

    fn ssh_form(
        &mut self,
        ui: &mut egui::Ui,
        form: &mut Form<GardeReport>,
        session: &mut SessionState,
    ) {
        Grid::new("ssh_form_grid")
            .num_columns(2)
            .spacing([10.0, 15.0])
            .show(ui, |ui| {
                // group
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Group:");
                });
                FormField::new(form, "group").ui(ui, TextEdit::singleline(&mut session.group));
                ui.end_row();

                // name
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Name:");
                });
                FormField::new(form, "name").ui(ui, TextEdit::singleline(&mut session.name));
                ui.end_row();

                // host
                let host_label = match session.auth_type {
                    AuthType::Password => "Host:",
                    AuthType::Config => "Host Alias:",
                };

                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(host_label);
                });

                ui.vertical_centered(|ui| {
                    ui.horizontal_centered(|ui| {
                        let host_edit = TextEdit::singleline(&mut session.host);
                        match session.auth_type {
                            AuthType::Password => {
                                FormField::new(form, "host")
                                    .ui(ui, host_edit.char_limit(15).desired_width(150.));
                            }
                            AuthType::Config => {
                                FormField::new(form, "host").ui(ui, host_edit);
                            }
                        }

                        if let AuthType::Password = session.auth_type {
                            FormField::new(form, "port").ui(
                                ui,
                                egui::DragValue::new(&mut session.port)
                                    .speed(1.)
                                    .range(1..=65535),
                            );
                        }
                    });
                });

                ui.end_row();

                // auth type
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Auth Type:");
                });
                ComboBox::from_id_salt(session.auth_type)
                    .selected_text(session.auth_type.to_string())
                    .width(160.)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut session.auth_type,
                            AuthType::Password,
                            AuthType::Password.to_string(),
                        );
                        ui.selectable_value(
                            &mut session.auth_type,
                            AuthType::Config,
                            AuthType::Config.to_string(),
                        );
                    });
                ui.end_row();

                // FIXME: Why is the line height smaller in this row?
                if let AuthType::Password = session.auth_type {
                    // username
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label("Username:");
                    });
                    FormField::new(form, "username")
                        .ui(ui, TextEdit::singleline(&mut session.username));
                    ui.end_row();

                    // password
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label("Password:");
                    });
                    FormField::new(form, "auth_data").ui(
                        ui,
                        TextEdit::singleline(&mut session.auth_data).password(true),
                    );
                    ui.end_row();
                }
            });
    }
}
