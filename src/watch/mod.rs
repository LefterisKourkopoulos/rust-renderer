//! Hot reloading the scene: watching its file and rebuilding off the main thread.
//!
//! Native only. wasm has neither a filesystem to watch nor threads to load on.

pub mod loader;
pub mod watcher;

pub use loader::{Loaded, SceneLoader};
pub use watcher::SceneWatcher;
