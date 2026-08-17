pub struct RendererConfig {
    pub clear_color: wgpu::Color,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            clear_color: wgpu::Color {
                r: 0.1,
                g: 0.2,
                b: 0.5,
                a: 1.0,
            },
        }
    }
}

pub struct CameraConfig {
    pub speed: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
    pub eye: [f32; 3],
    pub target: [f32; 3],
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            speed: 12.0,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
            eye: [0.0, 1.0, 2.0],
            target: [0.0, 0.0, 0.0],
        }
    }
}

pub struct InstanceGridConfig {
    pub instances_per_row: u32,
    pub space_between: f32,
}

impl Default for InstanceGridConfig {
    fn default() -> Self {
        Self {
            instances_per_row: 10,
            space_between: 3.0,
        }
    }
}

pub struct SceneConfig {
    pub camera: CameraConfig,
    pub grid: InstanceGridConfig,
    pub model_file: &'static str,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            camera: CameraConfig::default(),
            grid: InstanceGridConfig::default(),
            model_file: "cube.obj",
        }
    }
}
