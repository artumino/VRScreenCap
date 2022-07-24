use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}

impl Vertex {
    pub const ATTRIBS: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2];

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

impl Mesh {
    pub fn get_buffers(&self, device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer) {
        (
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(self.vertices.as_slice()),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(self.indices.as_slice()),
                usage: wgpu::BufferUsages::INDEX,
            }),
        )
    }

    pub fn indices(&self) -> u32 {
        self.indices.len() as u32
    }

    #[allow(unused)]
    pub fn get_rectangle(aspect_ratio: f32, scale: f32, distance: f32) -> Mesh {
        Mesh {
            //FIXME: Handle flipping properly
            vertices: vec![
                Vertex {
                    position: [-1.0 * scale * aspect_ratio, -1.0 * scale, distance],
                    tex_coords: [0.0, 1.0],
                },
                Vertex {
                    position: [-1.0 * scale * aspect_ratio, 1.0 * scale, distance],
                    tex_coords: [0.0, 0.0],
                },
                Vertex {
                    position: [1.0 * scale * aspect_ratio, 1.0 * scale, distance],
                    tex_coords: [1.0, 0.0],
                },
                Vertex {
                    position: [1.0 * scale * aspect_ratio, -1.0 * scale, distance],
                    tex_coords: [1.0, 1.0],
                },
            ],
            indices: QUAD_INDICES.to_vec(),
        }
    }

    //TODO: Actually use entity + mesh and world matrix in shader
    pub fn get_plane_rectangle(
        rows: u16,
        columns: u16,
        aspect_ratio: f32,
        scale: f32,
        distance: f32,
    ) -> Mesh {
        let mut vertices = vec![];
        let x_increment = 2.0 / (columns as f32);
        let y_increment = 2.0 / (columns as f32);
        for row in 0..rows {
            for column in 0..columns {
                vertices.push(Vertex {
                    position: [
                        (-1.0 + (column as f32) * x_increment) * scale * aspect_ratio,
                        (-1.0 + (row as f32) * y_increment) * scale,
                        distance,
                    ],
                    tex_coords: [
                        (column as f32) / (columns as f32),
                        1.0 - (row as f32) / (rows as f32),
                    ],
                });
            }
        }

        let mut indices = vec![];
        for row in 0..rows - 1 {
            for column in 0..columns - 1 {
                indices.push(row * columns + column);
                indices.push(row * columns + column + 1);
                indices.push((row + 1) * columns + column);
                indices.push((row + 1) * columns + column);
                indices.push(row * columns + column + 1);
                indices.push((row + 1) * columns + column + 1);
            }
        }

        Mesh {
            //FIXME: Handle flipping properly
            vertices: vertices,
            indices: indices,
        }
    }
}

#[allow(unused)]
pub const QUAD_INDICES: &[u16] = &[0, 2, 3, 0, 1, 2];
