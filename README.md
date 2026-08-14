# Rust Renderer

A real-time 3D renderer written in Rust on top of [wgpu](https://wgpu.rs/), running both natively
(macOS/Linux/Windows) and in the browser via WebAssembly from a single codebase. It follows the
[Learn WGPU](https://sotrh.github.io/learn-wgpu/) progression — vertex and index buffers, textures
and samplers, instanced draws, a depth buffer, and `.obj` model loading — with the stages factored
into standalone modules rather than left as one long tutorial file. The scene is a 10x10 grid of
instanced, textured cubes with an orbiting camera.

## Features

- **Instanced rendering** — a 10x10 grid (100 cubes) drawn from one mesh with a per-instance model
  matrix vertex buffer.
- **Model loading** — `.obj` geometry with `.mtl` materials parsed via [`tobj`](https://docs.rs/tobj),
  including the diffuse texture referenced by the material.
- **Depth buffer** — a depth attachment for correct occlusion, plus a toggleable debug overlay that
  samples the depth texture and renders it as grayscale.
- **Runtime-switchable textures** — cycle the diffuse source between each mesh's own `.mtl` material
  and two override textures without a rebuild.
- **Orbiting camera** — keyboard-driven camera with a view-projection uniform, corrected from
  cgmath's OpenGL-style `-1..1` depth range to wgpu's `0..1`.
- **Cross-platform** — one source tree targeting native (via `pollster` + `winit`) and
  `wasm32-unknown-unknown` (via `wasm-bindgen`, the WebGL backend, and `reqwest` for asset fetching).

## Controls

| Key | Action |
| --- | --- |
| `W` / `Up` | Move camera toward the target |
| `S` / `Down` | Move camera away from the target |
| `A` / `Left` | Orbit camera left |
| `D` / `Right` | Orbit camera right |
| `Space` | Cycle diffuse texture: model material -> `happy-tree.png` -> `centrica_logo.png` -> back |
| `F` | Toggle the depth-buffer debug overlay |
| `Esc` | Exit |

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

Assets are copied from `src/res/` into `OUT_DIR` by `build.rs` at compile time and loaded from there
at runtime, so no working-directory setup is needed.

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

> **Note on runtime asset loading in the browser.** On wasm, `.obj`/`.mtl`/texture files are fetched
> over HTTP at runtime rather than embedded, and the URL builder appends a `learn-wgpu` path segment
> to the page origin — assets are requested from `<origin>/learn-wgpu/<file>`. Serving the repo root
> at `/` alone will therefore 404 on those fetches; either host the assets under a `learn-wgpu/` path
> or adjust the URL logic in `src/assets.rs` to match your layout.

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

CI is unaffected, since it provisions the toolchain through rustup and adds the target via the
toolchain action.

## Project structure

The renderer is split so each Learn WGPU concept lives in its own module.

| Path | Responsibility |
| --- | --- |
| `src/main.rs` | Native binary entry point; delegates to the library's `run`. |
| `src/lib.rs` | Library root; declares the module tree, re-exports the public API, and holds the wasm `#[wasm_bindgen(start)]` entry point. |
| `src/app.rs` | `winit` `ApplicationHandler` implementation: window creation, event loop, and input dispatch. |
| `src/config.rs` | Centralized tunable constants such as grid size, spacing, and camera speed. |
| `src/renderer.rs` | Owns per-frame state and records the render pass that draws the scene. |
| `src/gfx/context.rs` | wgpu instance, adapter, device, queue, and surface configuration and resizing. |
| `src/gfx/texture.rs` | Texture and sampler creation, image decoding, and the depth texture. |
| `src/gfx/pipeline.rs` | Render pipeline construction shared by the scene and debug passes. |
| `src/gfx/vertex.rs` | Vertex types and their `VertexBufferLayout` descriptors. |
| `src/scene/camera.rs` | Camera, its view-projection uniform buffer and bind group, and the keyboard controller. |
| `src/scene/instance.rs` | Per-instance transforms and the grid layout, plus their raw GPU representation. |
| `src/scene/model.rs` | Mesh, material, and model types and the instanced draw helpers. |
| `src/assets.rs` | Platform-abstracted asset loading: filesystem on native, HTTP fetch on wasm. |
| `src/debug/depth.rs` | Depth-buffer debug overlay pipeline and its toggle. |
| `src/shaders/` | WGSL shaders for the main scene and the depth debug overlay. |
| `src/res/` | Models, materials, and textures, copied into `OUT_DIR` by `build.rs`. |
| `build.rs` | Copies `src/res/` into `OUT_DIR` so native builds can load assets by path. |

## Continuous integration

`.github/workflows/ci.yml` runs `cargo fmt --check`, then `cargo clippy -- -D warnings` and
`cargo check` for both the native and `wasm32-unknown-unknown` targets. Checking both matters here:
a large share of this crate's code sits behind `cfg(target_arch = "wasm32")` and its negation, so a
native-only check would silently let the other half rot.

## License

No license is currently declared for this project.
