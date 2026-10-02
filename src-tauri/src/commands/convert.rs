use super::CancelFlag;
use crate::engine;
use crate::models::{ConvertRequest, Failure, Notice, ProgressEvent};
use rayon::prelude::*;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs::OpenOptions, io::Write, path::PathBuf, sync::OnceLock};
use tauri::ipc::Channel;
use tauri::State;

fn conversion_log_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Conversor")
        .join("conversion.log")
}

fn log_conversion_failure(path: &PathBuf, job: &crate::models::JobSpec, output_dir: &PathBuf, error: &engine::ConvertError) {
    static LOG_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard = LOG_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let result = (|| -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        writeln!(
            file,
            "{timestamp} | ERROR | source={:?} | output_dir={:?} | target={} | detail={error:?}",
            job.item.path,
            output_dir,
            job.target_format
        )
    })();
    if let Err(log_error) = result {
        eprintln!("No se pudo escribir el log de conversión {}: {log_error}", path.display());
    }
}

#[tauri::command]
pub async fn start_conversion(
    request: ConvertRequest,
    on_event: Channel<ProgressEvent>,
    cancel: State<'_, CancelFlag>,
) -> Result<(), String> {
    cancel.0.store(false, Ordering::SeqCst);
    let threads = engine::thread_count(&request.thread_profile);
    let total = request.jobs.len();
    std::fs::create_dir_all(&request.output_dir).map_err(|e| e.to_string())?;
    let log_path = conversion_log_path();

    engine::begin_batch();
    let _ = on_event.send(ProgressEvent::Started { total, threads });

    let ConvertRequest { jobs, output_dir, conflict_policy, .. } = request;
    // Arc compartido con el estado: `cancel_conversion` lo activa en vivo.
    let cancelled_in = cancel.0.clone();
    let log_path_in = log_path.clone();

    let job = tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .map_err(|e| e.to_string())?;

        let done = AtomicUsize::new(0);
        let active = AtomicUsize::new(0);
        let succeeded = AtomicUsize::new(0);
        let skipped = AtomicUsize::new(0);
        let saved = AtomicI64::new(0);
        let failures = Mutex::new(Vec::<Failure>::new());
        let notices = Mutex::new(Vec::<Notice>::new());
        let passthrough = AtomicUsize::new(0);

        pool.install(|| {
            jobs.par_iter().for_each(|job| {
                if cancelled_in.load(Ordering::Relaxed) {
                    return;
                }
                active.fetch_add(1, Ordering::Relaxed);
                match engine::convert_one(job, &output_dir, conflict_policy) {
                    Ok(engine::Outcome::Converted { bytes_saved, mode, warnings }) => {
                        succeeded.fetch_add(1, Ordering::Relaxed);
                        saved.fetch_add(bytes_saved, Ordering::Relaxed);
                        if mode == engine::Mode::Passthrough {
                            passthrough.fetch_add(1, Ordering::Relaxed);
                        }
                        if !warnings.is_empty() {
                            let mut n = notices.lock().unwrap();
                            for message in warnings {
                                n.push(Notice {
                                    item_id: job.item.id.clone(),
                                    name: job.item.name.clone(),
                                    message,
                                });
                            }
                        }
                    }
                    Ok(engine::Outcome::Skipped) => {
                        skipped.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => {
                        log_conversion_failure(&log_path_in, job, &output_dir, &e);
                        failures.lock().unwrap().push(Failure {
                            item_id: job.item.id.clone(),
                            name: job.item.name.clone(),
                            reason: e.to_user_message(),
                        });
                    }
                }
                let still_active = active.fetch_sub(1, Ordering::Relaxed) - 1;
                let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                let elapsed = started.elapsed().as_secs_f64();
                let eta = Some(((elapsed / d as f64) * (total - d) as f64) as u64);
                let _ = on_event.send(ProgressEvent::Progress {
                    done: d,
                    total,
                    active_threads: still_active,
                    eta_seconds: eta,
                });
            });
        });

        let failures = failures.into_inner().unwrap();
        let skipped = skipped.load(Ordering::Relaxed);
        let _ = on_event.send(ProgressEvent::Finished {
            succeeded: succeeded.load(Ordering::Relaxed),
            skipped,
            failures,
            notices: notices.into_inner().unwrap(),
            passthrough: passthrough.load(Ordering::Relaxed),
            elapsed_ms: started.elapsed().as_millis() as u64,
            bytes_saved: saved.load(Ordering::Relaxed),
            output_dir,
            log_path,
        });
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| e.to_string())?;

    job
}

#[tauri::command]
pub fn cancel_conversion(cancel: State<'_, CancelFlag>) {
    cancel.0.store(true, Ordering::SeqCst);
}
