use rust_renderer::gfx::{GpuHandle, HdrLoader};
use rust_renderer::{Layouts, Scene, SceneConfig};

fn handle() -> Option<GpuHandle> {
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
        label: Some("scene mutation test device"),
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

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        width: 1280,
        height: 720,
        present_mode: wgpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        color_space: Default::default(),
        view_formats: vec![],
    };

    Some(GpuHandle { device, queue, config })
}

#[test]
fn set_model_swaps_the_model_but_keeps_the_sun_and_camera() {
    let Some(ctx) = handle() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };
    let layouts = Layouts::new(&ctx.device);
    let config = SceneConfig::default();

    let mut scene = pollster::block_on(Scene::new(&ctx, &config, &layouts)).expect("the default scene loads");
    let sun_before = scene.lights.directional().expect("the default scene has a sun");

    let bytes = rust_renderer::assets::embedded("cube_diorama.glb").expect("the diorama is embedded");
    scene
        .set_model(&ctx, &layouts, bytes, "cube_diorama.glb")
        .expect("the diorama loads as a model swap");

    assert!(!scene.obj_model.meshes.is_empty(), "the new model must have drawable geometry");
    assert!(!scene.instances.is_empty(), "the new model's instances must be built");

    let sun_after = scene.lights.directional().expect("the sun must survive a model swap");
    assert_eq!(
        sun_before.direction, sun_after.direction,
        "swapping the model must not touch the independently controlled sun"
    );
    assert_eq!(sun_before.intensity, sun_after.intensity);
}

#[test]
fn set_model_rejects_bytes_that_are_not_a_valid_glb() {
    let Some(ctx) = handle() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };
    let layouts = Layouts::new(&ctx.device);
    let config = SceneConfig::default();

    let mut scene = pollster::block_on(Scene::new(&ctx, &config, &layouts)).expect("the default scene loads");
    let mesh_count_before = scene.obj_model.meshes.len();

    let error = scene
        .set_model(&ctx, &layouts, b"not a glb at all", "upload.glb")
        .expect_err("garbage bytes must not silently become a model");
    assert!(error.to_string().contains("upload.glb"));

    assert_eq!(
        scene.obj_model.meshes.len(),
        mesh_count_before,
        "a failed upload must leave the current model in place"
    );
}

#[test]
fn set_skybox_replaces_the_environment_without_touching_the_model_or_lights() {
    let Some(ctx) = handle() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };
    let layouts = Layouts::new(&ctx.device);
    let config = SceneConfig::default();

    let mut scene = pollster::block_on(Scene::new(&ctx, &config, &layouts)).expect("the default scene loads");
    let mesh_count_before = scene.obj_model.meshes.len();
    let sun_before = scene.lights.directional().expect("the default scene has a sun");

    let hdr_loader = HdrLoader::new(&ctx.device);
    let sky_bytes = rust_renderer::assets::embedded("pure-sky-hdri.jpg").expect("the hdri is embedded");
    scene
        .set_skybox(&ctx, &hdr_loader, &layouts, sky_bytes, Some("test sky"))
        .expect("the equirect image loads as a new skybox");

    assert_eq!(scene.obj_model.meshes.len(), mesh_count_before, "the model must be untouched");
    let sun_after = scene.lights.directional().expect("the sun must survive a skybox swap");
    assert_eq!(sun_before.direction, sun_after.direction);
}

#[test]
fn set_time_of_day_moves_the_sun() {
    let Some(ctx) = handle() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };
    let layouts = Layouts::new(&ctx.device);
    let config = SceneConfig::default();

    let mut scene = pollster::block_on(Scene::new(&ctx, &config, &layouts)).expect("the default scene loads");

    scene.set_time_of_day(&ctx.queue, 0.0);
    let midnight = scene.lights.directional().expect("the sun exists");

    scene.set_time_of_day(&ctx.queue, 12.0);
    let noon = scene.lights.directional().expect("the sun exists");

    assert_ne!(midnight.direction, noon.direction);
    assert!(noon.intensity > midnight.intensity, "noon must be brighter than midnight");
}
