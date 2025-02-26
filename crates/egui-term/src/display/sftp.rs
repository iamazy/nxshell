use crate::{TermError, TerminalView};
use camino::Utf8PathBuf;
use egui::{Align2, CentralPanel, Context, Layout, TopBottomPanel, Window};
use egui_extras::TableBuilder;
use file_format::FileFormat;
use homedir::my_home;
use time::Duration;
use wezterm_ssh::{FilePermissions, FileType, Metadata, Sftp};

pub struct Entry {
    pub path: Utf8PathBuf,
    meta: Metadata,
}

pub struct SftpExplorer {
    pub sftp: Sftp,
    pub current_path: String,
    pub entries: Vec<Entry>,
    previous_path: Vec<Utf8PathBuf>,
    forward_path: Vec<Utf8PathBuf>,
}

impl SftpExplorer {
    pub fn new(sftp: Sftp) -> Result<Self, TermError> {
        let current_path = match my_home()? {
            Some(home) => home,
            None => {
                return Err(TermError::Any(anyhow::anyhow!(
                    "cannot find home directory"
                )))
            }
        };
        let current_path = match current_path.to_str() {
            Some(path) => path.to_owned(),
            None => {
                return Err(TermError::Any(anyhow::anyhow!(
                    "cannot convert path to unicode string"
                )))
            }
        };
        let entries = smol::block_on(async { sftp.read_dir(&current_path).await })?;
        let entries = entries
            .into_iter()
            .map(|(path, meta)| Entry { path, meta })
            .collect();
        Ok(Self {
            sftp,
            current_path,
            entries,
            previous_path: vec![],
            forward_path: vec![],
        })
    }
}

impl TerminalView<'_> {
    pub fn show_sftp_window(&mut self, ctx: &Context) {
        if let Some(explorer) = self.term_ctx.sftp_explorer {
            Window::new("Sftp Window")
                .open(self.term_ctx.show_sftp_window)
                .max_width(1000.)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    TopBottomPanel::bottom("sftp_bottom_panel").show_inside(ui, |ui| {
                        ui.with_layout(Layout::right_to_left(egui::Align::TOP), |_ui| {});
                    });

                    CentralPanel::default().show_inside(ui, |ui| {
                        egui::ScrollArea::both()
                            .auto_shrink([false; 2])
                            .show(ui, |ui| {
                                let text_size =
                                    egui::TextStyle::Body.resolve(ui.style()).size + 10.0;

                                TableBuilder::new(ui)
                                    .column(egui_extras::Column::initial(300.0))
                                    .column(egui_extras::Column::initial(100.0))
                                    .column(egui_extras::Column::initial(100.0))
                                    .column(egui_extras::Column::initial(100.0))
                                    .column(egui_extras::Column::initial(100.0))
                                    .column(egui_extras::Column::remainder())
                                    .resizable(true)
                                    .striped(true)
                                    .header(20.0, |mut header| {
                                        header.col(|ui| {
                                            ui.strong("Name");
                                        });
                                        header.col(|ui| {
                                            ui.strong("Type");
                                        });
                                        header.col(|ui| {
                                            ui.strong("Size");
                                        });
                                        header.col(|ui| {
                                            ui.strong("Last accessed");
                                        });
                                        header.col(|ui| {
                                            ui.strong("Last modified");
                                        });
                                        header.col(|ui| {
                                            ui.strong("Permissions");
                                        });
                                    })
                                    .body(|body| {
                                        body.rows(text_size, explorer.entries.len(), |mut row| {
                                            let row_index = row.index();

                                            if let Some(entry) = explorer.entries.get(row_index) {
                                                let file_name =
                                                    entry.path.file_name().unwrap_or_default();
                                                let entry_type = match entry.meta.ty {
                                                    FileType::File => {
                                                        let mut file_type = "File".to_string();
                                                        if let Ok(t) =
                                                            FileFormat::from_file(&entry.path)
                                                        {
                                                            if let Some(short_name) = t.short_name()
                                                            {
                                                                file_type =
                                                                    format!("{} File", short_name);
                                                            }
                                                        }
                                                        file_type
                                                    }
                                                    FileType::Dir => "Folder".to_string(),
                                                    FileType::Symlink => "Symlink".to_string(),
                                                    FileType::Other => "Other".to_string(),
                                                };

                                                row.col(|ui| {
                                                    let _entry_label = {
                                                        ui.push_id(file_name, |ui| {
                                                            ui.with_layout(
                                                                Layout::left_to_right(
                                                                    egui::Align::Min,
                                                                ),
                                                                |ui| {
                                                                    if ui
                                                                        .selectable_label(
                                                                            false, file_name,
                                                                        )
                                                                        .clicked()
                                                                    {
                                                                    }
                                                                },
                                                            )
                                                        })
                                                        .inner
                                                    };
                                                });
                                                row.col(|ui| {
                                                    ui.with_layout(
                                                        Layout::left_to_right(egui::Align::Min),
                                                        |ui| {
                                                            ui.label(entry_type);
                                                        },
                                                    );
                                                });

                                                row.col(|ui| {
                                                    if let Some(size) = entry.meta.size {
                                                        ui.with_layout(
                                                            Layout::left_to_right(egui::Align::Min),
                                                            |ui| {
                                                                ui.label(bytesize::to_string(
                                                                    size, false,
                                                                ));
                                                            },
                                                        );
                                                    }
                                                });

                                                row.col(|ui| {
                                                    if let Some(accessed) = entry.meta.accessed {
                                                        ui.with_layout(
                                                            Layout::left_to_right(egui::Align::Min),
                                                            |ui| {
                                                                ui.label(duration_to_string(
                                                                    Duration::milliseconds(
                                                                        accessed as i64,
                                                                    ),
                                                                ));
                                                            },
                                                        );
                                                    }
                                                });

                                                row.col(|ui| {
                                                    if let Some(modified) = entry.meta.modified {
                                                        ui.with_layout(
                                                            Layout::left_to_right(egui::Align::Min),
                                                            |ui| {
                                                                ui.label(duration_to_string(
                                                                    Duration::milliseconds(
                                                                        modified as i64,
                                                                    ),
                                                                ));
                                                            },
                                                        );
                                                    }
                                                });

                                                row.col(|ui| {
                                                    if let Some(permissions) =
                                                        entry.meta.permissions
                                                    {
                                                        ui.with_layout(
                                                            Layout::left_to_right(egui::Align::Min),
                                                            |ui| {
                                                                ui.label(to_rwx_string(
                                                                    permissions,
                                                                ));
                                                            },
                                                        );
                                                    }
                                                });
                                            }
                                        });
                                    });
                            });
                    });
                });
        }

        if !*self.term_ctx.show_sftp_window {
            *self.term_ctx.sftp_explorer = None;
        }
    }
}

pub fn duration_to_string(duration: Duration) -> String {
    if duration.whole_weeks() >= 1 {
        format!("{} weeks ago", duration.whole_weeks())
    } else if duration.whole_days() >= 1 {
        format!("{} days ago", duration.whole_days())
    } else if duration.whole_hours() >= 1 {
        format!("{} hours ago", duration.whole_days())
    } else if duration.whole_minutes() >= 1 {
        format!("{} minutes ago", duration.whole_minutes())
    } else {
        format!("{} seconds ago", duration.whole_seconds())
    }
}

pub fn to_rwx_string(permission: FilePermissions) -> String {
    fn perms_to_str(read: bool, write: bool, exec: bool) -> String {
        [
            if read { 'r' } else { '-' },
            if write { 'w' } else { '-' },
            if exec { 'x' } else { '-' },
        ]
        .iter()
        .collect()
    }
    format!(
        "{}{}{}",
        perms_to_str(
            permission.owner_read,
            permission.owner_write,
            permission.owner_exec
        ),
        perms_to_str(
            permission.group_read,
            permission.group_write,
            permission.group_exec
        ),
        perms_to_str(
            permission.other_read,
            permission.other_write,
            permission.other_exec
        ),
    )
}
