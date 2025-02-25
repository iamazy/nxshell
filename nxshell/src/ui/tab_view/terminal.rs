use egui_term::{TermType, Terminal, TerminalTheme};

#[derive(PartialEq)]
pub struct TerminalTab {
    pub terminal_theme: TerminalTheme,
    pub terminal: Terminal,
    pub term_type: TermType,
    pub show_sftp_window: bool,
}
