use cgmath::prelude::*;

use crate::config::InstanceGridConfig;
use crate::gfx::Vertex;

#[derive(Copy, Clone, Debug)]
pub struct Instance {
    model: cgmath::Matrix4<f32>,
}

impl Instance {
    pub fn from_trs(
        position: cgmath::Vector3<f32>,
        rotation: cgmath::Quaternion<f32>,
        scale: cgmath::Vector3<f32>,
    ) -> Self {
        Self::from_matrix(
            cgmath::Matrix4::from_translation(position)
                * cgmath::Matrix4::from(rotation)
                * cgmath::Matrix4::from_nonuniform_scale(scale.x, scale.y, scale.z),
        )
    }

    pub fn from_matrix(model: cgmath::Matrix4<f32>) -> Self {
        Self { model }
    }

    pub fn translation(&self) -> cgmath::Vector3<f32> {
        self.model.w.truncate()
    }

    pub fn to_raw(&self) -> InstanceRaw {
        InstanceRaw {
            model: self.model.into(),
            normal: normal_matrix(&self.model).into(),
        }
    }

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
                        cgmath::Quaternion::from_axis_angle(
                            cgmath::Vector3::unit_z(),
                            cgmath::Deg(0.0),
                        )
                    } else {
                        cgmath::Quaternion::from_axis_angle(position.normalize(), cgmath::Deg(45.0))
                    };

                    Instance::from_trs(position, rotation, cgmath::Vector3::new(1.0, 1.0, 1.0))
                })
            })
            .collect()
    }
}

fn normal_matrix(model: &cgmath::Matrix4<f32>) -> cgmath::Matrix3<f32> {
    let linear = cgmath::Matrix3::new(
        model.x.x, model.x.y, model.x.z, model.y.x, model.y.y, model.y.z, model.z.x, model.z.y,
        model.z.z,
    );

    match linear.invert() {
        Some(inverse) => inverse.transpose(),
        None => linear,
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceRaw {
    model: [[f32; 4]; 4],
    normal: [[f32; 3]; 3],
}

impl Vertex for InstanceRaw {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
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
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 9,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 19]>() as wgpu::BufferAddress,
                    shader_location: 10,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 22]>() as wgpu::BufferAddress,
                    shader_location: 11,
                    format: wgpu::VertexFormat::Float32x3,
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
            assert_close(instance.translation().y, 0.0, "instance y");
        }
    }

    #[test]
    fn grid_rows_are_spaced_by_the_configured_gap() {
        let config = default_config();
        let per_row = config.instances_per_row as usize;
        let instances = Instance::grid(&config);

        for row in 0..per_row {
            for column in 1..per_row {
                let previous = instances[row * per_row + column - 1].translation();
                let current = instances[row * per_row + column].translation();

                assert_close(current.x - previous.x, config.space_between, "x spacing");
                assert_close(current.z - previous.z, 0.0, "z within row");
            }
        }

        for row in 1..per_row {
            let previous = instances[(row - 1) * per_row].translation();
            let current = instances[row * per_row].translation();
            assert_close(current.z - previous.z, config.space_between, "z spacing");
        }
    }

    #[test]
    fn grid_is_offset_by_half_a_row_rather_than_centred() {
        let config = default_config();
        let instances = Instance::grid(&config);
        let min_x = instances
            .iter()
            .fold(f32::INFINITY, |a, i| a.min(i.translation().x));
        let max_x = instances
            .iter()
            .fold(f32::NEG_INFINITY, |a, i| a.max(i.translation().x));

        assert_close(min_x, -15.0, "min x");
        assert_close(max_x, 12.0, "max x");
        assert_close((min_x + max_x) / 2.0, -1.5, "x midpoint");
    }

    #[test]
    fn the_origin_instance_is_left_unrotated() {
        let instances = Instance::grid(&default_config());

        let origin = instances
            .iter()
            .find(|i| i.translation().is_zero())
            .expect("an even-sized grid places one instance on the origin");

        let raw = origin.to_raw();
        let identity: [[f32; 4]; 4] = cgmath::Matrix4::identity().into();
        for (row, expected_row) in raw.model.iter().zip(identity.iter()) {
            for (actual, expected) in row.iter().zip(expected_row.iter()) {
                assert_close(*actual, *expected, "origin model matrix");
            }
        }
    }

    #[test]
    fn every_grid_instance_is_a_rigid_transform() {
        for instance in Instance::grid(&default_config()) {
            let raw = instance.to_raw();
            let columns = [
                cgmath::Vector3::new(raw.model[0][0], raw.model[0][1], raw.model[0][2]),
                cgmath::Vector3::new(raw.model[1][0], raw.model[1][1], raw.model[1][2]),
                cgmath::Vector3::new(raw.model[2][0], raw.model[2][1], raw.model[2][2]),
            ];

            for column in columns {
                assert!(
                    column.x.is_finite() && column.y.is_finite() && column.z.is_finite(),
                    "model matrix must not contain NaN or infinities, got {column:?}"
                );
                assert_close(column.magnitude(), 1.0, "column magnitude");
            }

            assert_close(columns[0].dot(columns[1]), 0.0, "x . y");
            assert_close(columns[1].dot(columns[2]), 0.0, "y . z");
            assert_close(columns[2].dot(columns[0]), 0.0, "z . x");
        }
    }

    #[test]
    fn to_raw_puts_the_position_in_the_translation_column() {
        let instances = Instance::grid(&default_config());
        let instance = instances.last().expect("grid is not empty");
        let raw = instance.to_raw();
        let position = instance.translation();

        assert_close(raw.model[3][0], position.x, "translation x");
        assert_close(raw.model[3][1], position.y, "translation y");
        assert_close(raw.model[3][2], position.z, "translation z");
        assert_close(raw.model[3][3], 1.0, "translation w");
    }

    #[test]
    fn from_matrix_uploads_the_matrix_unchanged() {
        let mut sheared = cgmath::Matrix4::identity();
        sheared.y.x = 0.5;
        let raw = Instance::from_matrix(sheared).to_raw();
        assert_close(raw.model[1][0], 0.5, "shear term");
    }

    #[test]
    fn to_raw_scales_the_model_matrix_columns() {
        let instance = Instance::from_trs(
            cgmath::Vector3::new(0.0, 0.0, 0.0),
            cgmath::Quaternion::one(),
            cgmath::Vector3::new(2.0, 3.0, 4.0),
        );
        let raw = instance.to_raw();

        assert_close(raw.model[0][0], 2.0, "x column scale");
        assert_close(raw.model[1][1], 3.0, "y column scale");
        assert_close(raw.model[2][2], 4.0, "z column scale");
    }

    #[test]
    fn normal_matrix_inverts_non_uniform_scale() {
        let instance = Instance::from_trs(
            cgmath::Vector3::new(5.0, 6.0, 7.0),
            cgmath::Quaternion::one(),
            cgmath::Vector3::new(2.0, 4.0, 8.0),
        );
        let raw = instance.to_raw();

        assert_close(raw.normal[0][0], 0.5, "normal x");
        assert_close(raw.normal[1][1], 0.25, "normal y");
        assert_close(raw.normal[2][2], 0.125, "normal z");
    }

    #[test]
    fn normal_matrix_of_a_rotation_is_the_rotation_itself() {
        for instance in Instance::grid(&default_config()) {
            let raw = instance.to_raw();
            for row in 0..3 {
                for column in 0..3 {
                    assert_close(
                        raw.normal[row][column],
                        raw.model[row][column],
                        "normal matrix must equal the model 3x3 for a rigid transform",
                    );
                }
            }
        }
    }

    #[test]
    fn normal_matrix_stays_finite_for_a_collapsed_scale() {
        let instance = Instance::from_trs(
            cgmath::Vector3::new(0.0, 0.0, 0.0),
            cgmath::Quaternion::one(),
            cgmath::Vector3::new(1.0, 0.0, 1.0),
        );
        let raw = instance.to_raw();

        for row in raw.normal {
            for value in row {
                assert!(value.is_finite(), "a singular matrix must not produce NaN");
            }
        }
    }

    #[test]
    fn grid_of_one_places_a_single_instance_off_the_origin() {
        let config = InstanceGridConfig {
            instances_per_row: 1,
            space_between: 3.0,
        };
        let instances = Instance::grid(&config);

        assert_eq!(instances.len(), 1);
        assert_close(instances[0].translation().x, -1.5, "single instance x");
        assert_close(instances[0].translation().z, -1.5, "single instance z");
    }
}
