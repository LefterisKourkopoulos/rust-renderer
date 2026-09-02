use std::path::PathBuf;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum PipelineMode {
    #[default]
    Normal,
    Hdr,
}

pub struct RendererConfig {
    pub clear_color: wgpu::Color,
    pub pipeline_mode: PipelineMode,
    pub shadows: ShadowConfig,
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
            shadows: ShadowConfig::default(),
        }
    }
}

pub const MAX_CASCADES: usize = 4;

#[derive(Clone, Debug)]
pub struct ShadowConfig {
    pub enabled: bool,
    pub cascade_count: usize,
    pub resolution: u32,
    pub split_lambda: f32,
    pub z_mult: f32,
    pub depth_bias: i32,
    pub depth_bias_slope: f32,
    pub normal_offset: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for ShadowConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cascade_count: MAX_CASCADES,
            resolution: 2048,
            split_lambda: 0.7,
            z_mult: 10.0,
            depth_bias: 2,
            depth_bias_slope: 2.0,
            normal_offset: 0.02,
            near: 0.1,
            far: 30.0,
        }
    }
}

impl ShadowConfig {
    pub fn cascade_count(&self) -> usize {
        self.cascade_count.clamp(1, MAX_CASCADES)
    }

    pub fn range(&self, znear: f32, zfar: f32) -> (f32, f32) {
        let near = self.near.max(znear);
        (near, self.far.clamp(near + 1e-3, zfar))
    }
}

#[derive(Clone, Debug)]
pub struct SunConfig {
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
    pub latitude: f32,
    pub longitude: f32,
}

/// London's coordinates, the default location for scenes that don't specify one.
const LONDON_LATITUDE: f32 = 51.5074;
const LONDON_LONGITUDE: f32 = -0.1278;

impl Default for SunConfig {
    fn default() -> Self {
        Self {
            direction: [-0.4, -1.0, -0.3],
            color: [1.0, 0.98, 0.92],
            intensity: 1.5,
            latitude: LONDON_LATITUDE,
            longitude: LONDON_LONGITUDE,
        }
    }
}

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
pub struct SceneConfig {
    pub camera: CameraConfig,
    pub grid: InstanceGridConfig,
    pub model_file: String,
    pub base_dir: Option<PathBuf>,
    pub light_intensity_scale: f32,
    pub sun: SunConfig,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            camera: CameraConfig::close_up(),
            grid: InstanceGridConfig::default(),
            model_file: String::from("cube_diorama.glb"),
            base_dir: None,
            light_intensity_scale: 0.005,
            sun: SunConfig::default(),
        }
    }
}
