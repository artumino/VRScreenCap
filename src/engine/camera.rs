use anyhow::Context;
use cgmath::{Matrix4, Rad, SquareMatrix};
use openxr::Fovf;

use super::entity::Entity;

pub struct Camera {
    pub entity: Entity,
    pub projection: Matrix4<f32>,
    pub near: f32,
    pub far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            entity: Default::default(),
            projection: Matrix4::<f32>::identity(),
            near: 0.1,
            far: 3000.0,
        }
    }
}

impl Camera {
    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn update_projection_from_tangents(&mut self, fov: Fovf) {
        let tan_right = fov.angle_right.tan();
        let tan_left = fov.angle_left.tan();
        let tan_top = fov.angle_up.tan();
        let tan_bottom = fov.angle_down.tan();
        let tan_angle_width = tan_right - tan_left;
        let tan_angle_height = tan_top - tan_bottom;

        self.projection = Matrix4::new(
            2.0 / tan_angle_width,
            0.0,
            0.0,
            0.0,
            0.0,
            2.0 / tan_angle_height,
            0.0,
            0.0,
            (tan_right + tan_left) / tan_angle_width,
            (tan_top + tan_bottom) / tan_angle_height,
            -1.0,
            -1.0,
            0.0,
            0.0,
            -self.near,
            0.0,
        );
    }

    #[allow(unused)]
    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn update_projection(&mut self, fov: Rad<f32>, aspect_ratio: f32) {
        self.projection = cgmath::perspective(fov, aspect_ratio, self.near, self.far)
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn build_view_projection_matrix(&self) -> anyhow::Result<Matrix4<f32>> {
        Ok(self.projection
            * self
                .entity
                .world_matrix
                .invert()
                .context("Provided world matrix is not invertible")?)
    }
}

// We need this for Rust to store our data correctly for the shaders
#[repr(C)]
// This is so we can store this in a buffer
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    // We can't use cgmath with bytemuck directly so we'll have
    // to convert the Matrix4 into a 4x4 f32 array
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn new() -> Self {
        Self {
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn update_view_proj(&mut self, camera: &Camera) -> anyhow::Result<()> {
        self.view_proj = camera.build_view_projection_matrix()?.into();
        Ok(())
    }
}
