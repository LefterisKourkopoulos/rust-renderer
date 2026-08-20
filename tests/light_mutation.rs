use rust_renderer::Layouts;
use rust_renderer::scene::light::{Light, LightCollection, LightKind};

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

#[test]
fn set_directional_mutates_the_sun_and_leaves_other_lights_alone() {
    let Some((device, queue)) = device() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };
    let layouts = Layouts::new(&device);

    let point = Light::new([1.0, 2.0, 3.0], [1.0, 0.0, 0.0]).with_intensity(5.0);
    let sun = Light::directional([-0.4, -1.0, -0.3], [1.0, 0.98, 0.92], 1.5);
    let mut lights = LightCollection::new(&device, vec![point, sun], false, &layouts.light);

    lights.set_directional(&queue, [0.0, -1.0, 0.0], [0.2, 0.4, 1.0], 0.05);

    let updated = lights.directional().expect("a directional light still exists");
    assert_eq!(updated.direction, [0.0, -1.0, 0.0]);
    assert_eq!(updated.color, [0.2, 0.4, 1.0]);
    assert_eq!(updated.intensity, 0.05);
    assert_eq!(updated.kind, LightKind::Directional);
}

#[test]
fn set_directional_is_a_no_op_without_a_directional_light() {
    let Some((device, queue)) = device() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };
    let layouts = Layouts::new(&device);

    let point = Light::new([1.0, 2.0, 3.0], [1.0, 0.0, 0.0]);
    let mut lights = LightCollection::new(&device, vec![point], false, &layouts.light);

    lights.set_directional(&queue, [0.0, -1.0, 0.0], [1.0, 1.0, 1.0], 1.0);

    assert!(lights.directional().is_none());
    assert_eq!(lights.count(), 1, "the point light must still be the only light");
}
