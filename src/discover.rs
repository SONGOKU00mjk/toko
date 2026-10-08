use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct WallpaperEntry {
    pub name: String,
    pub path: PathBuf,
}

pub fn discover_wallpapers(dir: &Path) -> Result<Vec<WallpaperEntry>, String> {
    if !dir.is_dir() {
        return Err(format!("Directory not found: {}", dir.display()));
    }

    let read_dir =
        fs::read_dir(dir).map_err(|e| format!("Failed to read {}: {e}", dir.display()))?;

    let mut wallpapers = Vec::new();
    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !has_supported_extension(&path) {
            continue;
        }
        wallpapers.push(WallpaperEntry { name, path });
    }
    wallpapers.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(wallpapers)
}

fn has_supported_extension(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => matches!(
            ext.to_ascii_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "webp" | "gif"
        ),
        None => false,
    }
}
