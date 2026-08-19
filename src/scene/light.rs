use cgmath::Rotation3;
use wgpu::util::DeviceExt;

use super::model::ModelVertex;

#[derive(Copy, Clone)]
pub struct Light {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

impl Light {
    pub fn new(position: [f32; 3], color: [f32; 3]) -> Self {
        Self { position, color }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct LightRaw {
    position: [f32; 3],
    _padding: u32,
    color: [f32; 3],
    _padding2: u32,
}

impl From<Light> for LightRaw {
    fn from(light: Light) -> Self {
        Self {
            position: light.position,
            _padding: 0,
            color: light.color,
            _padding2: 0,
        }
    }
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
    buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

impl LightCollection {
    pub fn new(device: &wgpu::Device, lights: Vec<Light>) -> Self {
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

    pub fn add(&mut self, device: &wgpu::Device, light: Light) {
        self.lights.push(light);
        self.buffer = Self::create_buffer(device, &self.lights);
        self.bind_group = Self::create_bind_group(device, &self.bind_group_layout, &self.buffer);
    }

    pub fn update(&mut self, queue: &wgpu::Queue, dt: f32) {
        for light in &mut self.lights {
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
        let raw: Vec<LightRaw> = lights.iter().copied().map(LightRaw::from).collect();
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
