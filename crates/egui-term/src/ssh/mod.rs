use crate::errors::TermError;
use crate::errors::TermError::HostVerification;
use alacritty_terminal::event::{OnResize, WindowSize};
use alacritty_terminal::tty::{ChildEvent, EventedPty, EventedReadWrite};
use anyhow::Context;
use polling::{Event, PollMode, Poller};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use tracing::{error, trace};
use wezterm_ssh::{
    Child, ChildKiller, Config, FileDescriptor, MasterPty, PtySize, Session, SessionEvent,
    SshChildProcess, SshPty,
};

#[cfg(unix)]
use std::os::fd::{AsFd, AsRawFd};

#[cfg(windows)]
use std::os::windows::io::{AsRawSocket, AsSocket};

// Interest in PTY read/writes.
#[cfg(unix)]
const PTY_READ_WRITE_TOKEN: usize = 0;
#[cfg(windows)]
const PTY_READ_WRITE_TOKEN: usize = 2;
const PTY_CHILD_EVENT_TOKEN: usize = 1;

#[derive(Debug)]
pub struct Pty {
    pub pty: SshPty,
    pub child: SshChildProcess,
    pub signal: TcpStream,
}

impl Drop for Pty {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl EventedPty for Pty {
    fn next_child_event(&mut self) -> Option<ChildEvent> {
        match self.child.try_wait() {
            Ok(Some(status)) => Some(ChildEvent::Exited(Some(status.exit_code() as i32))),
            Ok(None) => None,
            Err(err) => {
                error!("Error checking child process termination: {}", err);
                None
            }
        }
    }
}

impl EventedReadWrite for Pty {
    type Reader = FileDescriptor;
    type Writer = FileDescriptor;

    unsafe fn register(
        &mut self,
        poller: &Arc<Poller>,
        mut interest: Event,
        mode: PollMode,
    ) -> std::io::Result<()> {
        interest.key = PTY_READ_WRITE_TOKEN;
        let _ = self.pty.reader.set_non_blocking(true);
        let _ = self.pty.writer.set_non_blocking(true);
        let _ = self.signal.set_nonblocking(true);

        #[cfg(unix)]
        {
            poller.add_with_mode(self.pty.reader.as_raw_fd(), interest, mode)?;
            poller.add_with_mode(self.pty.writer.as_raw_fd(), interest, mode)?;

            poller.add_with_mode(
                self.signal.as_raw_fd(),
                Event::readable(PTY_CHILD_EVENT_TOKEN),
                PollMode::Level,
            )?;
        }

        #[cfg(windows)]
        {
            poller.add_with_mode(self.pty.reader.as_raw_socket(), interest, mode)?;
            poller.add_with_mode(self.pty.writer.as_raw_socket(), interest, mode)?;

            poller.add_with_mode(
                self.signal.as_raw_socket(),
                Event::readable(crate::ssh::PTY_CHILD_EVENT_TOKEN),
                PollMode::Level,
            )?;
        }

        Ok(())
    }

    fn reregister(
        &mut self,
        poller: &Arc<Poller>,
        mut interest: Event,
        mode: PollMode,
    ) -> std::io::Result<()> {
        interest.key = PTY_READ_WRITE_TOKEN;

        #[cfg(unix)]
        {
            poller.modify_with_mode(self.pty.reader.as_fd(), interest, mode)?;
            poller.modify_with_mode(self.pty.writer.as_fd(), interest, mode)?;

            poller.modify_with_mode(
                self.signal.as_fd(),
                Event::readable(PTY_CHILD_EVENT_TOKEN),
                PollMode::Level,
            )?;
        }

        #[cfg(windows)]
        {
            poller.modify_with_mode(self.pty.reader.as_socket(), interest, mode)?;
            poller.modify_with_mode(self.pty.writer.as_socket(), interest, mode)?;

            poller.modify_with_mode(
                self.signal.as_socket(),
                Event::readable(crate::ssh::PTY_CHILD_EVENT_TOKEN),
                PollMode::Level,
            )?;
        }

        Ok(())
    }

    fn deregister(&mut self, poller: &Arc<Poller>) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            poller.delete(self.pty.reader.as_fd())?;
            poller.delete(self.pty.writer.as_fd())?;

            poller.delete(self.signal.as_fd())?;
        }

        #[cfg(windows)]
        {
            poller.delete(self.pty.reader.as_socket())?;
            poller.delete(self.pty.writer.as_socket())?;

            poller.delete(self.signal.as_socket())?;
        }

        Ok(())
    }

    fn reader(&mut self) -> &mut Self::Reader {
        &mut self.pty.reader
    }

    fn writer(&mut self) -> &mut Self::Writer {
        &mut self.pty.writer
    }
}

impl OnResize for Pty {
    fn on_resize(&mut self, window_size: WindowSize) {
        let size = PtySize {
            rows: window_size.num_lines,
            cols: window_size.num_cols,
            pixel_width: window_size.cell_width,
            pixel_height: window_size.cell_height,
        };

        let _ = self.pty.resize(size);
    }
}

impl Pty {
    pub fn new(mut opts: SshOptions) -> Result<Self, TermError> {
        let mut config = Config::new();
        config.add_default_config_files();

        let port = opts.port.unwrap_or(22);
        let mut config = config.for_host(opts.host);
        config.insert("port".to_string(), port.to_string());

        if let Some(user) = opts.user.take() {
            config.insert("user".to_string(), user);
        }
        smol::block_on(async move {
            let (session, events) = Session::connect(config)?;

            while let Ok(event) = events.recv().await {
                match event {
                    SessionEvent::Banner(banner) => {
                        if let Some(banner) = banner {
                            trace!("{}", banner);
                        }
                    }
                    SessionEvent::HostVerify(verify) => {
                        verify.answer(true).await.context("send verify response")?;
                    }
                    SessionEvent::Authenticate(auth) => {
                        if auth.prompts.is_empty() {
                            auth.answer(vec![]).await?;
                        } else if let Some(password) = opts.password.take() {
                            auth.answer(vec![password]).await?;
                        }
                    }
                    SessionEvent::HostVerificationFailed(failed) => {
                        error!("host verification failed: {failed}");
                        return Err(HostVerification(failed));
                    }
                    SessionEvent::Error(err) => {
                        error!("ssh login error: {err}");
                        return Err(TermError::Box(err.into()));
                    }
                    SessionEvent::Authenticated => break,
                }
            }

            let (pty, child) = session
                .request_pty("xterm-256color", PtySize::default(), None, None)
                .await?;

            let signal = tcp_signal()?;
            Ok(Pty { pty, child, signal })
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SshOptions {
    pub group: String,
    pub name: String,
    pub host: String,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub password: Option<String>,
}

fn tcp_signal() -> std::io::Result<TcpStream> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    TcpStream::connect(listener.local_addr()?)
}
