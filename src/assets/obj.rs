use std::io::{BufReader, Cursor};

use wgpu::util::DeviceExt;

use anyhow::anyhow;

use super::tangents;
use super::{embedded_string, load_normal_texture, load_texture};
use crate::gfx::Texture;
use crate::scene::model;

/// Loads an OBJ from `bytes`.
///
/// Its `.mtl` and texture references are resolved against the embedded asset table, not the
/// directory the OBJ came from, so only embedded OBJs load successfully. Hot reloading is
/// `.glb` only for that reason.
pub async fn load(
    bytes: &[u8],
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<model::Model> {
    let obj_text = std::str::from_utf8(bytes)
        .map_err(|e| anyhow!("{file_name} is not valid UTF-8, so it is not a readable OBJ: {e}"))?;
    let obj_cursor = Cursor::new(obj_text);
    let mut obj_reader = BufReader::new(obj_cursor);

    let (models, obj_materials) = tobj::load_obj_buf_async(
        &mut obj_reader,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |p| async move {
            match embedded_string(&p) {
                Ok(mat_text) => tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text))),
                Err(_) => Err(tobj::LoadError::OpenFileFailed),
            }
        },
    )
    .await?;

    // Load materials
    let mut materials = Vec::new();
    for m in obj_materials? {
        let diffuse_texture = load_texture(&m.diffuse_texture, device, queue)?;
        let normal_texture = if m.normal_texture.is_empty() {
            Texture::from_color(device, queue, [128, 128, 255, 255], true, "default_normal")
        } else {
            load_normal_texture(&m.normal_texture, device, queue)?
        };

        let uniform = model::MaterialUniform {
            base_color: [m.diffuse[0], m.diffuse[1], m.diffuse[2], m.dissolve],
            roughness: 1.0 - (m.shininess / 1000.0).clamp(0.0, 1.0),
            ..Default::default()
        };
        let uniform_buffer = uniform.buffer(device, &m.name);

        let bind_group = Texture::material_bind_group(
            device,
            layout,
            &diffuse_texture,
            &normal_texture,
            &uniform_buffer,
            &m.name,
        );

        materials.push(model::Material {
            name: m.name,
            diffuse_texture,
            normal_texture,
            uniform_buffer,
            bind_group,
        })
    }

    // Load meshes
    let meshes = models
        .into_iter()
        .enumerate()
        .map(|(mesh_index, m)| {
            let name = if m.name.is_empty() {
                format!("{file_name}#{mesh_index}")
            } else {
                m.name.clone()
            };

            let mut vertices = (0..m.mesh.positions.len() / 3)
                .map(|i| {
                    let tex_coords = if m.mesh.texcoords.is_empty() {
                        [0.0, 0.0]
                    } else {
                        [m.mesh.texcoords[i * 2], 1.0 - m.mesh.texcoords[i * 2 + 1]]
                    };
                    let normal = if m.mesh.normals.is_empty() {
                        [0.0, 0.0, 0.0]
                    } else {
                        [
                            m.mesh.normals[i * 3],
                            m.mesh.normals[i * 3 + 1],
                            m.mesh.normals[i * 3 + 2],
                        ]
                    };

                    model::ModelVertex {
                        position: [
                            m.mesh.positions[i * 3],
                            m.mesh.positions[i * 3 + 1],
                            m.mesh.positions[i * 3 + 2],
                        ],
                        tex_coords,
                        normal,
                        tangent: [0.0, 0.0, 0.0],
                        bitangent: [0.0, 0.0, 0.0],
                    }
                })
                .collect::<Vec<_>>();

            tangents::generate(&mut vertices, &m.mesh.indices);

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{name} Vertex Buffer")),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{name} Index Buffer")),
                contents: bytemuck::cast_slice(&m.mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            model::Mesh {
                name,
                vertex_buffer,
                index_buffer,
                num_elements: m.mesh.indices.len() as u32,
                material: m.mesh.material_id.unwrap_or(0),
                instances: None,
            }
        })
        .collect::<Vec<_>>();

    Ok(model::Model {
        meshes,
        materials,
        instances: Vec::new(),
        lights: Vec::new(),
    })
}
