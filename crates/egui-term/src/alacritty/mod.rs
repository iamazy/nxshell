use crate::errors::TermError;
use crate::ssh::{Pty, SshOptions};
use crate::types::Size;
use alacritty_terminal::event::{Event, EventListener, Notify, OnResize, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, Msg, Notifier};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Direction, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionRange, SelectionType};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::search::{Match, RegexIter, RegexSearch};
use alacritty_terminal::term::{cell::Cell, viewport_to_point, Config, Term, TermMode};
use alacritty_terminal::tty;
use alacritty_terminal::tty::{EventedPty, Options};
use copypasta::ClipboardContext;
use egui::Modifiers;
use parking_lot::MutexGuard;
use std::borrow::Cow;
use std::cmp::min;
use std::io::{Error as IoError, ErrorKind};
use std::ops::Index;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{mpsc, Arc};
use tracing::debug;

pub type PtyEvent = Event;

#[derive(Debug, Clone)]
pub enum BackendCommand {
    Write(Vec<u8>),
    Scroll(i32),
    Resize(Size, Size),
    SelectAll,
    SelectStart(SelectionType, f32, f32),
    SelectUpdate(f32, f32),
    ProcessLink(LinkAction, Point),
    MouseReport(MouseButton, Modifiers, Point, bool),
}

#[derive(Debug, Clone)]
pub enum MouseMode {
    Sgr,
    Normal(bool),
}

impl From<TermMode> for MouseMode {
    fn from(term_mode: TermMode) -> Self {
        if term_mode.contains(TermMode::SGR_MOUSE) {
            MouseMode::Sgr
        } else if term_mode.contains(TermMode::UTF8_MOUSE) {
            MouseMode::Normal(true)
        } else {
            MouseMode::Normal(false)
        }
    }
}

#[derive(Debug, Clone)]
pub enum MouseButton {
    LeftButton = 0,
    MiddleButton = 1,
    RightButton = 2,
    LeftMove = 32,
    MiddleMove = 33,
    RightMove = 34,
    NoneMove = 35,
    ScrollUp = 64,
    ScrollDown = 65,
    Other = 99,
}

#[derive(Debug, Clone)]
pub enum LinkAction {
    Clear,
    Hover,
    Open,
}

#[derive(Clone, Copy, Debug)]
pub struct TerminalSize {
    pub cell_width: u16,
    pub cell_height: u16,
    columns: u16,
    screen_lines: u16,
    pub layout_size: Size,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self {
            cell_width: 1,
            cell_height: 1,
            columns: 80,
            screen_lines: 50,
            layout_size: Size::default(),
        }
    }
}

impl Dimensions for TerminalSize {
    fn total_lines(&self) -> usize {
        self.screen_lines()
    }

    fn screen_lines(&self) -> usize {
        self.screen_lines as usize
    }

    fn columns(&self) -> usize {
        self.columns as usize
    }

    fn last_column(&self) -> Column {
        Column(self.columns as usize - 1)
    }

    fn bottommost_line(&self) -> Line {
        Line(self.screen_lines as i32 - 1)
    }
}

impl From<TerminalSize> for WindowSize {
    fn from(size: TerminalSize) -> Self {
        Self {
            num_lines: size.screen_lines,
            num_cols: size.columns,
            cell_width: size.cell_width,
            cell_height: size.cell_height,
        }
    }
}

#[derive(PartialEq)]
pub enum TermType {
    Regular { working_directory: Option<PathBuf> },
    Ssh { options: SshOptions },
}

pub struct Terminal {
    pub id: u64,
    pub url_regex: RegexSearch,
    pub term: Arc<FairMutex<Term<EventProxy>>>,
    pub size: TerminalSize,
    notifier: Notifier,
    pub hovered_hyperlink: Option<Match>,
}

impl PartialEq for Terminal {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Terminal {
    pub fn new(
        id: u64,
        app_context: egui::Context,
        term_type: TermType,
        term_size: TerminalSize,
        pty_event_proxy_sender: Sender<(u64, PtyEvent)>,
    ) -> Result<Self, TermError> {
        match term_type {
            TermType::Regular { working_directory } => {
                let opts = Options {
                    working_directory,
                    ..Default::default()
                };
                Self::new_with_pty(
                    id,
                    app_context,
                    term_size,
                    tty::new(&opts, term_size.into(), id)?,
                    pty_event_proxy_sender,
                )
            }
            TermType::Ssh { options } => Self::new_with_pty(
                id,
                app_context,
                term_size,
                Pty::new(options)?,
                pty_event_proxy_sender,
            ),
        }
    }

    pub fn new_regular(
        id: u64,
        app_context: egui::Context,
        working_directory: Option<PathBuf>,
        pty_event_proxy_sender: Sender<(u64, PtyEvent)>,
    ) -> Result<Self, TermError> {
        let typ = TermType::Regular { working_directory };
        Self::new(
            id,
            app_context,
            typ,
            TerminalSize::default(),
            pty_event_proxy_sender,
        )
    }

    pub fn new_ssh(
        id: u64,
        app_context: egui::Context,
        options: SshOptions,
        pty_event_proxy_sender: Sender<(u64, PtyEvent)>,
    ) -> Result<Self, TermError> {
        Self::new(
            id,
            app_context,
            TermType::Ssh { options },
            TerminalSize::default(),
            pty_event_proxy_sender,
        )
    }

    fn new_with_pty<Pty>(
        id: u64,
        app_context: egui::Context,
        term_size: TerminalSize,
        pty: Pty,
        pty_event_proxy_sender: Sender<(u64, PtyEvent)>,
    ) -> Result<Self, TermError>
    where
        Pty: EventedPty + OnResize + Send + 'static,
    {
        let config = Config::default();

        let (event_sender, event_receiver) = mpsc::channel();
        let event_proxy = EventProxy(event_sender);
        let term = Term::new(config, &term_size, event_proxy.clone());
        let term = Arc::new(FairMutex::new(term));
        let pty_event_loop = EventLoop::new(term.clone(), event_proxy, pty, false, false)?;
        let notifier = Notifier(pty_event_loop.channel());

        let url_regex = r#"(ipfs:|ipns:|magnet:|mailto:|gemini://|gopher://|https://|http://|news:|file://|git://|ssh:|ftp://)[^\u{0000}-\u{001F}\u{007F}-\u{009F}<>"\s{-}\^⟨⟩`]+"#;
        let url_regex =
            RegexSearch::new(url_regex).map_err(|err| IoError::new(ErrorKind::InvalidData, err))?;
        let _pty_event_loop_thread = pty_event_loop.spawn();
        let _pty_event_subscription = std::thread::Builder::new()
            .name(format!("pty_event_subscription_{id}"))
            .spawn(move || while let Ok(event) = event_receiver.recv() {
                pty_event_proxy_sender
                    .send((id, event.clone()))
                    .unwrap_or_else(|err| {
                        panic!("pty_event_subscription_{id}: sending PtyEvent is failed, error: {err}")
                    });
                app_context.request_repaint();
                if let Event::Exit = event {
                    break;
                }
            })?;

        debug!("create a terminal backend: {id}");
        Ok(Self {
            id,
            url_regex,
            term,
            size: term_size,
            notifier,
            hovered_hyperlink: None,
        })
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.notifier.0.send(Msg::Shutdown);
    }
}

pub struct TerminalContext<'a> {
    pub id: u64,
    pub terminal: MutexGuard<'a, Term<EventProxy>>,
    pub url_regex: &'a mut RegexSearch,
    pub size: &'a mut TerminalSize,
    pub notifier: &'a mut Notifier,
    pub hovered_hyperlink: &'a mut Option<Match>,
    pub clipboard: &'a mut ClipboardContext,
}

impl<'a> TerminalContext<'a> {
    pub fn new(terminal: &'a mut Terminal, clipboard: &'a mut ClipboardContext) -> Self {
        let term = terminal.term.lock();
        Self {
            id: terminal.id,
            terminal: term,
            url_regex: &mut terminal.url_regex,
            size: &mut terminal.size,
            notifier: &mut terminal.notifier,
            hovered_hyperlink: &mut terminal.hovered_hyperlink,
            clipboard,
        }
    }

    pub fn term_mode(&self) -> TermMode {
        *self.terminal.mode()
    }

    pub fn process_command(&mut self, cmd: BackendCommand) {
        match cmd {
            BackendCommand::Write(input) => {
                self.write_data(input);
            }
            BackendCommand::Scroll(delta) => {
                self.scroll(delta);
            }
            BackendCommand::Resize(layout_size, font_size) => {
                self.resize(layout_size, font_size);
            }
            BackendCommand::SelectAll => {
                self.select_all();
            }
            BackendCommand::SelectStart(selection_type, x, y) => {
                self.start_selection(selection_type, x, y);
            }
            BackendCommand::SelectUpdate(x, y) => {
                self.update_selection(x, y);
            }
            BackendCommand::ProcessLink(link_action, point) => {
                self.process_link(link_action, point);
            }
            BackendCommand::MouseReport(button, modifiers, point, pressed) => {
                self.mouse_report(button, modifiers, point, pressed);
            }
        };
    }

    pub fn to_range(&self) -> Option<SelectionRange> {
        match &self.terminal.selection {
            Some(s) => s.to_range(&self.terminal),
            None => None,
        }
    }

    #[inline]
    pub fn cursor_cell(&self) -> &Cell {
        let point = self.terminal.grid().cursor.point;
        &self.terminal.grid()[point.line][point.column]
    }

    pub fn selection_content(&self) -> String {
        self.terminal.selection_to_string().unwrap_or_default()
    }

    pub fn selection_is_empty(&self) -> bool {
        self.terminal
            .selection
            .as_ref()
            .is_none_or(Selection::is_empty)
    }

    pub fn write_data<I: Into<Cow<'static, [u8]>>>(&mut self, data: I) {
        self.write(data);
        self.terminal.scroll_display(Scroll::Bottom);
        self.terminal.selection = None;
    }

    fn process_link(&mut self, link_action: LinkAction, point: Point) {
        match link_action {
            LinkAction::Hover => {
                *self.hovered_hyperlink = regex_match_at(&self.terminal, point, self.url_regex);
            }
            LinkAction::Clear => {
                *self.hovered_hyperlink = None;
            }
            LinkAction::Open => {
                self.open_link();
            }
        };
    }

    fn open_link(&self) {
        if let Some(range) = &self.hovered_hyperlink {
            let start = range.start();
            let end = range.end();

            let mut url = String::from(self.terminal.grid().index(*start).c);
            for indexed in self.terminal.grid().iter_from(*start) {
                url.push(indexed.c);
                if indexed.point == *end {
                    break;
                }
            }

            let _ = open::that(url);
        }
    }

    fn mouse_report(&self, button: MouseButton, modifiers: Modifiers, point: Point, pressed: bool) {
        // Assure the mouse point is not in the scroll back.
        if point.line < 0 {
            return;
        }
        let mut mods = 0;
        if modifiers.contains(Modifiers::SHIFT) {
            mods += 4;
        }
        if modifiers.contains(Modifiers::ALT) {
            mods += 8;
        }
        if modifiers.contains(Modifiers::COMMAND) {
            mods += 16;
        }

        match MouseMode::from(*self.terminal.mode()) {
            MouseMode::Sgr => self.sgr_mouse_report(point, button as u8 + mods, pressed),
            MouseMode::Normal(is_utf8) => {
                if pressed {
                    self.normal_mouse_report(point, button as u8 + mods, is_utf8)
                } else {
                    self.normal_mouse_report(point, 3 + mods, is_utf8)
                }
            }
        }
    }

    fn sgr_mouse_report(&self, point: Point, button: u8, pressed: bool) {
        let c = if pressed { 'M' } else { 'm' };

        let msg = format!(
            "\x1b[<{};{};{}{}",
            button,
            point.column + 1,
            point.line + 1,
            c
        );

        self.write(msg.as_bytes().to_vec());
    }

    fn normal_mouse_report(&self, point: Point, button: u8, is_utf8: bool) {
        let Point { line, column } = point;
        let max_point = if is_utf8 { 2015 } else { 223 };

        if line >= max_point || column >= max_point {
            return;
        }

        let mut msg = vec![b'\x1b', b'[', b'M', 32 + button];

        let mouse_pos_encode = |pos: usize| -> Vec<u8> {
            let pos = 32 + 1 + pos;
            let first = 0xC0 + pos / 64;
            let second = 0x80 + (pos & 63);
            vec![first as u8, second as u8]
        };

        if is_utf8 && column >= Column(95) {
            msg.append(&mut mouse_pos_encode(column.0));
        } else {
            msg.push(32 + 1 + column.0 as u8);
        }

        if is_utf8 && line >= 95 {
            msg.append(&mut mouse_pos_encode(line.0 as usize));
        } else {
            msg.push(32 + 1 + line.0 as u8);
        }

        self.write(msg);
    }

    pub fn select_all(&mut self) {
        let start = Point::new(self.terminal.topmost_line(), Column(0));
        let end = Point::new(
            self.terminal.bottommost_line(),
            Column(self.terminal.columns()),
        );
        // whatever the side is
        let side = Side::Right;
        let mut selection = Selection::new(SelectionType::Simple, start, side);
        selection.update(end, side);
        // correct the value of side
        selection.include_all();
        self.terminal.selection = Some(selection);
    }

    fn start_selection(&mut self, selection_type: SelectionType, x: f32, y: f32) {
        let location = selection_point(x, y, self.size, self.terminal.grid().display_offset());
        self.terminal.selection = Some(Selection::new(
            selection_type,
            location,
            self.selection_side(x),
        ));
    }

    fn update_selection(&mut self, x: f32, y: f32) {
        let display_offset = self.terminal.grid().display_offset();
        if let Some(ref mut selection) = self.terminal.selection {
            let location = selection_point(x, y, self.size, display_offset);
            let side = selection_side(self.size.cell_width, x);
            selection.update(location, side);
        }
    }

    fn selection_side(&self, x: f32) -> Side {
        let cell_x = x as usize % self.size.cell_width as usize;
        let half_cell_width = (self.size.cell_width as f32 / 2.0) as usize;

        if cell_x > half_cell_width {
            Side::Right
        } else {
            Side::Left
        }
    }

    fn resize(&mut self, layout_size: Size, font_size: Size) {
        if layout_size == self.size.layout_size
            && font_size.width as u16 == self.size.cell_width
            && font_size.height as u16 == self.size.cell_height
        {
            return;
        }

        let lines = (layout_size.height / font_size.height.floor()) as u16;
        let cols = (layout_size.width / font_size.width.floor()) as u16;
        if lines > 0 && cols > 0 {
            *self.size = TerminalSize {
                layout_size,
                cell_height: font_size.height as u16,
                cell_width: font_size.width as u16,
                screen_lines: lines,
                columns: cols,
            };

            self.notifier.on_resize((*self.size).into());
            self.terminal.resize(*self.size);
        }
    }

    fn write<I: Into<Cow<'static, [u8]>>>(&self, input: I) {
        self.notifier.notify(input);
    }

    fn scroll(&mut self, delta_value: i32) {
        if delta_value != 0 {
            let scroll = Scroll::Delta(delta_value);
            if self
                .terminal
                .mode()
                .contains(TermMode::ALTERNATE_SCROLL | TermMode::ALT_SCREEN)
            {
                let line_cmd = if delta_value > 0 { b'A' } else { b'B' };
                let mut content = vec![];

                for _ in 0..delta_value.abs() {
                    content.push(0x1b);
                    content.push(b'O');
                    content.push(line_cmd);
                }

                self.notifier.notify(content);
            } else {
                self.terminal.grid_mut().scroll_display(scroll);
            }
        }
    }
}

pub fn selection_point(x: f32, y: f32, term_size: &TerminalSize, display_offset: usize) -> Point {
    let col = (x as usize) / (term_size.cell_width as usize);
    let col = min(Column(col), Column(term_size.columns as usize - 1));

    let line = (y as usize) / (term_size.cell_height as usize);
    let line = min(line, term_size.screen_lines as usize - 1);

    viewport_to_point(display_offset, Point::new(line, col))
}

fn selection_side(cell_width: u16, x: f32) -> Side {
    let cell_x = x as usize % cell_width as usize;
    let half_cell_width = (cell_width as f32 / 2.0) as usize;

    if cell_x > half_cell_width {
        Side::Right
    } else {
        Side::Left
    }
}

/// Based on alacritty/src/display/hint.rs > regex_match_at
/// Retrieve the match, if the specified point is inside the content matching the regex.
fn regex_match_at(
    terminal: &Term<EventProxy>,
    point: Point,
    regex: &mut RegexSearch,
) -> Option<Match> {
    visible_regex_match_iter(terminal, regex).find(|rm| rm.contains(&point))
}

/// Copied from alacritty/src/display/hint.rs:
/// Iterate over all visible regex matches.
fn visible_regex_match_iter<'a>(
    term: &'a Term<EventProxy>,
    regex: &'a mut RegexSearch,
) -> impl Iterator<Item = Match> + 'a {
    let viewport_start = Line(-(term.grid().display_offset() as i32));
    let viewport_end = viewport_start + term.bottommost_line();
    let mut start = term.line_search_left(Point::new(viewport_start, Column(0)));
    let mut end = term.line_search_right(Point::new(viewport_end, Column(0)));
    start.line = start.line.max(viewport_start - 100);
    end.line = end.line.min(viewport_end + 100);

    RegexIter::new(start, end, Direction::Right, term, regex)
        .skip_while(move |rm| rm.end().line < viewport_start)
        .take_while(move |rm| rm.start().line <= viewport_end)
}

#[derive(Clone)]
pub struct EventProxy(Sender<Event>);

impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        let _ = self.0.send(event);
    }
}
