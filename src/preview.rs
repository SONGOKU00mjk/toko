use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
    time::Duration,
};

use image::DynamicImage;
use ratatui::layout::Size;
use ratatui_image::{FilterType, Resize, picker::Picker, protocol::Protocol};

use crate::{
    animation::{DecodedGif, decode_and_prescale_gif},
    cache::{PreviewKey, ensure_cached, is_cached_fresh},
    discover::discover_wallpapers,
};

pub struct Job {
    pub generation: Option<u64>,
    pub key: PreviewKey,
    pub path: PathBuf,
    pub size: Size,
}

pub struct JobQueue {
    queue: Mutex<VecDeque<Job>>,
    condvar: Condvar,
}

impl JobQueue {
    fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            condvar: Condvar::new(),
        }
    }

    pub fn push_interactive(&self, job: Job) {
        let mut q = self.queue.lock().unwrap();
        q.retain(|j| j.generation.is_none());
        q.push_front(job);
        self.condvar.notify_one();
    }

    fn push_precache(&self, job: Job) {
        let mut q = self.queue.lock().unwrap();
        q.push_back(job);
        self.condvar.notify_one();
    }

    fn pop(&self) -> Job {
        let mut q = self.queue.lock().unwrap();
        while q.is_empty() {
            q = self.condvar.wait(q).unwrap();
        }
        q.pop_front().unwrap()
    }
}

pub enum PreviewResult {
    Static(Protocol),
    Animation {
        frames: Vec<DynamicImage>,
        delays: Vec<Duration>,
        first_protocol: Protocol,
    },
}

pub struct PreviewResponse {
    pub generation: u64,
    pub key: PreviewKey,
    pub result: Result<PreviewResult, String>,
}

pub struct PrecacheProgress {
    pub total: usize,
    pub completed: usize,
    pub scheduler_done: bool,
}

impl PrecacheProgress {
    pub fn new() -> Self {
        Self {
            total: 0,
            completed: 0,
            scheduler_done: false,
        }
    }
}

pub type SharedProgress = Arc<Mutex<PrecacheProgress>>;

fn is_gif(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("gif"))
}

fn prepare_preview(
    picker: &Picker,
    source: &Path,
    size: Size,
    max_dim: u32,
) -> Result<PreviewResult, String> {
    if is_gif(source)
        && let Some(DecodedGif { frames, delays }) = decode_and_prescale_gif(source, max_dim)?
    {
        let first_protocol = picker
            .new_protocol(
                frames[0].clone(),
                size,
                Resize::Fit(Some(FilterType::CatmullRom)),
            )
            .map_err(|e| format!("Failed to encode first GIF frame: {e}"))?;
        return Ok(PreviewResult::Animation {
            frames,
            delays,
            first_protocol,
        });
    }

    let img = ensure_cached(source, max_dim)?;
    picker
        .new_protocol(img, size, Resize::Fit(Some(FilterType::Lanczos3)))
        .map(PreviewResult::Static)
        .map_err(|e| format!("Failed to create preview: {e}"))
}

pub fn spawn_worker_pool(
    picker: Arc<Picker>,
    num_workers: usize,
    progress: SharedProgress,
    max_dim: u32,
) -> (Arc<JobQueue>, mpsc::Receiver<PreviewResponse>) {
    let queue = Arc::new(JobQueue::new());
    let (res_tx, res_rx) = mpsc::channel::<PreviewResponse>();

    for _ in 0..num_workers {
        let queue = Arc::clone(&queue);
        let picker = Arc::clone(&picker);
        let res_tx = res_tx.clone();
        let progress = Arc::clone(&progress);
        thread::spawn(move || {
            loop {
                let job = queue.pop();
                match job.generation {
                    Some(generation) => {
                        let result = prepare_preview(&picker, &job.path, job.size, max_dim);
                        let resp = PreviewResponse {
                            generation,
                            key: job.key,
                            result,
                        };
                        if res_tx.send(resp).is_err() {
                            break;
                        }
                    }
                    None => {
                        let _ = ensure_cached(&job.path, max_dim);
                        if let Ok(mut p) = progress.lock() {
                            p.completed += 1;
                        }
                    }
                }
            }
        });
    }
    drop(res_tx);

    (queue, res_rx)
}

pub fn spawn_precache_scheduler(
    queue: Arc<JobQueue>,
    progress: SharedProgress,
    wallpaper_dir: PathBuf,
    max_dim: u32,
) -> mpsc::Sender<Size> {
    let (tx, rx) = mpsc::channel::<Size>();
    thread::spawn(move || {
        let Ok(size) = rx.recv() else { return };
        let Ok(wallpapers) = discover_wallpapers(&wallpaper_dir) else {
            if let Ok(mut p) = progress.lock() {
                p.scheduler_done = true;
            }
            return;
        };

        let mut pending = Vec::new();
        for entry in wallpapers {
            if is_gif(&entry.path) {
                continue;
            }
            if !is_cached_fresh(&entry.path, max_dim) {
                pending.push(entry);
            }
        }

        if let Ok(mut p) = progress.lock() {
            p.total = pending.len();
            p.scheduler_done = true;
        }

        for entry in pending {
            queue.push_precache(Job {
                generation: None,
                key: PreviewKey::new(&entry.path, size),
                path: entry.path,
                size,
            });
        }
    });
    tx
}
