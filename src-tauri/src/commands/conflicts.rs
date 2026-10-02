use crate::models::{Conflict, JobSpec};
use std::path::PathBuf;

/// Antes de procesar: informa qué archivos de salida ya existen en destino.
#[tauri::command]
pub fn check_conflicts(jobs: Vec<JobSpec>, output_dir: PathBuf) -> Vec<Conflict> {
    jobs.iter()
        .filter_map(|job| {
            let stem = job.item.path.file_stem()?.to_string_lossy().into_owned();
            let target = output_dir.join(format!("{stem}.{}", job.target_format));
            target.exists().then(|| Conflict {
                item_id: job.item.id.clone(),
                existing_path: target,
            })
        })
        .collect()
}
