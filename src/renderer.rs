use crate::config::RendererConfig;
use crate::debug::DepthDebug;
use crate::gfx::{GpuContext, Texture, Vertex, create_render_pipeline, pipeline};
use crate::scene::Scene;
use crate::scene::instance::InstanceRaw;
use crate::scene::model::{DrawModel, ModelVertex};

pub struct Renderer {
    render_pipeline: wgpu::RenderPipeline,
    depth_texture: Texture,
    depth_debug: DepthDebug,
    config: RendererConfig,
}

impl Renderer {
    pub fn new(ctx: &GpuContext, scene: &Scene, config: RendererConfig) -> Self {
        let depth_texture =
            Texture::create_depth_texture(&ctx.device, &ctx.config, "depth_texture");
        let depth_debug = DepthDebug::new(&ctx.device, &depth_texture, ctx.config.format);

        let shader = ctx
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
            });

        let render_pipeline = create_render_pipeline(
            &ctx.device,
            &pipeline::RenderPipelineConfig {
                label: "Render Pipeline",
                bind_group_layouts: &[
                    scene.texture_bind_group_layout(),
                    scene.camera.bind_group_layout(),
                ],
                shader: &shader,
                vertex_buffers: &[ModelVertex::desc(), InstanceRaw::desc()],
                color_format: ctx.config.format,
                depth_write: true,
            },
        );

        Self {
            render_pipeline,
            depth_texture,
            depth_debug,
            config,
        }
    }

    pub fn resize(&mut self, ctx: &GpuContext) {
        self.depth_texture =
            Texture::create_depth_texture(&ctx.device, &ctx.config, "depth_texture");
        self.depth_debug.resize(&ctx.device, &self.depth_texture);
    }

    pub fn toggle_depth_debug(&mut self) {
        self.depth_debug.toggle();
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

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
            );
        }

        self.depth_debug.draw(&mut encoder, &view);

        ctx.queue.submit(std::iter::once(encoder.finish()));
        ctx.queue.present(output);

        Ok(())
    }
}
