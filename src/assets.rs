mod gltf_loader;
mod obj;
mod tangents;

use anyhow::anyhow;

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
    ("cube_diorama.glb", include_bytes!("res/cube_diorama.glb")),
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

/// The lowercased extension of `file_name`, or `None` if it has none.
fn extension(file_name: &str) -> Option<String> {
    let (_, extension) = file_name.rsplit_once('.')?;
    if extension.is_empty() {
        return None;
    }
    Some(extension.to_ascii_lowercase())
}

pub async fn load_model(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<model::Model> {
    match extension(file_name).as_deref() {
        Some("obj") => obj::load(file_name, device, queue, layout).await,
        Some("gltf") | Some("glb") => gltf_loader::load(file_name, device, queue, layout),
        Some(other) => Err(anyhow!(
            "unsupported model format {other:?} for {file_name}; expected obj, gltf or glb"
        )),
        None => Err(anyhow!(
            "cannot determine the model format of {file_name}: it has no file extension"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_is_lowercased() {
        assert_eq!(extension("cube.OBJ").as_deref(), Some("obj"));
        assert_eq!(extension("scene.GlB").as_deref(), Some("glb"));
    }

    #[test]
    fn extension_uses_the_last_dot() {
        assert_eq!(extension("archive.tar.gltf").as_deref(), Some("gltf"));
        assert_eq!(extension("res/some.dir/cube.obj").as_deref(), Some("obj"));
    }

    #[test]
    fn extension_is_none_when_absent() {
        assert_eq!(extension("cube"), None);
        assert_eq!(extension("cube."), None);
    }

    #[test]
    fn every_embedded_asset_name_is_unique() {
        for (index, (name, _)) in ASSETS.iter().enumerate() {
            let duplicate = ASSETS
                .iter()
                .skip(index + 1)
                .any(|(other, _)| other == name);
            assert!(!duplicate, "{name} is listed twice in ASSETS");
        }
    }

    #[test]
    fn the_diorama_is_embedded_and_is_a_binary_gltf() {
        let bytes = load_binary("cube_diorama.glb").expect("the diorama is embedded");
        assert!(
            bytes.starts_with(b"glTF"),
            "a .glb must start with the glTF magic"
        );
    }
}
