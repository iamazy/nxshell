mod alacritty;
mod bindings;
mod display;
mod errors;
mod font;
mod input;
mod ssh;
mod theme;
mod types;
mod view;

pub use alacritty::{PtyEvent, TermType, Terminal, TerminalContext};
pub use alacritty_terminal::term::TermMode;
pub use bindings::{Binding, BindingAction, InputKind, KeyboardBinding};
pub use errors::TermError;
pub use font::{FontSettings, TerminalFont};
pub use ssh::{Authentication, SshOptions};
pub use theme::{ColorPalette, TerminalTheme};
pub use view::{TerminalOptions, TerminalView};
