use ash::vk;
use wgpu::Texture;

pub mod geometry;
pub mod camera;
pub mod entity;
pub mod vr;
pub mod flat;


pub const TARGET_VULKAN_VERSION: u32 = vk::make_api_version(0, 1, 1, 0);

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
    pub surface_config: wgpu::SurfaceConfiguration,
    pub frame_targets: Vec<Texture>,
    pub frame_index: usize
}

pub trait WgpuLoader {
    fn load_wgpu(&mut self) -> Option<WgpuContext>;
}

pub trait WgpuRunner {
    fn run(&mut self, wgpu_context: &WgpuContext);
}