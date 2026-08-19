use rust_renderer::Texture;
use rust_renderer::assets;

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
fn the_diorama_loads_with_geometry_materials_and_instances() {
    let Some((device, queue)) = device() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };
    let layout = Texture::material_bind_group_layout(&device, "material");

    let model = pollster::block_on(assets::load_model(
        "cube_diorama.glb",
        &device,
        &queue,
        &layout,
    ))
    .expect("the diorama loads");

    assert_eq!(model.meshes.len(), 93, "one mesh per glTF primitive");
    assert_eq!(model.materials.len(), 48);
    assert!(!model.instances.is_empty(), "node transforms place the meshes");

    for mesh in &model.meshes {
        assert!(
            mesh.num_elements > 0 && mesh.num_elements % 3 == 0,
            "{} has {} indices, which is not a whole number of triangles",
            mesh.name,
            mesh.num_elements
        );
        assert!(
            mesh.material < model.materials.len(),
            "{} points at material {}, past the end of {} materials",
            mesh.name,
            mesh.material,
            model.materials.len()
        );

        let range = mesh
            .instances
            .clone()
            .expect("a glTF mesh is placed by specific nodes");
        assert!(
            range.start < range.end,
            "{} has the empty instance range {range:?}",
            mesh.name
        );
        assert!(
            range.end as usize <= model.instances.len(),
            "{} draws instances {range:?}, past the end of {} instances",
            mesh.name,
            model.instances.len()
        );
    }

    wait(&device);
}

#[test]
fn base_colour_maps_decode_as_srgb_and_normal_maps_stay_linear() {
    let Some((device, queue)) = device() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };
    let layout = Texture::material_bind_group_layout(&device, "material");

    let model = pollster::block_on(assets::load_model(
        "cube_diorama.glb",
        &device,
        &queue,
        &layout,
    ))
    .expect("the diorama loads");

    let mut checked = 0;
    for material in &model.materials {
        assert_eq!(
            material.diffuse_texture.texture.format(),
            wgpu::TextureFormat::Rgba8UnormSrgb,
            "{}'s base colour map must take the hardware sRGB decode",
            material.name
        );
        assert_eq!(
            material.normal_texture.texture.format(),
            wgpu::TextureFormat::Rgba8Unorm,
            "{}'s normal map holds vectors, not colours, so it must stay linear",
            material.name
        );
        checked += 1;
    }

    assert_eq!(checked, 48, "every material should have been checked");
    wait(&device);
}

#[test]
fn tiling_textures_keep_the_repeat_wrapping_the_file_asks_for() {
    let bytes = assets::embedded("cube_diorama.glb").expect("the diorama is embedded");
    let (document, _, _) = gltf::import_slice(bytes).expect("the diorama parses");

    let repeating = document
        .textures()
        .filter(|texture| {
            texture.sampler().wrap_s() == gltf::texture::WrappingMode::Repeat
                && texture.sampler().wrap_t() == gltf::texture::WrappingMode::Repeat
        })
        .count();

    assert_eq!(
        repeating,
        document.textures().len(),
        "every diorama texture is expected to wrap, so the loader must not hardcode ClampToEdge"
    );
}

#[test]
fn the_diorama_geometry_is_finite_and_roughly_room_sized() {
    let bytes = assets::embedded("cube_diorama.glb").expect("the diorama is embedded");
    let (document, buffers, _) = gltf::import_slice(bytes).expect("the diorama parses");

    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];

    for mesh in document.meshes() {
        for primitive in mesh.primitives() {
            let bounds = primitive.bounding_box();
            for axis in 0..3 {
                min[axis] = min[axis].min(bounds.min[axis]);
                max[axis] = max[axis].max(bounds.max[axis]);
            }
        }
    }

    for axis in 0..3 {
        assert!(
            min[axis].is_finite() && max[axis].is_finite(),
            "axis {axis} bounds are not finite: {}..{}",
            min[axis],
            max[axis]
        );
        let extent = max[axis] - min[axis];
        assert!(
            extent > 0.1 && extent < 100.0,
            "axis {axis} spans {extent} units, which no camera preset would frame"
        );
    }

    assert!(!buffers.is_empty(), "a GLB carries its own binary chunk");
}

#[test]
fn the_obj_path_still_defers_its_instancing_to_the_scene() {
    let Some((device, queue)) = device() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };
    let layout = Texture::material_bind_group_layout(&device, "material");

    let model =
        pollster::block_on(assets::load_model("cube.obj", &device, &queue, &layout)).expect("cube.obj loads");

    assert!(
        model.instances.is_empty(),
        "OBJ carries no scene graph, so it must not claim instances"
    );
    for mesh in &model.meshes {
        assert!(
            mesh.instances.is_none(),
            "{} must defer its instance range to the scene",
            mesh.name
        );
    }

    wait(&device);
}

#[test]
fn an_unknown_extension_is_rejected_rather_than_guessed() {
    let Some((device, queue)) = device() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };
    let layout = Texture::material_bind_group_layout(&device, "material");

    let error = match pollster::block_on(assets::load_model("cube.fbx", &device, &queue, &layout)) {
        Ok(_) => panic!("fbx is not a supported format, so loading it must fail"),
        Err(error) => error,
    };

    assert!(
        error.to_string().contains("unsupported model format"),
        "unexpected error: {error}"
    );
}
