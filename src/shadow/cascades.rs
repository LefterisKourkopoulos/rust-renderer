use cgmath::prelude::*;

use crate::scene::camera::{FrustumParams, OPENGL_TO_WGPU_MATRIX};

const UP: cgmath::Vector3<f32> = cgmath::Vector3::new(0.0, 1.0, 0.0);
const ALTERNATE_UP: cgmath::Vector3<f32> = cgmath::Vector3::new(0.0, 0.0, 1.0);

pub fn split_distances(znear: f32, zfar: f32, lambda: f32, count: usize) -> Vec<f32> {
    let count = count.max(1);
    let lambda = lambda.clamp(0.0, 1.0);
    let range = zfar - znear;
    let ratio = zfar / znear;

    (1..=count)
        .map(|i| {
            let fraction = i as f32 / count as f32;
            let uniform = znear + range * fraction;
            let logarithmic = znear * ratio.powf(fraction);
            uniform + (logarithmic - uniform) * lambda
        })
        .collect()
}

pub fn frustum_corners_world(
    view: cgmath::Matrix4<f32>,
    frustum: &FrustumParams,
    near: f32,
    far: f32,
) -> [cgmath::Vector4<f32>; 8] {
    let proj = cgmath::perspective(frustum.fovy, frustum.aspect, near, far);
    let inverse = (proj * view)
        .invert()
        .expect("a perspective projection times a view matrix is always invertible");

    let mut corners = [cgmath::Vector4::zero(); 8];
    let mut index = 0;
    for x in 0..2 {
        for y in 0..2 {
            for z in 0..2 {
                let ndc = cgmath::Vector4::new(
                    2.0 * x as f32 - 1.0,
                    2.0 * y as f32 - 1.0,
                    2.0 * z as f32 - 1.0,
                    1.0,
                );
                let corner = inverse * ndc;
                corners[index] = corner / corner.w;
                index += 1;
            }
        }
    }

    corners
}

pub fn light_view_proj(
    corners: &[cgmath::Vector4<f32>; 8],
    light_direction: cgmath::Vector3<f32>,
    z_mult: f32,
) -> cgmath::Matrix4<f32> {
    let direction = normalize_or(light_direction, -UP);

    let mut center = cgmath::Vector3::zero();
    for corner in corners {
        center += corner.truncate();
    }
    center /= corners.len() as f32;

    let up = if direction.dot(UP).abs() > 0.99 {
        ALTERNATE_UP
    } else {
        UP
    };

    let light_view = cgmath::Matrix4::look_at_rh(
        cgmath::Point3::from_vec(center - direction),
        cgmath::Point3::from_vec(center),
        up,
    );

    let mut min = cgmath::Vector3::from_value(f32::INFINITY);
    let mut max = cgmath::Vector3::from_value(f32::NEG_INFINITY);
    for corner in corners {
        let light_space = (light_view * corner).truncate();
        min = min.zip(light_space, f32::min);
        max = max.zip(light_space, f32::max);
    }

    let pad = (max.z - min.z) * (z_mult.max(1.0) - 1.0);

    let projection = cgmath::ortho(min.x, max.x, min.y, max.y, -(max.z + pad), -min.z);

    OPENGL_TO_WGPU_MATRIX * projection * light_view
}

fn normalize_or(
    vector: cgmath::Vector3<f32>,
    fallback: cgmath::Vector3<f32>,
) -> cgmath::Vector3<f32> {
    if vector.magnitude() < 1e-6 {
        fallback
    } else {
        vector.normalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-4;

    fn frustum() -> FrustumParams {
        FrustumParams {
            fovy: cgmath::Deg(45.0).into(),
            aspect: 16.0 / 9.0,
            znear: 0.05,
            zfar: 100.0,
        }
    }

    fn view() -> cgmath::Matrix4<f32> {
        cgmath::Matrix4::look_at_rh(
            cgmath::Point3::new(2.5, 2.0, 2.5),
            cgmath::Point3::new(0.0, 0.0, 0.0),
            UP,
        )
    }

    #[test]
    fn splits_are_monotonic_and_end_at_the_far_plane() {
        let splits = split_distances(0.05, 100.0, 0.7, 4);

        assert_eq!(splits.len(), 4);
        assert!(splits[0] > 0.05, "the first split must sit past znear");
        for pair in splits.windows(2) {
            assert!(pair[1] > pair[0], "splits must increase: {splits:?}");
        }
        assert!(
            (splits[3] - 100.0).abs() < EPSILON,
            "the last cascade has to close the frustum at zfar"
        );
    }

    #[test]
    fn lambda_zero_is_uniform_and_lambda_one_is_logarithmic() {
        let uniform = split_distances(1.0, 5.0, 0.0, 4);
        for (index, distance) in uniform.iter().enumerate() {
            let expected = 1.0 + 4.0 * (index + 1) as f32 / 4.0;
            assert!((distance - expected).abs() < EPSILON, "{uniform:?}");
        }

        let logarithmic = split_distances(1.0, 16.0, 1.0, 4);
        for (index, distance) in logarithmic.iter().enumerate() {
            let expected = 16f32.powf((index + 1) as f32 / 4.0);
            assert!((distance - expected).abs() < EPSILON, "{logarithmic:?}");
        }
    }

    #[test]
    fn a_single_cascade_covers_the_whole_range() {
        let splits = split_distances(0.05, 100.0, 0.7, 1);

        assert_eq!(splits.len(), 1);
        assert!((splits[0] - 100.0).abs() < EPSILON);
    }

    #[test]
    fn frustum_corners_round_trip_back_to_the_ndc_cube() {
        let frustum = frustum();
        let view = view();
        let corners = frustum_corners_world(view, &frustum, 0.05, 3.0);

        let proj = cgmath::perspective(frustum.fovy, frustum.aspect, 0.05, 3.0);
        for corner in &corners {
            let clip = proj * view * corner;
            let ndc = clip / clip.w;
            for axis in [ndc.x, ndc.y, ndc.z] {
                assert!(
                    (axis.abs() - 1.0).abs() < EPSILON,
                    "every corner must map back onto an NDC cube face, got {ndc:?}"
                );
            }
        }
    }

    #[test]
    fn every_frustum_corner_lands_inside_the_cascade_clip_volume() {
        let frustum = frustum();
        let corners = frustum_corners_world(view(), &frustum, 0.05, 3.0);
        let matrix = light_view_proj(&corners, cgmath::Vector3::new(-0.4, -1.0, -0.3), 10.0);

        for corner in &corners {
            let clip = matrix * corner;
            let ndc = clip / clip.w;
            assert!(
                ndc.x >= -1.0 - EPSILON && ndc.x <= 1.0 + EPSILON,
                "x out of range: {ndc:?}"
            );
            assert!(
                ndc.y >= -1.0 - EPSILON && ndc.y <= 1.0 + EPSILON,
                "y out of range: {ndc:?}"
            );
            assert!(
                ndc.z >= -EPSILON && ndc.z <= 1.0 + EPSILON,
                "depth must land in [0, 1] for Depth32Float, got {ndc:?}"
            );
        }
    }

    #[test]
    fn a_light_pointing_straight_down_stays_finite() {
        let frustum = frustum();
        let corners = frustum_corners_world(view(), &frustum, 0.05, 3.0);
        let matrix = light_view_proj(&corners, -UP, 10.0);

        for corner in &corners {
            let clip = matrix * corner;
            let ndc = clip / clip.w;
            for axis in [ndc.x, ndc.y, ndc.z] {
                assert!(
                    axis.is_finite(),
                    "a light parallel to the up vector must not produce NaN"
                );
            }
            assert!(ndc.z >= -EPSILON && ndc.z <= 1.0 + EPSILON, "{ndc:?}");
        }
    }

    #[test]
    fn a_zero_light_direction_does_not_produce_nan() {
        let frustum = frustum();
        let corners = frustum_corners_world(view(), &frustum, 0.05, 3.0);
        let matrix = light_view_proj(&corners, cgmath::Vector3::zero(), 10.0);

        for corner in &corners {
            let clip = matrix * corner;
            let ndc = clip / clip.w;
            for axis in [ndc.x, ndc.y, ndc.z] {
                assert!(
                    axis.is_finite(),
                    "a zero direction must fall back to a sane light"
                );
            }
        }
    }

    fn depth_span(corners: &[cgmath::Vector4<f32>; 8], matrix: cgmath::Matrix4<f32>) -> f32 {
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for corner in corners {
            let clip = matrix * corner;
            let depth = clip.z / clip.w;
            min = min.min(depth);
            max = max.max(depth);
        }
        max - min
    }

    #[test]
    fn z_mult_reserves_depth_range_for_casters_outside_the_frustum() {
        let frustum = frustum();
        let corners = frustum_corners_world(view(), &frustum, 0.05, 3.0);
        let direction = cgmath::Vector3::new(-0.4, -1.0, -0.3);

        let tight = depth_span(&corners, light_view_proj(&corners, direction, 1.0));
        let padded = depth_span(&corners, light_view_proj(&corners, direction, 10.0));

        assert!(
            (tight - 1.0).abs() < EPSILON,
            "z_mult 1 should fit the frustum exactly, got {tight}"
        );
        assert!(
            padded < tight,
            "a larger z_mult reserves range for off-frustum casters, so the frustum \
             itself occupies less of it"
        );
    }

    #[test]
    fn the_frustum_keeps_its_depth_range_regardless_of_light_distance() {
        let frustum = frustum();
        let corners = frustum_corners_world(view(), &frustum, 0.05, 3.0);
        let direction = cgmath::Vector3::new(-0.4, -1.0, -0.3);

        let near_span = depth_span(&corners, light_view_proj(&corners, direction, 10.0));
        let far_corners = corners.map(|corner| corner + (direction * 100.0).extend(0.0));
        let far_span = depth_span(&far_corners, light_view_proj(&far_corners, direction, 10.0));

        assert!(
            (near_span - far_span).abs() < EPSILON,
            "depth span must be light-position independent, got {near_span} vs {far_span}"
        );
    }
}
