use homedir::GetHomeError;
use wezterm_ssh::{HostVerificationFailed, SftpChannelError};

#[derive(Debug, thiserror::Error)]
pub enum TermError {
    #[error("{0}")]
    Any(#[from] anyhow::Error),
    #[error("{0}")]
    Box(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[error(transparent)]
    HostVerification(HostVerificationFailed),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    GetHome(#[from] GetHomeError),
    #[error("{0}")]
    SftpChannel(#[from] SftpChannelError),
}
