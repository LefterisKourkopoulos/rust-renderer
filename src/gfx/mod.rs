//! Reusable GPU plumbing. Nothing in here knows about this app's scene, input
//! or window handling, so it can be lifted into another project as-is.

pub mod context;
pub mod pipeline;
pub mod texture;
pub mod vertex;

pub use context::GpuContext;
pub use pipeline::{RenderPipelineConfig, create_render_pipeline};
pub use texture::Texture;
pub use vertex::Vertex;
