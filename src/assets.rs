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
    Texture::from_bytes(device, queue, data, file_name)
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
        let bind_group = diffuse_texture.bind_group(device, layout, &m.name);

        materials.push(model::Material {
            name: m.name,
            diffuse_texture,
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

            let vertices = (0..m.mesh.positions.len() / 3)
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
                    }
                })
                .collect::<Vec<_>>();

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
