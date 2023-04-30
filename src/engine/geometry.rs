use std::io::{BufReader, Cursor};

use wgpu::util::DeviceExt;

pub trait Vertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a>;
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelVertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}

impl ModelVertex {
    pub const ATTRIBS: &'static [wgpu::VertexAttribute] = &wgpu::vertex_attr_array![0 => Float32x3,
                                    1 => Float32x2,
                                    2 => Float32x3];
}

impl Vertex for ModelVertex {
    #[cfg_attr(feature = "profiling", profiling::function)]
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

pub struct Mesh {
    num_indeces: u32,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

impl Mesh {
    #[cfg_attr(feature = "profiling", profiling::function)]
    fn get_buffers(
        device: &wgpu::Device,
        vertices: &[ModelVertex],
        indices: &[u32],
    ) -> (wgpu::Buffer, wgpu::Buffer) {
        (
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
        )
    }

    pub fn indices(&self) -> u32 {
        self.num_indeces
    }

    pub fn vertex_buffer(&self) -> &wgpu::Buffer {
        &self.vertex_buffer
    }

    pub fn index_buffer(&self) -> &wgpu::Buffer {
        &self.index_buffer
    }

    //TODO: Actually use entity + mesh and world matrix in shader
    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn get_plane_rectangle(
        device: &wgpu::Device,
        rows: u32,
        columns: u32,
        aspect_ratio: f32,
        scale: f32,
        distance: f32,
    ) -> Mesh {
        let mut vertices = vec![];
        let x_increment = 2.0 / (columns as f32);
        let y_increment = 2.0 / (rows as f32);
        for row in 0..rows {
            for column in 0..columns {
                vertices.push(ModelVertex {
                    position: [
                        (-1.0 + (column as f32) * x_increment) * scale * aspect_ratio,
                        (-1.0 + (row as f32) * y_increment) * scale,
                        distance,
                    ],
                    tex_coords: [
                        (column as f32) / ((columns - 1) as f32),
                        1.0 - (row as f32) / ((rows - 1) as f32),
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

        let (vertex_buffer, index_buffer) = Mesh::get_buffers(&device, &vertices, &indices);
        Mesh {
            //FIXME: Handle flipping properly
            num_indeces: indices.len() as u32,
            vertex_buffer,
            index_buffer,
        }
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn from_asset(
        device: &wgpu::Device,
        asset: &'static [u8],
        scale: f32,
        distance: f32,
    ) -> Mesh {
        let obj_cursor = Cursor::new(asset);
        let mut obj_reader = BufReader::new(obj_cursor);
        let (models, _) = tobj::load_obj_buf(
            &mut obj_reader,
            &tobj::LoadOptions {
                triangulate: true,
                single_index: true,
                ..Default::default()
            },
            |_| Err(tobj::LoadError::ReadError),
        )
        .unwrap();

        let mesh = &models[0].mesh;
        let vertices = (0..mesh.positions.len() / 3)
            .map(|i| ModelVertex {
                position: [
                    mesh.positions[i * 3] * scale,
                    mesh.positions[i * 3 + 1] * scale,
                    mesh.positions[i * 3 + 2] * scale + distance,
                ],
                tex_coords: [mesh.texcoords[i * 2], mesh.texcoords[i * 2 + 1]],
            })
            .collect::<Vec<_>>();
        let indices = mesh.indices.clone();
        let (vertex_buffer, index_buffer) = Mesh::get_buffers(&device, &vertices, &indices);
        Mesh {
            num_indeces: indices.len() as u32,
            vertex_buffer: vertex_buffer,
            index_buffer: index_buffer,
        }
    }
}
