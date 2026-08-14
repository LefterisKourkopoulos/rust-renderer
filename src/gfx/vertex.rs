/// Implemented by any `#[repr(C)]` type that is fed to a vertex buffer, so a
/// pipeline can ask for its layout without knowing the concrete type.
pub trait Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static>;
}
