//! The on-disk scene description and its translation into a [`SceneConfig`].
//!
//! Reading a scene from a file is what makes hot reloading possible at all: the configuration
//! used to be hardcoded Rust, so changing the scene meant a rebuild. The format is native only,
//! since wasm has no filesystem to read it from.

use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};
use serde::Deserialize;

use crate::config::{CameraConfig, InstanceGridConfig, SceneConfig, SunConfig};

/// The scene description as written on disk.
///
/// Every field is optional and falls back to the matching [`SceneConfig`] default, so a usable
/// scene file can be a single `model = "..."` line. Unknown keys are rejected rather than
/// ignored: silently skipping a typo would look exactly like a save that changed nothing.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneFile {
    model: Option<String>,
    light_intensity_scale: Option<f32>,
    camera: Option<Camera>,
    sun: Option<Sun>,
    grid: Option<Grid>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Camera {
    position: Option<[f32; 3]>,
    yaw: Option<f32>,
    pitch: Option<f32>,
    fovy: Option<f32>,
    znear: Option<f32>,
    zfar: Option<f32>,
    speed: Option<f32>,
    sensitivity: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sun {
    direction: Option<[f32; 3]>,
    color: Option<[f32; 3]>,
    intensity: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Grid {
    instances_per_row: Option<u32>,
    space_between: Option<f32>,
}

/// Reads and parses the scene file at `path`.
///
/// Relative asset paths inside it are resolved against its own directory, so a scene file and the
/// `.glb` beside it can be moved around together.
pub fn load(path: &Path) -> anyhow::Result<SceneConfig> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read the scene file {}", path.display()))?;

    // A scene file directly in the current directory has no parent component, so fall back to
    // "." rather than to "no base directory at all", which would skip the disk entirely.
    let base_dir = match path.parent() {
        Some(parent) if parent.as_os_str().is_empty() => PathBuf::from("."),
        Some(parent) => parent.to_path_buf(),
        None => PathBuf::from("."),
    };

    parse(&text, Some(base_dir)).with_context(|| format!("in scene file {}", path.display()))
}

/// Parses a scene description, resolving its relative paths against `base_dir`.
pub fn parse(text: &str, base_dir: Option<PathBuf>) -> anyhow::Result<SceneConfig> {
    let file: SceneFile = toml::from_str(text)?;
    file.into_config(base_dir)
}

impl SceneFile {
    fn into_config(self, base_dir: Option<PathBuf>) -> anyhow::Result<SceneConfig> {
        let defaults = SceneConfig::default();

        let config = SceneConfig {
            model_file: self.model.unwrap_or(defaults.model_file),
            base_dir,
            light_intensity_scale: self
                .light_intensity_scale
                .unwrap_or(defaults.light_intensity_scale),
            camera: match self.camera {
                Some(camera) => camera.into_config(defaults.camera),
                None => defaults.camera,
            },
            sun: match self.sun {
                Some(sun) => sun.into_config(defaults.sun),
                None => defaults.sun,
            },
            grid: match self.grid {
                Some(grid) => grid.into_config(defaults.grid),
                None => defaults.grid,
            },
        };

        validate(&config)?;
        Ok(config)
    }
}

impl Camera {
    fn into_config(self, defaults: CameraConfig) -> CameraConfig {
        CameraConfig {
            position: self.position.unwrap_or(defaults.position),
            yaw: self.yaw.unwrap_or(defaults.yaw),
            pitch: self.pitch.unwrap_or(defaults.pitch),
            fovy: self.fovy.unwrap_or(defaults.fovy),
            znear: self.znear.unwrap_or(defaults.znear),
            zfar: self.zfar.unwrap_or(defaults.zfar),
            speed: self.speed.unwrap_or(defaults.speed),
            sensitivity: self.sensitivity.unwrap_or(defaults.sensitivity),
        }
    }
}

impl Sun {
    fn into_config(self, defaults: SunConfig) -> SunConfig {
        SunConfig {
            direction: self.direction.unwrap_or(defaults.direction),
            color: self.color.unwrap_or(defaults.color),
            intensity: self.intensity.unwrap_or(defaults.intensity),
        }
    }
}

impl Grid {
    fn into_config(self, defaults: InstanceGridConfig) -> InstanceGridConfig {
        InstanceGridConfig {
            instances_per_row: self.instances_per_row.unwrap_or(defaults.instances_per_row),
            space_between: self.space_between.unwrap_or(defaults.space_between),
        }
    }
}

/// Rejects the values that would otherwise fail deep inside the renderer, where the message no
/// longer points back at the line of the scene file that caused it.
///
/// Worth being strict about on a hot reload path: a rejected file leaves the previous scene on
/// screen with an explanation, whereas a NaN or an empty draw call just looks broken.
fn validate(config: &SceneConfig) -> anyhow::Result<()> {
    if config.model_file.trim().is_empty() {
        return Err(anyhow!("model must name a file"));
    }

    // Each of these checks the *positive* condition and negates the whole thing, so a NaN — which
    // compares false against everything — is rejected rather than slipping through.
    let camera = &config.camera;
    if !(camera.znear.is_finite() && camera.znear > 0.0) {
        return Err(anyhow!(
            "camera.znear must be greater than zero, got {}",
            camera.znear
        ));
    }
    if !(camera.zfar.is_finite() && camera.zfar > camera.znear) {
        return Err(anyhow!(
            "camera.zfar ({}) must be greater than camera.znear ({})",
            camera.zfar,
            camera.znear
        ));
    }
    if !(camera.fovy > 0.0 && camera.fovy < 180.0) {
        return Err(anyhow!(
            "camera.fovy must be between 0 and 180 degrees, got {}",
            camera.fovy
        ));
    }
    for (name, value) in [("speed", camera.speed), ("sensitivity", camera.sensitivity)] {
        if !value.is_finite() {
            return Err(anyhow!(
                "camera.{name} must be a finite number, got {value}"
            ));
        }
    }
    if !camera.position.iter().all(|v| v.is_finite()) {
        return Err(anyhow!(
            "camera.position must be finite, got {:?}",
            camera.position
        ));
    }

    // A zero direction has no normalized form, so the sun would light nothing and the shadow
    // cascades would be built from a degenerate basis.
    let sun = &config.sun;
    if sun.direction.iter().all(|v| *v == 0.0) {
        return Err(anyhow!(
            "sun.direction must not be all zeros: it is a direction, not a position"
        ));
    }
    if !sun.direction.iter().all(|v| v.is_finite()) {
        return Err(anyhow!(
            "sun.direction must be finite, got {:?}",
            sun.direction
        ));
    }
    if !sun.intensity.is_finite() || sun.intensity < 0.0 {
        return Err(anyhow!(
            "sun.intensity must be zero or more, got {}",
            sun.intensity
        ));
    }
    if !sun.color.iter().all(|v| v.is_finite() && *v >= 0.0) {
        return Err(anyhow!("sun.color channels must be zero or more"));
    }

    if !config.light_intensity_scale.is_finite() || config.light_intensity_scale < 0.0 {
        return Err(anyhow!(
            "light_intensity_scale must be zero or more, got {}",
            config.light_intensity_scale
        ));
    }

    // Only used when the model brings no placements of its own, but a zero here would produce an
    // empty instance buffer, which wgpu rejects outright.
    if config.grid.instances_per_row == 0 {
        return Err(anyhow!("grid.instances_per_row must be at least 1"));
    }
    if !config.grid.space_between.is_finite() {
        return Err(anyhow!(
            "grid.space_between must be a finite number, got {}",
            config.grid.space_between
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_scene_file_is_the_default_scene() {
        let config = parse("", None).expect("an empty file is legal");
        let defaults = SceneConfig::default();

        assert_eq!(config.model_file, defaults.model_file);
        assert_eq!(config.camera.position, defaults.camera.position);
        assert_eq!(config.sun.direction, defaults.sun.direction);
    }

    #[test]
    fn every_field_round_trips_from_toml() {
        let config = parse(
            r#"
            model = "town.glb"
            light_intensity_scale = 0.01

            [camera]
            position = [1.0, 2.0, 3.0]
            yaw = -90.0
            pitch = -10.0
            fovy = 60.0
            znear = 0.2
            zfar = 500.0
            speed = 4.0
            sensitivity = 2.0

            [sun]
            direction = [0.0, -1.0, 0.0]
            color = [1.0, 0.5, 0.25]
            intensity = 3.0

            [grid]
            instances_per_row = 4
            space_between = 1.5
            "#,
            None,
        )
        .expect("the file is valid");

        assert_eq!(config.model_file, "town.glb");
        assert_eq!(config.light_intensity_scale, 0.01);
        assert_eq!(config.camera.position, [1.0, 2.0, 3.0]);
        assert_eq!(config.camera.yaw, -90.0);
        assert_eq!(config.camera.pitch, -10.0);
        assert_eq!(config.camera.fovy, 60.0);
        assert_eq!(config.camera.znear, 0.2);
        assert_eq!(config.camera.zfar, 500.0);
        assert_eq!(config.camera.speed, 4.0);
        assert_eq!(config.camera.sensitivity, 2.0);
        assert_eq!(config.sun.direction, [0.0, -1.0, 0.0]);
        assert_eq!(config.sun.color, [1.0, 0.5, 0.25]);
        assert_eq!(config.sun.intensity, 3.0);
        assert_eq!(config.grid.instances_per_row, 4);
        assert_eq!(config.grid.space_between, 1.5);
    }

    #[test]
    fn a_partial_section_keeps_the_defaults_for_its_other_fields() {
        let config = parse("[camera]\nfovy = 70.0\n", None).expect("a partial section is legal");
        let defaults = SceneConfig::default();

        assert_eq!(config.camera.fovy, 70.0);
        assert_eq!(
            config.camera.position, defaults.camera.position,
            "an unmentioned field must keep its default rather than reset to zero"
        );
    }

    #[test]
    fn a_misspelled_key_is_an_error_rather_than_a_silent_no_op() {
        let error = parse("modle = \"town.glb\"", None).expect_err("modle is not a key");

        assert!(
            error.to_string().contains("modle"),
            "the error should name the offending key, got: {error}"
        );
    }

    #[test]
    fn a_misspelled_key_inside_a_section_is_also_rejected() {
        assert!(parse("[sun]\nintensty = 2.0", None).is_err());
    }

    #[test]
    fn the_example_scene_file_in_the_repo_is_valid() {
        let text = include_str!("../scenes/default.toml");
        let config = parse(text, None).expect("the shipped example must parse");

        assert_eq!(config.model_file, "cube_diorama.glb");
    }

    #[test]
    fn a_zero_sun_direction_is_rejected_rather_than_normalized_to_nan() {
        let error = parse("[sun]\ndirection = [0.0, 0.0, 0.0]", None)
            .expect_err("a zero direction has no normalized form");

        assert!(error.to_string().contains("sun.direction"));
    }

    #[test]
    fn a_far_plane_behind_the_near_plane_is_rejected() {
        let error = parse("[camera]\nznear = 10.0\nzfar = 1.0", None)
            .expect_err("an inverted depth range is not renderable");

        assert!(error.to_string().contains("zfar"));
    }

    #[test]
    fn a_zero_near_plane_is_rejected_because_the_projection_divides_by_it() {
        assert!(parse("[camera]\nznear = 0.0", None).is_err());
    }

    #[test]
    fn an_empty_instance_grid_is_rejected_because_wgpu_rejects_empty_buffers() {
        let error = parse("[grid]\ninstances_per_row = 0", None)
            .expect_err("zero instances would create a zero-sized vertex buffer");

        assert!(error.to_string().contains("instances_per_row"));
    }

    #[test]
    fn a_negative_sun_intensity_is_rejected() {
        assert!(parse("[sun]\nintensity = -1.0", None).is_err());
    }

    #[test]
    fn a_blank_model_name_is_rejected() {
        assert!(parse("model = \"   \"", None).is_err());
    }

    #[test]
    fn a_nan_is_rejected_rather_than_propagating_into_every_matrix() {
        // TOML has no NaN literal, so it arrives via an expression that evaluates to one.
        assert!(
            parse("[camera]\nznear = nan", None).is_err(),
            "a NaN near plane would make every projected vertex NaN"
        );
        assert!(parse("[camera]\nzfar = nan", None).is_err());
        assert!(parse("[camera]\nfovy = nan", None).is_err());
        assert!(parse("[sun]\nintensity = nan", None).is_err());
        assert!(parse("[grid]\nspace_between = nan", None).is_err());
    }

    #[test]
    fn an_infinite_value_is_rejected_too() {
        assert!(parse("[camera]\nzfar = inf", None).is_err());
        assert!(parse("[camera]\nznear = inf", None).is_err());
    }

    #[test]
    fn a_scene_file_in_the_current_directory_resolves_assets_against_it() {
        let dir = std::env::temp_dir().join("rust-renderer-scene-file-base");
        std::fs::create_dir_all(&dir).expect("create the temp dir");
        let path = dir.join("scene.toml");
        std::fs::write(&path, "model = \"town.glb\"").expect("write the scene file");

        let config = load(&path).expect("the scene file parses");

        assert_eq!(
            config.base_dir.as_deref(),
            Some(dir.as_path()),
            "assets must resolve next to the scene file, not against the process directory"
        );

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_bare_file_name_still_gets_a_base_directory() {
        let config = parse("model = \"town.glb\"", Some(PathBuf::from("."))).expect("valid");

        assert_eq!(config.base_dir.as_deref(), Some(Path::new(".")));
    }

    #[test]
    fn a_missing_scene_file_names_the_path_it_tried() {
        let error = load(Path::new("/tmp/definitely-not-here/scene.toml")).expect_err("no file");

        assert!(error.to_string().contains("scene.toml"));
    }
}
