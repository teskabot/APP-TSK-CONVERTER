use crate::models::{MediaItem, MediaKind};
use std::path::PathBuf;
use std::path::Path;
use walkdir::WalkDir;

const IMAGE_EXT: &[&str] = &[
    "png", "webp", "jpg", "jpeg", "heic", "heif", "tif", "tiff", "svg", "cr2", "nef", "arw",
];
const AUDIO_EXT: &[&str] = &["webm", "mp3", "wav", "flac", "aac", "m4a", "ogg", "opus"];
const VIDEO_EXT: &[&str] = &["mp4", "mov", "mkv", "avi", "m4v"];

fn classify(path: &Path) -> Option<(MediaKind, bool, String)> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    if IMAGE_EXT.contains(&ext.as_str()) {
        Some((MediaKind::Image, false, ext))
    } else if AUDIO_EXT.contains(&ext.as_str()) {
        Some((MediaKind::Audio, false, ext))
    } else if VIDEO_EXT.contains(&ext.as_str()) {
        Some((MediaKind::Audio, true, ext))
    } else {
        None
    }
}

/// Recibe archivos y/o carpetas (drag & drop o diálogo) y devuelve los
/// archivos multimedia soportados, recorriendo carpetas de forma recursiva.
#[tauri::command]
pub fn scan_paths(paths: Vec<PathBuf>) -> Vec<MediaItem> {
    let mut items = Vec::new();
    for root in paths {
        for entry in WalkDir::new(&root).follow_links(false).into_iter().flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let Some((kind, is_video, ext)) = classify(path) else { continue };
            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
            items.push(MediaItem {
                id: uuid::Uuid::new_v4().to_string(),
                path: path.to_path_buf(),
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                ext,
                kind,
                is_video,
                size_bytes,
            });
        }
    }
    items
}
