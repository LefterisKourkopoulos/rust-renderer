use std::ops::Range;

use cgmath::InnerSpace;

use super::instance::Instance;
use super::light::Light;
use crate::gfx::{Texture, Vertex};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelVertex {
    pub position: [f32; 3],
    pub tex_coords: [f32; 2],
    pub normal: [f32; 3],
    pub tangent: [f32; 3],
    pub bitangent: [f32; 3],
}

impl Vertex for ModelVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<ModelVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 11]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    pub instances: Vec<Instance>,
    pub lights: Vec<Light>,
    pub bounds: Bounds,
}

/// An axis-aligned world-space bounding box, used to frame the camera on a freshly loaded model
/// regardless of the scale or origin it was authored at.
#[derive(Copy, Clone, Debug)]
pub struct Bounds {
    pub min: cgmath::Point3<f32>,
    pub max: cgmath::Point3<f32>,
}

impl Bounds {
    pub fn empty() -> Self {
        Self {
            min: cgmath::Point3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY),
            max: cgmath::Point3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
        }
    }

    pub fn include(&mut self, point: cgmath::Point3<f32>) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.min.z = self.min.z.min(point.z);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
        self.max.z = self.max.z.max(point.z);
    }

    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x
    }

    pub fn corners(&self) -> [cgmath::Point3<f32>; 8] {
        [
            cgmath::Point3::new(self.min.x, self.min.y, self.min.z),
            cgmath::Point3::new(self.max.x, self.min.y, self.min.z),
            cgmath::Point3::new(self.min.x, self.max.y, self.min.z),
            cgmath::Point3::new(self.max.x, self.max.y, self.min.z),
            cgmath::Point3::new(self.min.x, self.min.y, self.max.z),
            cgmath::Point3::new(self.max.x, self.min.y, self.max.z),
            cgmath::Point3::new(self.min.x, self.max.y, self.max.z),
            cgmath::Point3::new(self.max.x, self.max.y, self.max.z),
        ]
    }

    pub fn center(&self) -> cgmath::Point3<f32> {
        cgmath::Point3::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
            (self.min.z + self.max.z) * 0.5,
        )
    }

    /// Half the length of the box's diagonal: the radius of a sphere that contains it.
    pub fn radius(&self) -> f32 {
        (self.max - self.min).magnitude() * 0.5
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniform {
    pub base_color: [f32; 4],
    pub emissive: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
    pub _padding: [f32; 3],
}

impl Default for MaterialUniform {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            emissive: [0.0, 0.0, 0.0],
            metallic: 0.0,
            roughness: 1.0,
            _padding: [0.0; 3],
        }
    }
}

impl MaterialUniform {
    pub fn buffer(&self, device: &wgpu::Device, label: &str) -> wgpu::Buffer {
        use wgpu::util::DeviceExt;

        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label} Material Uniform")),
            contents: bytemuck::bytes_of(self),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        })
    }
}

pub struct Material {
    pub name: String,
    #[allow(dead_code)]
    pub diffuse_texture: Texture,
    #[allow(dead_code)]
    pub normal_texture: Texture,
    #[allow(dead_code)]
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}

pub struct Mesh {
    pub name: String,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
    pub material: usize,
    pub instances: Option<Range<u32>>,
}

pub trait DrawModel<'a> {
    #[allow(dead_code)]
    fn draw_mesh(
        &mut self,
        mesh: &'a Mesh,
        material: &'a Material,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
        shadow_bind_group: &'a wgpu::BindGroup,
    );

    fn draw_mesh_instanced(
        &mut self,
        mesh: &'a Mesh,
        material: &'a Material,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        diffuse_override: Option<&'a wgpu::BindGroup>,
        light_bind_group: &'a wgpu::BindGroup,
        shadow_bind_group: &'a wgpu::BindGroup,
    );

    fn draw_model(
        &mut self,
        model: &'a Model,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
        shadow_bind_group: &'a wgpu::BindGroup,
    );

    fn draw_model_instanced(
        &mut self,
        model: &'a Model,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
        diffuse_override: Option<&'a wgpu::BindGroup>,
        light_bind_group: &'a wgpu::BindGroup,
        shadow_bind_group: &'a wgpu::BindGroup,
    );

    fn draw_model_depth(&mut self, model: &'a Model, instances: Range<u32>);
}

impl<'a, 'b> DrawModel<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_mesh(
        &mut self,
        mesh: &'b Mesh,
        material: &'b Material,
        camera_bind_group: &'b wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
        shadow_bind_group: &'a wgpu::BindGroup,
    ) {
        self.draw_mesh_instanced(
            mesh,
            material,
            0..1,
            camera_bind_group,
            None,
            light_bind_group,
            shadow_bind_group,
        );
    }

    fn draw_mesh_instanced(
        &mut self,
        mesh: &'b Mesh,
        material: &'b Material,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        diffuse_override: Option<&'b wgpu::BindGroup>,
        light_bind_group: &'a wgpu::BindGroup,
        shadow_bind_group: &'a wgpu::BindGroup,
    ) {
        self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.set_bind_group(0, diffuse_override.unwrap_or(&material.bind_group), &[]);
        self.set_bind_group(1, camera_bind_group, &[]);
        self.set_bind_group(2, light_bind_group, &[]);
        self.set_bind_group(3, shadow_bind_group, &[]);
        self.draw_indexed(0..mesh.num_elements, 0, instances);
    }

    fn draw_model(
        &mut self,
        model: &'a Model,
        camera_bind_group: &'a wgpu::BindGroup,
        light_bind_group: &'a wgpu::BindGroup,
        shadow_bind_group: &'a wgpu::BindGroup,
    ) {
        self.draw_model_instanced(
            model,
            0..1,
            camera_bind_group,
            None,
            light_bind_group,
            shadow_bind_group,
        );
    }

    fn draw_model_instanced(
        &mut self,
        model: &'b Model,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
        diffuse_override: Option<&'b wgpu::BindGroup>,
        light_bind_group: &'a wgpu::BindGroup,
        shadow_bind_group: &'a wgpu::BindGroup,
    ) {
        for mesh in &model.meshes {
            let material = &model.materials[mesh.material];
            let range = mesh.instances.clone().unwrap_or_else(|| instances.clone());
            self.draw_mesh_instanced(
                mesh,
                material,
                range,
                camera_bind_group,
                diffuse_override,
                light_bind_group,
                shadow_bind_group,
            );
        }
    }

    fn draw_model_depth(&mut self, model: &'b Model, instances: Range<u32>) {
        for mesh in &model.meshes {
            let range = mesh.instances.clone().unwrap_or_else(|| instances.clone());
            self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            self.draw_indexed(0..mesh.num_elements, 0, range);
        }
    }
}

#[cfg(test)]
mod material_uniform_tests {
    use super::MaterialUniform;

    #[test]
    fn matches_the_shader_struct_layout() {
        assert_eq!(std::mem::size_of::<MaterialUniform>(), 48, "struct size");
        assert_eq!(std::mem::offset_of!(MaterialUniform, base_color), 0);
        assert_eq!(std::mem::offset_of!(MaterialUniform, emissive), 16);
        assert_eq!(std::mem::offset_of!(MaterialUniform, metallic), 28);
        assert_eq!(std::mem::offset_of!(MaterialUniform, roughness), 32);
    }

    #[test]
    fn the_default_is_a_neutral_multiplier() {
        let default = MaterialUniform::default();
        assert_eq!(default.base_color, [1.0; 4]);
        assert_eq!(default.emissive, [0.0; 3]);
    }
}
