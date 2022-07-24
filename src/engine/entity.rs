use cgmath::{Vector3, Quaternion, Matrix4, SquareMatrix, Zero};

pub struct Entity {
    pub id: usize,
    pub parent_id: Option<usize>,
    pub position: Vector3<f32>,
    pub rotation: Quaternion<f32>,
    pub scale: Vector3<f32>,
    pub world_matrix: Matrix4<f32>,
    pub local_matrix: Matrix4<f32>
}

impl Default for Entity {
    fn default() -> Self {
        Self { 
            id: Default::default(), //TODO registry
            parent_id: None, 
            position: Vector3::<f32>::zero(), 
            rotation: Quaternion::<f32>::zero(), 
            scale: Vector3::<f32>::new(1.0, 1.0, 1.0), 
            world_matrix: Matrix4::identity(),
            local_matrix: Matrix4::identity()
        }
    }
}

impl Entity {
    pub fn new(id: usize, position: Vector3<f32>, rotation: Quaternion<f32>, scale: Vector3<f32>) -> Self {
        Entity {
            id,
            parent_id: None,
            position,
            rotation,
            scale,
            world_matrix: Matrix4::identity(),
            local_matrix: Matrix4::identity()
        }
    }
    
    pub fn update_matrices(&mut self, registry: &[Entity]) {
        self.local_matrix = Matrix4::from_nonuniform_scale(self.scale.x, self.scale.y, self.scale.z)
            * Matrix4::from(self.rotation)
            * Matrix4::from_translation(self.position);
        
        if let Some(parent_id) = self.parent_id {
            self.world_matrix = registry[parent_id].world_matrix * self.local_matrix;
        } else {
            self.world_matrix = self.local_matrix;
        }
    }
}