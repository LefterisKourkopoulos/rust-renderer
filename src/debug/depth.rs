use crate::gfx::pipeline::{self, RenderPipelineConfig};
use crate::gfx::texture::Texture;

pub struct DepthDebug {
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    render_pipeline: wgpu::RenderPipeline,
    pub enabled: bool,
}

impl DepthDebug {
    pub fn new(
        device: &wgpu::Device,
        depth_texture: &Texture,
        color_format: wgpu::TextureFormat,
    ) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = Texture::bind_group_layout(
            device,
            wgpu::TextureSampleType::Depth,
            "depth_bind_group_layout",
        );

        let bind_group = depth_texture.bind_group_with_sampler(
            device,
            &bind_group_layout,
            &sampler,
            "depth_bind_group",
        );

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Depth Debug Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/depth_debug.wgsl").into()),
        });

        let render_pipeline = pipeline::create_render_pipeline(
            device,
            &RenderPipelineConfig {
                label: "Depth Debug Pipeline",
                bind_group_layouts: &[&bind_group_layout],
                shader: &shader,
                vertex_buffers: &[],
                color_format,
                depth_write: false,
            },
        );

        Self {
            sampler,
            bind_group_layout,
            bind_group,
            render_pipeline,
            enabled: false,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, depth_texture: &Texture) {
        self.bind_group = depth_texture.bind_group_with_sampler(
            device,
            &self.bind_group_layout,
            &self.sampler,
            "depth_bind_group",
        );
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        if !self.enabled {
            return;
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Depth Debug Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.render_pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
