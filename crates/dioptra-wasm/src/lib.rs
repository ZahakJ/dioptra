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
        Engine { session: Session::new() }
    }

    /// JSON command in, JSON response out.
    pub fn dispatch(&mut self, json: &str) -> String {
        self.session.dispatch_json(json)
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
