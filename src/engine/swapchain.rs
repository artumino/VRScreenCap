use ash::vk::{self, Handle};
use wgpu::{Device, Extent3d};

use super::{
    formats::InternalColorFormat,
    texture::{Texture2D, Unbound},
};

pub struct Swapchain {
    internal_swapchain: openxr::Swapchain<openxr::Vulkan>,
    textures: Vec<Texture2D<Unbound>>,
}

pub struct SwapchainCreationInfo {
    pub resolution: vk::Extent2D,
    pub vk_format: vk::Format,
    pub texture_format: InternalColorFormat,
    pub usage_flags: openxr::SwapchainUsageFlags,
    pub view_count: u32,
}

impl Swapchain {
    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn new(
        label: &'static str,
        xr_session: &openxr::Session<openxr::Vulkan>,
        device: &Device,
        creation_info: SwapchainCreationInfo,
    ) -> anyhow::Result<Self> {
        let SwapchainCreationInfo {
            resolution,
            vk_format,
            texture_format,
            usage_flags,
            view_count,
        } = creation_info;

        let xr_swapchain = xr_session.create_swapchain(&openxr::SwapchainCreateInfo {
            create_flags: openxr::SwapchainCreateFlags::EMPTY,
            usage_flags,
            format: vk_format.as_raw() as _,
            sample_count: 1,
            width: resolution.width,
            height: resolution.height,
            face_count: 1,
            array_size: view_count,
            mip_count: 1,
        })?;
        let swapcain_textures: Vec<_> = xr_swapchain
            .enumerate_images()?
            .into_iter()
            .map(vk::Image::from_raw)
            .enumerate()
            .filter_map(|(idx, image)| {
                Texture2D::<Unbound>::from_vk_image(
                    format!("{} {}", label, idx).as_str(),
                    device,
                    image,
                    Extent3d {
                        width: resolution.width,
                        height: resolution.height,
                        depth_or_array_layers: view_count,
                    },
                    texture_format,
                )
                .ok()
            })
            .collect();
        Ok(Self {
            internal_swapchain: xr_swapchain,
            textures: swapcain_textures,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }

    pub fn internal(&self) -> &openxr::Swapchain<openxr::Vulkan> {
        &self.internal_swapchain
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn wait_next_image(&mut self) -> Result<&wgpu::TextureView, anyhow::Error> {
        let image_index = self.internal_swapchain.acquire_image()?;
        self.internal_swapchain
            .wait_image(openxr::Duration::INFINITE)?;
        Ok(&self.textures[image_index as usize].view)
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn release_image(&mut self) -> Result<(), anyhow::Error> {
        self.internal_swapchain.release_image()?;
        Ok(())
    }
}
