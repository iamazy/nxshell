#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use egui::ViewportBuilder;
use nxshell::app::NxShell;
use nxshell::consts::PKG_NAME;
use std::io::stdout;
use tracing::Level;
use tracing_subscriber::fmt::writer::MakeWriterExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

fn init_log() {
    let env_filter = EnvFilter::new(format!("{PKG_NAME}=info"));
    let formatting_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_level(true)
        .with_ansi(true)
        .with_line_number(true)
        .with_writer(stdout.with_max_level(Level::INFO));

    Registry::default()
        .with(env_filter)
        .with(formatting_layer)
        .init();
}

pub fn main() -> eframe::Result<()> {
    init_log();

    let options = eframe::NativeOptions {
        centered: true,
        viewport: ViewportBuilder::default().with_min_inner_size((1000.0, 600.0)),
        ..Default::default()
    };
    NxShell::start(options)
}
