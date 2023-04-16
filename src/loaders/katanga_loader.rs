use anyhow::{bail, Context};
use ash::vk::{self, ImageCreateInfo};
use wgpu::{Device, Instance, TextureFormat};
use wgpu_hal::{api::Vulkan, MemoryFlags, TextureDescriptor, TextureUses};
use windows::{Win32::{
    Foundation::{CloseHandle, HANDLE},
    Graphics::{
        Direct3D::D3D_DRIVER_TYPE_HARDWARE,
        Direct3D11::{
            D3D11CreateDevice, ID3D11Texture2D, D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION,
            D3D11_TEXTURE2D_DESC,
        },
        Direct3D12::{D3D12CreateDevice, ID3D12Device, ID3D12Resource},
    },
    System::Memory::{MapViewOfFile, OpenFileMappingA, UnmapViewOfFile, FILE_MAP_ALL_ACCESS, MEMORYMAPPEDVIEW_HANDLE},
}, core::s, core::w};

use crate::{conversions::{map_texture_format, unmap_texture_format, vulkan_image_to_texture}, engine::texture::Texture2D};

use super::{Loader, TextureSource};

#[derive(Default)]
pub struct KatangaLoaderContext {
    katanga_file_handle: HANDLE,
    katanga_file_mapping: MEMORYMAPPEDVIEW_HANDLE,
    current_address: usize,
}

impl Loader for KatangaLoaderContext {
    fn load(
        &mut self,
        _instance: &Instance,
        device: &Device,
    ) -> anyhow::Result<TextureSource> {
        self.katanga_file_handle =
            unsafe { OpenFileMappingA(FILE_MAP_ALL_ACCESS.0, false, s!("Local\\KatangaMappedFile"))? };
        log::info!("Handle: {:?}", self.katanga_file_handle);

        self.katanga_file_mapping = unsafe {
            MapViewOfFile(
                self.katanga_file_handle,
                FILE_MAP_ALL_ACCESS,
                0,
                0,
                std::mem::size_of::<usize>(),
            )?
        };

        if self.katanga_file_mapping.is_invalid() {
            bail!("Cannot map file!");
        }

        let address = unsafe { *(self.katanga_file_mapping.0 as *mut usize) };
        self.current_address = address;
        let tex_handle = self.current_address as vk::HANDLE;
        log::info!("{:#01x}", tex_handle as usize);

        let tex_info = get_d3d11_texture_info(HANDLE(tex_handle as isize)).or_else(|err| {
            log::warn!("Not a D3D11Texture {}", err);
            get_d3d12_texture_info().map_err(|err| {
                log::warn!("Not a D3D12Texture {}", err);
                err
            })
        })?;

        let tex_handle = tex_info.actual_handle as vk::HANDLE;
        if tex_info.actual_handle != self.current_address {
            log::info!("Actual Handle: {:?}", self.katanga_file_handle);
        }

        let vk_format = map_texture_format(tex_info.format);

        log::info!("Mapped DXGI format to {:?}", tex_info.format);
        log::info!("Mapped WGPU format to Vulkan {:?}", vk_format);

        let raw_image: Option<anyhow::Result<vk::Image>> = unsafe {
            device.as_hal::<Vulkan, _, _>(|device| {
                device.map(|device| {
                    let raw_device = device.raw_device();
                    //let raw_phys_device = device.raw_physical_device();
                    let handle_type = match tex_info.external_api {
                        ExternalApi::D3D11 => vk::ExternalMemoryHandleTypeFlags::D3D11_TEXTURE_KMT_KHR,
                        ExternalApi::D3D12 => vk::ExternalMemoryHandleTypeFlags::D3D12_RESOURCE_KHR,
                    };

                    let mut import_memory_info = vk::ImportMemoryWin32HandleInfoKHR::builder()
                        .handle_type(handle_type)
                        .handle(tex_handle);

                    let allocate_info = vk::MemoryAllocateInfo::builder()
                        .push_next(&mut import_memory_info)
                        .memory_type_index(0);

                    let allocated_memory = raw_device.allocate_memory(&allocate_info, None)?;

                    let mut ext_create_info =
                        vk::ExternalMemoryImageCreateInfo::builder().handle_types(handle_type);

                    let image_create_info = ImageCreateInfo::builder()
                        .push_next(&mut ext_create_info)
                        //.push_next(&mut dedicated_creation_info)
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(vk_format)
                        .extent(vk::Extent3D {
                            width: tex_info.width,
                            height: tex_info.height,
                            depth: tex_info.array_size,
                        })
                        .mip_levels(tex_info.mip_levels)
                        .array_layers(tex_info.array_size)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                        .sharing_mode(vk::SharingMode::CONCURRENT);

                    let raw_image = raw_device.create_image(&image_create_info, None)?;

                    raw_device.bind_image_memory(raw_image, allocated_memory, 0)?;

                    Ok(raw_image)
                })
            })
        };

        if let Some(Ok(raw_image)) = raw_image {
            let texture = vulkan_image_to_texture(
                device,
                raw_image,
                wgpu::TextureDescriptor {
                    label: "KatangaStream".into(),
                    size: wgpu::Extent3d {
                        width: tex_info.width,
                        height: tex_info.height,
                        depth_or_array_layers: tex_info.array_size,
                    },
                    mip_level_count: tex_info.mip_levels,
                    sample_count: tex_info.sample_count,
                    dimension: wgpu::TextureDimension::D2,
                    format: tex_info.format,
                    view_formats:  &[],
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                },
                TextureDescriptor {
                    label: "KatangaStream".into(),
                    size: wgpu::Extent3d {
                        width: tex_info.width,
                        height: tex_info.height,
                        depth_or_array_layers: tex_info.array_size,
                    },
                    mip_level_count: tex_info.mip_levels,
                    sample_count: tex_info.sample_count,
                    dimension: wgpu::TextureDimension::D2,
                    format: tex_info.format,
                    view_formats:  vec!(),
                    usage: TextureUses::EXCLUSIVE,
                    memory_flags: MemoryFlags::empty(),
                },
            );

            return Ok(TextureSource {
                texture: Texture2D::from_wgpu(device, texture),
                width: tex_info.width,
                height: tex_info.height,
                stereo_mode: crate::loaders::StereoMode::FullSbs,
            });
        }

        bail!("Cannot open shared texture!")
    }

    fn is_invalid(&self) -> bool {
        let address = unsafe { *(self.katanga_file_mapping.0 as *mut usize) };
        self.current_address != address
    }
}

impl Drop for KatangaLoaderContext {
    fn drop(&mut self) {
        log::info!("Dropping KatangaLoaderContext");

        if !self.katanga_file_mapping.is_invalid()
            && unsafe { bool::from(UnmapViewOfFile(self.katanga_file_mapping)) }
        {
            log::info!("Unmapped file!");
        }

        if !self.katanga_file_handle.is_invalid()
            && unsafe { bool::from(CloseHandle(self.katanga_file_handle)) }
        {
            log::info!("Closed handle!");
        }
    }
}

struct ExternalTextureInfo {
    external_api: ExternalApi,
    width: u32,
    height: u32,
    array_size: u32,
    sample_count: u32,
    mip_levels: u32,
    format: TextureFormat,
    actual_handle: usize,
}

enum ExternalApi {
    D3D11,
    D3D12,
}

fn get_d3d11_texture_info(handle: HANDLE) -> anyhow::Result<ExternalTextureInfo> {
    let mut d3d11_device = None;
    let mut d3d11_device_context = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            None,
            D3D11_CREATE_DEVICE_FLAG(0),
            None,
            D3D11_SDK_VERSION,
            Some(&mut d3d11_device),
            None,
            Some(&mut d3d11_device_context),
        )?
    };
    let mut d3d11_texture: Option<ID3D11Texture2D> = None;
    unsafe {
        d3d11_device.as_ref()
            .context("DX11 device not initialized")?
            .OpenSharedResource(handle, &mut d3d11_texture)
    }?;
    let d3d11_texture = d3d11_texture.context("Failed to open shared DX11 texture")?;
    let mut texture_desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { d3d11_texture.GetDesc(&mut texture_desc) };

    log::info!(
        "Got texture from DX11 with format {:?}",
        texture_desc.Format
    );

    Ok(ExternalTextureInfo {
        external_api: ExternalApi::D3D11,
        width: texture_desc.Width,
        height: texture_desc.Height,
        array_size: texture_desc.ArraySize,
        sample_count: texture_desc.SampleDesc.Count,
        mip_levels: texture_desc.MipLevels,
        format: unmap_texture_format(texture_desc.Format),
        actual_handle: handle.0 as usize
    })
}

fn get_d3d12_texture_info() -> anyhow::Result<ExternalTextureInfo> {
    let mut d3d12_device: Option<ID3D12Device> = None;
    unsafe {
        D3D12CreateDevice(
            None,
            windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_12_0,
            &mut d3d12_device,
        )
    }?;

    let d3d12_device = d3d12_device.as_ref()
        .context("DX12 device not initialized")?;

    let named_handle = unsafe {
            d3d12_device.OpenSharedHandleByName(w!("DX12VRStream"), 0x10000000) //GENERIC_ALL
    }?;

    let mut d3d12_texture: Option<ID3D12Resource> = None;
    unsafe {
        d3d12_device.OpenSharedHandle(named_handle, &mut d3d12_texture)
    }?;
    let d3d12_texture = d3d12_texture.context("Failed to open shared DX12 texture")?;

    let tex_info = unsafe { d3d12_texture.GetDesc() };

    log::info!("Got texture from DX12 with format {:?}", tex_info.Format);

    Ok(ExternalTextureInfo {
        external_api: ExternalApi::D3D12,
        width: tex_info.Width as u32,
        height: tex_info.Height,
        array_size: tex_info.DepthOrArraySize as u32,
        sample_count: tex_info.SampleDesc.Count,
        mip_levels: tex_info.MipLevels as u32,
        format: unmap_texture_format(tex_info.Format),
        actual_handle: named_handle.0 as usize
    })
}
