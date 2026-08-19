pub mod camera;
pub mod instance;
pub mod model;
pub mod light;

use wgpu::util::DeviceExt;

use crate::assets;
use crate::config::SceneConfig;
use crate::gfx::{CubeTexture, GpuHandle, HdrLoader, Layouts, Texture};
use light::{Light, LightCollection};
use camera::{CameraMove, CameraState};
use instance::Instance;
use model::Model;

pub struct Scene {
    pub obj_model: Model,
    pub instances: Vec<Instance>,
    pub instance_buffer: wgpu::Buffer,
    pub camera: CameraState,
    pub lights: LightCollection,
    diffuse_overrides: Vec<DiffuseOverride>,
    diffuse_override: Option<usize>,
    #[allow(dead_code)]
    sky_texture: CubeTexture,
    environment_bind_group: wgpu::BindGroup,
}

struct DiffuseOverride {
    #[allow(dead_code)]
    texture: Texture,
    #[allow(dead_code)]
    normal_texture: Texture,
    #[allow(dead_code)]
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl Scene {
    /// Builds a scene from `config`.
    ///
    /// Takes a [`GpuHandle`] rather than the full [`GpuContext`](crate::gfx::GpuContext) so it can
    /// run on a background thread while the previous scene keeps rendering; the surface stays on
    /// the main thread with the window.
    pub async fn new(
        ctx: &GpuHandle,
        config: &SceneConfig,
        layouts: &Layouts,
    ) -> anyhow::Result<Self> {
        let override_names = ["happy-tree.png", "centrica_logo.png"];

        let mut diffuse_overrides = Vec::with_capacity(override_names.len());
        for name in override_names {
            let texture = assets::load_texture(name, &ctx.device, &ctx.queue)?;
            let normal_texture = Texture::from_color(
                &ctx.device,
                &ctx.queue,
                [128, 128, 255, 255],
                true,
                "diffuse_override_default_normal",
            );
            let uniform_buffer =
                model::MaterialUniform::default().buffer(&ctx.device, "diffuse_override");
            let bind_group = Texture::material_bind_group(
                &ctx.device,
                &layouts.material,
                &texture,
                &normal_texture,
                &uniform_buffer,
                "diffuse_override_bind_group",
            );
            diffuse_overrides.push(DiffuseOverride {
                texture,
                normal_texture,
                uniform_buffer,
                bind_group,
            });
        }

        // Camera
        let camera = CameraState::new(&ctx.device, &ctx.config, &config.camera, &layouts.camera);

        // Model
        let obj_model = assets::load_model_from(
            config.base_dir.as_deref(),
            &config.model_file,
            &ctx.device,
            &ctx.queue,
            &layouts.material,
        )
        .await?;

        let instances = if obj_model.instances.is_empty() {
            Instance::grid(&config.grid)
        } else {
            obj_model.instances.clone()
        };
        let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
        let instance_buffer = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Instance Buffer"),
                contents: bytemuck::cast_slice(&instance_data),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let (mut lights, animate) = if obj_model.lights.is_empty() {
            let fallback = vec![
                // Intensity 20 compensates for the inverse square falloff the shader now applies:
                // at the grid's 3-4 unit light distance that costs roughly a factor of 12, so this
                // keeps the demo grid about as bright as it was before attenuation existed.
                //Light::new([2.0, 2.0, 2.0], [1.0, 0.0, 0.0]).with_intensity(20.0),
                //Light::new([-2.0, 2.0, 2.0], [0.0, 1.0, 0.0]).with_intensity(20.0),
                //Light::new([2.0, 2.0, -2.0], [0.0, 0.0, 1.0]).with_intensity(20.0),
                //Light::new([-2.0, 2.0, -2.0], [1.0, 1.0, 1.0]).with_intensity(20.0),
            ];
            (fallback, true)
        } else {
            let scaled = obj_model
                .lights
                .iter()
                .map(|light| Light {
                    intensity: light.intensity * config.light_intensity_scale,
                    ..*light
                })
                .collect();
            (scaled, false)
        };

        lights.insert(
            0,
            Light::directional(
                config.sun.direction,
                config.sun.color,
                config.sun.intensity,
            ),
        );

        let lights = LightCollection::new(&ctx.device, lights, animate, &layouts.light);

        // Skybox
        let hdr_loader = HdrLoader::new(&ctx.device);
        let sky_bytes = assets::embedded("pure-sky-hdri.jpg")?;
        let sky_texture = hdr_loader.from_equirect_bytes(
            &ctx.device,
            &ctx.queue,
            sky_bytes,
            1080,
            Some("Sky Texture"),
        )?;

        let environment_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("environment_bind_group"),
            layout: &layouts.environment,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(sky_texture.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sky_texture.sampler()),
                },
            ],
        });

        Ok(Self {
            obj_model,
            instances,
            instance_buffer,
            camera,
            lights,
            diffuse_overrides,
            diffuse_override: None,
            sky_texture,
            environment_bind_group,
        })
    }

    pub fn environment_bind_group(&self) -> &wgpu::BindGroup {
        &self.environment_bind_group
    }

    pub fn diffuse_override(&self) -> Option<&wgpu::BindGroup> {
        self.diffuse_override
            .map(|index| &self.diffuse_overrides[index].bind_group)
    }

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

    pub fn set_camera_look(&mut self, dx: f64, dy: f64) {
        self.camera.set_look(dx, dy);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.camera.resize(width, height);
    }

    pub fn update(&mut self, queue: &wgpu::Queue, dt: f32) {
        self.camera.update(queue, dt);
        self.lights.update(queue, dt);
    }
}
