use rust_renderer::Vertex;
use rust_renderer::config::ShadowConfig;
use rust_renderer::scene::instance::InstanceRaw;
use rust_renderer::scene::model::ModelVertex;
use rust_renderer::shadow::ShadowPass;
use rust_renderer::{Texture, gfx};

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

fn wait(device: &wgpu::Device) {
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .expect("device poll");
}

#[test]
fn the_scene_pipeline_builds_against_the_real_shader() {
    let Some((device, _queue)) = device() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };

    let material_layout = Texture::material_bind_group_layout(&device, "material");
    let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("camera"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let light_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("light"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let shadow_layout = ShadowPass::bind_group_layout(&device);

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../src/shaders/shader.wgsl").into()),
    });

    let _pipeline = gfx::create_render_pipeline(
        &device,
        &gfx::pipeline::RenderPipelineConfig::new(
            "Render Pipeline",
            &[
                &material_layout,
                &camera_layout,
                &light_layout,
                &shadow_layout,
            ],
            &shader,
            wgpu::TextureFormat::Bgra8UnormSrgb,
        )
        .vertex_buffers(&[ModelVertex::desc(), InstanceRaw::desc()]),
    );

    wait(&device);
}

#[test]
fn the_shadow_pass_builds_against_the_real_shader() {
    let Some((device, _queue)) = device() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };

    let _shadow = ShadowPass::new(&device, ShadowConfig::default());

    wait(&device);
}

#[test]
fn a_material_bind_group_binds_two_textures_and_a_uniform() {
    let Some((device, queue)) = device() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };

    let layout = Texture::material_bind_group_layout(&device, "material");
    let diffuse = Texture::from_color(&device, &queue, [255, 255, 255, 255], false, "white");
    let normal = Texture::from_color(&device, &queue, [128, 128, 255, 255], true, "flat_normal");
    let uniform = rust_renderer::scene::model::MaterialUniform::default();
    let uniform_buffer = uniform.buffer(&device, "material");

    let _bind_group = Texture::material_bind_group(
        &device,
        &layout,
        &diffuse,
        &normal,
        &uniform_buffer,
        "material",
    );

    wait(&device);
}
