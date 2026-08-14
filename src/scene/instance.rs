use cgmath::prelude::*;

use crate::config::InstanceGridConfig;
use crate::gfx::Vertex;

pub struct Instance {
    position: cgmath::Vector3<f32>,
    rotation: cgmath::Quaternion<f32>,
}

impl Instance {
    pub fn to_raw(&self) -> InstanceRaw {
        InstanceRaw {
            model: (cgmath::Matrix4::from_translation(self.position)
                * cgmath::Matrix4::from(self.rotation))
            .into(),
        }
    }

    /// Builds an NxN grid of instances on the XZ plane, each rotated about its
    /// own offset from the origin.
    pub fn grid(config: &InstanceGridConfig) -> Vec<Self> {
        let per_row = config.instances_per_row;
        let space_between = config.space_between;

        (0..per_row)
            .flat_map(|z| {
                (0..per_row).map(move |x| {
                    let x = space_between * (x as f32 - per_row as f32 / 2.0);
                    let z = space_between * (z as f32 - per_row as f32 / 2.0);

                    let position = cgmath::Vector3 { x, y: 0.0, z };

                    let rotation = if position.is_zero() {
                        // A zero vector is not a valid rotation axis.
                        cgmath::Quaternion::from_axis_angle(
                            cgmath::Vector3::unit_z(),
                            cgmath::Deg(0.0),
                        )
                    } else {
                        cgmath::Quaternion::from_axis_angle(position.normalize(), cgmath::Deg(45.0))
                    };

                    Instance { position, rotation }
                })
            })
            .collect()
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceRaw {
    model: [[f32; 4]; 4],
}

impl Vertex for InstanceRaw {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            // A mat4x4 is passed as four consecutive vec4 attributes.
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    fn assert_close(actual: f32, expected: f32, what: &str) {
        assert!(
            (actual - expected).abs() < EPSILON,
            "{what}: expected {expected}, got {actual}"
        );
    }

    /// The values the app ships with, so the numbers below are the real ones.
    fn default_config() -> InstanceGridConfig {
        InstanceGridConfig::default()
    }

    #[test]
    fn grid_has_one_instance_per_cell() {
        let config = default_config();
        let instances = Instance::grid(&config);

        let per_row = config.instances_per_row as usize;
        assert_eq!(instances.len(), per_row * per_row);
        assert_eq!(instances.len(), 100);
    }

    #[test]
    fn grid_lies_flat_on_the_xz_plane() {
        let instances = Instance::grid(&default_config());

        for instance in &instances {
            assert_close(instance.position.y, 0.0, "instance y");
        }
    }

    #[test]
    fn grid_rows_are_spaced_by_the_configured_gap() {
        let config = default_config();
        let per_row = config.instances_per_row as usize;
        let instances = Instance::grid(&config);

        // Instances are emitted x-major within each z row.
        for row in 0..per_row {
            for column in 1..per_row {
                let previous = &instances[row * per_row + column - 1];
                let current = &instances[row * per_row + column];

                assert_close(
                    current.position.x - previous.position.x,
                    config.space_between,
                    "x spacing",
                );
                assert_close(
                    current.position.z - previous.position.z,
                    0.0,
                    "z within row",
                );
            }
        }

        // Consecutive rows are one gap apart in z.
        for row in 1..per_row {
            let previous = &instances[(row - 1) * per_row];
            let current = &instances[row * per_row];
            assert_close(
                current.position.z - previous.position.z,
                config.space_between,
                "z spacing",
            );
        }
    }

    #[test]
    fn grid_is_offset_by_half_a_row_rather_than_centred() {
        let config = default_config();
        let instances = Instance::grid(&config);

        // `space_between * (x - per_row / 2.0)` puts a cell exactly on the
        // origin, which for an even `per_row` leaves one extra column on the
        // negative side: 10 columns run -15.0..=12.0, not -13.5..=13.5. This is
        // the shipped look, so assert the offset grid rather than "fixing" it.
        let min_x = instances
            .iter()
            .fold(f32::INFINITY, |a, i| a.min(i.position.x));
        let max_x = instances
            .iter()
            .fold(f32::NEG_INFINITY, |a, i| a.max(i.position.x));

        assert_close(min_x, -15.0, "min x");
        assert_close(max_x, 12.0, "max x");
        assert_close((min_x + max_x) / 2.0, -1.5, "x midpoint");
    }

    #[test]
    fn origin_instance_gets_a_finite_identity_rotation() {
        let instances = Instance::grid(&default_config());

        // `position.normalize()` on the zero vector is NaN, so the origin cell
        // takes the guarded branch instead.
        let origin = instances
            .iter()
            .find(|i| i.position.is_zero())
            .expect("an even-sized grid places one instance on the origin");

        let r = origin.rotation;
        assert!(
            r.s.is_finite() && r.v.x.is_finite() && r.v.y.is_finite() && r.v.z.is_finite(),
            "origin rotation must not contain NaN or infinities, got {r:?}"
        );
        assert_close(r.magnitude(), 1.0, "origin rotation magnitude");
        assert_close(r.s, 1.0, "origin rotation scalar");
    }

    #[test]
    fn every_instance_rotation_is_a_unit_quaternion() {
        for instance in Instance::grid(&default_config()) {
            let r = instance.rotation;
            assert!(
                r.s.is_finite() && r.v.x.is_finite() && r.v.y.is_finite() && r.v.z.is_finite(),
                "rotation at {:?} is not finite: {r:?}",
                instance.position
            );
            assert_close(r.magnitude(), 1.0, "rotation magnitude");
        }
    }

    #[test]
    fn to_raw_puts_the_position_in_the_translation_column() {
        let instances = Instance::grid(&default_config());
        let instance = instances.last().expect("grid is not empty");
        let raw = instance.to_raw();

        // Column-major, so the last column is the translation.
        assert_close(raw.model[3][0], instance.position.x, "translation x");
        assert_close(raw.model[3][1], instance.position.y, "translation y");
        assert_close(raw.model[3][2], instance.position.z, "translation z");
        assert_close(raw.model[3][3], 1.0, "translation w");
    }

    #[test]
    fn grid_of_one_places_a_single_instance_off_the_origin() {
        let config = InstanceGridConfig {
            instances_per_row: 1,
            space_between: 3.0,
        };
        let instances = Instance::grid(&config);

        assert_eq!(instances.len(), 1);
        // Same half-row offset as above: 3.0 * (0 - 0.5).
        assert_close(instances[0].position.x, -1.5, "single instance x");
        assert_close(instances[0].position.z, -1.5, "single instance z");
    }
}
