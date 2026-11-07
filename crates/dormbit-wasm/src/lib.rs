//! DORMBIT browser demo core (judge-demo Task 38): `dormbit-core` compiled to WebAssembly
//! over an in-memory virtual home folder. No real disk, no network.
//!
//! Any new dependency must pass
//! `cargo build -p dormbit-wasm --target wasm32-unknown-unknown` (ADR-001).
#![forbid(unsafe_code)]

pub mod engine;

/// Re-exported so downstream crates can confirm they link the same core.
pub const CORE_VERSION: &str = dormbit_core::VERSION;

#[cfg(target_arch = "wasm32")]
mod bindings {
    use wasm_bindgen::prelude::*;

    use super::engine::Engine;

    fn js_err(e: dormbit_core::model::DormbitError) -> JsValue {
        JsValue::from_str(&serde_json::to_string(&e).unwrap_or_else(|_| e.message.clone()))
    }

    #[wasm_bindgen]
    pub struct DemoEngine {
        inner: Engine,
    }

    #[wasm_bindgen]
    impl DemoEngine {
        /// Loads the fixture tar and Sidecar. Errors are `DormbitError` JSON.
        #[wasm_bindgen(constructor)]
        pub fn new(fixture_tar: &[u8], sidecar_json: &str) -> Result<DemoEngine, JsValue> {
            Engine::new(fixture_tar, sidecar_json)
                .map(|inner| DemoEngine { inner })
                .map_err(js_err)
        }

        /// Runs one command; `args_json` and the result are JSON text.
        pub fn call(&mut self, command: &str, args_json: &str) -> Result<String, JsValue> {
            let args: serde_json::Value = serde_json::from_str(if args_json.is_empty() {
                "{}"
            } else {
                args_json
            })
            .map_err(|e| JsValue::from_str(&format!("bad args: {e}")))?;
            let out = self.inner.call(command, &args).map_err(js_err)?;
            Ok(out.to_string())
        }

        pub fn capsule_bytes(&self) -> Result<Vec<u8>, JsValue> {
            self.inner.capsule_bytes().map_err(js_err)
        }

        pub fn restored_file_bytes(&self, path: &str) -> Result<Vec<u8>, JsValue> {
            self.inner.restored_file_bytes(path).map_err(js_err)
        }

        pub fn reset(&mut self) {
            self.inner.reset();
        }

        pub fn state_digest(&self) -> String {
            self.inner.state_digest()
        }
    }
}
