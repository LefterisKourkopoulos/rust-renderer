use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    event::*, 
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey}, 
    window::Window
}

/// State Struct
pub struct State {
    window: Arc<Window>,
}

impl State {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        Ok(Self 
            { window,
         })
    }

    pub fn resize(&mut self, _width: u32, _height: u32) {
        // Handle window resize
    }

    pub fn render(&mut self) {
        self.window.request_redraw();

        // Handle rendering
    }
}

/// App Struct
pub struct App {
    #[cfg(target_arch = "wasm32")]
    proxy: Option<winit::event_loop::EventLoopProxy<State>>,
    state: Option<State>,
}

impl App {
pub fn new(#[cfg(target_arch = "wasm32")] event_loop: &EventLoop<State>) -> Self {
        #[cfg(target_arch = "wasm32")]
        let proxy = Some(event_loop.create_proxy());
        Self {
            state: None,
            #[cfg(target_arch = "wasm32")]
            proxy,
        }
    }
}