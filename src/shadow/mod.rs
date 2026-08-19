pub mod cascades;

use wgpu::util::DeviceExt;

use crate::config::{MAX_CASCADES, ShadowConfig};
use crate::gfx::Vertex;
use crate::gfx::pipeline::{self, RenderPipelineConfig};
use crate::gfx::texture::Texture;
use crate::scene::camera::CameraState;
use crate::scene::instance::InstanceRaw;
use crate::scene::model::{DrawModel, Model, ModelVertex};

const CASCADE_SLOT: u64 = 256;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShadowUniform {
    light_view_proj: [[[f32; 4]; 4]; MAX_CASCADES],
    splits: [f32; MAX_CASCADES],
    cascade_count: u32,
    resolution: f32,
    normal_offset: f32,
    debug_mode: u32,
}

impl ShadowUniform {
    fn new(config: &ShadowConfig) -> Self {
        Self {
            light_view_proj: [cgmath::Matrix4::from_scale(1.0).into(); MAX_CASCADES],
            splits: [0.0; MAX_CASCADES],
            cascade_count: 0,
            resolution: config.resolution as f32,
            normal_offset: config.normal_offset,
            debug_mode: 0,
        }
    }
}

pub struct ShadowPass {
    depth: Texture,
    layer_views: Vec<wgpu::TextureView>,
    uniform: ShadowUniform,
    buffer: wgpu::Buffer,
    cascade_buffer: wgpu::Buffer,
    cascade_bind_groups: Vec<wgpu::BindGroup>,
    bind_group: wgpu::BindGroup,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    config: ShadowConfig,
}

impl ShadowPass {
    pub fn new(device: &wgpu::Device, config: ShadowConfig) -> Self {
        let cascade_count = config.cascade_count();

        let depth = Texture::create_depth_array(
            device,
            config.resolution,
            cascade_count as u32,
            "shadow_map",
        );

        let layer_views = (0..cascade_count)
            .map(|layer| depth.depth_layer_view(layer as u32, &format!("shadow_map_layer_{layer}")))
            .collect();

        let uniform = ShadowUniform::new(&config);
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shadow Uniform Buffer"),
            contents: bytemuck::bytes_of(&uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let cascade_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Cascade Buffer"),
            size: CASCADE_SLOT * cascade_count as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let cascade_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow_cascade_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let cascade_bind_groups = (0..cascade_count)
            .map(|layer| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("shadow_cascade_bind_group"),
                    layout: &cascade_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &cascade_buffer,
                            offset: CASCADE_SLOT * layer as u64,
                            size: std::num::NonZeroU64::new(64),
                        }),
                    }],
                })
            })
            .collect();

        let bind_group_layout = Self::bind_group_layout(device);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&depth.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&depth.sampler),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shadow"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/shadow.wgsl").into()),
        });

        let pipeline = pipeline::create_render_pipeline(
            device,
            &RenderPipelineConfig {
                label: "Shadow Pipeline",
                bind_group_layouts: &[&cascade_layout],
                shader: &shader,
                vertex_buffers: &[ModelVertex::desc(), InstanceRaw::desc()],
                color_format: None,
                depth_write: true,
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Front),
                depth_bias: wgpu::DepthBiasState {
                    constant: config.depth_bias,
                    slope_scale: config.depth_bias_slope,
                    clamp: 0.0,
                },
            },
        );

        Self {
            depth,
            layer_views,
            uniform,
            buffer,
            cascade_buffer,
            cascade_bind_groups,
            bind_group,
            bind_group_layout,
            pipeline,
            config,
        }
    }

    pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        sample_type: wgpu::TextureSampleType::Depth,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        })
    }

    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn depth_view(&self) -> &wgpu::TextureView {
        &self.depth.view
    }

    pub fn depth_texture(&self) -> &wgpu::Texture {
        &self.depth.texture
    }

    pub fn cascade_count(&self) -> usize {
        self.config.cascade_count()
    }

    pub fn set_debug_mode(&mut self, enabled: bool) {
        self.uniform.debug_mode = u32::from(enabled);
    }

    pub fn update(&mut self, queue: &wgpu::Queue, camera: &CameraState, light_direction: [f32; 3]) {
        let frustum = camera.frustum();
        let view = camera.view();
        let count = self.config.cascade_count();
        let (shadow_near, shadow_far) = self.config.range(frustum.znear, frustum.zfar);

        let splits =
            cascades::split_distances(shadow_near, shadow_far, self.config.split_lambda, count);

        let mut near = shadow_near;
        for (index, far) in splits.iter().copied().enumerate() {
            let corners = cascades::frustum_corners_world(view, &frustum, near * 0.98, far);
            let matrix =
                cascades::light_view_proj(&corners, light_direction.into(), self.config.z_mult);

            let raw: [[f32; 4]; 4] = matrix.into();
            self.uniform.light_view_proj[index] = raw;
            self.uniform.splits[index] = far;

            queue.write_buffer(
                &self.cascade_buffer,
                CASCADE_SLOT * index as u64,
                bytemuck::bytes_of(&raw),
            );

            near = far;
        }

        self.uniform.cascade_count = count as u32;
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&self.uniform));
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        model: &Model,
        instance_buffer: &wgpu::Buffer,
        instances: u32,
    ) {
        for (layer, view) in self.layer_views.iter().enumerate() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow Pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.cascade_bind_groups[layer], &[]);
            pass.set_vertex_buffer(1, instance_buffer.slice(..));
            pass.draw_model_depth(model, 0..instances);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_uniform_matches_the_shader_struct_layout() {
        assert_eq!(std::mem::size_of::<ShadowUniform>(), 288, "struct size");
        assert_eq!(std::mem::offset_of!(ShadowUniform, light_view_proj), 0);
        assert_eq!(std::mem::offset_of!(ShadowUniform, splits), 256);
        assert_eq!(std::mem::offset_of!(ShadowUniform, cascade_count), 272);
        assert_eq!(std::mem::offset_of!(ShadowUniform, resolution), 276);
        assert_eq!(std::mem::offset_of!(ShadowUniform, normal_offset), 280);
        assert_eq!(std::mem::offset_of!(ShadowUniform, debug_mode), 284);
        assert_eq!(
            std::mem::size_of::<ShadowUniform>() % 16,
            0,
            "uniform buffers need a 16 byte aligned size"
        );
    }

    #[test]
    fn a_cascade_matrix_fits_inside_its_alignment_slot() {
        assert!(
            std::mem::size_of::<[[f32; 4]; 4]>() as u64 <= CASCADE_SLOT,
            "each cascade matrix has to fit in one uniform alignment slot"
        );
    }

    #[test]
    fn a_fresh_uniform_reports_no_cascades() {
        let uniform = ShadowUniform::new(&ShadowConfig::default());

        assert_eq!(
            uniform.cascade_count, 0,
            "the uniform must not advertise cascades before the first update"
        );
    }
}
