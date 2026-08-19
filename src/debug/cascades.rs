use wgpu::util::DeviceExt;

use crate::gfx::pipeline::{self, RenderPipelineConfig};

pub struct CascadeDebug {
    bind_group: wgpu::BindGroup,
    buffer: wgpu::Buffer,
    render_pipeline: wgpu::RenderPipeline,
    layer: u32,
    cascade_count: u32,
    enabled: bool,
}

impl CascadeDebug {
    pub fn new(
        device: &wgpu::Device,
        shadow_view: &wgpu::TextureView,
        cascade_count: u32,
        color_format: wgpu::TextureFormat,
    ) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("cascade_debug_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cascade Debug Layer Buffer"),
            contents: bytemuck::cast_slice(&[0u32; 4]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cascade_debug_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        sample_type: wgpu::TextureSampleType::Depth,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cascade_debug_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer.as_entire_binding(),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Cascade Debug Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/cascade_debug.wgsl").into()),
        });

        let render_pipeline = pipeline::create_render_pipeline(
            device,
            &RenderPipelineConfig::new(
                "Cascade Debug Pipeline",
                &[&bind_group_layout],
                &shader,
                color_format,
            )
            .depth_write(false),
        );

        Self {
            bind_group,
            buffer,
            render_pipeline,
            layer: 0,
            cascade_count: cascade_count.max(1),
            enabled: false,
        }
    }

    pub fn cycle(&mut self, queue: &wgpu::Queue) {
        if !self.enabled {
            self.enabled = true;
            self.layer = 0;
        } else if self.layer + 1 < self.cascade_count {
            self.layer += 1;
        } else {
            self.enabled = false;
            self.layer = 0;
        }

        queue.write_buffer(
            &self.buffer,
            0,
            bytemuck::cast_slice(&[self.layer, 0, 0, 0]),
        );
    }

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        if !self.enabled {
            return;
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Cascade Debug Pass"),
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
