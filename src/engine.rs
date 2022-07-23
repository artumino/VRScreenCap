pub mod geometry;
pub mod camera;
pub mod entity;
pub mod vr;

struct EngineContext {}

impl EngineContext {
    pub fn new() -> Self {
        EngineContext {}
    }

    pub fn run(&mut self) {
        
    }
}

pub struct WgpuContext {
    pub instance: wgpu::Instance,
    pub device: wgpu::Device,
    pub physical_device: wgpu::Adapter,
    pub queue: wgpu::Queue,
    pub queue_family_index: u32,
    pub queue_index: u32
}

pub trait WgpuLoader {
    fn load_wgpu(&mut self) -> Option<WgpuContext>;
}