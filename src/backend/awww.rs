use std::{path::Path, process::Command, time::Duration};

use super::{BackendError, WallpaperBackend, run_with_timeout};

pub struct AwwwBackend;

impl WallpaperBackend for AwwwBackend {
    fn name(&self) -> &'static str {
        "awww"
    }

    fn set_wallpaper(&mut self, path: &Path) -> Result<(), BackendError> {
        let output = Command::new("awww")
            .arg("img")
            .arg(path)
            .arg("--transition-type")
            .arg("simple")
            .arg("--transition-duration")
            .arg("0.5")
            .output()
            .map_err(BackendError::Io)?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let msg = if stderr.is_empty() {
                format!("awww exited with status {}", output.status)
            } else {
                stderr
            };
            Err(BackendError::ExecutionFailed(msg))
        }
    }
}

pub(super) fn is_usable() -> bool {
    let mut cmd = Command::new("awww");
    cmd.arg("query");
    match run_with_timeout(&mut cmd, Duration::from_millis(500)) {
        Some(out) => out.status.success(),
        None => false,
    }
}
