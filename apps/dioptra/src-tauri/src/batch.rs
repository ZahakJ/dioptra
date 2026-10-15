//! Dioptra Lab batch analysis on the desktop: files are read and written here, so large
//! folders never cross the IPC bridge. `batch_file` runs one input; `watch_start` polls a
//! folder every two seconds and analyses each new file once its size has stopped changing,
//! writing the results to an `analysed` subfolder and sending a `dioptra://watch` event.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use dioptra_lab::batch::{BatchItem, BatchOptions};
use dioptra_lab::watch;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

/// The running watcher's stop flag.
#[derive(Default)]
pub struct Watching(Mutex<Option<Arc<AtomicBool>>>);

/// The event a watched folder sends for each file, or when watching stops.
const WATCH_EVENT: &str = "dioptra://watch";

#[derive(Clone, Serialize)]
struct WatchEvent {
    dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    item: Option<BatchItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    stopped: bool,
}

fn require_lab<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let engine = app.state::<crate::Engine>();
    let session = engine
        .0
        .lock()
        .map_err(|_| "the engine is busy".to_string())?;
    session.require_lab("Batch analysis")
}

fn template(base64: &str) -> Result<Vec<u8>, String> {
    dioptra_core::util::base64_decode(base64)
}

/// Apply the template to one file and write the outputs into `out_dir`.
#[tauri::command]
pub async fn batch_file<R: Runtime>(
    app: AppHandle<R>,
    template_base64: String,
    table: String,
    input: String,
    out_dir: String,
    options: Option<BatchOptions>,
) -> Result<BatchItem, String> {
    require_lab(&app)?;
    let tpl = template(&template_base64)?;
    let opts = options.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        watch::run_file(&tpl, &table, Path::new(&input), Path::new(&out_dir), &opts)
    })
    .await
    .map_err(|e| e.to_string())
}

/// Write `summary.csv` into `out_dir` for the items of a batch.
#[tauri::command]
pub fn batch_summary(out_dir: String, items: Vec<BatchItem>) -> Result<String, String> {
    watch::write_summary(Path::new(&out_dir), &items).map(|p| p.to_string_lossy().into_owned())
}

/// A file's bytes as base64 (a template chosen on disk).
#[tauri::command]
pub fn batch_read(path: String) -> Result<String, String> {
    std::fs::read(&path)
        .map(|b| dioptra_core::util::base64_encode(&b))
        .map_err(|e| format!("{path}: {e}"))
}

/// The data files directly inside a folder.
#[tauri::command]
pub fn batch_list(dir: String) -> Result<Vec<String>, String> {
    watch::data_files(Path::new(&dir))
        .map(|v| {
            v.into_iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect()
        })
        .map_err(|e| format!("{dir}: {e}"))
}

/// Start watching a folder (stopping any earlier watch).
#[tauri::command]
pub fn watch_start<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, Watching>,
    dir: String,
    template_base64: String,
    table: String,
    options: Option<BatchOptions>,
) -> Result<(), String> {
    require_lab(&app)?;
    let tpl = template(&template_base64)?;
    let opts = options.unwrap_or_default();
    let mut watcher = watch::Watcher::new(&dir).map_err(|e| format!("{dir}: {e}"))?;
    let flag = Arc::new(AtomicBool::new(true));
    if let Some(old) = state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .replace(flag.clone())
    {
        old.store(false, Ordering::SeqCst);
    }
    std::thread::spawn(move || {
        let out: PathBuf = Path::new(&dir).join(watch::OUTPUT_DIR);
        let mut items: Vec<BatchItem> = Vec::new();
        let send = |item: Option<BatchItem>, error: Option<String>, stopped: bool| {
            let ev = WatchEvent {
                dir: dir.clone(),
                item,
                error,
                stopped,
            };
            if let Err(e) = app.emit(WATCH_EVENT, ev) {
                log::warn!("watch event not delivered: {e}");
            }
        };
        while flag.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(2));
            if !flag.load(Ordering::SeqCst) {
                break;
            }
            let ready = match watcher.poll() {
                Ok(r) => r,
                Err(e) => {
                    send(None, Some(format!("{dir}: {e}")), true);
                    return;
                }
            };
            for p in ready {
                if let Err(e) = require_lab(&app) {
                    send(None, Some(e), true);
                    return;
                }
                let item = watch::run_file(&tpl, &table, &p, &out, &opts);
                items.push(item.clone());
                if let Err(e) = watch::write_summary(&out, &items) {
                    send(None, Some(e), false);
                }
                send(Some(item), None, false);
            }
        }
        send(None, None, true);
    });
    Ok(())
}

/// Stop watching.
#[tauri::command]
pub fn watch_stop(state: State<'_, Watching>) -> Result<(), String> {
    if let Some(flag) = state.0.lock().map_err(|e| e.to_string())?.take() {
        flag.store(false, Ordering::SeqCst);
    }
    Ok(())
}
