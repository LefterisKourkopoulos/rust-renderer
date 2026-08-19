# Rust Renderer

A real-time 3D renderer written in Rust on top of [wgpu](https://wgpu.rs/), running both natively
(macOS/Linux/Windows) and in the browser via WebAssembly from a single codebase. It began as a walk
through the [Learn WGPU](https://sotrh.github.io/learn-wgpu/) progression — vertex and index
buffers, textures and samplers, instanced draws, a depth buffer, model loading — with each stage
factored into its own module rather than left as one long tutorial file, and has since grown glTF
loading, cascaded shadow maps, punctual lights, a skybox, and hot reloading of the scene.

## Features

- **Scene hot reloading** — the scene is described by a TOML file. Save it and the renderer rebuilds
  in place, on a background thread, while the previous scene keeps rendering; a broken file leaves
  what is on screen alone and logs why.
- **glTF / GLB loading** — geometry, PBR materials, textures, node hierarchies and punctual lights
  from a single self-contained `.glb`, via [`gltf`](https://docs.rs/gltf). `.obj` with `.mtl` is
  still supported for the embedded assets, via [`tobj`](https://docs.rs/tobj).
- **Cascaded shadow maps** — four cascades over the view frustum, with a comparison sampler,
  configurable depth and normal-offset bias, and a debug view of the individual layers.
- **Multiple punctual lights** — directional, point and spot lights in a storage buffer, with
  inverse-square falloff and cone attenuation. Lights declared by the model are used as-is.
- **Skybox** — an equirectangular HDRI converted to a cubemap by a compute pass at startup.
- **Instanced rendering** — one mesh drawn many times from a per-instance model matrix vertex
  buffer, either from the glTF node placements or from a generated grid.
- **Debug overlays** — the depth buffer as grayscale, and the shadow cascades tinted per layer or
  shown one layer at a time.
- **Cross-platform** — one source tree targeting native (via `pollster` + `winit`) and
  `wasm32-unknown-unknown` (via `wasm-bindgen`). Hot reloading is native only, since wasm has no
  filesystem to watch and no threads to load on.

## Controls

| Key | Action |
| --- | --- |
| `W` / `Up` | Move forward |
| `S` / `Down` | Move backward |
| `A` / `Left` | Strafe left |
| `D` / `Right` | Strafe right |
| `E` | Move up |
| `Q` | Move down |
| Mouse | Look around (the cursor is grabbed on startup) |
| `C` | Cycle diffuse texture: model material -> `happy-tree.png` -> `centrica_logo.png` -> back |
| `F` | Toggle the depth-buffer debug overlay |
| `G` | Toggle per-cascade tinting |
| `H` | Cycle which shadow cascade layer is shown |
| `R` | Reload the scene file now, without waiting for a save |
| `Esc` | Exit |

## The scene file

`scenes/default.toml` describes the scene, and is used automatically when no `--scene` is given:

```sh
cargo run                          # uses scenes/default.toml if it exists
cargo run -- --scene my/scene.toml # or point at another one
```

Every key is optional and falls back to the built-in default, so a usable scene file can be a
single line. Unknown keys are rejected rather than ignored — a typo that silently did nothing would
be indistinguishable from a save that changed nothing.

```toml
model = "cube_diorama.glb"
light_intensity_scale = 0.005

[camera]
position = [2.5, 2.0, 2.5]
yaw = -135.0
pitch = -20.0
fovy = 45.0
znear = 0.05
zfar = 100.0
speed = 2.0
sensitivity = 1.0

[sun]
direction = [-0.4, -1.0, -0.3]
color = [1.0, 0.98, 0.92]
intensity = 1.5

[grid]
instances_per_row = 10
space_between = 3.0
```

`model` is looked for on disk relative to the scene file first, then in the table of assets embedded
in the binary — which is how `cube_diorama.glb` resolves with no file on disk at all, and how the
wasm build works. `[grid]` only applies when the model brings no node placements of its own.

Values that would fail deep inside the renderer are rejected at parse time, where the message can
still point at the offending key: a zero `sun.direction` has no normalized form, a `zfar` behind
`znear` is not renderable, a zero `instances_per_row` would create the empty vertex buffer wgpu
rejects, and NaN anywhere propagates into every matrix it touches.

Without a scene file the renderer falls back to the built-in scene and hot reloading is off.

### How the reload works

Saving the file rebuilds the scene on a worker thread and swaps it in when it is ready, so the
frame rate is never held hostage to decoding a `.glb`. Three details are load-bearing:

- Bind group layouts live in `gfx::Layouts`, owned above *both* the scene and the renderer. A
  pipeline validates its bind groups by pointer identity, so a scene built later has to bind
  against the same layout objects the pipelines were created with.
- The watch is on the scene file's **parent directory**, not the file. Most editors save by writing
  a temporary file and renaming it over the target, which replaces the inode and detaches a watch
  registered on the file itself.
- Events are debounced and compared against the *canonicalized* directory. notify resolves symlinks
  in the paths it reports, and on macOS `/var` is a symlink to `/private/var`, so a merely absolute
  path would never compare equal and every save would be ignored.

Reloading is `.glb` only. An `.obj` resolves its `.mtl` and textures against the embedded asset
table rather than its own directory, so an on-disk one is rejected with an explanation instead of
half-loading.

## Prerequisites

- A Rust toolchain for **edition 2024** (Rust 1.85+), installed via [rustup](https://rustup.rs/).
- A GPU with a working Vulkan, Metal, DX12, or WebGL backend.
- For the web target: the `wasm32-unknown-unknown` target and `wasm-pack`.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

## Build and run

### Native

```sh
cargo run
```

Models, textures and the HDRI in `src/res/` are embedded into the binary with `include_bytes!`, so
there is no working-directory setup and no asset directory to ship. A scene file may name a model
on disk instead, which is what hot reloading acts on.

### Web

```sh
wasm-pack build --target web
```

This emits a `pkg/` directory containing `rust_renderer.js` and the `.wasm` binary. The `index.html`
at the repo root imports `./pkg/rust_renderer.js` and renders into a full-page `<canvas id="canvas">`,
so serve the **repository root** (not `pkg/`) over HTTP and open `index.html`:

```sh
python3 -m http.server 8000
```

ES module imports are blocked under the `file://` scheme, so opening `index.html` directly from disk
will not work — it must be served over HTTP.

`Cargo.toml` sets `wasm-opt = false` under `[package.metadata.wasm-pack.profile.release]`; `wasm-opt`
has historically choked on the WebAssembly features that current `wasm-bindgen` output relies on, so
it is disabled rather than worked around.

Since every asset is embedded, the wasm build fetches nothing at runtime and serving the repository
root is enough. Hot reloading, the scene file and `--scene` are all native-only.

### Toolchain gotcha: Homebrew vs. rustup

If a wasm build fails with:

```
error[E0463]: can't find crate for `core`
error[E0463]: can't find crate for `std`
```

then the `cargo`/`rustc` first on your `PATH` is probably Homebrew's (`/opt/homebrew/bin`) rather
than rustup's. The Homebrew `rust` formula ships only the host target's standard library, and
`rustup target add` cannot install into it — the target looks installed under rustup while the
Homebrew compiler that actually runs knows nothing about it. Confirm with:

```sh
which -a cargo rustc   # is /opt/homebrew/bin ahead of rustup's shims?
cargo --version        # a "(Homebrew)" suffix means you are on the wrong one
```

The fix is to put rustup's shims ahead of Homebrew on your `PATH`, for example:

```sh
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo build --target wasm32-unknown-unknown
```

## Project structure

| Path | Responsibility |
| --- | --- |
| `src/main.rs` | Native binary entry point; delegates to the library's `run`. |
| `src/lib.rs` | Library root; declares the module tree, re-exports the public API, and holds the wasm `#[wasm_bindgen(start)]` entry point. |
| `src/app.rs` | `winit` `ApplicationHandler`: window creation, event loop, input dispatch, `--scene` parsing, and the scene swap. |
| `src/config.rs` | The configuration types the renderer and scene are built from, and their defaults. |
| `src/scene_file.rs` | The TOML scene description, its translation into a `SceneConfig`, and its validation. |
| `src/watch/watcher.rs` | Watches the scene file's directory and reports debounced saves. |
| `src/watch/loader.rs` | Rebuilds the scene on a worker thread and hands it back when ready. |
| `src/renderer.rs` | Owns the pipelines and records the passes that draw a scene. |
| `src/gfx/context.rs` | wgpu instance, adapter, device, queue, and surface configuration and resizing. |
| `src/gfx/layouts.rs` | Every bind group layout, owned above both the scene and the renderer so a reload stays compatible. |
| `src/gfx/texture.rs` | Texture and sampler creation, image decoding, the depth texture, and cubemaps. |
| `src/gfx/hdr.rs`, `src/gfx/hdr_loader.rs` | The HDR render target and the equirectangular-to-cubemap compute pass. |
| `src/gfx/pipeline.rs` | Render pipeline construction shared by the scene and debug passes. |
| `src/gfx/vertex.rs` | Vertex types and their `VertexBufferLayout` descriptors. |
| `src/scene/mod.rs` | Assembles a scene: model, instances, camera, lights and environment. |
| `src/scene/camera.rs` | Camera, its view-projection uniform buffer and bind group, and the controller. |
| `src/scene/instance.rs` | Per-instance transforms and the grid layout, plus their raw GPU representation. |
| `src/scene/light.rs` | Light types, the storage buffer they live in, and the debug geometry. |
| `src/scene/model.rs` | Mesh, material and model types and the instanced draw helpers. |
| `src/shadow/` | The cascaded shadow map pass and the cascade split and projection maths. |
| `src/assets.rs` | Asset resolution: a file on disk first, then the embedded table. |
| `src/assets/gltf_loader.rs` | glTF/GLB import: meshes, materials, textures, node transforms and lights. |
| `src/assets/obj.rs` | OBJ/MTL import, for the embedded assets. |
| `src/debug/` | The depth-buffer and shadow-cascade debug overlays. |
| `src/shaders/` | WGSL for the scene, lights, sky, shadows, HDR tonemapping and the debug overlays. |
| `src/res/` | Models, materials, textures and the HDRI, embedded with `include_bytes!`. |
| `scenes/` | Scene description files. |

## Tests

```sh
cargo test
```

Unit tests cover the maths and parsing directly. The integration tests in `tests/` need a working
GPU adapter and skip themselves with a message when none is available, so they pass on a headless
machine without pretending to have run: `tests/hot_reload.rs` drives the real watcher and loader
against a real device, `tests/shadow_render.rs` renders and reads back shadow depth,
`tests/pipeline_creation.rs` builds the pipelines against the real shaders, and
`tests/model_loading.rs` loads the embedded diorama.

## License

No license is currently declared for this project.
