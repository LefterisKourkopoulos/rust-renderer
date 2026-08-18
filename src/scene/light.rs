use cgmath::Rotation3;
use wgpu::util::DeviceExt;

use super::model::ModelVertex;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct LightUniform {
    position: [f32; 3],
    _padding: u32,
    color: [f32; 3],
    _padding2: u32,
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

pub struct Light {
    position: [f32; 3],
    color: [f32; 3],
    buffer: wgpu::Buffer,
    uniform: LightUniform,
    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
}

impl Light {
    pub fn new(
        device: &wgpu::Device,
        position: [f32; 3],
        color: [f32; 3]
    ) -> Self {
        let mut uniform = LightUniform {
            position: position,
            _padding: 0,
            color: color,
            _padding2: 0,
        };

        let buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Light VB"),
                contents: bytemuck::cast_slice(&[uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: None,
            });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: None,
        });

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
            position,
            color,
            buffer,
            uniform,
            bind_group,
            bind_group_layout,
            vertex_buffer,
            index_buffer,
            num_indices: CUBE_INDICES.len() as u32,
        }
    }

    pub fn update(&mut self, queue: &wgpu::Queue, dt: f32) {
        let old_position: cgmath::Vector3<f32> = self.uniform.position.into();
        self.uniform.position =
            (cgmath::Quaternion::from_axis_angle((0.0, 1.0, 0.0).into(), cgmath::Deg(60.0 * dt))
                * old_position)
                .into();
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.uniform]));
    }
}