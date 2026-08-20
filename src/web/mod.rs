//! The browser control surface: the only place `#[wasm_bindgen]` appears in this crate. Everything
//! it calls into (`Engine::load_glb`/`set_skybox`/`set_time_of_day`) is plain, cfg-free Rust that
//! also works natively — this module is just the JS-facing wrapper around it.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;

use crate::app::{App, Engine};

/// A handle to a running renderer, returned by `init` once its `Engine` is ready. Cloning the
/// inner `Rc` (not the handle itself) is how `App`'s winit callbacks and this handle share the
/// same live `Engine`.
#[wasm_bindgen]
pub struct RendererHandle {
    engine: Rc<RefCell<Option<Engine>>>,
}

impl RendererHandle {
    pub(crate) fn from_engine(engine: Rc<RefCell<Option<Engine>>>) -> Self {
        Self { engine }
    }
}

#[wasm_bindgen]
impl RendererHandle {
    /// Creates and starts a renderer attached to the canvas with the given DOM id, resolving
    /// once the GPU device is ready (or rejecting if e.g. WebGPU is unavailable).
    pub fn init(canvas_id: String) -> js_sys::Promise {
        console_error_panic_hook::set_once();
        // Only the first call actually installs the logger; later calls (e.g. after the
        // zero-config `run_web` entry point already ran) are expected to fail and are ignored.
        let _ = console_log::init_with_level(log::Level::Info);

        let engine: Rc<RefCell<Option<Engine>>> = Rc::new(RefCell::new(None));
        let event_loop = winit::event_loop::EventLoop::with_user_event()
            .build()
            .expect("the winit event loop can always be built on the web");

        let mut event_loop = Some(event_loop);
        let engine_for_app = engine.clone();
        js_sys::Promise::new(&mut |resolve, reject| {
            let Some(event_loop) = event_loop.take() else {
                return;
            };
            let app = App::new_controlled(
                &event_loop,
                canvas_id.clone(),
                engine_for_app.clone(),
                resolve,
                reject,
            );
            winit::platform::web::EventLoopExtWebSys::spawn_app(event_loop, app);
        })
    }

    #[wasm_bindgen(js_name = loadGlb)]
    pub fn load_glb(&self, bytes: &[u8], file_name: &str) -> Result<(), JsValue> {
        let mut guard = self.engine.borrow_mut();
        let engine = guard.as_mut().ok_or_else(not_ready)?;
        engine.load_glb(bytes, file_name).map_err(to_js_error)
    }

    #[wasm_bindgen(js_name = setSkybox)]
    pub fn set_skybox(&self, bytes: &[u8]) -> Result<(), JsValue> {
        let mut guard = self.engine.borrow_mut();
        let engine = guard.as_mut().ok_or_else(not_ready)?;
        engine.set_skybox(bytes).map_err(to_js_error)
    }

    #[wasm_bindgen(js_name = setTimeOfDay)]
    pub fn set_time_of_day(&self, hour: f32) -> Result<(), JsValue> {
        let mut guard = self.engine.borrow_mut();
        let engine = guard.as_mut().ok_or_else(not_ready)?;
        engine.set_time_of_day(hour);
        Ok(())
    }
}

fn not_ready() -> JsValue {
    JsValue::from_str("the renderer is not initialized yet")
}

fn to_js_error(e: anyhow::Error) -> JsValue {
    JsValue::from_str(&format!("{e:#}"))
}
