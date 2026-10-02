mod commands;
mod engine;
mod models;

use commands::CancelFlag;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(CancelFlag::default())
        .invoke_handler(tauri::generate_handler![
            commands::system::get_system_info,
            commands::scan::scan_paths,
            commands::conflicts::check_conflicts,
            commands::convert::start_conversion,
            commands::convert::cancel_conversion,
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar la aplicación");
}
