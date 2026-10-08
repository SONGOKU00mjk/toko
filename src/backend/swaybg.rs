use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

use crate::cache::cache_dir;

use super::{BackendError, WallpaperBackend, run_with_timeout};

pub struct SwaybgBackend {
    child: Option<Child>,
}

impl SwaybgBackend {
    pub fn new() -> Self {
        Self { child: None }
    }

    fn reap_child(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl WallpaperBackend for SwaybgBackend {
    fn name(&self) -> &'static str {
        "swaybg"
    }

    fn set_wallpaper(&mut self, path: &Path) -> Result<(), BackendError> {
        self.reap_child();

        if let Some(pid) = read_swaybg_pid() {
            if is_swaybg_process(pid) {
                let _ = Command::new("kill").arg(pid.to_string()).status();
                thread::sleep(Duration::from_millis(150));
                if is_swaybg_process(pid) {
                    let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
                }
            }
            remove_swaybg_pid();
        }

        let mut child = Command::new("swaybg")
            .arg("-i")
            .arg(path)
            .arg("-m")
            .arg("fill")
            .arg("-o")
            .arg("*")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(BackendError::Io)?;

        thread::sleep(Duration::from_millis(120));

        match child.try_wait() {
            Ok(Some(status)) => Err(BackendError::ExecutionFailed(format!(
                "swaybg exited immediately with {status} \
                 (compositor may not support wlr-layer-shell)"
            ))),
            Ok(None) => {
                let pid = child.id();
                if let Err(e) = write_swaybg_pid(pid) {
                    eprintln!("warning: could not write swaybg pid file: {e}");
                }
                self.child = Some(child);
                Ok(())
            }
            Err(e) => Err(BackendError::Io(e)),
        }
    }
}

fn swaybg_pid_path() -> Option<PathBuf> {
    cache_dir().map(|d| d.join("swaybg.pid"))
}

fn read_swaybg_pid() -> Option<u32> {
    let path = swaybg_pid_path()?;
    let text = fs::read_to_string(&path).ok()?;
    text.trim().parse::<u32>().ok()
}

fn write_swaybg_pid(pid: u32) -> io::Result<()> {
    let path = swaybg_pid_path().ok_or_else(|| io::Error::other("cache dir unavailable"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, format!("{pid}\n"))
}

fn remove_swaybg_pid() {
    if let Some(path) = swaybg_pid_path() {
        let _ = fs::remove_file(path);
    }
}

fn is_swaybg_process(pid: u32) -> bool {
    let proc_dir = PathBuf::from(format!("/proc/{pid}"));
    if !proc_dir.is_dir() {
        return false;
    }
    match fs::read_to_string(proc_dir.join("comm")) {
        Ok(s) => s.trim() == "swaybg",
        Err(_) => false,
    }
}

pub(super) fn is_available() -> bool {
    let mut cmd = Command::new("swaybg");
    cmd.arg("--version");
    match run_with_timeout(&mut cmd, Duration::from_millis(500)) {
        Some(out) => out.status.success(),
        None => false,
    }
}
