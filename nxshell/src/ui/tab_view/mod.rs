mod session;
mod terminal;

use crate::app::{NxShell, NxShellOptions};
use crate::consts::GLOBAL_COUNTER;
use crate::ui::tab_view::session::SessionList;
use copypasta::ClipboardContext;
use egui::{Label, Response, Sense, Ui};
use egui_dock::{DockArea, Style};
use egui_phosphor::regular::{DRONE, NUMPAD};
use egui_term::{
    Authentication, PtyEvent, TermType, Terminal, TerminalContext, TerminalOptions, TerminalTheme,
    TerminalView,
};
use homedir::my_home;
use std::error::Error;
use std::sync::mpsc::Sender;
use terminal::TerminalTab;
use tracing::error;

#[derive(PartialEq)]
enum TabInner {
    Term(TerminalTab),
    SessionList(SessionList),
}

#[derive(PartialEq)]
pub struct Tab {
    inner: TabInner,
    id: u64,
}

impl Tab {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn term(
        ctx: egui::Context,
        typ: TermType,
        command_sender: Sender<(u64, PtyEvent)>,
    ) -> Result<Self, Box<dyn Error>> {
        let id = GLOBAL_COUNTER.next();

        let terminal = match typ {
            TermType::Ssh { ref options } => {
                Terminal::new_ssh(id, ctx, options.clone(), command_sender)?
            }
            _ => Terminal::new_regular(id, ctx, my_home()?, command_sender)?,
        };

        Ok(Self {
            id,
            inner: TabInner::Term(TerminalTab {
                terminal,
                terminal_theme: TerminalTheme::default(),
                term_type: typ,
            }),
        })
    }

    pub fn session_list() -> Self {
        let id = GLOBAL_COUNTER.next();

        Self {
            id,
            inner: TabInner::SessionList(SessionList {}),
        }
    }
}

struct TabViewer<'a> {
    command_sender: &'a Sender<(u64, PtyEvent)>,
    options: &'a mut NxShellOptions,
    clipboard: &'a mut ClipboardContext,
}

impl egui_dock::TabViewer for TabViewer<'_> {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match &mut tab.inner {
            TabInner::Term(term) => match term.term_type {
                TermType::Ssh { ref options } => {
                    let icon = match options.auth {
                        Authentication::Config => DRONE,
                        Authentication::Password(..) => NUMPAD,
                    };
                    format!("{icon} {}", options.name).into()
                }
                TermType::Regular { .. } => "local".into(),
            },
            TabInner::SessionList(_) => "sessions".into(),
        }
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        match &mut tab.inner {
            TabInner::Term(tab) => {
                let term_ctx = TerminalContext::new(&mut tab.terminal, self.clipboard);
                let term_opt = TerminalOptions {
                    font: &mut self.options.term_font,
                    multi_exec: &mut self.options.multi_exec,
                    theme: &mut tab.terminal_theme,
                    default_font_size: self.options.term_font_size,
                    active_tab_id: &mut self.options.active_tab_id,
                };

                let terminal = TerminalView::new(ui, term_ctx, term_opt)
                    .set_focus(true)
                    .set_size(ui.available_size());
                ui.add(terminal);
            }
            TabInner::SessionList(_list) => {
                ui.collapsing("Tab body", |ui| {
                    ui.add(
                        Label::new("Rounding")
                            .sense(Sense::click())
                            .selectable(false),
                    );
                    ui.separator();

                    ui.label("Stroke color:");
                    ui.label("Background color:");
                });
            }
        }
    }

    fn on_tab_button(&mut self, tab: &mut Self::Tab, response: &Response) {
        if response.hovered() {
            if let TabInner::Term(term) = &mut tab.inner {
                if let TermType::Ssh { options } = &term.term_type {
                    if let Authentication::Password(..) = options.auth {
                        response.show_tooltip_text(format!(
                            "{}:{}",
                            options.host,
                            options.port.unwrap_or(22)
                        ));
                    }
                }
            }
        }
    }

    fn closeable(&mut self, tab: &mut Self::Tab) -> bool {
        matches!(&mut tab.inner, TabInner::Term(_))
    }

    fn on_close(&mut self, tab: &mut Self::Tab) -> bool {
        match self.command_sender.send((tab.id, PtyEvent::Exit)) {
            Err(err) => {
                error!("close tab {} failed: {err}", tab.id);
                false
            }
            Ok(_) => true,
        }
    }
}

impl NxShell {
    pub fn tab_view(&mut self, ctx: &egui::Context) {
        if self.opts.show_dock_panel {
            DockArea::new(&mut self.dock_state)
                .show_add_buttons(false)
                .show_leaf_collapse_buttons(false)
                .style(Style::from_egui(ctx.style().as_ref()))
                .show(
                    ctx,
                    &mut TabViewer {
                        command_sender: &self.command_sender,
                        options: &mut self.opts,
                        clipboard: &mut self.clipboard,
                    },
                );
        }
    }
}
