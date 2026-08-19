//! End-to-end hot reload: a scene file on disk, a real GPU device, a real watcher.
//!
//! The unit tests cover parsing and event classification in isolation. What they cannot show is
//! that saving a file actually produces a new scene built against the layouts the pipelines use,
//! which is the whole feature.

#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rust_renderer::Layouts;
use rust_renderer::gfx::GpuHandle;
use rust_renderer::watch::{Loaded, SceneLoader, SceneWatcher};

/// How long a poll loop waits before giving up. Generous: it covers spawning a thread, decoding a
/// `.glb` and running the cubemap pass on whatever GPU is available.
const TIMEOUT: Duration = Duration::from_secs(60);

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
        label: Some("hot reload test device"),
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

    Some(GpuHandle {
        device,
        queue,
        config,
    })
}

/// A scratch directory of its own per test, so parallel tests do not see each other's saves.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rust-renderer-hot-reload-{name}"));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("create the scratch directory");
    dir
}

fn write(path: &Path, contents: &str) {
    std::fs::write(path, contents).expect("write the scene file");
}

/// Polls the loader until it produces something, or the timeout expires.
fn wait_for_load(loader: &mut SceneLoader) -> Loaded {
    let deadline = Instant::now() + TIMEOUT;

    while Instant::now() < deadline {
        if let Some(loaded) = loader.poll() {
            return loaded;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    panic!("the loader produced nothing within {TIMEOUT:?}");
}

#[test]
fn a_scene_file_loads_into_a_usable_scene_on_a_worker_thread() {
    let Some(ctx) = handle() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };
    let layouts = Layouts::new(&ctx.device);

    let dir = scratch("loads");
    let path = dir.join("scene.toml");
    write(&path, "model = \"cube_diorama.glb\"\n");

    let mut loader = SceneLoader::new(ctx, path.clone());
    assert!(!loader.is_loading(), "a fresh loader is idle");

    loader.request(&layouts);
    assert!(loader.is_loading(), "the request should be in flight");

    match wait_for_load(&mut loader) {
        Loaded::Scene(scene) => {
            assert!(
                !scene.obj_model.meshes.is_empty(),
                "the loaded scene must have drawable geometry"
            );
            assert!(
                !scene.instances.is_empty(),
                "an empty instance buffer would draw nothing"
            );
        }
        Loaded::Failed(e) => panic!("the scene should have loaded: {e:#}"),
    }

    assert!(
        !loader.is_loading(),
        "the loader is idle again once collected"
    );
}

#[test]
fn a_reload_replaces_the_scene_with_the_saved_contents() {
    let Some(ctx) = handle() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };
    let layouts = Layouts::new(&ctx.device);

    let dir = scratch("replaces");
    let path = dir.join("scene.toml");
    write(&path, "[grid]\ninstances_per_row = 2\n");

    let mut loader = SceneLoader::new(ctx, path.clone());
    loader.request(&layouts);
    let first = match wait_for_load(&mut loader) {
        Loaded::Scene(scene) => scene,
        Loaded::Failed(e) => panic!("the first load should succeed: {e:#}"),
    };

    // The diorama places its own meshes, so the grid is what a scene without placements would use.
    // Change the sun instead, which always takes effect.
    write(&path, "[sun]\nintensity = 9.0\n");
    loader.request(&layouts);
    let second = match wait_for_load(&mut loader) {
        Loaded::Scene(scene) => scene,
        Loaded::Failed(e) => panic!("the second load should succeed: {e:#}"),
    };

    let before = first
        .lights
        .directional()
        .expect("the sun is always present")
        .intensity;
    let after = second
        .lights
        .directional()
        .expect("the sun is always present")
        .intensity;

    assert_ne!(
        before, after,
        "the reloaded scene must reflect the saved file, not the previous one"
    );
    assert_eq!(
        after, 9.0,
        "the sun intensity should come from the new file"
    );
}

#[test]
fn a_broken_scene_file_fails_the_load_instead_of_producing_a_scene() {
    let Some(ctx) = handle() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };
    let layouts = Layouts::new(&ctx.device);

    let dir = scratch("broken");
    let path = dir.join("scene.toml");
    write(&path, "model = \"cube_diorama.glb\"\nthis is not toml\n");

    let mut loader = SceneLoader::new(ctx, path);
    loader.request(&layouts);

    match wait_for_load(&mut loader) {
        Loaded::Failed(_) => {}
        Loaded::Scene(_) => panic!(
            "a syntactically broken file must fail the load so the caller keeps the old scene"
        ),
    }
}

#[test]
fn a_scene_naming_a_missing_model_fails_the_load() {
    let Some(ctx) = handle() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };
    let layouts = Layouts::new(&ctx.device);

    let dir = scratch("missing-model");
    let path = dir.join("scene.toml");
    write(&path, "model = \"no-such-model.glb\"\n");

    let mut loader = SceneLoader::new(ctx, path);
    loader.request(&layouts);

    match wait_for_load(&mut loader) {
        Loaded::Failed(e) => {
            let message = format!("{e:#}");
            assert!(
                message.contains("no-such-model.glb"),
                "the error should name the model it could not find, got: {message}"
            );
        }
        Loaded::Scene(_) => panic!("a model that exists nowhere cannot produce a scene"),
    }
}

#[test]
fn a_model_beside_the_scene_file_is_preferred_over_the_embedded_one() {
    let Some(ctx) = handle() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };
    let layouts = Layouts::new(&ctx.device);

    let dir = scratch("disk-model");
    let path = dir.join("scene.toml");
    write(&path, "model = \"cube_diorama.glb\"\n");

    // Same name as the embedded asset, but not a valid glTF. If the loader reads it, the load
    // fails; if it silently used the embedded copy instead, it would succeed and the disk-first
    // rule would be untested.
    std::fs::write(dir.join("cube_diorama.glb"), b"not a glb at all")
        .expect("write the stand-in model");

    let mut loader = SceneLoader::new(ctx, path);
    loader.request(&layouts);

    match wait_for_load(&mut loader) {
        Loaded::Failed(_) => {}
        Loaded::Scene(_) => panic!(
            "the model next to the scene file must win over the embedded asset of the same name"
        ),
    }
}

#[test]
fn a_burst_of_requests_while_loading_collapses_into_one_follow_up() {
    let Some(ctx) = handle() else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };
    let layouts = Layouts::new(&ctx.device);

    let dir = scratch("burst");
    let path = dir.join("scene.toml");
    write(&path, "model = \"cube_diorama.glb\"\n");

    let mut loader = SceneLoader::new(ctx, path);
    loader.request(&layouts);
    for _ in 0..5 {
        loader.request(&layouts);
    }

    // The in-flight load, then exactly one follow-up for the whole burst.
    for round in 1..=2 {
        match wait_for_load(&mut loader) {
            Loaded::Scene(_) => {}
            Loaded::Failed(e) => panic!("load {round} should succeed: {e:#}"),
        }
    }

    assert!(
        !loader.is_loading(),
        "five requests during one load must queue a single reload, not five"
    );
    assert!(
        loader.poll().is_none(),
        "nothing further should be pending once the follow-up completed"
    );
}

#[test]
fn saving_the_scene_file_is_noticed_by_the_watcher() {
    let dir = scratch("watch-save");
    let path = dir.join("scene.toml");
    write(&path, "model = \"cube_diorama.glb\"\n");

    let mut watcher = SceneWatcher::new(&path).expect("the directory exists");

    // Give notify a moment to register before the write that should be seen.
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        !watcher.poll(),
        "an untouched file must not report a change"
    );

    write(&path, "[sun]\nintensity = 4.0\n");

    let deadline = Instant::now() + TIMEOUT;
    let mut noticed = false;
    while Instant::now() < deadline {
        if watcher.poll() {
            noticed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(noticed, "saving the scene file should report a change");
    assert!(
        !watcher.poll(),
        "the change must be reported once, not on every subsequent poll"
    );
}

#[test]
fn saving_a_neighbouring_file_does_not_report_a_change() {
    let dir = scratch("watch-neighbour");
    let path = dir.join("scene.toml");
    write(&path, "model = \"cube_diorama.glb\"\n");

    let mut watcher = SceneWatcher::new(&path).expect("the directory exists");
    std::thread::sleep(Duration::from_millis(200));

    std::fs::write(dir.join("notes.txt"), "unrelated").expect("write the neighbour");

    // Long enough for the event to arrive and clear the debounce window if it were accepted.
    std::thread::sleep(Duration::from_millis(500));

    assert!(
        !watcher.poll(),
        "the watch covers the whole directory, so unrelated saves must be filtered out"
    );
}

#[test]
fn a_burst_of_saves_is_debounced_into_a_single_change() {
    let dir = scratch("watch-debounce");
    let path = dir.join("scene.toml");
    write(&path, "model = \"cube_diorama.glb\"\n");

    let mut watcher = SceneWatcher::new(&path).expect("the directory exists");
    std::thread::sleep(Duration::from_millis(200));

    for intensity in 1..=8 {
        write(&path, &format!("[sun]\nintensity = {intensity}.0\n"));
    }

    let deadline = Instant::now() + TIMEOUT;
    let mut changes = 0;
    while Instant::now() < deadline {
        if watcher.poll() {
            changes += 1;
            // Keep polling past the first hit: an undebounced watcher would report the rest here.
            let settle = Instant::now() + Duration::from_millis(500);
            while Instant::now() < settle {
                if watcher.poll() {
                    changes += 1;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(
        changes, 1,
        "eight writes in a row should coalesce into one reload, got {changes}"
    );
}
