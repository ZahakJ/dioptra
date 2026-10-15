//! Dioptra desktop shell: one engine command, `dispatch`, over `dioptra_core::api::Session`,
//! plus the native menu on macOS (built from the UI's command registry, so there is one list of
//! commands) and remembered window size and position.
use std::sync::Mutex;

use dioptra_core::api::Session;
use serde::Deserialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Runtime, State};

struct Engine(Mutex<Session>);

#[tauri::command]
fn dispatch(engine: State<'_, Engine>, json: String) -> String {
    let mut session = engine.0.lock().expect("engine mutex");
    session.dispatch_json(&json)
}

/// One menu of the command tree, as the UI's registry describes it.
#[derive(Debug, Deserialize)]
struct MenuGroupDto {
    title: String,
    /// Groups of items; a separator goes between groups.
    groups: Vec<Vec<MenuItemDto>>,
}

#[derive(Debug, Deserialize)]
struct MenuItemDto {
    id: String,
    title: String,
    /// A Tauri accelerator such as `CmdOrCtrl+Shift+S`, only for keys that are safe to take
    /// away from the web view (the UI decides).
    accelerator: Option<String>,
    /// `cut`, `copy` or `paste`: the system item, so text fields keep working.
    predefined: Option<String>,
}

/// The event the UI listens to; its payload is a command id.
const MENU_EVENT: &str = "dioptra://menu";

fn build_menu<R: Runtime>(app: &AppHandle<R>, menus: &[MenuGroupDto]) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;
    // The application menu every Mac app has.
    let app_menu = Submenu::new(app, "Dioptra", true)?;
    app_menu.append(&PredefinedMenuItem::about(
        app,
        Some("About Dioptra"),
        None,
    )?)?;
    app_menu.append(&PredefinedMenuItem::separator(app)?)?;
    app_menu.append(&PredefinedMenuItem::services(app, None)?)?;
    app_menu.append(&PredefinedMenuItem::separator(app)?)?;
    app_menu.append(&PredefinedMenuItem::hide(app, None)?)?;
    app_menu.append(&PredefinedMenuItem::hide_others(app, None)?)?;
    app_menu.append(&PredefinedMenuItem::show_all(app, None)?)?;
    app_menu.append(&PredefinedMenuItem::separator(app)?)?;
    app_menu.append(&PredefinedMenuItem::quit(app, None)?)?;
    menu.append(&app_menu)?;
    for m in menus {
        let sub = Submenu::new(app, &m.title, true)?;
        for (gi, group) in m.groups.iter().enumerate() {
            if gi > 0 {
                sub.append(&PredefinedMenuItem::separator(app)?)?;
            }
            for it in group {
                match it.predefined.as_deref() {
                    Some("cut") => sub.append(&PredefinedMenuItem::cut(app, None)?)?,
                    Some("copy") => sub.append(&PredefinedMenuItem::copy(app, None)?)?,
                    Some("paste") => sub.append(&PredefinedMenuItem::paste(app, None)?)?,
                    _ => sub.append(&MenuItem::with_id(
                        app,
                        it.id.as_str(),
                        &it.title,
                        true,
                        it.accelerator.as_deref(),
                    )?)?,
                }
            }
        }
        if m.title == "View" {
            sub.append(&PredefinedMenuItem::separator(app)?)?;
            sub.append(&PredefinedMenuItem::fullscreen(app, None)?)?;
        }
        menu.append(&sub)?;
    }
    let window = Submenu::new(app, "Window", true)?;
    window.append(&PredefinedMenuItem::minimize(app, None)?)?;
    window.append(&PredefinedMenuItem::maximize(app, None)?)?;
    window.append(&PredefinedMenuItem::close_window(app, None)?)?;
    menu.append(&window)?;
    Ok(menu)
}

/// Install the native menu. Only macOS shows one; elsewhere the UI's own top bar holds the
/// command tree, so this builds the menu (keeping the code checked) and returns without
/// installing it.
#[tauri::command]
fn set_native_menu<R: Runtime>(
    app: AppHandle<R>,
    menus: Vec<MenuGroupDto>,
) -> Result<bool, String> {
    let menu = build_menu(&app, &menus).map_err(|e| e.to_string())?;
    if cfg!(target_os = "macos") {
        app.set_menu(menu).map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Where the Dioptra Lab key is kept: `license.key` in the app's config directory.
fn license_path<R: Runtime>(app: &AppHandle<R>) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("license.key"))
}

/// The stored Dioptra Lab key, if any. The engine checks it (`set_license`).
#[tauri::command]
fn license_key_read<R: Runtime>(app: AppHandle<R>) -> Result<Option<String>, String> {
    match std::fs::read_to_string(license_path(&app)?) {
        Ok(k) => Ok(Some(k)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Store a Dioptra Lab key, or remove it with `None`.
#[tauri::command]
fn license_key_write<R: Runtime>(app: AppHandle<R>, key: Option<String>) -> Result<(), String> {
    let path = license_path(&app)?;
    match key {
        Some(k) => {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            std::fs::write(&path, k.trim()).map_err(|e| e.to_string())
        }
        None => match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        },
    }
}

/// Build and run the application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Engine(Mutex::new(Session::new())))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .on_menu_event(|app, event| {
            let id = event.id().0.clone();
            if let Err(e) = app.emit(MENU_EVENT, id) {
                log::warn!("menu event not delivered: {e}");
            }
        })
        .invoke_handler(tauri::generate_handler![
            dispatch,
            set_native_menu,
            license_key_read,
            license_key_write
        ])
        .run(tauri::generate_context!())
        .expect("error while running Dioptra");
}
