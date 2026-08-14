//! A small wgpu renderer.
//!
//! The crate is split so that each layer only knows about the one below it:
//! [`gfx`] is reusable GPU plumbing, [`scene`] is the data being drawn,
//! [`renderer`] turns one into the other, and [`app`] owns the window and input.

pub mod app;
pub mod assets;
pub mod config;
pub mod debug;
pub mod gfx;
pub mod renderer;
pub mod scene;

pub use app::{Action, App, Engine, run};
pub use config::{CameraConfig, InstanceGridConfig, RendererConfig, SceneConfig};
pub use gfx::{GpuContext, Texture, Vertex};
pub use renderer::Renderer;
pub use scene::Scene;

#[cfg(target_arch = "wasm32")]
pub use app::run_web;
