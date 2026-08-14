use std::sync::Arc;

use winit::window::Window;

/// Owns the wgpu handles that live for the whole run: the surface we present
/// to, the device/queue we record work on, and the surface's current
/// configuration.
///
/// Deliberately free of any app-specific knowledge — it has no idea what is
/// being drawn, only how to hand out frames to draw into.
pub struct GpuContext {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    /// The surface starts out unconfigured; the first `resize` sizes it. Until
    /// then there is nothing valid to render into.
    pub is_surface_configured: bool,
}

impl GpuContext {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            flags: Default::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });

        // The surface borrows the window, so it is created from the same Arc the
        // app holds and is 'static for as long as that Arc lives.
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: true,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default()
                },
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);

        // Prefer an sRGB target so the shader can write linear colors and let the
        // hardware do the conversion.
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };

        Ok(Self {
            surface,
            device,
            queue,
            config,
            is_surface_configured: false,
        })
    }

    /// Reconfigures the surface for a new size. Size-dependent resources owned
    /// elsewhere (the depth texture, the camera's aspect ratio) are the caller's
    /// responsibility.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.is_surface_configured = true;
        }
    }

    /// Tries to get the next frame to render into.
    ///
    /// `Ok(None)` means "no frame this time, try again next tick" — the surface
    /// isn't ready yet, the frame timed out, or it was reconfigured underneath
    /// us. `Err` is reserved for the unrecoverable case.
    pub fn acquire_frame(&mut self) -> anyhow::Result<Option<wgpu::SurfaceTexture>> {
        if !self.is_surface_configured {
            return Ok(None);
        }

        match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(surface_texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(surface_texture) => Ok(Some(surface_texture)),
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => Ok(None),
            wgpu::CurrentSurfaceTexture::Outdated => {
                // Stale configuration, e.g. the window was resized mid-frame.
                // Reconfigure and pick the frame up on the next tick.
                self.surface.configure(&self.device, &self.config);
                Ok(None)
            }
            wgpu::CurrentSurfaceTexture::Lost => anyhow::bail!("Lost Device"),
        }
    }
}
