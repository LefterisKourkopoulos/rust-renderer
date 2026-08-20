pub mod app;
pub mod assets;
pub mod config;
pub mod debug;
pub mod gfx;
pub mod renderer;
pub mod scene;
#[cfg(not(target_arch = "wasm32"))]
pub mod scene_file;
pub mod shadow;
#[cfg(not(target_arch = "wasm32"))]
pub mod watch;
#[cfg(target_arch = "wasm32")]
pub mod web;

pub use app::{Action, App, Engine, run};
pub use config::{CameraConfig, InstanceGridConfig, PipelineMode, RendererConfig, SceneConfig};
pub use gfx::{GpuContext, Layouts, Texture, Vertex};
pub use renderer::Renderer;
pub use scene::Scene;

#[cfg(target_arch = "wasm32")]
pub use app::run_web;
#[cfg(target_arch = "wasm32")]
pub use web::RendererHandle;
