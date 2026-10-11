//! Dioptra desktop shell: one Tauri command, `dispatch`, over `dioptra_core::api::Session`.
use std::sync::Mutex;

use dioptra_core::api::Session;
use tauri::State;

struct Engine(Mutex<Session>);

#[tauri::command]
fn dispatch(engine: State<'_, Engine>, json: String) -> String {
    let mut session = engine.0.lock().expect("engine mutex");
    session.dispatch_json(&json)
}

/// Build and run the application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Engine(Mutex::new(Session::new())))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![dispatch])
        .run(tauri::generate_context!())
        .expect("error while running Dioptra");
}
