use anyhow::Context;
use ash::vk::{self, ImageCreateInfo};
use wgpu::{Device, TextureFormat};
use wgpu_hal::{api::Vulkan, MemoryFlags, TextureDescriptor, TextureUses};

use crate::{
    conversions::vulkan_image_to_texture,
    engine::{
        formats::InternalColorFormat,
        texture::{Texture2D, Unbound},
    },
};

pub(crate) struct ExternalTextureInfo {
    pub(crate) external_api: ExternalApi,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) array_size: u32,
    pub(crate) sample_count: u32,
    pub(crate) mip_levels: u32,
    pub(crate) format: InternalColorFormat,
    pub(crate) actual_handle: usize,
}

pub(crate) enum ExternalApi {
    D3D11,
    D3D12,
}

impl ExternalTextureInfo {
    #[cfg_attr(feature = "profiling", profiling::function)]
    pub(crate) fn map_as_wgpu_texture(
        &self,
        label: &str,
        device: &Device,
    ) -> anyhow::Result<Texture2D<Unbound>> {
        let tex_handle = self.actual_handle as vk::HANDLE;
        let vk_format = self.format.try_into()?;
        let raw_image: Option<anyhow::Result<vk::Image>> = unsafe {
            device.as_hal::<Vulkan, _, _>(|device| {
                device.map(|device| {
                    let raw_device = device.raw_device();
                    //let raw_phys_device = device.raw_physical_device();
                    let handle_type = match self.external_api {
                        ExternalApi::D3D11 => {
                            vk::ExternalMemoryHandleTypeFlags::D3D11_TEXTURE_KMT_KHR
                        }
                        ExternalApi::D3D12 => vk::ExternalMemoryHandleTypeFlags::D3D12_RESOURCE_KHR,
                    };

                    let mut ext_create_info =
                        vk::ExternalMemoryImageCreateInfo::builder().handle_types(handle_type);

                    let image_create_info = ImageCreateInfo::builder()
                        .push_next(&mut ext_create_info)
                        //.push_next(&mut dedicated_creation_info)
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(vk_format)
                        .extent(vk::Extent3D {
                            width: self.width,
                            height: self.height,
                            depth: self.array_size,
                        })
                        .mip_levels(self.mip_levels)
                        .array_layers(self.array_size)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::SAMPLED)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE);

                    let raw_image = raw_device.create_image(&image_create_info, None)?;
                    let img_requirements = raw_device.get_image_memory_requirements(raw_image);

                    let mut import_memory_info = vk::ImportMemoryWin32HandleInfoKHR::builder()
                        .handle_type(handle_type)
                        .handle(tex_handle);

                    let allocate_info = vk::MemoryAllocateInfo::builder()
                        .push_next(&mut import_memory_info)
                        .allocation_size(img_requirements.size)
                        .memory_type_index(0);

                    let allocated_memory = raw_device.allocate_memory(&allocate_info, None)?;
                    raw_device.bind_image_memory(raw_image, allocated_memory, 0)?;

                    Ok(raw_image)
                })
            })
        };

        let raw_image = raw_image
            .context("Failed to get hal device")?
            .context("Failed to map external texture")?;

        let wgpu_texture_format: TextureFormat = self.format.try_into()?;
        let texture = vulkan_image_to_texture(
            device,
            raw_image,
            wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: self.width,
                    height: self.height,
                    depth_or_array_layers: self.array_size,
                },
                mip_level_count: self.mip_levels,
                sample_count: self.sample_count,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu_texture_format,
                view_formats: &[],
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            },
            TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: self.width,
                    height: self.height,
                    depth_or_array_layers: self.array_size,
                },
                mip_level_count: self.mip_levels,
                sample_count: self.sample_count,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu_texture_format,
                view_formats: vec![],
                usage: TextureUses::RESOURCE | TextureUses::COPY_SRC,
                memory_flags: MemoryFlags::empty(),
            },
        );

        Ok(Texture2D::<Unbound>::from_wgpu(device, texture))
    }
}
