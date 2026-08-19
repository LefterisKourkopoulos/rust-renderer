use std::collections::HashMap;

use anyhow::{Context, anyhow};
use cgmath::SquareMatrix;
use wgpu::util::DeviceExt;

use super::tangents;
use crate::gfx::Texture;
use crate::scene::instance::Instance;
use crate::scene::light::{Light, LightKind};
use crate::scene::model;

const FLAT_NORMAL: [u8; 4] = [128, 128, 255, 255];

pub fn load(
    bytes: &[u8],
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<model::Model> {
    let (document, buffers, images) = gltf::import_slice(bytes)
        .with_context(|| format!("{file_name} is not a readable glTF or GLB file"))?;

    let placements = placements(&document);
    let materials = load_materials(&document, &images, device, queue, layout, file_name)?;
    let (meshes, instances) = load_meshes(&document, &buffers, &placements, device, file_name)?;
    let lights = load_lights(&document, &placements);

    if meshes.is_empty() {
        return Err(anyhow!(
            "{file_name} contains no drawable geometry: no scene node references a mesh"
        ));
    }

    Ok(model::Model {
        meshes,
        materials,
        instances,
        lights,
    })
}

fn placements(document: &gltf::Document) -> HashMap<usize, cgmath::Matrix4<f32>> {
    let mut placements = HashMap::new();

    let Some(scene) = document.default_scene().or_else(|| document.scenes().next()) else {
        return placements;
    };

    let mut stack: Vec<_> = scene
        .nodes()
        .map(|node| (node, cgmath::Matrix4::identity()))
        .collect();

    while let Some((node, parent)) = stack.pop() {
        let world = parent * cgmath::Matrix4::from(node.transform().matrix());
        placements.insert(node.index(), world);

        for child in node.children() {
            stack.push((child, world));
        }
    }

    placements
}

fn instances_of(
    document: &gltf::Document,
    placements: &HashMap<usize, cgmath::Matrix4<f32>>,
    mesh_index: usize,
) -> Vec<cgmath::Matrix4<f32>> {
    document
        .nodes()
        .filter(|node| node.mesh().map(|mesh| mesh.index()) == Some(mesh_index))
        .filter_map(|node| placements.get(&node.index()).copied())
        .collect()
}

fn load_meshes(
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
    placements: &HashMap<usize, cgmath::Matrix4<f32>>,
    device: &wgpu::Device,
    file_name: &str,
) -> anyhow::Result<(Vec<model::Mesh>, Vec<Instance>)> {
    let mut meshes = Vec::new();
    let mut instances = Vec::new();

    for mesh in document.meshes() {
        let transforms = instances_of(document, placements, mesh.index());
        if transforms.is_empty() {
            continue;
        }

        let mesh_name = mesh.name().unwrap_or("mesh");

        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                log::warn!(
                    "{file_name}: skipping primitive {} of {mesh_name}, mode {:?} is not supported",
                    primitive.index(),
                    primitive.mode()
                );
                continue;
            }

            let name = format!("{mesh_name}#{}", primitive.index());
            let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|b| &b.0[..]));

            let positions: Vec<[f32; 3]> = reader
                .read_positions()
                .ok_or_else(|| anyhow!("{file_name}: primitive {name} has no POSITION attribute"))?
                .collect();

            let mut normals = reader
                .read_normals()
                .map(|n| n.collect::<Vec<_>>())
                .unwrap_or_default();

            normals.resize(positions.len(), [0.0, 0.0, 0.0]);

            let mut tex_coords = reader
                .read_tex_coords(0)
                .map(|t| t.into_f32().collect::<Vec<_>>())
                .unwrap_or_default();
            tex_coords.resize(positions.len(), [0.0, 0.0]);

            let tangents: Vec<[f32; 4]> = reader
                .read_tangents()
                .map(|t| t.collect())
                .unwrap_or_default();

            let indices: Vec<u32> = match reader.read_indices() {
                Some(indices) => indices.into_u32().collect(),
                // Non-indexed primitives are legal; the vertices are simply consumed in order.
                None => (0..positions.len() as u32).collect(),
            };

            let vertices = build_vertices(&positions, &normals, &tex_coords, &tangents, &indices);

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{name} Vertex Buffer")),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{name} Index Buffer")),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let start = instances.len() as u32;
            instances.extend(transforms.iter().copied().map(Instance::from_matrix));
            let end = instances.len() as u32;

            meshes.push(model::Mesh {
                name,
                vertex_buffer,
                index_buffer,
                num_elements: indices.len() as u32,
                material: primitive
                    .material()
                    .index()
                    .unwrap_or(document.materials().len()),
                instances: Some(start..end),
            });
        }
    }

    Ok((meshes, instances))
}

fn build_vertices(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    tex_coords: &[[f32; 2]],
    tangents: &[[f32; 4]],
    indices: &[u32],
) -> Vec<model::ModelVertex> {
    let mut vertices: Vec<model::ModelVertex> = positions
        .iter()
        .enumerate()
        .map(|(i, position)| model::ModelVertex {
            position: *position,
            tex_coords: tex_coords[i],
            normal: normals[i],
            tangent: [0.0, 0.0, 0.0],
            bitangent: [0.0, 0.0, 0.0],
        })
        .collect();

    if tangents.len() == vertices.len() {
        for (vertex, tangent) in vertices.iter_mut().zip(tangents) {
            let normal = cgmath::Vector3::from(vertex.normal);
            let t = cgmath::Vector3::new(tangent[0], tangent[1], tangent[2]);
            vertex.tangent = t.into();
            vertex.bitangent = (normal.cross(t) * tangent[3]).into();
        }
    } else {
        tangents::generate(&mut vertices, indices);
    }

    vertices
}

fn load_materials(
    document: &gltf::Document,
    images: &[gltf::image::Data],
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    file_name: &str,
) -> anyhow::Result<Vec<model::Material>> {
    let mut cache = TextureCache::default();
    let mut materials = Vec::with_capacity(document.materials().len() + 1);

    for material in document.materials() {
        let name = material
            .name()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("material {}", materials.len()));
        let pbr = material.pbr_metallic_roughness();

        let diffuse_texture = match pbr.base_color_texture() {
            Some(info) => cache.get(&info.texture(), true, images, device, queue, file_name)?,
            None => white(device, queue),
        };
        let normal_texture = match material.normal_texture() {
            Some(info) => cache.get(&info.texture(), false, images, device, queue, file_name)?,
            None => flat_normal(device, queue),
        };

        let emissive = material.emissive_factor();
        let strength = material.emissive_strength().unwrap_or(1.0);
        let uniform = model::MaterialUniform {
            base_color: pbr.base_color_factor(),
            emissive: [
                emissive[0] * strength,
                emissive[1] * strength,
                emissive[2] * strength,
            ],
            metallic: pbr.metallic_factor(),
            roughness: pbr.roughness_factor(),
            ..Default::default()
        };
        let uniform_buffer = uniform.buffer(device, &name);

        let bind_group = Texture::material_bind_group(
            device,
            layout,
            &diffuse_texture,
            &normal_texture,
            &uniform_buffer,
            &name,
        );

        materials.push(model::Material {
            name,
            diffuse_texture,
            normal_texture,
            uniform_buffer,
            bind_group,
        });
    }

    let diffuse_texture = white(device, queue);
    let normal_texture = flat_normal(device, queue);
    let uniform_buffer = model::MaterialUniform::default().buffer(device, "gltf_default");
    let bind_group = Texture::material_bind_group(
        device,
        layout,
        &diffuse_texture,
        &normal_texture,
        &uniform_buffer,
        "gltf_default",
    );
    materials.push(model::Material {
        name: String::from("gltf_default"),
        diffuse_texture,
        normal_texture,
        uniform_buffer,
        bind_group,
    });

    Ok(materials)
}

fn white(device: &wgpu::Device, queue: &wgpu::Queue) -> Texture {
    Texture::from_color(device, queue, [255, 255, 255, 255], false, "gltf_white")
}

fn flat_normal(device: &wgpu::Device, queue: &wgpu::Queue) -> Texture {
    Texture::from_color(device, queue, FLAT_NORMAL, true, "gltf_flat_normal")
}

#[derive(Default)]
struct TextureCache {
    textures: HashMap<(usize, bool), Texture>,
}

impl TextureCache {
    fn get(
        &mut self,
        texture: &gltf::Texture,
        srgb: bool,
        images: &[gltf::image::Data],
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        file_name: &str,
    ) -> anyhow::Result<Texture> {
        let index = texture.source().index();

        if let Some(cached) = self.textures.get(&(index, srgb)) {
            return Ok(cached.clone());
        }

        let image = images.get(index).ok_or_else(|| {
            anyhow!("{file_name}: texture refers to image {index}, which the file does not contain")
        })?;

        let pixels = to_rgba8(image).ok_or_else(|| {
            anyhow!(
                "{file_name}: image {index} has unsupported pixel format {:?}",
                image.format
            )
        })?;

        let address_mode = address_mode(texture.sampler().wrap_s());
        let label = format!(
            "{file_name} image {index}{}",
            if srgb { "" } else { " (linear)" }
        );

        let loaded = Texture::from_rgba8(
            device,
            queue,
            &pixels,
            image.width,
            image.height,
            srgb,
            address_mode,
            Some(&label),
        )?;

        Ok(self.textures.entry((index, srgb)).or_insert(loaded).clone())
    }
}

fn address_mode(mode: gltf::texture::WrappingMode) -> wgpu::AddressMode {
    match mode {
        gltf::texture::WrappingMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        gltf::texture::WrappingMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
        gltf::texture::WrappingMode::Repeat => wgpu::AddressMode::Repeat,
    }
}

fn to_rgba8(image: &gltf::image::Data) -> Option<Vec<u8>> {
    use gltf::image::Format;

    let pixel_count = image.width as usize * image.height as usize;

    let expand = |channels: usize, fill: [u8; 4]| {
        let mut out = Vec::with_capacity(pixel_count * 4);
        for pixel in image.pixels.chunks_exact(channels) {
            let mut rgba = fill;
            rgba[..channels].copy_from_slice(pixel);
            out.extend_from_slice(&rgba);
        }
        out
    };

    match image.format {
        Format::R8G8B8A8 => Some(image.pixels.clone()),
        Format::R8G8B8 => Some(expand(3, [0, 0, 0, 255])),
        Format::R8G8 => Some(expand(2, [0, 0, 0, 255])),
        Format::R8 => Some(expand(1, [0, 0, 0, 255])),
        _ => None,
    }
}

/// The lights the file declares, placed and oriented by the nodes that reference them.
fn load_lights(
    document: &gltf::Document,
    placements: &HashMap<usize, cgmath::Matrix4<f32>>,
) -> Vec<Light> {
    let mut lights = Vec::new();

    for node in document.nodes() {
        let Some(light) = node.light() else {
            continue;
        };
        let Some(world) = placements.get(&node.index()) else {
            // Outside the active scene, so it does not light it.
            continue;
        };

        let kind = match light.kind() {
            gltf::khr_lights_punctual::Kind::Directional => LightKind::Directional,
            gltf::khr_lights_punctual::Kind::Point => LightKind::Point,
            gltf::khr_lights_punctual::Kind::Spot {
                inner_cone_angle,
                outer_cone_angle,
            } => LightKind::Spot {
                inner_cone_angle,
                outer_cone_angle,
            },
        };

        let direction = (world * cgmath::Vector4::new(0.0, 0.0, -1.0, 0.0)).truncate();

        lights.push(Light {
            position: world.w.truncate().into(),
            direction: direction.into(),
            color: light.color(),
            intensity: light.intensity(),
            range: light.range().unwrap_or(0.0),
            kind,
        });
    }

    lights
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diorama() -> gltf::Document {
        let bytes = super::super::embedded("cube_diorama.glb").expect("the diorama is embedded");
        let (document, _, _) = gltf::import_slice(bytes).expect("the diorama parses");
        document
    }

    #[test]
    fn glt_f_uvs_reach_the_vertex_buffer_unflipped() {
        let vertices = build_vertices(
            &[[0.0; 3]; 3],
            &[[0.0, 0.0, 1.0]; 3],
            &[[0.25, 0.75], [0.0, 1.0], [1.0, 0.0]],
            &[],
            &[0, 1, 2],
        );

        assert_eq!(
            vertices.iter().map(|v| v.tex_coords).collect::<Vec<_>>(),
            vec![[0.25, 0.75], [0.0, 1.0], [1.0, 0.0]],
            "glTF V must be passed through, not flipped to 1.0 - v"
        );
    }

    #[test]
    fn a_supplied_tangent_basis_keeps_the_handedness_from_its_w_component() {
        let normals = [[0.0, 0.0, 1.0]; 2];
        let vertices = build_vertices(
            &[[0.0; 3]; 2],
            &normals,
            &[[0.0, 0.0]; 2],
            &[[1.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0, -1.0]],
            &[0, 1, 0],
        );

        assert_eq!(vertices[0].tangent, [1.0, 0.0, 0.0]);
        assert_eq!(vertices[0].bitangent, [0.0, 1.0, 0.0]);
        assert_eq!(
            vertices[1].bitangent,
            [0.0, -1.0, 0.0],
            "a W of -1 must mirror the bitangent"
        );
    }

    #[test]
    fn a_primitive_without_tangents_still_gets_a_basis() {
        let vertices = build_vertices(
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            &[[0.0, 0.0, 1.0]; 3],
            &[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            &[],
            &[0, 1, 2],
        );

        for vertex in &vertices {
            let length = cgmath::InnerSpace::magnitude(cgmath::Vector3::from(vertex.tangent));
            assert!(
                length > 1e-6,
                "tangent {:?} is degenerate, so the TBN matrix would collapse",
                vertex.tangent
            );
        }
    }

    #[test]
    fn every_scene_node_gets_a_world_transform() {
        let document = diorama();
        let placements = placements(&document);
        let scene = document.default_scene().expect("the diorama has a scene");

        assert_eq!(placements.len(), 56);
        for node in scene.nodes() {
            assert!(
                placements.contains_key(&node.index()),
                "root node {} was not placed",
                node.index()
            );
        }
    }

    #[test]
    fn child_transforms_are_composed_with_their_parent() {
        let document = diorama();
        let placements = placements(&document);

        let parent = document
            .nodes()
            .find(|node| node.children().count() > 0)
            .expect("the diorama has one node with children");
        let parent_world = placements[&parent.index()];

        for child in parent.children() {
            let expected = parent_world * cgmath::Matrix4::from(child.transform().matrix());
            let actual = placements[&child.index()];

            let actual: [[f32; 4]; 4] = actual.into();
            let expected: [[f32; 4]; 4] = expected.into();
            for (actual_row, expected_row) in actual.iter().zip(expected.iter()) {
                for (a, b) in actual_row.iter().zip(expected_row.iter()) {
                    assert!(
                        (a - b).abs() < 1e-6,
                        "child {} world transform is not parent * local",
                        child.index()
                    );
                }
            }
        }
    }

    #[test]
    fn every_mesh_the_scene_references_is_placed_at_least_once() {
        let document = diorama();
        let placements = placements(&document);

        let placed = document
            .meshes()
            .filter(|mesh| !instances_of(&document, &placements, mesh.index()).is_empty())
            .count();

        assert_eq!(placed, 50, "all 50 diorama meshes should be placed");
    }

    #[test]
    fn rgb_images_are_expanded_to_opaque_rgba() {
        let image = gltf::image::Data {
            pixels: vec![10, 20, 30, 40, 50, 60],
            format: gltf::image::Format::R8G8B8,
            width: 2,
            height: 1,
        };

        assert_eq!(
            to_rgba8(&image).expect("RGB is supported"),
            vec![10, 20, 30, 255, 40, 50, 60, 255]
        );
    }

    #[test]
    fn sixteen_bit_images_are_rejected_rather_than_misread() {
        let image = gltf::image::Data {
            pixels: vec![0; 8],
            format: gltf::image::Format::R16G16B16A16,
            width: 1,
            height: 1,
        };

        assert!(to_rgba8(&image).is_none());
    }

    #[test]
    fn the_diorama_declares_no_lights_so_the_scene_must_supply_them() {
        let document = diorama();
        let placements = placements(&document);
        assert!(load_lights(&document, &placements).is_empty());
    }
}
