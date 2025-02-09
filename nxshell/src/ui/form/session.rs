use crate::app::NxShell;
use crate::db::Session;
use crate::errors::{error_toast, NxError};
use egui::emath::Numeric;
use egui::{
    Align2, CentralPanel, Context, Grid, Id, Layout, Order, TextBuffer, TextEdit, TopBottomPanel,
    Window,
};
use egui_form::garde::GardeReport;
use egui_form::{Form, FormField};
use egui_term::{SshOptions, TermType};
use egui_toast::Toasts;
use garde::Validate;
use orion::aead::{seal, SecretKey};
use std::ops::RangeInclusive;
use tracing::error;

#[derive(Debug, Clone, Validate)]
pub struct SessionState {
    #[garde(length(min = 0, max = 256))]
    pub group: String,
    #[garde(length(min = 0, max = 256))]
    pub name: String,
    #[garde(ip)]
    pub host: String,
    #[garde(range(min = 1, max = 65535))]
    pub port: u16,
    #[garde(length(min = 1, max = 256))]
    pub username: String,
    #[garde(length(min = 1, max = 256))]
    pub password: String,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            group: String::default(),
            name: String::default(),
            host: String::default(),
            port: 22,
            username: String::default(),
            password: String::default(),
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

                        ssh_form(ui, &mut form, &mut session_state);
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
        let typ = TermType::Ssh {
            options: SshOptions {
                host: session.host.to_string(),
                port: Some(session.port),
                user: Some(session.username.to_string()),
                password: Some(session.password.to_string()),
            },
        };
        self.add_shell_tab(ctx.clone(), typ)?;

        let secret_key = SecretKey::generate(32)?; // 32字节密钥
        let secret_data = seal(&secret_key, session.password.as_bytes())?;

        self.db.insert_session(Session {
            group: session.group.to_string(),
            name: session.name.to_string(),
            host: session.host.to_string(),
            port: session.port,
            username: session.username.to_string(),
            secret_data,
            secret_key: secret_key.unprotected_as_bytes().to_vec(),
            ..Default::default()
        })?;

        if let Ok(sessions) = self.db.find_all_sessions() {
            self.state_manager.sessions = Some(sessions);
        }
        Ok(())
    }
}

fn ssh_form(ui: &mut egui::Ui, form: &mut Form<GardeReport>, session: &mut SessionState) {
    Grid::new("ssh_form_grid")
        .num_columns(2)
        .spacing([10.0, 8.0])
        .show(ui, |ui| {
            // group
            form_text_edit(ui, form, "Group:", &mut session.group, false);
            // name
            form_text_edit(ui, form, "Name:", &mut session.name, false);
            // host
            form_text_edit(ui, form, "Host:", &mut session.host, false);
            // port
            form_drag_value(ui, form, "Port:", &mut session.port, 1..=65535);
            // username
            form_text_edit(ui, form, "Username:", &mut session.username, false);
            // password
            form_text_edit(ui, form, "Password:", &mut session.password, true);
        });
}

fn form_text_edit(
    ui: &mut egui::Ui,
    form: &mut Form<GardeReport>,
    label: &str,
    text: &mut dyn TextBuffer,
    is_password: bool,
) {
    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
        ui.label(label);
    });
    FormField::new(form, label.trim_end_matches(':').to_lowercase())
        .ui(ui, TextEdit::singleline(text).password(is_password));
    ui.end_row();
}

fn form_drag_value<Num: Numeric>(
    ui: &mut egui::Ui,
    form: &mut Form<GardeReport>,
    label: &str,
    value: &mut Num,
    range: RangeInclusive<Num>,
) {
    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
        ui.label(label);
    });
    FormField::new(form, label.trim_end_matches(':').to_lowercase())
        .ui(ui, egui::DragValue::new(value).speed(1.).range(range));
    ui.end_row();
}
