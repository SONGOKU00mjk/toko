use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use ratatui::layout::{Rect, Size};
use ratatui_image::{picker::Picker, protocol::Protocol};

use crate::{
    animation::AnimationState,
    backend::WallpaperBackend,
    cache::PreviewKey,
    discover::{WallpaperEntry, discover_wallpapers},
    preview::{Job, JobQueue, PreviewResponse, PreviewResult, SharedProgress},
};

const STATUS_OK: Duration = Duration::from_secs(3);
const STATUS_ERR: Duration = Duration::from_secs(5);
const STATUS_INFO: Duration = Duration::from_secs(2);

pub struct App {
    pub wallpapers: Vec<WallpaperEntry>,
    pub selected: usize,
    pub should_quit: bool,
    pub message: Option<String>,

    pub status_message: Option<String>,
    pub status_expires_at: Option<Instant>,

    pub fullscreen: bool,

    pub cache: HashMap<PreviewKey, Protocol>,
    pub preview_key: Option<PreviewKey>,
    pub preview_error: Option<String>,
    pub preview_area: Rect,
    pub loading: bool,

    pub animation: Option<AnimationState>,
    pub picker: Arc<Picker>,

    pub queue: Arc<JobQueue>,
    pub response_rx: mpsc::Receiver<PreviewResponse>,
    pub precache_tx: mpsc::Sender<Size>,
    pub precache_triggered: bool,
    pub generation: u64,
    pub needs_reload: bool,

    pub progress: SharedProgress,
    pub backend: Option<Box<dyn WallpaperBackend>>,

    pub dimension_cache: HashMap<PathBuf, Option<(u32, u32)>>,
}

impl App {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        queue: Arc<JobQueue>,
        response_rx: mpsc::Receiver<PreviewResponse>,
        precache_tx: mpsc::Sender<Size>,
        progress: SharedProgress,
        backend: Option<Box<dyn WallpaperBackend>>,
        picker: Arc<Picker>,
        wallpaper_dir: &Path,
    ) -> Self {
        let (wallpapers, message) = match discover_wallpapers(wallpaper_dir) {
            Ok(list) if list.is_empty() => (
                list,
                Some(String::from(
                    "No supported images found in the wallpaper directory",
                )),
            ),
            Ok(list) => (list, None),
            Err(err) => (Vec::new(), Some(err)),
        };

        Self {
            wallpapers,
            selected: 0,
            should_quit: false,
            message,
            status_message: None,
            status_expires_at: None,
            fullscreen: false,
            cache: HashMap::new(),
            preview_key: None,
            preview_error: None,
            preview_area: Rect::default(),
            loading: false,
            animation: None,
            picker,
            queue,
            response_rx,
            precache_tx,
            precache_triggered: false,
            generation: 0,
            needs_reload: true,
            progress,
            backend,
            dimension_cache: HashMap::new(),
        }
    }

    pub fn set_status(&mut self, text: String, duration: Duration) {
        self.status_message = Some(text);
        self.status_expires_at = Some(Instant::now() + duration);
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
        self.status_expires_at = None;
    }

    pub fn tick_status(&mut self) {
        if let Some(expires) = self.status_expires_at
            && Instant::now() >= expires
        {
            self.clear_status();
        }
    }

    pub fn tick_animation(&mut self) {
        let size = Size::new(self.preview_area.width, self.preview_area.height);
        if let Some(state) = self.animation.as_mut() {
            state.tick(size);
        }
    }

    pub fn get_dimensions(&mut self, path: &Path) -> Option<(u32, u32)> {
        if let Some(cached) = self.dimension_cache.get(path) {
            return *cached;
        }
        let dims = image::ImageReader::open(path)
            .ok()
            .and_then(|r| r.into_dimensions().ok());
        self.dimension_cache.insert(path.to_path_buf(), dims);
        dims
    }

    pub fn next(&mut self) {
        if !self.wallpapers.is_empty() {
            self.selected = (self.selected + 1) % self.wallpapers.len();
        }
        self.clear_status();
    }

    pub fn previous(&mut self) {
        if !self.wallpapers.is_empty() {
            if self.selected == 0 {
                self.selected = self.wallpapers.len() - 1;
            } else {
                self.selected -= 1;
            }
        }
        self.clear_status();
    }

    pub fn first(&mut self) {
        if !self.wallpapers.is_empty() {
            self.selected = 0;
        }
        self.clear_status();
    }

    pub fn last(&mut self) {
        if !self.wallpapers.is_empty() {
            self.selected = self.wallpapers.len() - 1;
        }
        self.clear_status();
    }

    pub fn select(&mut self) {
        if let Some(w) = self.wallpapers.get(self.selected) {
            let text = format!("Selected: {}", w.name);
            self.set_status(text, STATUS_INFO);
        }
    }

    pub fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        self.preview_key = None;
        self.preview_error = None;
        self.loading = true;
        self.needs_reload = true;
    }

    pub fn set_wallpaper(&mut self) {
        let Some(entry) = self.wallpapers.get(self.selected) else {
            self.set_status("No wallpaper selected".to_string(), STATUS_INFO);
            return;
        };
        let name = entry.name.clone();
        let path = entry.path.clone();

        let Some(backend) = self.backend.as_mut() else {
            self.set_status(
                "No wallpaper backend available (need awww or swaybg)".to_string(),
                STATUS_ERR,
            );
            return;
        };

        let outcome: Result<&'static str, String> = match backend.set_wallpaper(&path) {
            Ok(()) => Ok(backend.name()),
            Err(err) => Err(err.to_string()),
        };

        match outcome {
            Ok(bname) => {
                self.set_status(format!("✓ Wallpaper set: {name} (via {bname})"), STATUS_OK)
            }
            Err(msg) => self.set_status(format!("✗ Failed to set wallpaper: {msg}"), STATUS_ERR),
        }
    }

    pub fn load_preview(&mut self) {
        self.generation += 1;
        self.preview_error = None;
        self.preview_key = None;
        self.loading = false;

        if self.wallpapers.is_empty()
            || self.preview_area.width == 0
            || self.preview_area.height == 0
        {
            self.animation = None;
            return;
        }

        let entry_path = match self.wallpapers.get(self.selected) {
            Some(e) => e.path.clone(),
            None => {
                self.animation = None;
                return;
            }
        };

        let size = Size::new(self.preview_area.width, self.preview_area.height);

        if let Some(state) = self.animation.as_ref()
            && state.matches(&entry_path)
        {
            return;
        }

        self.animation = None;

        let key = PreviewKey::new(&entry_path, size);
        if self.cache.contains_key(&key) {
            self.preview_key = Some(key);
            return;
        }

        self.loading = true;
        let job = Job {
            generation: Some(self.generation),
            key,
            path: entry_path,
            size,
        };
        self.queue.push_interactive(job);
    }

    pub fn poll_preview(&mut self) -> bool {
        let mut changed = false;
        while let Ok(response) = self.response_rx.try_recv() {
            if response.generation != self.generation {
                continue;
            }
            self.loading = false;
            match response.result {
                Ok(PreviewResult::Static(protocol)) => {
                    self.cache.insert(response.key.clone(), protocol);
                    self.preview_key = Some(response.key);
                    self.animation = None;
                    self.preview_error = None;
                }
                Ok(PreviewResult::Animation {
                    frames,
                    delays,
                    first_protocol,
                }) => {
                    let source_path = response.key.path().to_path_buf();
                    let size = Size::new(self.preview_area.width, self.preview_area.height);
                    let state = AnimationState::new(
                        source_path,
                        frames,
                        delays,
                        first_protocol,
                        size,
                        Arc::clone(&self.picker),
                    );
                    self.preview_key = None;
                    self.animation = Some(state);
                    self.preview_error = None;
                }
                Err(err) => {
                    self.preview_key = None;
                    self.animation = None;
                    self.preview_error = Some(err);
                }
            }
            changed = true;
        }
        changed
    }
}
