use cgmath::{Matrix4, Rad};

use super::entity::Entity;

pub struct Camera {
    pub entity: Entity,
    pub fov: Rad<f32>,
    pub aspect_ratio: f32,
    pub near: f32,
    pub far: f32,
    pub projection_matrix: Matrix4<f32>
}

impl Camera {
    pub fn update_projection_matrix(&mut self) {
        self.projection_matrix = cgmath::perspective(self.fov, self.aspect_ratio, self.near, self.far);
    }
}