//! wasm-bindgen facade: the same `dispatch` the Tauri app exposes.
#![forbid(unsafe_code)]

use dioptra_core::api::Session;
use wasm_bindgen::prelude::*;

/// An engine session living in the browser.
#[wasm_bindgen]
pub struct Engine {
    session: Session,
}

#[wasm_bindgen]
impl Engine {
    /// Create a new session.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Engine {
        let mut session = Session::new();
        // `SystemTime` is unavailable on wasm32; stamp history and provenance with the
        // browser clock instead.
        session.set_clock(Some(Box::new(|| {
            String::from(js_sys::Date::new_0().to_iso_string())
        })));
        Engine { session }
    }

    /// JSON command in, JSON response out.
    pub fn dispatch(&mut self, json: &str) -> String {
        dioptra_lab::dispatch_json(&mut self.session, json)
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
