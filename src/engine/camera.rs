use cgmath::{Matrix4, Rad, SquareMatrix};
use openxr::Fovf;

use super::entity::Entity;

pub struct Camera {
    pub entity: Entity,
    pub projection: Matrix4<f32>,
    pub near: f32,
    pub far: f32
}

impl Default for Camera {
    fn default() -> Self {
        Self { 
            entity: Default::default(), 
            projection: Matrix4::<f32>::identity(), 
            near: 0.1, 
            far: 3000.0 
        }
    }
}

impl Camera {
    pub fn update_projection_from_tangents(&mut self, fov: Fovf){
        // =============================================================================
        // math code adapted from
        // https://github.com/KhronosGroup/OpenXR-SDK-Source/blob/master/src/common/xr_linear.h
        // Copyright (c) 2017 The Khronos Group Inc.
        // Copyright (c) 2016 Oculus VR, LLC.
        // SPDX-License-Identifier: Apache-2.0
        // =============================================================================

        let near_z = self.near;
        let far_z = self.far; 

        let tan_angle_left = fov.angle_left.tan();
        let tan_angle_right = fov.angle_right.tan();

        let tan_angle_down = fov.angle_down.tan();
        let tan_angle_up = fov.angle_up.tan();

        let tan_angle_width = tan_angle_right - tan_angle_left;

        // Set to tanAngleDown - tanAngleUp for a clip space with positive Y
        // down (Vulkan). Set to tanAngleUp - tanAngleDown for a clip space with
        // positive Y up (OpenGL / D3D / Metal).
        // const float tanAngleHeight =
        //     graphicsApi == GRAPHICS_VULKAN ? (tanAngleDown - tanAngleUp) : (tanAngleUp - tanAngleDown);
        let tan_angle_height = tan_angle_up - tan_angle_down;

        // Set to nearZ for a [-1,1] Z clip space (OpenGL / OpenGL ES).
        // Set to zero for a [0,1] Z clip space (Vulkan / D3D / Metal).
        let offset_z = near_z;

        // normal projection
        self.projection = Matrix4::new(2. / tan_angle_width, 0., (tan_angle_right + tan_angle_left) / tan_angle_width, 0., 
        0., 2. / tan_angle_height, (tan_angle_up + tan_angle_down) / tan_angle_height, 0., 
        0., 0., -(far_z + offset_z) / (far_z - near_z), -(far_z * (near_z + offset_z)) / (far_z - near_z), 
        0., 0., -1., 0.);
        self.projection.transpose_self();
    }

    pub fn update_projection(&mut self, fov: Rad<f32>, aspect_ratio: f32) {
        self.projection = cgmath::perspective(fov, aspect_ratio, self.near, self.far)
    }

    pub fn build_view_projection_matrix(&self) -> Matrix4<f32> {
        OPENGL_TO_WGPU_MATRIX * self.projection * self.entity.world_matrix.invert().unwrap()
        //OPENGL_TO_WGPU_MATRIX * self.entity.world_matrix.invert().unwrap() * self.projection
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
    pub fn new() -> Self {
        Self {
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera) {
        self.view_proj = camera.build_view_projection_matrix().into();
    }
}

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);
