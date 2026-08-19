use rust_renderer::config::{CameraConfig, ShadowConfig};
use rust_renderer::scene::camera::CameraState;
use rust_renderer::scene::instance::Instance;
use rust_renderer::scene::model::{Mesh, Model, ModelVertex};
use rust_renderer::shadow::ShadowPass;
use wgpu::util::DeviceExt;

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        flags: Default::default(),
        memory_budget_thresholds: Default::default(),
        backend_options: Default::default(),
        display: None,
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        compatible_surface: None,
        force_fallback_adapter: false,
        apply_limit_buckets: true,
    }))
    .ok()?;

    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("test device"),
        required_features: wgpu::Features::empty(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        required_limits: wgpu::Limits::default(),
        memory_hints: Default::default(),
        trace: wgpu::Trace::Off,
    }))
    .ok()?;

    device.on_uncaptured_error(std::sync::Arc::new(|error| {
        panic!("wgpu validation error: {error}");
    }));

    Some((device, queue))
}

fn surface_config() -> wgpu::SurfaceConfiguration {
    wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        width: 1280,
        height: 720,
        present_mode: wgpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        color_space: Default::default(),
        view_formats: vec![],
    }
}

fn ground_quad(device: &wgpu::Device) -> Model {
    quad_at(device, 0.0)
}

fn quad_at(device: &wgpu::Device, height: f32) -> Model {
    let vertex = |position: [f32; 3]| ModelVertex {
        position,
        tex_coords: [0.0, 0.0],
        normal: [0.0, 1.0, 0.0],
        tangent: [1.0, 0.0, 0.0],
        bitangent: [0.0, 0.0, 1.0],
    };

    let vertices = [
        vertex([-2.0, height, -2.0]),
        vertex([2.0, height, -2.0]),
        vertex([2.0, height, 2.0]),
        vertex([-2.0, height, 2.0]),
    ];
    let indices: [u32; 12] = [0, 1, 2, 2, 3, 0, 2, 1, 0, 0, 3, 2];

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("test vertices"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("test indices"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    Model {
        meshes: vec![Mesh {
            name: String::from("ground"),
            vertex_buffer,
            index_buffer,
            num_elements: indices.len() as u32,
            material: 0,
            instances: None,
        }],
        materials: Vec::new(),
        instances: Vec::new(),
        lights: Vec::new(),
    }
}

fn read_layer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    shadow: &ShadowPass,
    resolution: u32,
    layer: u32,
) -> Vec<f32> {
    let bytes_per_row = resolution * 4;
    assert_eq!(bytes_per_row % 256, 0, "test resolution must align");

    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (bytes_per_row * resolution) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("readback"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: shadow.depth_texture(),
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: 0,
                y: 0,
                z: layer,
            },
            aspect: wgpu::TextureAspect::DepthOnly,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(resolution),
            },
        },
        wgpu::Extent3d {
            width: resolution,
            height: resolution,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(std::iter::once(encoder.finish()));

    buffer.slice(..).map_async(wgpu::MapMode::Read, |result| {
        result.expect("map readback buffer");
    });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .expect("device poll");

    let view = buffer
        .slice(..)
        .get_mapped_range()
        .expect("map the readback range");
    let depths = bytemuck::cast_slice::<u8, f32>(&view).to_vec();
    drop(view);
    buffer.unmap();
    depths
}

#[test]
fn the_cascade_pass_writes_caster_depth_into_its_layer() {
    let Some((device, queue)) = device() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };

    let resolution = 256;
    let mut shadow = ShadowPass::new(
        &device,
        ShadowConfig {
            resolution,
            ..ShadowConfig::default()
        },
    );

    let camera = CameraState::new(&device, &surface_config(), &CameraConfig::default());
    shadow.update(&queue, &camera, [-0.4, -1.0, -0.3]);

    let model = ground_quad(&device);
    let instance_data = [Instance::from_matrix(cgmath::Matrix4::from_scale(1.0)).to_raw()];
    let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("test instances"),
        contents: bytemuck::cast_slice(&instance_data),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("shadow"),
    });
    shadow.render(&mut encoder, &model, &instance_buffer, 1);
    queue.submit(std::iter::once(encoder.finish()));

    let mut total_written = 0;
    for layer in 0..shadow.cascade_count() as u32 {
        let depths = read_layer(&device, &queue, &shadow, resolution, layer);

        total_written += depths.iter().filter(|depth| **depth < 1.0).count();

        for depth in &depths {
            assert!(
                (0.0..=1.0).contains(depth),
                "depth outside [0, 1] in layer {layer} means the ortho near/far mapping \
                 is wrong, got {depth}"
            );
        }
    }

    assert!(
        total_written > 0,
        "every cascade cleared to 1.0 and rasterized nothing - the caster never landed \
         inside any cascade's clip volume"
    );
}

#[test]
fn the_nearest_cascade_covers_a_caster_right_in_front_of_the_camera() {
    let Some((device, queue)) = device() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };

    let resolution = 256;
    let mut shadow = ShadowPass::new(
        &device,
        ShadowConfig {
            resolution,
            ..ShadowConfig::default()
        },
    );

    let camera = CameraState::new(
        &device,
        &surface_config(),
        &CameraConfig {
            position: [0.0, 1.5, 0.0],
            pitch: -89.0,
            yaw: 0.0,
            ..CameraConfig::default()
        },
    );
    shadow.update(&queue, &camera, [-0.4, -1.0, -0.3]);

    let model = ground_quad(&device);
    let instance_data = [Instance::from_matrix(cgmath::Matrix4::from_scale(1.0)).to_raw()];
    let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("test instances"),
        contents: bytemuck::cast_slice(&instance_data),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("shadow"),
    });
    shadow.render(&mut encoder, &model, &instance_buffer, 1);
    queue.submit(std::iter::once(encoder.finish()));

    let depths = read_layer(&device, &queue, &shadow, resolution, 0);
    let written = depths.iter().filter(|depth| **depth < 1.0).count();

    assert!(
        written > 0,
        "cascade 0 rasterized nothing even though the caster fills the near frustum"
    );
}

fn shade_one_pixel(device: &wgpu::Device, queue: &wgpu::Queue, shadow: &ShadowPass) -> [f32; 4] {
    use rust_renderer::Texture;
    use rust_renderer::gfx::{self, Vertex};
    use rust_renderer::scene::instance::InstanceRaw;

    let format = wgpu::TextureFormat::Rgba8Unorm;

    let material_layout = Texture::material_bind_group_layout(device, "material");
    let white = Texture::from_color(device, queue, [255, 255, 255, 255], false, "white");
    let flat_normal = Texture::from_color(device, queue, [128, 128, 255, 255], true, "flat_normal");
    let material_uniform = rust_renderer::scene::model::MaterialUniform::default();
    let material_buffer = material_uniform.buffer(device, "material");
    let material_bind_group = Texture::material_bind_group(
        device,
        &material_layout,
        &white,
        &flat_normal,
        &material_buffer,
        "material",
    );

    let camera = CameraState::new(
        device,
        &surface_config(),
        &CameraConfig {
            position: [0.0, 1.5, 0.0],
            pitch: -89.0,
            yaw: 0.0,
            ..CameraConfig::default()
        },
    );

    let lights = rust_renderer::scene::light::LightCollection::new(
        device,
        vec![rust_renderer::scene::light::Light::directional(
            [-0.4, -1.0, -0.3],
            [1.0, 1.0, 1.0],
            1.0,
        )],
        false,
    );

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../src/shaders/shader.wgsl").into()),
    });

    let pipeline = gfx::create_render_pipeline(
        device,
        &gfx::pipeline::RenderPipelineConfig::new(
            "Render Pipeline",
            &[
                &material_layout,
                camera.bind_group_layout(),
                &lights.bind_group_layout,
                shadow.layout(),
            ],
            &shader,
            format,
        )
        .vertex_buffers(&[ModelVertex::desc(), InstanceRaw::desc()]),
    );

    let color = Texture::create_2d_texture(
        device,
        1,
        1,
        format,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        wgpu::FilterMode::Nearest,
        Some("shade target"),
    );
    let depth = Texture::create_depth_texture(
        device,
        &wgpu::SurfaceConfiguration {
            width: 1,
            height: 1,
            ..surface_config()
        },
        "shade depth",
    );

    let model = ground_quad(device);
    let instance_data = [Instance::from_matrix(cgmath::Matrix4::from_scale(1.0)).to_raw()];
    let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("shade instances"),
        contents: bytemuck::cast_slice(&instance_data),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("shade"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("shade pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color.view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth.view,
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

        let mesh = &model.meshes[0];
        pass.set_pipeline(&pipeline);
        pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.slice(..));
        pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.set_bind_group(0, &material_bind_group, &[]);
        pass.set_bind_group(1, camera.bind_group(), &[]);
        pass.set_bind_group(2, &lights.bind_group, &[]);
        pass.set_bind_group(3, shadow.bind_group(), &[]);
        pass.draw_indexed(0..mesh.num_elements, 0, 0..1);
    }

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("shade readback"),
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &color.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(1),
            },
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(std::iter::once(encoder.finish()));

    readback.slice(..).map_async(wgpu::MapMode::Read, |result| {
        result.expect("map shade readback");
    });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .expect("device poll");

    let view = readback
        .slice(..)
        .get_mapped_range()
        .expect("map the shade range");
    let pixel = [
        view[0] as f32 / 255.0,
        view[1] as f32 / 255.0,
        view[2] as f32 / 255.0,
        view[3] as f32 / 255.0,
    ];
    drop(view);
    readback.unmap();
    pixel
}

#[test]
fn a_shadow_pass_that_never_updated_leaves_the_scene_lit() {
    let Some((device, queue)) = device() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };

    let shadow = ShadowPass::new(
        &device,
        ShadowConfig {
            resolution: 256,
            enabled: false,
            ..ShadowConfig::default()
        },
    );

    let unshadowed = shade_one_pixel(&device, &queue, &shadow);

    assert!(
        unshadowed[0] > 0.2,
        "a shadow pass with no rendered depth must not occlude anything, got {unshadowed:?}"
    );
}

#[test]
fn an_updated_shadow_pass_occludes_ground_under_a_caster() {
    let Some((device, queue)) = device() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };

    let mut shadow = ShadowPass::new(
        &device,
        ShadowConfig {
            resolution: 256,
            ..ShadowConfig::default()
        },
    );

    let camera = CameraState::new(
        &device,
        &surface_config(),
        &CameraConfig {
            position: [0.0, 1.5, 0.0],
            pitch: -89.0,
            yaw: 0.0,
            ..CameraConfig::default()
        },
    );
    shadow.update(&queue, &camera, [-0.4, -1.0, -0.3]);

    let occluder = quad_at(&device, 0.75);
    let instance_data = [Instance::from_matrix(cgmath::Matrix4::from_scale(1.0)).to_raw()];
    let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("test instances"),
        contents: bytemuck::cast_slice(&instance_data),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("shadow"),
    });
    shadow.render(&mut encoder, &occluder, &instance_buffer, 1);
    queue.submit(std::iter::once(encoder.finish()));

    let shadowed = shade_one_pixel(&device, &queue, &shadow);

    assert!(
        shadowed[0] < 0.2,
        "the ground under the caster must be occluded, which is what proves the sampling \
         path actually reads the map; got {shadowed:?}"
    );
}
