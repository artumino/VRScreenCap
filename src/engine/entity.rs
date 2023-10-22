use cgmath::{Matrix4, Quaternion, SquareMatrix, Vector3, Zero};

pub struct Entity {
    pub id: usize,
    pub parent_id: Option<usize>,
    pub position: Vector3<f32>,
    pub rotation: Quaternion<f32>,
    pub scale: Vector3<f32>,
    pub world_matrix: Matrix4<f32>,
    pub local_matrix: Matrix4<f32>,
    uniform_matrix: ModelUniform,
}

impl Default for Entity {
    #[cfg_attr(feature = "profiling", profiling::function)]
    fn default() -> Self {
        //TODO registry
        Self::new(
            Default::default(),
            Vector3::zero(),
            Quaternion::zero(),
            Vector3::new(1.0, 1.0, 1.0),
        )
    }
}

impl Entity {
    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn new(
        id: usize,
        position: Vector3<f32>,
        rotation: Quaternion<f32>,
        scale: Vector3<f32>,
    ) -> Self {
        let mut entity = Entity {
            id,
            parent_id: None,
            position,
            rotation,
            scale,
            world_matrix: Matrix4::identity(),
            local_matrix: Matrix4::identity(),
            uniform_matrix: ModelUniform::new(),
        };

        entity.update_matrices(&[]);
        entity
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn update_matrices(&mut self, registry: &[Entity]) {
        self.local_matrix = Matrix4::from_translation(self.position)
            * Matrix4::from(self.rotation)
            * Matrix4::from_nonuniform_scale(self.scale.x, self.scale.y, self.scale.z);

        if let Some(parent_id) = self.parent_id {
            self.world_matrix = registry[parent_id].world_matrix * self.local_matrix;
        } else {
            self.world_matrix = self.local_matrix;
        }

        self.uniform_matrix.model_matrix = self.world_matrix.into();
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn uniform(&self) -> ModelUniform {
        self.uniform_matrix
    }
}

// We need this for Rust to store our data correctly for the shaders
#[repr(C)]
// This is so we can store this in a buffer
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelUniform {
    // We can't use cgmath with bytemuck directly so we'll have
    // to convert the Matrix4 into a 4x4 f32 array
    model_matrix: [[f32; 4]; 4],
}

impl ModelUniform {
    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn new() -> Self {
        Self {
            model_matrix: cgmath::Matrix4::identity().into(),
        }
    }
}
