mod session;
mod terminal;

use crate::app::{NxShell, NxShellOptions};
use crate::consts::GLOBAL_COUNTER;
use crate::ui::tab_view::session::SessionList;
use copypasta::ClipboardContext;
use egui::{Label, Order, Response, Sense, Ui};
use egui_dock::{node_index::NodeIndex, surface_index::SurfaceIndex, DockArea, Style};
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

const TAB_BTN_WIDTH: f32 = 100.0;

#[derive(Debug, Clone)]
pub enum TabEvent {
    Rename(u64), // tab id
}

#[derive(PartialEq)]
enum TabInner {
    Term(TerminalTab),
    SessionList(SessionList),
}

#[derive(PartialEq)]
pub struct Tab {
    inner: TabInner,
    id: u64,
    custom_title: Option<String>,
    rename_buffer: String,
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
            custom_title: None,
            rename_buffer: String::new(),
        })
    }

    pub fn session_list() -> Self {
        let id = GLOBAL_COUNTER.next();

        Self {
            id,
            inner: TabInner::SessionList(SessionList {}),
            custom_title: None,
            rename_buffer: String::new(),
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
        if let Some(title) = &tab.custom_title {
            return title.clone().into();
        }
        let tab_id = tab.id();
        match &mut tab.inner {
            TabInner::Term(term) => match term.term_type {
                TermType::Ssh { ref options } => {
                    let icon = match options.auth {
                        Authentication::Config => DRONE,
                        Authentication::Password(..) => NUMPAD,
                    };
                    if tab_id > 0 {
                        format!("{icon} {} ({tab_id})", options.name).into()
                    } else {
                        format!("{icon} {}", options.name).into()
                    }
                }
                TermType::Regular { .. } => {
                    if tab_id > 0 {
                        format!("local ({tab_id})").into()
                    } else {
                        "local".into()
                    }
                }
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

    fn context_menu(
        &mut self,
        ui: &mut Ui,
        tab: &mut Self::Tab,
        _surface: SurfaceIndex,
        _node: NodeIndex,
    ) {
        ui.set_width(TAB_BTN_WIDTH);
        let rename_btn_response = ui.button("Rename Tab");
        if rename_btn_response.clicked() {
            self.options.tab_events.push(TabEvent::Rename(tab.id()));
            ui.close_menu();
        }

        ui.separator();
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

    pub fn rename_tab_view(&mut self, ctx: &egui::Context) {
        if let Some(tab_id) = self.opts.renaming_tab_id {
            if let Some((_, tab)) = self
                .dock_state
                .iter_all_tabs_mut()
                .find(|(_, tab)| tab.id() == tab_id)
            {
                let popup_id = egui::Id::new(format!("rename_tab_{}", tab_id));
                let mut close_popup = false;

                self.opts.surrender_focus();
                egui::Area::new("modal_mask".into())
                    .order(egui::Order::Middle)
                    .interactable(true)
                    .show(ctx, |ui| {
                        let screen_rect = ui.ctx().screen_rect();
                        let painter = ui.painter();
                        painter.rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(96));
                        ui.allocate_rect(screen_rect, egui::Sense::click_and_drag());
                    });

                egui::Window::new("Rename Tab View")
                    .id(popup_id)
                    .title_bar(true)
                    .collapsible(false)
                    .resizable(false)
                    .order(Order::Foreground)
                    .open(&mut self.opts.show_rename_view.borrow_mut())
                    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label("Please input a new name for the tab:");
                        let text_id = egui::Id::new(format!("rename_tab_text_{}", tab_id));

                        ui.add(egui::TextEdit::singleline(&mut tab.rename_buffer).id(text_id));
                        ui.memory_mut(|mem| mem.request_focus(text_id));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                            if ui.button("Cancel").clicked() {
                                ui.set_width(50.0);
                                tab.rename_buffer.clear();
                                close_popup = true;
                            }

                            ui.add_space(20.0);

                            if ui.button("OK").clicked() {
                                ui.set_width(50.0);
                                if !tab.rename_buffer.is_empty() {
                                    tab.custom_title = Some(tab.rename_buffer.clone());
                                }
                                tab.rename_buffer.clear();
                                close_popup = true;
                            }
                        });
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            if !tab.rename_buffer.is_empty() {
                                tab.custom_title = Some(tab.rename_buffer.clone());
                            }
                            tab.rename_buffer.clear();
                            close_popup = true;
                        } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            tab.rename_buffer.clear();
                            close_popup = true;
                        }
                    });
                if close_popup || !*self.opts.show_rename_view.borrow() {
                    self.opts.renaming_tab_id = None;
                    *self.opts.show_rename_view.borrow_mut() = false;
                    tab.rename_buffer.clear();
                }
            }
        } else {
            self.opts.renaming_tab_id = None;

            if let Some(event) = self.opts.tab_events.pop() {
                match event {
                    TabEvent::Rename(tab_id) => {
                        self.opts.renaming_tab_id = Some(tab_id);
                        *self.opts.show_rename_view.borrow_mut() = true;
                    }
                }
            }
        }
    }
}
