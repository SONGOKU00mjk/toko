mod awww;
mod swaybg;

use std::{
    env, io,
    path::Path,
    process::{Command, Output, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

use awww::AwwwBackend;
use swaybg::SwaybgBackend;

use crate::config::BackendPreference;

#[derive(Debug)]
pub enum BackendError {
    ExecutionFailed(String),
    Io(io::Error),
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendError::ExecutionFailed(msg) => write!(f, "Execution failed: {msg}"),
            BackendError::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

pub trait WallpaperBackend {
    fn name(&self) -> &'static str;
    fn set_wallpaper(&mut self, path: &Path) -> Result<(), BackendError>;
}

fn run_with_timeout(cmd: &mut Command, timeout: Duration) -> Option<Output> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = cmd.spawn().ok()?;
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let out = child.wait_with_output();
        let _ = tx.send(out);
    });
    rx.recv_timeout(timeout).ok()?.ok()
}

pub fn detect_backend(preference: &BackendPreference) -> Option<Box<dyn WallpaperBackend>> {
    if env::var("WAYLAND_DISPLAY").is_err() {
        return None;
    }
    match preference {
        BackendPreference::Awww => {
            if awww::is_usable() {
                Some(Box::new(AwwwBackend))
            } else {
                None
            }
        }
        BackendPreference::Swaybg => {
            if swaybg::is_available() {
                Some(Box::new(SwaybgBackend::new()))
            } else {
                None
            }
        }
        BackendPreference::Auto => {
            if awww::is_usable() {
                Some(Box::new(AwwwBackend))
            } else if swaybg::is_available() {
                Some(Box::new(SwaybgBackend::new()))
            } else {
                None
            }
        }
    }
}
