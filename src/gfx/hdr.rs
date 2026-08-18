use wgpu::Operations;


pub struct HdrPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    // texture: texture::Texture,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
}

impl HdrPipeline {
    // pub fn new (device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Self {
        
    // }
}