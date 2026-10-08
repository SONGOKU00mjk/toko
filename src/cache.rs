use std::{
    env, fs, io,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use ratatui::layout::Size;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PreviewKey {
    path: PathBuf,
    width: u16,
    height: u16,
}

impl PreviewKey {
    pub fn new(path: &Path, size: Size) -> Self {
        Self {
            path: path.to_path_buf(),
            width: size.width,
            height: size.height,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn cache_dir() -> Option<PathBuf> {
    if let Ok(xdg) = env::var("XDG_CACHE_HOME")
        && !xdg.is_empty()
    {
        return Some(PathBuf::from(xdg).join("toko"));
    }
    env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".cache").join("toko"))
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

const CACHE_VERSION: u8 = 4;

pub fn cache_stem(path: &Path, max_dim: u32) -> String {
    let mut buf = Vec::new();
    buf.extend_from_slice(path.to_string_lossy().as_bytes());
    buf.push(CACHE_VERSION);
    buf.extend_from_slice(&max_dim.to_le_bytes());
    format!("{:016x}", fnv1a(&buf))
}

struct Fingerprint {
    mtime_secs: u64,
    size: u64,
}

fn fingerprint(path: &Path) -> Option<Fingerprint> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let secs = mtime.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(Fingerprint {
        mtime_secs: secs,
        size: meta.len(),
    })
}

fn read_meta(path: &Path) -> Option<Fingerprint> {
    let text = fs::read_to_string(path).ok()?;
    let mut mtime_secs = None;
    let mut size = None;
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('=') {
            match k {
                "mtime" => mtime_secs = v.parse().ok(),
                "size" => size = v.parse().ok(),
                _ => {}
            }
        }
    }
    Some(Fingerprint {
        mtime_secs: mtime_secs?,
        size: size?,
    })
}

fn write_meta(path: &Path, fp: &Fingerprint) -> io::Result<()> {
    fs::write(path, format!("mtime={}\nsize={}\n", fp.mtime_secs, fp.size))
}

pub fn is_cached_fresh(source: &Path, max_dim: u32) -> bool {
    let Some(dir) = cache_dir() else { return false };
    let stem = cache_stem(source, max_dim);
    let png = dir.join(format!("{stem}.png"));
    let meta = dir.join(format!("{stem}.meta"));
    if !png.exists() || !meta.exists() {
        return false;
    }
    let Some(cached) = read_meta(&meta) else {
        return false;
    };
    let Some(current) = fingerprint(source) else {
        return false;
    };
    cached.mtime_secs == current.mtime_secs && cached.size == current.size
}

pub fn ensure_cached(source: &Path, max_dim: u32) -> Result<image::DynamicImage, String> {
    let dir = cache_dir().ok_or_else(|| "Cache directory not available".to_string())?;
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("warning: could not create cache dir: {e}");
    }

    let stem = cache_stem(source, max_dim);
    let png_path = dir.join(format!("{stem}.png"));
    let meta_path = dir.join(format!("{stem}.meta"));

    let source_fp = fingerprint(source);

    if png_path.exists() && meta_path.exists() {
        let cached = read_meta(&meta_path);
        let fresh = matches!(
            (&cached, &source_fp),
            (Some(c), Some(s)) if c.mtime_secs == s.mtime_secs && c.size == s.size
        );
        if fresh {
            match image::ImageReader::open(&png_path)
                .map_err(|e| e.to_string())
                .and_then(|r| r.decode().map_err(|e| e.to_string()))
            {
                Ok(img) => return Ok(img),
                Err(e) => eprintln!("warning: cache png unusable ({e}), regenerating"),
            }
        }
    }

    let src_img = image::ImageReader::open(source)
        .map_err(|e| format!("Failed to open image: {e}"))?
        .decode()
        .map_err(|e| format!("Failed to decode image: {e}"))?;

    let pre = if src_img.width() > max_dim || src_img.height() > max_dim {
        src_img.resize(max_dim, max_dim, image::imageops::FilterType::CatmullRom)
    } else {
        src_img
    };

    if let Err(e) = pre.save(&png_path) {
        eprintln!("warning: could not write cache png: {e}");
    } else if let Some(fp) = source_fp.as_ref()
        && let Err(e) = write_meta(&meta_path, fp)
    {
        eprintln!("warning: could not write cache meta: {e}");
    }

    Ok(pre)
}
