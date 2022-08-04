use std::{error::Error, ffi::c_void, ptr};

use ash::vk::{self, ImageCreateInfo};
use wgpu::{Device, Instance, TextureFormat};
use wgpu_hal::{api::Vulkan, MemoryFlags, TextureDescriptor, TextureUses};
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    Graphics::{
        Direct3D::D3D_DRIVER_TYPE_HARDWARE,
        Direct3D11::{
            D3D11CreateDevice, ID3D11Texture2D, D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION,
            D3D11_TEXTURE2D_DESC,
        }
    },
    System::Memory::{MapViewOfFile, OpenFileMappingA, UnmapViewOfFile, FILE_MAP_ALL_ACCESS},
};

use crate::conversions::{vulkan_image_to_texture, map_texture_format, unmap_texture_format};

use super::{Loader, TextureSource};

pub struct KatangaLoaderContext {
    katanga_file_handle: HANDLE,
    katanga_file_mapping: *mut c_void,
    current_address: usize,
}

impl Loader for KatangaLoaderContext {
    fn load(
        &mut self,
        _instance: &Instance,
        device: &Device,
    ) -> Result<TextureSource, Box<dyn Error>> {
        self.katanga_file_handle =
            unsafe { OpenFileMappingA(FILE_MAP_ALL_ACCESS.0, false, "Local\\KatangaMappedFile")? };
        log::info!("Handle: {:?}", self.katanga_file_handle);

        self.katanga_file_mapping =
            unsafe { MapViewOfFile(self.katanga_file_handle, FILE_MAP_ALL_ACCESS, 0, 0, 4) };
        if self.katanga_file_mapping.is_null() {
            return Err("Cannot map file!".into());
        }

        let address = unsafe { *(self.katanga_file_mapping as *mut usize) };
        self.current_address = address | 0xFFFFFFFF00000000;
        let tex_handle = self.current_address as vk::HANDLE;
        log::info!("{:#01x}", tex_handle as usize);

        let tex_info = get_d3d11_texture_info(HANDLE(tex_handle as isize))?;
        let vk_format = map_texture_format(tex_info.format);

        log::info!("Mapped DXGI format to {:?}", tex_info.format);
        log::info!("Mapped WGPU format to Vulkan {:?}", vk_format);

        let raw_image: Option<Result<vk::Image, Box<dyn Error>>> = unsafe {
            device.as_hal::<Vulkan, _, _>(|device| {
                device.map(|device| {
                    let raw_device = device.raw_device();
                    //let raw_phys_device = device.raw_physical_device();
                    let handle_type = vk::ExternalMemoryHandleTypeFlags::D3D11_TEXTURE_KMT;

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
                        .sharing_mode(vk::SharingMode::EXCLUSIVE);

                    let raw_image = raw_device.create_image(&image_create_info, None)?;

                    raw_device.bind_image_memory(raw_image, allocated_memory, 0)?;

                    Ok(raw_image)
                })
            })
        };

        if let Some(Ok(raw_image)) = raw_image {
            let texture = vulkan_image_to_texture(device, raw_image, 
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
                    usage: TextureUses::EXCLUSIVE,
                    memory_flags: MemoryFlags::empty(),
            });

            return Ok(TextureSource {
                texture,
                width: tex_info.width,
                height: tex_info.height,
                stereo_mode: crate::loaders::StereoMode::FSBS,
            });
        }

        return Err("Cannot open shared texture!".into());
    }

    fn is_invalid(&self) -> bool {
        let address = unsafe { *(self.katanga_file_mapping as *mut usize) } | 0xFFFFFFFF00000000;
        return self.current_address != address;
    }
}

impl Default for KatangaLoaderContext {
    fn default() -> Self {
        Self {
            katanga_file_handle: Default::default(),
            katanga_file_mapping: ptr::null_mut(),
            current_address: 0,
        }
    }
}

impl Drop for KatangaLoaderContext {
    fn drop(&mut self) {
        log::info!("Dropping KatangaLoaderContext");

        if !self.katanga_file_mapping.is_null()
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

struct D3D11TextureInfoAdapter {
    width: u32,
    height: u32,
    array_size: u32,
    sample_count: u32,
    mip_levels: u32,
    format: TextureFormat,
}

fn get_d3d11_texture_info(handle: HANDLE) -> Result<D3D11TextureInfoAdapter, Box<dyn Error>> {
    let mut d3d11_device = None;
    let mut d3d11_device_context = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            None,
            D3D11_CREATE_DEVICE_FLAG(0),
            vec![].as_slice(),
            D3D11_SDK_VERSION,
            &mut d3d11_device,
            ptr::null_mut(),
            &mut d3d11_device_context,
        )?
    };
    let mut d3d11_texture: Option<ID3D11Texture2D> = None;
    unsafe {
        d3d11_device
            .unwrap()
            .OpenSharedResource(handle, &mut d3d11_texture)
    }?;
    let mut texture_desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { d3d11_texture.unwrap().GetDesc(&mut texture_desc) };

    log::info!("Got texture from DX11 with format {:?}", texture_desc.Format);

    Ok(D3D11TextureInfoAdapter {
        width: texture_desc.Width,
        height: texture_desc.Height,
        array_size: texture_desc.ArraySize,
        sample_count: texture_desc.SampleDesc.Count,
        mip_levels: texture_desc.MipLevels,
        format: unmap_texture_format(texture_desc.Format),
    })
}