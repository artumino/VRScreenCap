use anyhow::{bail, Context};
use ash::vk::{self, ImageCreateInfo};
use wgpu::{Device, Instance, Queue, TextureFormat};
use wgpu_hal::{api::Vulkan, MemoryFlags, TextureDescriptor, TextureUses};
use windows::{
    core::{s, w, PCWSTR},
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        Graphics::{
            Direct3D::D3D_DRIVER_TYPE_HARDWARE,
            Direct3D11::{
                D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D,
                D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
            },
            Direct3D12::{D3D12CreateDevice, ID3D12Device, ID3D12Resource},
        },
        System::Memory::{
            MapViewOfFile, OpenFileMappingA, UnmapViewOfFile, FILE_MAP_READ,
            MEMORYMAPPEDVIEW_HANDLE,
        },
    },
};

use crate::{
    conversions::vulkan_image_to_texture,
    engine::{
        formats::InternalColorFormat,
        texture::{Bound, Texture2D, Unbound},
    },
    loaders::StereoMode,
};

use super::{Loader, TextureSource};

pub struct KatangaLoaderContext {
    katanga_file_handle: HANDLE,
    katanga_file_mapping: MEMORYMAPPEDVIEW_HANDLE,
    current_address: usize,
    d3d11: Option<D3D11Context>,
    d3d12: Option<D3D12Context>,
}

impl KatangaLoaderContext {
    fn unmap(&mut self) {
        if self.katanga_file_mapping.is_invalid()
            || bool::from(unsafe { UnmapViewOfFile(self.katanga_file_mapping) })
        {
            self.katanga_file_mapping = MEMORYMAPPEDVIEW_HANDLE::default();
        }

        if self.katanga_file_handle.is_invalid()
            || bool::from(unsafe { CloseHandle(self.katanga_file_handle) })
        {
            self.katanga_file_handle = HANDLE::default();
        }
    }

    fn map_katanga_file(&mut self) -> anyhow::Result<()> {
        if !self.katanga_file_handle.is_invalid() && !self.katanga_file_mapping.is_invalid() {
            return Ok(());
        }

        self.unmap();

        self.katanga_file_handle = match unsafe {
            OpenFileMappingA(FILE_MAP_READ.0, false, s!("Local\\KatangaMappedFile"))
        } {
            Ok(handle) => handle,
            Err(_) => {
                self.unmap();
                bail!("Cannot open file mapping!")
            }
        };
        log::trace!("Handle: {:?}", self.katanga_file_handle);

        self.katanga_file_mapping = match unsafe {
            MapViewOfFile(
                self.katanga_file_handle,
                FILE_MAP_READ,
                0,
                0,
                std::mem::size_of::<usize>(),
            )
        } {
            Ok(handle) => handle,
            Err(_) => {
                self.unmap();
                bail!("Cannot map file view!")
            }
        };

        if self.katanga_file_mapping.is_invalid() {
            self.unmap();
            bail!("Cannot map file view!");
        }

        Ok(())
    }
}

impl Default for KatangaLoaderContext {
    fn default() -> Self {
        Self {
            d3d11: D3D11Context::new().ok(),
            d3d12: D3D12Context::new().ok(),
            katanga_file_handle: HANDLE::default(),
            katanga_file_mapping: MEMORYMAPPEDVIEW_HANDLE::default(),
            current_address: 0,
        }
    }
}

struct D3D11Context {
    device: ID3D11Device,
    _device_context: ID3D11DeviceContext,
}

impl D3D11Context {
    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn new() -> anyhow::Result<Self> {
        let mut device = None;
        let mut device_context = None;
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                D3D11_CREATE_DEVICE_FLAG(0),
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut device_context),
            )?
        };

        Ok(Self {
            device: device.context("Failed to create D3D11 device")?,
            _device_context: device_context.context("Failed to create D3D11 device context")?,
        })
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    fn get_d3d11_texture_info(&self, handle: HANDLE) -> anyhow::Result<ExternalTextureInfo> {
        let mut d3d11_texture: Option<ID3D11Texture2D> = None;
        unsafe { self.device.OpenSharedResource(handle, &mut d3d11_texture) }?;
        let d3d11_texture = d3d11_texture.context("Failed to open shared DX11 texture")?;
        let mut texture_desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { d3d11_texture.GetDesc(&mut texture_desc) };

        let format: InternalColorFormat = texture_desc.Format.try_into()?;
        log::info!("Got texture from DX11 with format {:?}", format);

        Ok(ExternalTextureInfo {
            external_api: ExternalApi::D3D11,
            width: texture_desc.Width,
            height: texture_desc.Height,
            array_size: texture_desc.ArraySize,
            sample_count: texture_desc.SampleDesc.Count,
            mip_levels: texture_desc.MipLevels,
            format: format.try_into()?,
            actual_handle: handle.0 as usize,
        })
    }
}

struct D3D12Context {
    device: ID3D12Device,
}

impl D3D12Context {
    pub fn new() -> anyhow::Result<Self> {
        let mut d3d12_device: Option<ID3D12Device> = None;
        unsafe {
            D3D12CreateDevice(
                None,
                windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_12_0,
                &mut d3d12_device,
            )
        }?;

        let d3d12_device = d3d12_device.context("DX12 device not initialized")?;

        Ok(Self {
            device: d3d12_device,
        })
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    fn get_d3d12_named_texture_info(
        &self,
        texture_name: PCWSTR,
    ) -> anyhow::Result<ExternalTextureInfo> {
        let named_handle = unsafe {
            self.device.OpenSharedHandleByName(texture_name, 0x10000000) //GENERIC_ALL
        }?;

        let mut d3d12_texture: Option<ID3D12Resource> = None;
        unsafe {
            self.device
                .OpenSharedHandle(named_handle, &mut d3d12_texture)
        }?;
        let d3d12_texture = d3d12_texture.context("Failed to open shared DX12 texture")?;

        let tex_info = unsafe { d3d12_texture.GetDesc() };

        let format: InternalColorFormat = tex_info.Format.try_into()?;
        log::info!("Got texture from DX12 with format {:?}", format);

        Ok(ExternalTextureInfo {
            external_api: ExternalApi::D3D12,
            width: tex_info.Width as u32,
            height: tex_info.Height,
            array_size: tex_info.DepthOrArraySize as u32,
            sample_count: tex_info.SampleDesc.Count,
            mip_levels: tex_info.MipLevels as u32,
            format: format.try_into()?,
            actual_handle: named_handle.0 as usize,
        })
    }
}

impl Loader for KatangaLoaderContext {
    #[cfg_attr(feature = "profiling", profiling::function)]
    fn load(&mut self, _instance: &Instance, device: &Device) -> anyhow::Result<TextureSource> {
        self.map_katanga_file()?;

        let address = unsafe { *(self.katanga_file_mapping.0 as *mut usize) };
        self.current_address = address;
        let tex_handle = self.current_address as vk::HANDLE;
        log::info!("{:#01x}", tex_handle as usize);

        let tex_info = self
            .d3d11
            .as_ref()
            .map(|d3d11| d3d11.get_d3d11_texture_info(HANDLE(tex_handle as isize)))
            .unwrap_or(Err(anyhow::anyhow!("No D3D11 device found")))
            .or_else(|_| {
                self.d3d12
                    .as_ref()
                    .map(|d3d12| d3d12.get_d3d12_named_texture_info(w!("DX12VRStream")))
                    .unwrap_or(Err(anyhow::anyhow!("No D3D11 device found")))
            })?;

        let tex_handle = tex_info.actual_handle as vk::HANDLE;
        if tex_info.actual_handle != self.current_address {
            log::info!("Actual Handle: {:?}", self.katanga_file_handle);
        }

        let format: InternalColorFormat = tex_info.format.try_into()?;
        log::info!("Mapped DXGI format to {:?}", format);

        let vk_format = format.try_into()?;
        log::info!("Mapped WGPU format to Vulkan {:?}", vk_format);

        let raw_image: Option<anyhow::Result<vk::Image>> = unsafe {
            device.as_hal::<Vulkan, _, _>(|device| {
                device.map(|device| {
                    let raw_device = device.raw_device();
                    //let raw_phys_device = device.raw_physical_device();
                    let handle_type = match tex_info.external_api {
                        ExternalApi::D3D11 => {
                            vk::ExternalMemoryHandleTypeFlags::D3D11_TEXTURE_KMT_KHR
                        }
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
                        .usage(vk::ImageUsageFlags::TRANSFER_SRC | vk::ImageUsageFlags::SAMPLED)
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
                    view_formats: &[],
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
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
                    view_formats: vec![],
                    usage: TextureUses::RESOURCE | TextureUses::COPY_SRC,
                    memory_flags: MemoryFlags::empty(),
                },
            );

            return Ok(TextureSource {
                texture: Texture2D::<Unbound>::from_wgpu(device, texture),
                width: tex_info.width,
                height: tex_info.height,
                stereo_mode: Some(StereoMode::FullSbs),
            });
        }

        bail!("Cannot open shared texture!")
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    fn is_invalid(&self) -> bool {
        if self.katanga_file_mapping.is_invalid() || self.katanga_file_handle.is_invalid() {
            return true;
        }

        let address = unsafe { *(self.katanga_file_mapping.0 as *mut usize) };
        self.current_address != address
    }

    // No update needed for Katanga
    #[cfg_attr(feature = "profiling", profiling::function)]
    fn update(
        &mut self,
        _instance: &Instance,
        _device: &Device,
        _queue: &Queue,
        _texture: &Texture2D<Bound>,
    ) -> anyhow::Result<()> {
        self.unmap();
        self.map_katanga_file()?;
        Ok(())
    }
}

impl Drop for KatangaLoaderContext {
    #[cfg_attr(feature = "profiling", profiling::function)]
    fn drop(&mut self) {
        log::info!("Dropping KatangaLoaderContext");
        self.unmap();
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
