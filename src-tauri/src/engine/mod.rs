//! Motor de conversión.
//!   audio.rs -> FFmpeg/ffprobe como sidecar (passthrough bit-exact, bits, tags, carátula)
//!   image.rs -> pendiente (image-rs / mozjpeg / libwebp / libheif / libraw)

pub mod audio;

use crate::models::{ConflictPolicy, JobSpec, MediaKind};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("archivo no encontrado")]
    Missing,
    #[error("archivo dañado o ilegible")]
    Corrupt,
    #[error("archivo protegido")]
    Protected,
    #[error("formato no soportado")]
    Unsupported,
    #[error("el archivo no tiene pista de audio")]
    NoAudio,
    #[error("falta FFmpeg o un códec en esta instalación")]
    EngineMissing,
    #[error("este formato aún no está disponible")]
    NotYet,
    #[error("fallo de conversión: {0}")]
    Failed(String),
    #[error("error de disco: {0}")]
    Io(#[from] std::io::Error),
}

impl ConvertError {
    /// Mensajes cortos, en lenguaje claro, para "Archivos con problemas".
    pub fn to_user_message(&self) -> String {
        match self {
            ConvertError::Missing => "No se encontró el archivo".into(),
            ConvertError::Corrupt => "Archivo dañado".into(),
            ConvertError::Protected => "Archivo protegido".into(),
            ConvertError::Unsupported => "Formato no compatible".into(),
            ConvertError::NoAudio => "No tiene pista de audio".into(),
            ConvertError::EngineMissing => "Falta un componente de la app. Reinstálala".into(),
            ConvertError::NotYet => "La conversión de imágenes aún no está implementada".into(),
            ConvertError::Failed(detail) => format!("Falló la conversión: {detail}"),
            ConvertError::Io(_) => "No se pudo escribir en la carpeta de destino".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Copia directa o remux: el flujo original no se tocó (bit-exact).
    Passthrough,
    Reencoded,
}

pub enum Outcome {
    Converted { bytes_saved: i64, mode: Mode, warnings: Vec<String> },
    Skipped,
}

pub fn thread_count(profile: &str) -> usize {
    let cores = num_cpus::get().max(1);
    let n = match profile {
        "efficient" => cores / 2,
        "turbo" => cores,
        "balanced" => (cores as f32 * 0.8) as usize,
        _ => cores.saturating_sub(1), // por defecto: N - 1
    };
    n.max(1)
}

// ---------- Reserva de destinos (evita que dos hilos escriban el mismo archivo) ----------

fn reserved() -> &'static Mutex<HashSet<String>> {
    static R: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Llamar al inicio de cada lote.
pub fn begin_batch() {
    reserved().lock().unwrap().clear();
}

fn key(p: &Path) -> String {
    p.to_string_lossy().to_lowercase() // macOS/Windows no distinguen mayúsculas
}

/// Decide el archivo de salida. Dos orígenes con el mismo nombre dentro del
/// mismo lote (p. ej. `a/x.mp3` y `b/x.wav` → `x.flac`) nunca se pisan,
/// ni siquiera con "Sobrescribir": el segundo se renombra.
pub(crate) fn resolve_target(job: &JobSpec, dir: &Path, policy: ConflictPolicy) -> Option<PathBuf> {
    let stem = job.item.path.file_stem()?.to_string_lossy().into_owned();
    let ext = &job.target_format;
    let mut taken = reserved().lock().unwrap();

    let base = dir.join(format!("{stem}.{ext}"));
    let clash_in_batch = taken.contains(&key(&base));

    if !clash_in_batch {
        match policy {
            ConflictPolicy::Skip if base.exists() => return None,
            ConflictPolicy::Overwrite => {
                taken.insert(key(&base));
                return Some(base);
            }
            _ => {}
        }
    }

    let mut target = base;
    if clash_in_batch || policy == ConflictPolicy::Rename {
        let mut n = 0;
        while target.exists() || taken.contains(&key(&target)) {
            n += 1;
            target = dir.join(format!("{stem}_{n}.{ext}"));
        }
    }
    taken.insert(key(&target));
    Some(target)
}

pub fn convert_one(
    job: &JobSpec,
    output_dir: &Path,
    policy: ConflictPolicy,
) -> Result<Outcome, ConvertError> {
    if !job.item.path.exists() {
        return Err(ConvertError::Missing);
    }
    let Some(target) = resolve_target(job, output_dir, policy) else {
        return Ok(Outcome::Skipped);
    };
    match job.item.kind {
        MediaKind::Audio => audio::convert(job, &target),
        // Honesto antes que falso: no reportar éxito sin haber convertido nada.
        MediaKind::Image => Err(ConvertError::NotYet),
    }
}
