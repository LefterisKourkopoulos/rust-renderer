use cgmath::prelude::*;
use wgpu::util::DeviceExt;

use crate::config::CameraConfig;

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::from_cols(
    cgmath::Vector4::new(1.0, 0.0, 0.0, 0.0),
    cgmath::Vector4::new(0.0, 1.0, 0.0, 0.0),
    cgmath::Vector4::new(0.0, 0.0, 0.5, 0.0),
    cgmath::Vector4::new(0.0, 0.0, 0.5, 1.0),
);

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CameraMove {
    Forward,
    Backward,
    Left,
    Right,
}

struct Camera {
    eye: cgmath::Point3<f32>,
    target: cgmath::Point3<f32>,
    up: cgmath::Vector3<f32>,
    aspect: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,
}

impl Camera {
    fn build_view_projection_matrix(&self) -> cgmath::Matrix4<f32> {
        let view = cgmath::Matrix4::look_at_rh(self.eye, self.target, self.up);
        let proj = cgmath::perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);

        proj * view
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    fn new() -> Self {
        Self {
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }

    fn update_view_proj(&mut self, camera: &Camera) {
        self.view_proj = (OPENGL_TO_WGPU_MATRIX * camera.build_view_projection_matrix()).into();
    }
}

struct CameraController {
    speed: f32,
    is_forward_pressed: bool,
    is_backward_pressed: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
}

impl CameraController {
    fn new(speed: f32) -> Self {
        Self {
            speed,
            is_forward_pressed: false,
            is_backward_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
        }
    }

    fn set_move(&mut self, direction: CameraMove, is_pressed: bool) {
        match direction {
            CameraMove::Forward => self.is_forward_pressed = is_pressed,
            CameraMove::Backward => self.is_backward_pressed = is_pressed,
            CameraMove::Left => self.is_left_pressed = is_pressed,
            CameraMove::Right => self.is_right_pressed = is_pressed,
        }
    }

    fn update_camera(&self, camera: &mut Camera, dt: f32) {
        let step = self.speed * dt;

        let forward = camera.target - camera.eye;
        let forward_norm = forward.normalize();
        let forward_mag = forward.magnitude();

        if self.is_forward_pressed && forward_mag > step {
            camera.eye += forward_norm * step;
        }
        if self.is_backward_pressed {
            camera.eye -= forward_norm * step;
        }

        let right = forward_norm.cross(camera.up);
        let forward = camera.target - camera.eye;
        let forward_mag = forward.magnitude();

        if self.is_right_pressed {
            camera.eye = camera.target - (forward + right * step).normalize() * forward_mag;
        }
        if self.is_left_pressed {
            camera.eye = camera.target - (forward - right * step).normalize() * forward_mag;
        }
    }
}

pub struct CameraState {
    camera: Camera,
    uniform: CameraUniform,
    buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    controller: CameraController,
}

impl CameraState {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        camera_config: &CameraConfig,
    ) -> Self {
        let camera = Camera {
            eye: camera_config.eye.into(),
            target: camera_config.target.into(),
            up: cgmath::Vector3::unit_y(),
            aspect: config.width as f32 / config.height as f32,
            fovy: camera_config.fovy,
            znear: camera_config.znear,
            zfar: camera_config.zfar,
        };

        let mut uniform = CameraUniform::new();
        uniform.update_view_proj(&camera);

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("camera_bind_group_layout"),
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        Self {
            camera,
            uniform,
            buffer,
            bind_group_layout,
            bind_group,
            controller: CameraController::new(camera_config.speed),
        }
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.camera.aspect = width as f32 / height as f32;
    }

    pub fn set_move(&mut self, direction: CameraMove, is_pressed: bool) {
        self.controller.set_move(direction, is_pressed);
    }

    pub fn update(&mut self, queue: &wgpu::Queue, dt: f32) {
        self.controller.update_camera(&mut self.camera, dt);
        self.uniform.update_view_proj(&self.camera);
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[self.uniform]));
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

    fn test_camera() -> Camera {
        Camera {
            eye: (0.0, 0.0, 10.0).into(),
            target: (0.0, 0.0, 0.0).into(),
            up: cgmath::Vector3::unit_y(),
            aspect: 1.0,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
        }
    }

    #[test]
    fn opengl_to_wgpu_maps_ndc_depth_into_zero_to_one() {
        let near = OPENGL_TO_WGPU_MATRIX * cgmath::Vector4::new(0.0, 0.0, -1.0, 1.0);
        let far = OPENGL_TO_WGPU_MATRIX * cgmath::Vector4::new(0.0, 0.0, 1.0, 1.0);

        assert_close(near.z / near.w, 0.0, "OpenGL z=-1 maps to wgpu z=0");
        assert_close(far.z / far.w, 1.0, "OpenGL z=1 maps to wgpu z=1");
    }

    #[test]
    fn opengl_to_wgpu_leaves_x_and_y_untouched() {
        let point = cgmath::Vector4::new(0.25, -0.75, 0.5, 1.0);
        let mapped = OPENGL_TO_WGPU_MATRIX * point;

        assert_close(mapped.w, 1.0, "w");
        assert_close(mapped.x / mapped.w, point.x, "x");
        assert_close(mapped.y / mapped.w, point.y, "y");
        // Midpoint of the OpenGL range lands at the midpoint of wgpu's.
        assert_close(mapped.z / mapped.w, 0.75, "z");
    }

    #[test]
    fn forward_moves_the_eye_towards_the_target_by_speed_times_dt() {
        let mut camera = test_camera();
        let mut controller = CameraController::new(2.0);
        controller.set_move(CameraMove::Forward, true);

        controller.update_camera(&mut camera, 0.5);

        assert_close(camera.eye.z, 9.0, "eye z after forward");
        assert_close(camera.eye.x, 0.0, "eye x is unchanged");
        assert_close(camera.eye.y, 0.0, "eye y is unchanged");
    }

    #[test]
    fn backward_moves_the_eye_away_from_the_target() {
        let mut camera = test_camera();
        let mut controller = CameraController::new(2.0);
        controller.set_move(CameraMove::Backward, true);

        controller.update_camera(&mut camera, 0.5);

        assert_close(camera.eye.z, 11.0, "eye z after backward");
    }

    #[test]
    fn forward_does_not_move_past_the_target() {
        let mut camera = test_camera();
        let mut controller = CameraController::new(2.0);
        controller.set_move(CameraMove::Forward, true);

        controller.update_camera(&mut camera, 20.0);

        assert_close(camera.eye.z, 10.0, "eye is left where it was");
        assert!(
            camera.eye.z > camera.target.z,
            "eye must not cross the target"
        );
    }

    #[test]
    fn releasing_a_direction_stops_the_movement() {
        let mut camera = test_camera();
        let mut controller = CameraController::new(2.0);

        controller.set_move(CameraMove::Forward, true);
        controller.update_camera(&mut camera, 0.5);
        let moved_to = camera.eye;

        controller.set_move(CameraMove::Forward, false);
        controller.update_camera(&mut camera, 0.5);

        assert_close(camera.eye.z, moved_to.z, "eye z after release");
    }

    #[test]
    fn forward_and_backward_held_together_cancel_out() {
        let mut camera = test_camera();
        let mut controller = CameraController::new(2.0);
        controller.set_move(CameraMove::Forward, true);
        controller.set_move(CameraMove::Backward, true);

        controller.update_camera(&mut camera, 0.5);

        assert_close(camera.eye.z, 10.0, "eye z is unchanged");
    }

    #[test]
    fn strafing_keeps_the_eye_at_the_same_distance_from_the_target() {
        let mut camera = test_camera();
        let start_distance = (camera.eye - camera.target).magnitude();
        let mut controller = CameraController::new(2.0);
        controller.set_move(CameraMove::Right, true);

        controller.update_camera(&mut camera, 0.5);

        let distance = (camera.eye - camera.target).magnitude();
        assert_close(distance, start_distance, "orbit radius");
        assert!(
            camera.eye.x < 0.0,
            "right strafe moves the eye to -x, got {}",
            camera.eye.x
        );
    }

    #[test]
    fn left_and_right_strafes_are_mirror_images() {
        let mut controller = CameraController::new(2.0);

        let mut right = test_camera();
        controller.set_move(CameraMove::Right, true);
        controller.update_camera(&mut right, 0.5);
        controller.set_move(CameraMove::Right, false);

        let mut left = test_camera();
        controller.set_move(CameraMove::Left, true);
        controller.update_camera(&mut left, 0.5);

        assert_close(left.eye.x, -right.eye.x, "mirrored x");
        assert_close(left.eye.z, right.eye.z, "matching z");
    }

    #[test]
    fn a_zero_delta_time_leaves_the_camera_alone() {
        let mut camera = test_camera();
        let mut controller = CameraController::new(2.0);
        controller.set_move(CameraMove::Forward, true);
        controller.set_move(CameraMove::Right, true);

        controller.update_camera(&mut camera, 0.0);

        assert_close(camera.eye.x, 0.0, "eye x");
        assert_close(camera.eye.y, 0.0, "eye y");
        assert_close(camera.eye.z, 10.0, "eye z");
    }
}
