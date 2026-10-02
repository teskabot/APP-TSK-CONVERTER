use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Image,
    Audio,
}

/// Archivo detectado en el escaneo. `is_video` marca contenedores de video
/// cuyo audio se extraerá (se agrupan en la sección AUDIO).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    pub id: String,
    pub path: PathBuf,
    pub name: String,
    pub ext: String,
    pub kind: MediaKind,
    pub is_video: bool,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum QualityPreset {
    Lossless,
    Balanced,
    Compact,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConflictPolicy {
    Rename,
    Overwrite,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSpec {
    pub item: MediaItem,
    /// Extensión de salida en minúsculas: "webm", "flac", "png", "webp"...
    pub target_format: String,
    pub preset: QualityPreset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertRequest {
    pub jobs: Vec<JobSpec>,
    pub output_dir: PathBuf,
    pub conflict_policy: ConflictPolicy,
    /// "efficient" | "balanced" | "turbo"
    pub thread_profile: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub cpu_cores: usize,
    pub default_threads: usize,
    pub default_output_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub item_id: String,
    pub existing_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub item_id: String,
    pub name: String,
    /// Mensaje en lenguaje claro, listo para mostrar.
    pub reason: String,
}

/// Aviso informativo (no es un error): el archivo se convirtió, pero con un ajuste.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub item_id: String,
    pub name: String,
    pub message: String,
}

/// Eventos que el backend envía al frontend durante el lote.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum ProgressEvent {
    #[serde(rename_all = "camelCase")]
    Started { total: usize, threads: usize },
    #[serde(rename_all = "camelCase")]
    Progress {
        done: usize,
        total: usize,
        active_threads: usize,
        eta_seconds: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Finished {
        succeeded: usize,
        skipped: usize,
        failures: Vec<Failure>,
        notices: Vec<Notice>,
        /// Archivos que se copiaron o remuxaron sin recodificar.
        passthrough: usize,
        elapsed_ms: u64,
        bytes_saved: i64,
        output_dir: PathBuf,
        log_path: PathBuf,
    },
}
