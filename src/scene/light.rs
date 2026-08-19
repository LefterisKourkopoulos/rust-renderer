use bytemuck::Zeroable;
use cgmath::Rotation3;
use wgpu::util::DeviceExt;

use super::model::ModelVertex;

/// The `KHR_lights_punctual` light types.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum LightKind {
    Directional,
    Point,
    Spot {
        inner_cone_angle: f32,
        outer_cone_angle: f32,
    },
}

impl LightKind {
    fn tag(&self) -> u32 {
        match self {
            LightKind::Directional => 0,
            LightKind::Point => 1,
            LightKind::Spot { .. } => 2,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Light {
    pub position: [f32; 3],
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub kind: LightKind,
}

impl Light {
    pub fn new(position: [f32; 3], color: [f32; 3]) -> Self {
        Self {
            position,
            direction: [0.0, -1.0, 0.0],
            color,
            intensity: 1.0,
            range: 0.0,
            kind: LightKind::Point,
        }
    }

    pub fn with_intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity;
        self
    }

    pub fn directional(direction: [f32; 3], color: [f32; 3], intensity: f32) -> Self {
        Self {
            position: [0.0; 3],
            direction,
            color,
            intensity,
            range: 0.0,
            kind: LightKind::Directional,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct LightRaw {
    position: [f32; 3],
    kind: u32,
    color: [f32; 3],
    intensity: f32,
    direction: [f32; 3],
    range: f32,
    cos_inner: f32,
    cos_outer: f32,
    _padding: [f32; 2],
}

impl From<Light> for LightRaw {
    fn from(light: Light) -> Self {
        let (cos_inner, cos_outer) = match light.kind {
            LightKind::Spot {
                inner_cone_angle,
                outer_cone_angle,
            } => (inner_cone_angle.cos(), outer_cone_angle.cos()),
            _ => (1.0, -1.0),
        };

        Self {
            position: light.position,
            kind: light.kind.tag(),
            color: light.color,
            intensity: light.intensity,
            direction: normalize(light.direction),
            range: light.range,
            cos_inner,
            cos_outer,
            _padding: [0.0; 2],
        }
    }
}

fn normalize(direction: [f32; 3]) -> [f32; 3] {
    let length =
        (direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2])
            .sqrt();

    if length < 1e-6 {
        return [0.0, -1.0, 0.0];
    }

    [
        direction[0] / length,
        direction[1] / length,
        direction[2] / length,
    ]
}

const CUBE_VERTICES: [ModelVertex; 8] = [
    ModelVertex { position: [-0.5, -0.5, -0.5], tex_coords: [0.0, 0.0], normal: [0.0, 0.0, 0.0], tangent: [0.0, 0.0, 0.0], bitangent: [0.0, 0.0, 0.0] },
    ModelVertex { position: [0.5, -0.5, -0.5], tex_coords: [0.0, 0.0], normal: [0.0, 0.0, 0.0], tangent: [0.0, 0.0, 0.0], bitangent: [0.0, 0.0, 0.0] },
    ModelVertex { position: [0.5, 0.5, -0.5], tex_coords: [0.0, 0.0], normal: [0.0, 0.0, 0.0], tangent: [0.0, 0.0, 0.0], bitangent: [0.0, 0.0, 0.0] },
    ModelVertex { position: [-0.5, 0.5, -0.5], tex_coords: [0.0, 0.0], normal: [0.0, 0.0, 0.0], tangent: [0.0, 0.0, 0.0], bitangent: [0.0, 0.0, 0.0] },
    ModelVertex { position: [-0.5, -0.5, 0.5], tex_coords: [0.0, 0.0], normal: [0.0, 0.0, 0.0], tangent: [0.0, 0.0, 0.0], bitangent: [0.0, 0.0, 0.0] },
    ModelVertex { position: [0.5, -0.5, 0.5], tex_coords: [0.0, 0.0], normal: [0.0, 0.0, 0.0], tangent: [0.0, 0.0, 0.0], bitangent: [0.0, 0.0, 0.0] },
    ModelVertex { position: [0.5, 0.5, 0.5], tex_coords: [0.0, 0.0], normal: [0.0, 0.0, 0.0], tangent: [0.0, 0.0, 0.0], bitangent: [0.0, 0.0, 0.0] },
    ModelVertex { position: [-0.5, 0.5, 0.5], tex_coords: [0.0, 0.0], normal: [0.0, 0.0, 0.0], tangent: [0.0, 0.0, 0.0], bitangent: [0.0, 0.0, 0.0] },
];

const CUBE_INDICES: [u32; 36] = [
    4, 5, 6, 6, 7, 4,
    1, 0, 3, 3, 2, 1,
    0, 4, 7, 7, 3, 0,
    5, 1, 2, 2, 6, 5,
    3, 7, 6, 6, 2, 3,
    0, 1, 5, 5, 4, 0,
];

pub struct LightCollection {
    lights: Vec<Light>,
    animate: bool,
    buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

impl LightCollection {
    pub fn new(device: &wgpu::Device, lights: Vec<Light>, animate: bool) -> Self {
        let buffer = Self::create_buffer(device, &lights);

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("light_bind_group_layout"),
            });

        let bind_group = Self::create_bind_group(device, &bind_group_layout, &buffer);

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light Vertex Buffer"),
            contents: bytemuck::cast_slice(&CUBE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light Index Buffer"),
            contents: bytemuck::cast_slice(&CUBE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            lights,
            animate,
            buffer,
            bind_group,
            bind_group_layout,
            vertex_buffer,
            index_buffer,
            num_indices: CUBE_INDICES.len() as u32,
        }
    }

    pub fn count(&self) -> u32 {
        self.lights.len() as u32
    }

    pub fn directional(&self) -> Option<Light> {
        self.lights
            .iter()
            .find(|light| light.kind == LightKind::Directional)
            .copied()
    }

    pub fn add(&mut self, device: &wgpu::Device, light: Light) {
        self.lights.push(light);
        self.buffer = Self::create_buffer(device, &self.lights);
        self.bind_group = Self::create_bind_group(device, &self.bind_group_layout, &self.buffer);
    }

    pub fn update(&mut self, queue: &wgpu::Queue, dt: f32) {
        if !self.animate || self.lights.is_empty() {
            return;
        }

        for light in &mut self.lights {
            if light.kind == LightKind::Directional {
                continue;
            }

            let old_position: cgmath::Vector3<f32> = light.position.into();
            light.position = (cgmath::Quaternion::from_axis_angle(
                (0.0, 1.0, 0.0).into(),
                cgmath::Deg(60.0 * dt),
            ) * old_position)
                .into();
        }

        let raw: Vec<LightRaw> = self.lights.iter().copied().map(LightRaw::from).collect();
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&raw));
    }

    fn create_buffer(device: &wgpu::Device, lights: &[Light]) -> wgpu::Buffer {
        let mut raw: Vec<LightRaw> = lights.iter().copied().map(LightRaw::from).collect();
        if raw.is_empty() {
            raw.push(LightRaw::zeroed());
        }

        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Light Storage Buffer"),
            contents: bytemuck::cast_slice(&raw),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        })
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("light_bind_group"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    #[test]
    fn light_raw_matches_the_shader_struct_layout() {
        assert_eq!(std::mem::size_of::<LightRaw>(), 64, "struct size");
        assert_eq!(std::mem::offset_of!(LightRaw, position), 0);
        assert_eq!(std::mem::offset_of!(LightRaw, kind), 12);
        assert_eq!(std::mem::offset_of!(LightRaw, color), 16);
        assert_eq!(std::mem::offset_of!(LightRaw, intensity), 28);
        assert_eq!(std::mem::offset_of!(LightRaw, direction), 32);
        assert_eq!(std::mem::offset_of!(LightRaw, range), 44);
        assert_eq!(std::mem::offset_of!(LightRaw, cos_inner), 48);
        assert_eq!(std::mem::offset_of!(LightRaw, cos_outer), 52);
    }

    #[test]
    fn kind_tags_match_the_shader_constants() {
        assert_eq!(LightKind::Directional.tag(), 0);
        assert_eq!(LightKind::Point.tag(), 1);
        assert_eq!(
            LightKind::Spot {
                inner_cone_angle: 0.0,
                outer_cone_angle: 1.0,
            }
            .tag(),
            2
        );
    }

    #[test]
    fn spot_cone_angles_become_cosines_with_the_inner_one_larger() {
        let raw = LightRaw::from(Light {
            kind: LightKind::Spot {
                inner_cone_angle: 0.0,
                outer_cone_angle: std::f32::consts::FRAC_PI_4,
            },
            ..Light::new([0.0; 3], [1.0; 3])
        });

        assert!((raw.cos_inner - 1.0).abs() < EPSILON);
        assert!((raw.cos_outer - std::f32::consts::FRAC_1_SQRT_2).abs() < EPSILON);
        assert!(
            raw.cos_inner > raw.cos_outer,
            "smoothstep(cos_outer, cos_inner, ..) needs the inner cosine to be the larger one"
        );
    }

    #[test]
    fn directions_are_normalized() {
        let raw = LightRaw::from(Light {
            direction: [0.0, 0.0, -4.0],
            ..Light::new([0.0; 3], [1.0; 3])
        });

        assert_eq!(raw.direction, [0.0, 0.0, -1.0]);
    }

    #[test]
    fn a_zero_direction_does_not_become_nan() {
        let raw = LightRaw::from(Light {
            direction: [0.0, 0.0, 0.0],
            ..Light::new([0.0; 3], [1.0; 3])
        });

        for component in raw.direction {
            assert!(component.is_finite(), "a zero direction must not produce NaN");
        }
    }

    #[test]
    fn a_default_light_is_an_unbounded_point_light() {
        let light = Light::new([1.0, 2.0, 3.0], [1.0, 0.0, 0.0]);

        assert_eq!(light.kind, LightKind::Point);
        assert_eq!(light.range, 0.0, "0 means no distance cutoff");
        assert_eq!(light.intensity, 1.0);
    }
}
