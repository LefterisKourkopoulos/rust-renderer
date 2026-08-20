mod gltf_loader;
mod obj;
mod tangents;

use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

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

pub fn embedded(file_name: &str) -> anyhow::Result<&'static [u8]> {
    ASSETS
        .iter()
        .find(|(name, _)| *name == file_name)
        .map(|(_, bytes)| *bytes)
        .ok_or_else(|| anyhow!("no embedded asset named {file_name}"))
}

pub fn embedded_string(file_name: &str) -> anyhow::Result<&'static str> {
    let bytes = embedded(file_name)?;
    std::str::from_utf8(bytes).map_err(|e| anyhow!("embedded asset {file_name} is not UTF-8: {e}"))
}

pub enum Source {
    Disk(Vec<u8>),
    Embedded(&'static [u8]),
}

impl Source {
    pub fn bytes(&self) -> &[u8] {
        match self {
            Source::Disk(bytes) => bytes,
            Source::Embedded(bytes) => bytes,
        }
    }

    pub fn is_disk(&self) -> bool {
        matches!(self, Source::Disk(_))
    }
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let origin = if self.is_disk() { "disk" } else { "embedded" };
        write!(f, "Source::{origin}({} bytes)", self.bytes().len())
    }
}

pub fn resolve(base_dir: Option<&Path>, path: &str) -> anyhow::Result<Source> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let candidate = match base_dir {
            Some(dir) => dir.join(path),
            None => PathBuf::from(path),
        };

        match std::fs::read(&candidate) {
            Ok(bytes) => return Ok(Source::Disk(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(anyhow!("cannot read {}: {e}", candidate.display()));
            }
        }
    }

    let name = file_name(path);
    match embedded(name) {
        Ok(bytes) => Ok(Source::Embedded(bytes)),
        Err(_) => Err(match base_dir {
            Some(dir) => anyhow!(
                "cannot find {path}: it is not at {} and there is no embedded asset named {name}",
                dir.join(path).display()
            ),
            None => anyhow!("cannot find {path}: no such file and no embedded asset named {name}"),
        }),
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

pub fn load_texture(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<Texture> {
    let data = embedded(file_name)?;
    Texture::from_bytes(device, queue, data, file_name, false)
}

pub fn load_normal_texture(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<Texture> {
    let data = embedded(file_name)?;
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

/// Loads a glTF/GLB model directly from bytes, bypassing disk/embedded resolution entirely.
///
/// This is the seam a browser upload (or any other caller that already has the bytes in memory,
/// with no path to resolve) plugs into.
pub fn load_glb(
    bytes: &[u8],
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<model::Model> {
    gltf_loader::load(bytes, file_name, device, queue, layout)
}

pub async fn load_model(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<model::Model> {
    load_model_from(None, file_name, device, queue, layout).await
}

pub async fn load_model_from(
    base_dir: Option<&Path>,
    path: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<model::Model> {
    let extension = extension(path);

    match extension.as_deref() {
        Some("obj") | Some("gltf") | Some("glb") => {}
        Some(other) => {
            return Err(anyhow!(
                "unsupported model format {other:?} for {path}; expected obj, gltf or glb"
            ));
        }
        None => {
            return Err(anyhow!(
                "cannot determine the model format of {path}: it has no file extension"
            ));
        }
    }

    let source = resolve(base_dir, path)?;
    let bytes = source.bytes();

    match extension.as_deref() {
        Some("obj") if source.is_disk() => Err(anyhow!(
            "{path} is an OBJ on disk, which is not supported: its .mtl and textures are \
             resolved against the embedded assets, not its own directory. Use a .glb instead."
        )),
        Some("obj") => obj::load(bytes, path, device, queue, layout).await,
        _ => gltf_loader::load(bytes, path, device, queue, layout),
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
        let bytes = embedded("cube_diorama.glb").expect("the diorama is embedded");
        assert!(
            bytes.starts_with(b"glTF"),
            "a .glb must start with the glTF magic"
        );
    }

    #[test]
    fn file_name_takes_the_last_path_component() {
        assert_eq!(file_name("models/scene/cube.glb"), "cube.glb");
        assert_eq!(file_name("cube.glb"), "cube.glb");
    }

    #[test]
    fn resolving_an_embedded_name_does_not_touch_the_disk() {
        let source = resolve(None, "cube_diorama.glb").expect("the diorama is embedded");

        assert!(
            !source.is_disk(),
            "an embedded asset must resolve without a filesystem read"
        );
        assert!(source.bytes().starts_with(b"glTF"));
    }

    #[test]
    fn a_path_still_finds_the_asset_embedded_under_its_bare_name() {
        let source = resolve(None, "models/cube_diorama.glb")
            .expect("the bare file name is in the embedded table");

        assert!(source.bytes().starts_with(b"glTF"));
    }

    #[test]
    fn a_name_that_is_neither_on_disk_nor_embedded_is_reported_with_both_places_tried() {
        let error = resolve(Some(Path::new("/tmp/scenes")), "nope.glb")
            .expect_err("nothing named nope.glb exists");
        let message = error.to_string();

        assert!(
            message.contains("/tmp/scenes/nope.glb") && message.contains("nope.glb"),
            "the error should name the path it tried, got: {message}"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_file_on_disk_wins_over_an_embedded_asset_of_the_same_name() {
        let dir = std::env::temp_dir().join("rust-renderer-resolve-test");
        std::fs::create_dir_all(&dir).expect("create the temp dir");
        let path = dir.join("cube_diorama.glb");
        std::fs::write(&path, b"glTF from disk").expect("write the stand-in file");

        let source = resolve(Some(&dir), "cube_diorama.glb").expect("the file is on disk");

        assert!(source.is_disk(), "a real file must take priority");
        assert_eq!(source.bytes(), b"glTF from disk");

        std::fs::remove_file(&path).ok();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_relative_model_path_is_resolved_against_the_base_directory() {
        let dir = std::env::temp_dir().join("rust-renderer-resolve-nested");
        std::fs::create_dir_all(dir.join("models")).expect("create the nested temp dirs");
        let path = dir.join("models/local.glb");
        std::fs::write(&path, b"glTF nested").expect("write the stand-in file");

        let source = resolve(Some(&dir), "models/local.glb").expect("the nested file is on disk");

        assert_eq!(
            source.bytes(),
            b"glTF nested",
            "a relative path must be joined onto the base directory"
        );

        std::fs::remove_file(&path).ok();
    }
}
