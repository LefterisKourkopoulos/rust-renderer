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

const SAFE_FRAC_PI_2: f32 = std::f32::consts::FRAC_PI_2 - 0.0001;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CameraMove {
    Forward,
    Backward,
    Left,
    Right,
    Up,
    Down,
}

struct Camera {
    position: cgmath::Point3<f32>,
    yaw: cgmath::Rad<f32>,
    pitch: cgmath::Rad<f32>,
}

impl Camera {
    fn calc_matrix(&self) -> cgmath::Matrix4<f32> {
        let (sin_pitch, cos_pitch) = self.pitch.0.sin_cos();
        let (sin_yaw, cos_yaw) = self.yaw.0.sin_cos();

        cgmath::Matrix4::look_to_rh(
            self.position,
            cgmath::Vector3::new(cos_pitch * cos_yaw, sin_pitch, cos_pitch * sin_yaw).normalize(),
            cgmath::Vector3::unit_y(),
        )
    }
}

struct Projection {
    aspect: f32,
    fovy: cgmath::Rad<f32>,
    znear: f32,
    zfar: f32,
}

impl Projection {
    fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
    }

    fn calc_matrix(&self) -> cgmath::Matrix4<f32> {
        cgmath::perspective(self.fovy, self.aspect, self.znear, self.zfar)
    }
}

#[derive(Copy, Clone, Debug)]
pub struct FrustumParams {
    pub fovy: cgmath::Rad<f32>,
    pub aspect: f32,
    pub znear: f32,
    pub zfar: f32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_position: [f32; 4],
    view: [[f32; 4]; 4],
    view_proj: [[f32; 4]; 4],
    inv_proj: [[f32; 4]; 4],
    inv_view: [[f32; 4]; 4],
}

impl CameraUniform {
    fn new() -> Self {
        Self {
            view_position: [0.0; 4],
            view: cgmath::Matrix4::identity().into(),
            view_proj: cgmath::Matrix4::identity().into(),
            inv_proj: cgmath::Matrix4::identity().into(),
            inv_view: cgmath::Matrix4::identity().into(),
        }
    }

    fn update_view_proj(&mut self, camera: &Camera, projection: &Projection) {
        self.view_position = camera.position.to_homogeneous().into();
        let proj = projection.calc_matrix();
        let view = camera.calc_matrix();
        let view_proj = OPENGL_TO_WGPU_MATRIX * proj * view;
        self.view = view.into();
        self.view_proj = view_proj.into();
        self.inv_proj = (OPENGL_TO_WGPU_MATRIX * proj).invert().unwrap().into();
        self.inv_view = view.transpose().into();
    }
}

struct CameraController {
    speed: f32,
    sensitivity: f32,
    is_forward_pressed: bool,
    is_backward_pressed: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
    is_up_pressed: bool,
    is_down_pressed: bool,
    rotate_horizontal: f32,
    rotate_vertical: f32,
}

impl CameraController {
    fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            speed,
            sensitivity,
            is_forward_pressed: false,
            is_backward_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
            is_up_pressed: false,
            is_down_pressed: false,
            rotate_horizontal: 0.0,
            rotate_vertical: 0.0,
        }
    }

    fn set_move(&mut self, direction: CameraMove, is_pressed: bool) {
        match direction {
            CameraMove::Forward => self.is_forward_pressed = is_pressed,
            CameraMove::Backward => self.is_backward_pressed = is_pressed,
            CameraMove::Left => self.is_left_pressed = is_pressed,
            CameraMove::Right => self.is_right_pressed = is_pressed,
            CameraMove::Up => self.is_up_pressed = is_pressed,
            CameraMove::Down => self.is_down_pressed = is_pressed,
        }
    }

    fn set_look(&mut self, mouse_dx: f64, mouse_dy: f64) {
        self.rotate_horizontal += mouse_dx as f32;
        self.rotate_vertical += mouse_dy as f32;
    }

    fn update_camera(&mut self, camera: &mut Camera, dt: f32) {
        let (sin_yaw, cos_yaw) = camera.yaw.0.sin_cos();
        let forward = cgmath::Vector3::new(cos_yaw, 0.0, sin_yaw).normalize();
        let right = cgmath::Vector3::new(-sin_yaw, 0.0, cos_yaw).normalize();

        let step = self.speed * dt;
        if self.is_forward_pressed {
            camera.position += forward * step;
        }
        if self.is_backward_pressed {
            camera.position -= forward * step;
        }
        if self.is_right_pressed {
            camera.position += right * step;
        }
        if self.is_left_pressed {
            camera.position -= right * step;
        }
        if self.is_up_pressed {
            camera.position.y += step;
        }
        if self.is_down_pressed {
            camera.position.y -= step;
        }

        camera.yaw += cgmath::Rad(self.rotate_horizontal.to_radians() * self.sensitivity * dt);
        camera.pitch += cgmath::Rad(-self.rotate_vertical.to_radians() * self.sensitivity * dt);

        self.rotate_horizontal = 0.0;
        self.rotate_vertical = 0.0;

        if camera.pitch < -cgmath::Rad(SAFE_FRAC_PI_2) {
            camera.pitch = -cgmath::Rad(SAFE_FRAC_PI_2);
        } else if camera.pitch > cgmath::Rad(SAFE_FRAC_PI_2) {
            camera.pitch = cgmath::Rad(SAFE_FRAC_PI_2);
        }
    }
}

pub struct CameraState {
    camera: Camera,
    projection: Projection,
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
            position: camera_config.position.into(),
            yaw: cgmath::Deg(camera_config.yaw).into(),
            pitch: cgmath::Deg(camera_config.pitch).into(),
        };

        let projection = Projection {
            aspect: config.width as f32 / config.height as f32,
            fovy: cgmath::Deg(camera_config.fovy).into(),
            znear: camera_config.znear,
            zfar: camera_config.zfar,
        };

        let mut uniform = CameraUniform::new();
        uniform.update_view_proj(&camera, &projection);

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
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
            projection,
            uniform,
            buffer,
            bind_group_layout,
            bind_group,
            controller: CameraController::new(camera_config.speed, camera_config.sensitivity),
        }
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn view(&self) -> cgmath::Matrix4<f32> {
        self.camera.calc_matrix()
    }

    pub fn frustum(&self) -> FrustumParams {
        FrustumParams {
            fovy: self.projection.fovy,
            aspect: self.projection.aspect,
            znear: self.projection.znear,
            zfar: self.projection.zfar,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.projection.resize(width, height);
    }

    pub fn set_move(&mut self, direction: CameraMove, is_pressed: bool) {
        self.controller.set_move(direction, is_pressed);
    }

    pub fn set_look(&mut self, mouse_dx: f64, mouse_dy: f64) {
        self.controller.set_look(mouse_dx, mouse_dy);
    }

    pub fn update(&mut self, queue: &wgpu::Queue, dt: f32) {
        self.controller.update_camera(&mut self.camera, dt);
        self.uniform.update_view_proj(&self.camera, &self.projection);
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
            position: (0.0, 0.0, 10.0).into(),
            yaw: cgmath::Deg(-90.0).into(),
            pitch: cgmath::Deg(0.0).into(),
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
    fn forward_moves_the_eye_towards_the_look_direction_by_speed_times_dt() {
        let mut camera = test_camera();
        let mut controller = CameraController::new(2.0, 0.4);
        controller.set_move(CameraMove::Forward, true);

        controller.update_camera(&mut camera, 0.5);

        // Yaw -90deg faces -z.
        assert_close(camera.position.z, 9.0, "position z after forward");
        assert_close(camera.position.x, 0.0, "position x is unchanged");
        assert_close(camera.position.y, 0.0, "position y is unchanged");
    }

    #[test]
    fn backward_moves_the_eye_away_from_the_look_direction() {
        let mut camera = test_camera();
        let mut controller = CameraController::new(2.0, 0.4);
        controller.set_move(CameraMove::Backward, true);

        controller.update_camera(&mut camera, 0.5);

        assert_close(camera.position.z, 11.0, "position z after backward");
    }

    #[test]
    fn releasing_a_direction_stops_the_movement() {
        let mut camera = test_camera();
        let mut controller = CameraController::new(2.0, 0.4);

        controller.set_move(CameraMove::Forward, true);
        controller.update_camera(&mut camera, 0.5);
        let moved_to = camera.position;

        controller.set_move(CameraMove::Forward, false);
        controller.update_camera(&mut camera, 0.5);

        assert_close(camera.position.z, moved_to.z, "position z after release");
    }

    #[test]
    fn forward_and_backward_held_together_cancel_out() {
        let mut camera = test_camera();
        let mut controller = CameraController::new(2.0, 0.4);
        controller.set_move(CameraMove::Forward, true);
        controller.set_move(CameraMove::Backward, true);

        controller.update_camera(&mut camera, 0.5);

        assert_close(camera.position.z, 10.0, "position z is unchanged");
    }

    #[test]
    fn strafing_moves_perpendicular_to_the_look_direction() {
        let mut camera = test_camera();
        let mut controller = CameraController::new(2.0, 0.4);
        controller.set_move(CameraMove::Right, true);

        controller.update_camera(&mut camera, 0.5);

        // Facing -z, right strafe moves along +x.
        assert_close(camera.position.z, 10.0, "position z is unchanged");
        assert!(
            camera.position.x > 0.0,
            "right strafe moves the eye to +x, got {}",
            camera.position.x
        );
    }

    #[test]
    fn left_and_right_strafes_are_mirror_images() {
        let mut controller = CameraController::new(2.0, 0.4);

        let mut right = test_camera();
        controller.set_move(CameraMove::Right, true);
        controller.update_camera(&mut right, 0.5);
        controller.set_move(CameraMove::Right, false);

        let mut left = test_camera();
        controller.set_move(CameraMove::Left, true);
        controller.update_camera(&mut left, 0.5);

        assert_close(left.position.x, -right.position.x, "mirrored x");
        assert_close(left.position.z, right.position.z, "matching z");
    }

    #[test]
    fn a_zero_delta_time_leaves_the_camera_alone() {
        let mut camera = test_camera();
        let mut controller = CameraController::new(2.0, 0.4);
        controller.set_move(CameraMove::Forward, true);
        controller.set_move(CameraMove::Right, true);

        controller.update_camera(&mut camera, 0.0);

        assert_close(camera.position.x, 0.0, "position x");
        assert_close(camera.position.y, 0.0, "position y");
        assert_close(camera.position.z, 10.0, "position z");
    }

    #[test]
    fn up_and_down_move_along_the_world_y_axis() {
        let mut camera = test_camera();
        let mut controller = CameraController::new(2.0, 0.4);
        controller.set_move(CameraMove::Up, true);

        controller.update_camera(&mut camera, 0.5);

        assert_close(camera.position.y, 1.0, "position y after up");
    }

    #[test]
    fn mouse_motion_rotates_yaw_and_pitch_then_resets() {
        let mut camera = test_camera();
        let mut controller = CameraController::new(2.0, 1.0);
        controller.set_look(10.0, 5.0);

        let yaw_before = camera.yaw;
        let pitch_before = camera.pitch;
        controller.update_camera(&mut camera, 1.0);

        assert!(camera.yaw.0 > yaw_before.0, "positive dx increases yaw");
        assert!(
            camera.pitch.0 < pitch_before.0,
            "positive dy (mouse moving down) decreases pitch"
        );
        assert_close(controller.rotate_horizontal, 0.0, "horizontal resets");
        assert_close(controller.rotate_vertical, 0.0, "vertical resets");
    }

    #[test]
    fn pitch_is_clamped_to_avoid_gimbal_flip() {
        let mut camera = test_camera();
        let mut controller = CameraController::new(2.0, 1.0);
        controller.set_look(0.0, -100000.0);

        controller.update_camera(&mut camera, 1.0);

        assert!(camera.pitch.0 <= SAFE_FRAC_PI_2, "pitch clamped at the top");
    }
}
