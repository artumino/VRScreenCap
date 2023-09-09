use ash::vk;

pub mod camera;
pub mod entity;
pub mod formats;
pub mod geometry;
pub mod input;
pub mod jitter;
pub mod screen;
pub mod space;
pub mod swapchain;
pub mod texture;
pub mod vr;

pub const TARGET_VULKAN_VERSION: u32 = vk::make_api_version(0, 1, 1, 0);

//TODO: Actually modularize engine...

pub struct WgpuContext {
    pub vk_entry: ash::Entry,
    pub vk_instance_ptr: u64,
    pub vk_phys_device_ptr: u64,
    pub vk_device_ptr: u64,
    pub family_queue_index: u32,
    pub instance: wgpu::Instance,
    pub device: wgpu::Device,
    pub physical_device: wgpu::Adapter,
    pub queue: wgpu::Queue,
    debug_utils: Option<ash::extensions::ext::DebugUtils>,
    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
}

pub trait WgpuLoader {
    fn load_wgpu(&mut self) -> anyhow::Result<WgpuContext>;
}

pub trait WgpuRunner {
    fn run(&mut self, wgpu_context: &WgpuContext);
}
