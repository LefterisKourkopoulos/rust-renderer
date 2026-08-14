//! What is being drawn: the loaded model, its instances, and the camera.

pub mod camera;
pub mod instance;
pub mod model;

use wgpu::util::DeviceExt;

use crate::assets;
use crate::config::SceneConfig;
use crate::gfx::{GpuContext, Texture};
use camera::{CameraMove, CameraState};
use instance::Instance;
use model::Model;

/// The scene owns its GPU buffers alongside the CPU-side data that produced
/// them, so the two can never drift apart.
pub struct Scene {
    pub obj_model: Model,
    pub instances: Vec<Instance>,
    pub instance_buffer: wgpu::Buffer,
    pub camera: CameraState,
    /// The layout shared by the model's per-material bind groups and the
    /// override bind groups below; they are interchangeable at draw time only
    /// because they were built from this one layout.
    texture_bind_group_layout: wgpu::BindGroupLayout,
    /// Extra diffuse textures the Space key can substitute for the model's own
    /// material. Add an entry here and the cycle picks it up.
    diffuse_overrides: Vec<DiffuseOverride>,
    /// Index into `diffuse_overrides`; `None` means "use each mesh's material".
    diffuse_override: Option<usize>,
}

/// One selectable override texture. The bind group is built up front so that
/// cycling is a plain index change with no GPU work in the input path; the
/// texture is kept because its view and sampler must outlive that bind group.
struct DiffuseOverride {
    #[allow(dead_code)]
    texture: Texture,
    bind_group: wgpu::BindGroup,
}

impl Scene {
    pub async fn new(ctx: &GpuContext, config: &SceneConfig) -> anyhow::Result<Self> {
        let instances = Instance::grid(&config.grid);
        let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
        let instance_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Instance Buffer"),
                contents: bytemuck::cast_slice(&instance_data),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let texture_bind_group_layout = Texture::bind_group_layout(
            &ctx.device,
            wgpu::TextureSampleType::Float { filterable: true },
            "texture_bind_group_layout",
        );

        let override_names = ["happy-tree.png", "centrica_logo.png"];

        let mut diffuse_overrides = Vec::with_capacity(override_names.len());
        for name in override_names {
            let texture = assets::load_texture(name, &ctx.device, &ctx.queue)?;
            let bind_group = texture.bind_group(
                &ctx.device,
                &texture_bind_group_layout,
                "diffuse_override_bind_group",
            );
            diffuse_overrides.push(DiffuseOverride {
                texture,
                bind_group,
            });
        }

        let camera = CameraState::new(&ctx.device, &ctx.config, &config.camera);

        let obj_model = assets::load_model(
            config.model_file,
            &ctx.device,
            &ctx.queue,
            &texture_bind_group_layout,
        )
        .await?;

        Ok(Self {
            obj_model,
            instances,
            instance_buffer,
            camera,
            texture_bind_group_layout,
            diffuse_overrides,
            diffuse_override: None,
        })
    }

    pub fn texture_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.texture_bind_group_layout
    }

    /// The override to draw with, or `None` to use each mesh's own material.
    pub fn diffuse_override(&self) -> Option<&wgpu::BindGroup> {
        self.diffuse_override
            .map(|index| &self.diffuse_overrides[index].bind_group)
    }

    /// Steps through the override textures and back to the material:
    /// material -> first override -> ... -> last override -> material.
    pub fn cycle_diffuse(&mut self) {
        self.diffuse_override = match self.diffuse_override {
            None if self.diffuse_overrides.is_empty() => None,
            None => Some(0),
            Some(index) if index + 1 < self.diffuse_overrides.len() => Some(index + 1),
            Some(_) => None,
        };
    }

    pub fn set_camera_move(&mut self, direction: CameraMove, is_pressed: bool) {
        self.camera.set_move(direction, is_pressed);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.camera.resize(width, height);
    }

    /// Advances the scene by `dt` seconds and uploads whatever changed.
    pub fn update(&mut self, queue: &wgpu::Queue, dt: f32) {
        self.camera.update(queue, dt);
    }
}
