use egui::WidgetText;
use egui_toast::{Toast, ToastKind, ToastOptions};
use orion::errors::UnknownCryptoError;
use std::string::FromUtf8Error;

#[derive(Debug, thiserror::Error)]
pub enum NxError {
    #[error("{0}")]
    Plain(String),
    #[error("{0}")]
    Box(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[error("{0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("{0}")]
    UnknownCrypto(#[from] UnknownCryptoError),
    #[error("{0}")]
    FromUtf8(#[from] FromUtf8Error),
}

pub fn error_toast<E: Into<WidgetText>>(err: E) -> Toast {
    Toast {
        text: err.into(),
        kind: ToastKind::Error,
        options: ToastOptions::default()
            .duration_in_seconds(5.0)
            .show_progress(true),
        ..Default::default()
    }
}
