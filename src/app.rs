use std::sync::Arc;
use std::time::Duration;

use winit::{
    application::ApplicationHandler,
    event::*,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window},
};

use crate::config::{RendererConfig, SceneConfig};
use crate::gfx::GpuContext;
use crate::renderer::Renderer;
use crate::scene::Scene;
use crate::scene::camera::CameraMove;

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use winit::platform::web::EventLoopExtWebSys;

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Action {
    Exit,
    CycleDiffuse,
    ToggleDepthDebug,
    MoveCamera {
        direction: CameraMove,
        is_pressed: bool,
    },
    LookCamera {
        dx: f64,
        dy: f64,
    },
}

fn action_for_key(key_code: KeyCode, is_pressed: bool) -> Option<Action> {
    let movement = |direction| {
        Some(Action::MoveCamera {
            direction,
            is_pressed,
        })
    };

    match key_code {
        KeyCode::Escape if is_pressed => Some(Action::Exit),
        KeyCode::KeyC if is_pressed => Some(Action::CycleDiffuse),
        KeyCode::KeyF if is_pressed => Some(Action::ToggleDepthDebug),
        KeyCode::KeyW | KeyCode::ArrowUp => movement(CameraMove::Forward),
        KeyCode::KeyS | KeyCode::ArrowDown => movement(CameraMove::Backward),
        KeyCode::KeyA | KeyCode::ArrowLeft => movement(CameraMove::Left),
        KeyCode::KeyD | KeyCode::ArrowRight => movement(CameraMove::Right),
        KeyCode::KeyE => movement(CameraMove::Up),
        KeyCode::KeyQ => movement(CameraMove::Down),
        _ => None,
    }
}

pub struct Engine {
    window: Arc<Window>,
    ctx: GpuContext,
    renderer: Renderer,
    scene: Scene,
    last_update: Instant,
}

impl Engine {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let ctx = GpuContext::new(window.clone()).await?;
        let scene = Scene::new(&ctx, &SceneConfig::default()).await?;
        let renderer = Renderer::new(&ctx, &scene, RendererConfig::default());

        Ok(Self {
            window,
            ctx,
            renderer,
            scene,
            last_update: Instant::now(),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        self.ctx.resize(width, height);
        self.renderer.resize(&self.ctx);
        self.scene.resize(width, height);
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::Exit => {}
            Action::CycleDiffuse => self.scene.cycle_diffuse(),
            Action::ToggleDepthDebug => self.renderer.toggle_depth_debug(),
            Action::MoveCamera {
                direction,
                is_pressed,
            } => self.scene.set_camera_move(direction, is_pressed),
            Action::LookCamera { dx, dy } => self.scene.set_camera_look(dx, dy),
        }
    }

    fn update(&mut self, actions: &[Action]) {
        for action in actions {
            self.apply(*action);
        }

        let now = Instant::now();
        let dt: Duration = now.duration_since(self.last_update);
        self.last_update = now;

        self.scene.update(&self.ctx.queue, dt.as_secs_f32());
    }

    fn render(&mut self) -> anyhow::Result<()> {
        self.renderer.render(&mut self.ctx, &self.scene)
    }
}

pub struct App {
    #[cfg(target_arch = "wasm32")]
    proxy: Option<winit::event_loop::EventLoopProxy<Engine>>,
    engine: Option<Engine>,
    pending_actions: Vec<Action>,
}

impl App {
    #[allow(clippy::new_without_default)]
    pub fn new(#[cfg(target_arch = "wasm32")] event_loop: &EventLoop<Engine>) -> Self {
        #[cfg(target_arch = "wasm32")]
        let proxy = Some(event_loop.create_proxy());
        Self {
            engine: None,
            pending_actions: Vec::new(),
            #[cfg(target_arch = "wasm32")]
            proxy,
        }
    }
}

impl ApplicationHandler<Engine> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        #[allow(unused_mut)]
        let mut window_attributes = Window::default_attributes();
        window_attributes.title = String::from("WebGPU Renderer");

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;

            const CANVAS_ID: &str = "canvas";

            let window = wgpu::web_sys::window().unwrap_throw();
            let document = window.document().unwrap_throw();
            let canvas = document.get_element_by_id(CANVAS_ID).unwrap_throw();
            let html_canvas_element = canvas.unchecked_into();
            window_attributes = window_attributes.with_canvas(Some(html_canvas_element));
        }

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        #[cfg(not(target_arch = "wasm32"))]
        {
            grab_cursor(&window);
            self.engine = Some(pollster::block_on(Engine::new(window)).unwrap());
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(proxy) = self.proxy.take() {
                wasm_bindgen_futures::spawn_local(async move {
                    assert!(
                        proxy
                            .send_event(
                                Engine::new(window)
                                    .await
                                    .expect("Unable to create canvas!!!")
                            )
                            .is_ok()
                    )
                });
            }
        }
    }

    #[allow(unused_mut)]
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, mut event: Engine) {
        #[cfg(target_arch = "wasm32")]
        {
            event.window.request_redraw();
            event.resize(
                event.window.inner_size().width,
                event.window.inner_size().height,
            );
        }
        self.engine = Some(event);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let engine = match &mut self.engine {
            Some(engine) => engine,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => engine.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                engine.update(&self.pending_actions);
                self.pending_actions.clear();

                let result = engine.render();
                engine.window.request_redraw();

                if let Err(e) = result {
                    log::error!("{e}");
                    event_loop.exit();
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: key_state,
                        ..
                    },
                ..
            } => match action_for_key(code, key_state.is_pressed()) {
                Some(Action::Exit) => event_loop.exit(),
                Some(action) => self.pending_actions.push(action),
                None => {}
            },
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            self.pending_actions.push(Action::LookCamera { dx, dy });
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn grab_cursor(window: &Window) {
    if window
        .set_cursor_grab(CursorGrabMode::Locked)
        .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
        .is_ok()
    {
        window.set_cursor_visible(false);
    }
}

pub fn run() -> anyhow::Result<()> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
    }
    #[cfg(target_arch = "wasm32")]
    {
        console_log::init_with_level(log::Level::Info).unwrap_throw();
    }

    let event_loop = EventLoop::with_user_event().build()?;
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut app = App::new();
        event_loop.run_app(&mut app)?;
    }
    #[cfg(target_arch = "wasm32")]
    {
        let app = App::new(&event_loop);
        event_loop.spawn_app(app);
    }

    Ok(())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn run_web() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    run().unwrap_throw();

    Ok(())
}
