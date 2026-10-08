use std::{
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

use image::DynamicImage;
use ratatui::layout::Size;
use ratatui_image::{FilterType, Resize, picker::Picker, protocol::Protocol};

const MIN_FRAME_DELAY: Duration = Duration::from_millis(20);

pub struct DecodedGif {
    pub frames: Vec<DynamicImage>,
    pub delays: Vec<Duration>,
}

struct EncodeRequest {
    id: u64,
    frame: DynamicImage,
    size: Size,
}

struct EncodeResponse {
    id: u64,
    result: Result<Protocol, String>,
}

pub fn decode_and_prescale_gif(path: &Path, max_dim: u32) -> Result<Option<DecodedGif>, String> {
    use image::AnimationDecoder;
    use std::io::BufReader;

    let file = std::fs::File::open(path).map_err(|e| format!("Failed to open GIF: {e}"))?;
    let decoder = image::codecs::gif::GifDecoder::new(BufReader::new(file))
        .map_err(|e| format!("Failed to read GIF: {e}"))?;

    let mut frames: Vec<DynamicImage> = Vec::new();
    let mut delays: Vec<Duration> = Vec::new();

    for frame in decoder.into_frames() {
        let frame = frame.map_err(|e| format!("Failed to decode GIF frame: {e}"))?;
        let delay = Duration::from(frame.delay());
        let delay = if delay < MIN_FRAME_DELAY {
            MIN_FRAME_DELAY
        } else {
            delay
        };
        delays.push(delay);

        let img = DynamicImage::ImageRgba8(frame.into_buffer());
        let scaled = if img.width() > max_dim || img.height() > max_dim {
            img.resize(max_dim, max_dim, FilterType::CatmullRom)
        } else {
            img
        };
        frames.push(scaled);
    }

    if frames.len() <= 1 {
        return Ok(None);
    }

    Ok(Some(DecodedGif { frames, delays }))
}

pub struct AnimationState {
    pub source_path: PathBuf,
    frames: Vec<DynamicImage>,
    delays: Vec<Duration>,
    current: usize,
    next_frame_at: Instant,
    pub displayed: Option<Protocol>,
    pending: bool,
    latest_id: u64,
    last_size: Size,
    request_tx: mpsc::Sender<EncodeRequest>,
    response_rx: mpsc::Receiver<EncodeResponse>,
}

impl AnimationState {
    pub fn new(
        source_path: PathBuf,
        frames: Vec<DynamicImage>,
        delays: Vec<Duration>,
        first_protocol: Protocol,
        first_size: Size,
        picker: Arc<Picker>,
    ) -> Self {
        let (request_tx, request_rx) = mpsc::channel::<EncodeRequest>();
        let (response_tx, response_rx) = mpsc::channel::<EncodeResponse>();

        thread::spawn(move || {
            while let Ok(mut req) = request_rx.recv() {
                while let Ok(newer) = request_rx.try_recv() {
                    req = newer;
                }
                let result = picker
                    .new_protocol(
                        req.frame,
                        req.size,
                        Resize::Fit(Some(FilterType::CatmullRom)),
                    )
                    .map_err(|e| format!("Failed to encode GIF frame: {e}"));
                if response_tx
                    .send(EncodeResponse { id: req.id, result })
                    .is_err()
                {
                    break;
                }
            }
        });

        let first_delay = delays.first().copied().unwrap_or(MIN_FRAME_DELAY);

        Self {
            source_path,
            frames,
            delays,
            current: 0,
            next_frame_at: Instant::now() + first_delay,
            displayed: Some(first_protocol),
            pending: false,
            latest_id: 0,
            last_size: first_size,
            request_tx,
            response_rx,
        }
    }

    pub fn matches(&self, path: &Path) -> bool {
        self.source_path == path
    }

    pub fn tick(&mut self, size: Size) {
        while let Ok(resp) = self.response_rx.try_recv() {
            self.pending = false;
            if resp.id == self.latest_id
                && let Ok(p) = resp.result
            {
                self.displayed = Some(p);
            }
        }

        let size_changed = size != self.last_size;
        self.last_size = size;

        let frame_due = self.frames.len() > 1 && Instant::now() >= self.next_frame_at;

        if frame_due {
            self.current = (self.current + 1) % self.frames.len();
            let delay = self.delays[self.current];
            self.next_frame_at = Instant::now() + delay;
        }

        if (size_changed || frame_due) && !self.pending {
            self.send_current(size);
        }
    }

    fn send_current(&mut self, size: Size) {
        self.latest_id = self.latest_id.wrapping_add(1);
        let frame = self.frames[self.current].clone();
        let req = EncodeRequest {
            id: self.latest_id,
            frame,
            size,
        };
        self.pending = self.request_tx.send(req).is_ok();
    }
}
