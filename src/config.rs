#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum PipelineMode {
    #[default]
    Normal,
    Hdr,
}

pub struct RendererConfig {
    pub clear_color: wgpu::Color,
    pub pipeline_mode: PipelineMode,
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
            pipeline_mode: PipelineMode::default(),
        }
    }
}

pub struct CameraConfig {
    pub speed: f32,
    pub sensitivity: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
    pub position: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            speed: 12.0,
            sensitivity: 1.0,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
            position: [5.0, 5.0, 5.0],
            yaw: -135.0,
            pitch: -35.264389,
        }
    }
}

impl CameraConfig {
    pub fn close_up() -> Self {
        Self {
            speed: 2.0,
            znear: 0.05,
            position: [2.5, 2.0, 2.5],
            pitch: -20.0,
            ..Self::default()
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
    pub light_intensity_scale: f32,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            camera: CameraConfig::close_up(),
            grid: InstanceGridConfig::default(),
            model_file: "cube_diorama.glb",
            light_intensity_scale: 0.005,
        }
    }
}
