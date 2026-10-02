use crate::models::SystemInfo;

#[tauri::command]
pub fn get_system_info() -> SystemInfo {
    let cores = num_cpus::get();
    SystemInfo {
        cpu_cores: cores,
        default_threads: cores.saturating_sub(1).max(1),
        default_output_dir: dirs::desktop_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
            .join("Archivos Convertidos"),
    }
}
