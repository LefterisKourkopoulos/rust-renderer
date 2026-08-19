use crate::config::{PipelineMode, RendererConfig};
use crate::debug::{CascadeDebug, DepthDebug};
use crate::gfx::{
    GpuContext, HdrPipeline, Layouts, Texture, Vertex, create_render_pipeline, pipeline,
};
use crate::scene::Scene;
use crate::scene::instance::InstanceRaw;
use crate::scene::model::{DrawModel, ModelVertex};
use crate::shadow::ShadowPass;

pub struct Renderer {
    render_pipeline: wgpu::RenderPipeline,
    light_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    depth_texture: Texture,
    depth_debug: DepthDebug,
    cascade_debug: CascadeDebug,
    shadow: ShadowPass,
    cascade_tint: bool,
    hdr: Option<HdrPipeline>,
    config: RendererConfig,
}

impl Renderer {
    /// Builds every pipeline from `layouts` alone, so the renderer is independent of any
    /// particular scene and a scene swapped in later stays compatible.
    pub fn new(ctx: &GpuContext, layouts: &Layouts, config: RendererConfig) -> Self {
        let depth_texture =
            Texture::create_depth_texture(&ctx.device, &ctx.config, "depth_texture");
        let depth_debug = DepthDebug::new(&ctx.device, &depth_texture, ctx.config.format);

        let shadow = ShadowPass::new(&ctx.device, config.shadows.clone(), &layouts.shadow);
        let cascade_debug = CascadeDebug::new(
            &ctx.device,
            shadow.depth_view(),
            shadow.cascade_count() as u32,
            ctx.config.format,
        );

        let hdr = match config.pipeline_mode {
            PipelineMode::Hdr => Some(HdrPipeline::new(&ctx.device, &ctx.config)),
            PipelineMode::Normal => None,
        };
        let color_format = hdr
            .as_ref()
            .map(HdrPipeline::format)
            .unwrap_or(ctx.config.format);

        // Instance Rendering Pipeline
        let instance_shader = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
            });

        let render_pipeline = create_render_pipeline(
            &ctx.device,
            &pipeline::RenderPipelineConfig::new(
                "Render Pipeline",
                &[
                    &layouts.material,
                    &layouts.camera,
                    &layouts.light,
                    &layouts.shadow,
                ],
                &instance_shader,
                color_format,
            )
            .vertex_buffers(&[ModelVertex::desc(), InstanceRaw::desc()]),
        );

        // Light Rendering Pipeline
        let light_shader = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Light"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/light.wgsl").into()),
            });

        let light_pipeline = create_render_pipeline(
            &ctx.device,
            &pipeline::RenderPipelineConfig::new(
                "Light Pipeline",
                &[&layouts.camera, &layouts.light],
                &light_shader,
                color_format,
            )
            .vertex_buffers(&[ModelVertex::desc()]),
        );

        // Sky Rendering Pipeline
        let sky_shader = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Sky"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sky.wgsl").into()),
            });

        let sky_pipeline = create_render_pipeline(
            &ctx.device,
            &pipeline::RenderPipelineConfig::new(
                "Sky Pipeline",
                &[&layouts.camera, &layouts.environment],
                &sky_shader,
                color_format,
            ),
        );

        Self {
            render_pipeline,
            light_pipeline,
            sky_pipeline,
            depth_texture,
            depth_debug,
            cascade_debug,
            shadow,
            cascade_tint: false,
            hdr,
            config,
        }
    }

    pub fn resize(&mut self, ctx: &GpuContext) {
        self.depth_texture =
            Texture::create_depth_texture(&ctx.device, &ctx.config, "depth_texture");
        self.depth_debug.resize(&ctx.device, &self.depth_texture);
        if let Some(hdr) = &mut self.hdr {
            hdr.resize(&ctx.device, ctx.config.width, ctx.config.height);
        }
    }

    pub fn toggle_depth_debug(&mut self) {
        self.depth_debug.toggle();
    }

    pub fn toggle_cascade_debug(&mut self) {
        self.cascade_tint = !self.cascade_tint;
        self.shadow.set_debug_mode(self.cascade_tint);
    }

    pub fn cycle_shadow_layer(&mut self, ctx: &GpuContext) {
        self.cascade_debug.cycle(&ctx.queue);
    }

    pub fn update(&mut self, ctx: &GpuContext, scene: &Scene) {
        if !self.config.shadows.enabled {
            return;
        }

        if let Some(sun) = scene.lights.directional() {
            self.shadow.update(&ctx.queue, &scene.camera, sun.direction);
        }
    }

    pub fn render(&mut self, ctx: &mut GpuContext, scene: &Scene) -> anyhow::Result<()> {
        let Some(output) = ctx.acquire_frame()? else {
            return Ok(());
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        let color_view = self.hdr.as_ref().map(HdrPipeline::view).unwrap_or(&view);

        if self.config.shadows.enabled {
            self.shadow.render(
                &mut encoder,
                &scene.obj_model,
                &scene.instance_buffer,
                scene.instances.len() as u32,
            );
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.config.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
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

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_vertex_buffer(1, scene.instance_buffer.slice(..));
            render_pass.draw_model_instanced(
                &scene.obj_model,
                0..scene.instances.len() as u32,
                scene.camera.bind_group(),
                scene.diffuse_override(),
                &scene.lights.bind_group,
                self.shadow.bind_group(),
            );

            render_pass.set_pipeline(&self.light_pipeline);
            render_pass.set_vertex_buffer(0, scene.lights.vertex_buffer.slice(..));
            render_pass.set_index_buffer(scene.lights.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.set_bind_group(0, scene.camera.bind_group(), &[]);
            render_pass.set_bind_group(1, &scene.lights.bind_group, &[]);
            render_pass.draw_indexed(0..scene.lights.num_indices, 0, 0..scene.lights.count());

            render_pass.set_pipeline(&self.sky_pipeline);
            render_pass.set_bind_group(0, scene.camera.bind_group(), &[]);
            render_pass.set_bind_group(1, scene.environment_bind_group(), &[]);
            render_pass.draw(0..3, 0..1);
        }

        self.depth_debug.draw(&mut encoder, &view);
        self.cascade_debug.draw(&mut encoder, &view);

        if let Some(hdr) = &self.hdr {
            hdr.process(&mut encoder, &view);
        }

        ctx.queue.submit(std::iter::once(encoder.finish()));
        ctx.queue.present(output);

        Ok(())
    }
}
