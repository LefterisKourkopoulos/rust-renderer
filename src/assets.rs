use std::io::{BufReader, Cursor};

use anyhow::anyhow;
use wgpu::util::DeviceExt;

use crate::gfx::Texture;
use crate::scene::model;

const ASSETS: &[(&str, &[u8])] = &[
    ("cube.obj", include_bytes!("res/cube.obj")),
    ("cube.mtl", include_bytes!("res/cube.mtl")),
    ("cube-diffuse.jpg", include_bytes!("res/cube-diffuse.jpg")),
    ("cube-normal.png", include_bytes!("res/cube-normal.png")),
    ("happy-tree.png", include_bytes!("res/happy-tree.png")),
    ("centrica_logo.png", include_bytes!("res/centrica_logo.png")),
    ("pure-sky-hdri.jpg", include_bytes!("res/pure-sky-hdri.jpg")),
];

pub fn load_binary(file_name: &str) -> anyhow::Result<&'static [u8]> {
    ASSETS
        .iter()
        .find(|(name, _)| *name == file_name)
        .map(|(_, bytes)| *bytes)
        .ok_or_else(|| anyhow!("no embedded asset named {file_name}"))
}

pub fn load_string(file_name: &str) -> anyhow::Result<&'static str> {
    let bytes = load_binary(file_name)?;
    std::str::from_utf8(bytes).map_err(|e| anyhow!("embedded asset {file_name} is not UTF-8: {e}"))
}

pub fn load_texture(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<Texture> {
    let data = load_binary(file_name)?;
    Texture::from_bytes(device, queue, data, file_name, false)
}

pub fn load_normal_texture(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<Texture> {
    let data = load_binary(file_name)?;
    Texture::from_bytes(device, queue, data, file_name, true)
}

pub async fn load_model(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<model::Model> {
    let obj_text = load_string(file_name)?;
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
            match load_string(&p) {
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
        let bind_group =
            Texture::material_bind_group(device, layout, &diffuse_texture, &normal_texture, &m.name);

        materials.push(model::Material {
            name: m.name,
            diffuse_texture,
            normal_texture,
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

            let mut triangle_count = vec![0u32; vertices.len()];
            for chunk in m.mesh.indices.chunks_exact(3) {
                let [i0, i1, i2] = [chunk[0] as usize, chunk[1] as usize, chunk[2] as usize];
                let (pos0, pos1, pos2) = (
                    cgmath::Vector3::from(vertices[i0].position),
                    cgmath::Vector3::from(vertices[i1].position),
                    cgmath::Vector3::from(vertices[i2].position),
                );
                let (uv0, uv1, uv2) = (
                    cgmath::Vector2::from(vertices[i0].tex_coords),
                    cgmath::Vector2::from(vertices[i1].tex_coords),
                    cgmath::Vector2::from(vertices[i2].tex_coords),
                );

                let delta_pos1 = pos1 - pos0;
                let delta_pos2 = pos2 - pos0;
                let delta_uv1 = uv1 - uv0;
                let delta_uv2 = uv2 - uv0;

                let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
                let tangent = (delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r;
                let bitangent = (delta_pos2 * delta_uv1.x - delta_pos1 * delta_uv2.x) * r;

                for i in [i0, i1, i2] {
                    vertices[i].tangent =
                        (cgmath::Vector3::from(vertices[i].tangent) + tangent).into();
                    vertices[i].bitangent =
                        (cgmath::Vector3::from(vertices[i].bitangent) + bitangent).into();
                    triangle_count[i] += 1;
                }
            }

            for (vertex, &count) in vertices.iter_mut().zip(triangle_count.iter()) {
                if count > 0 {
                    vertex.tangent =
                        (cgmath::Vector3::from(vertex.tangent) / count as f32).into();
                    vertex.bitangent =
                        (cgmath::Vector3::from(vertex.bitangent) / count as f32).into();
                }
            }

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
            }
        })
        .collect::<Vec<_>>();

    Ok(model::Model { meshes, materials })
}
