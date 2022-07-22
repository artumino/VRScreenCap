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
        },
        Dxgi::Common::{
            DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
            DXGI_FORMAT_BC1_UNORM, DXGI_FORMAT_BC1_UNORM_SRGB, DXGI_FORMAT_BC2_UNORM,
            DXGI_FORMAT_BC2_UNORM_SRGB, DXGI_FORMAT_BC3_UNORM, DXGI_FORMAT_BC3_UNORM_SRGB,
            DXGI_FORMAT_BC4_SNORM, DXGI_FORMAT_BC4_UNORM, DXGI_FORMAT_BC5_SNORM,
            DXGI_FORMAT_BC5_UNORM, DXGI_FORMAT_BC6H_SF16, DXGI_FORMAT_BC6H_UF16,
            DXGI_FORMAT_BC7_UNORM, DXGI_FORMAT_BC7_UNORM_SRGB, DXGI_FORMAT_D24_UNORM_S8_UINT,
            DXGI_FORMAT_D32_FLOAT, DXGI_FORMAT_D32_FLOAT_S8X24_UINT, DXGI_FORMAT_R10G10B10A2_UNORM,
            DXGI_FORMAT_R11G11B10_FLOAT, DXGI_FORMAT_R16G16B16A16_FLOAT,
            DXGI_FORMAT_R16G16B16A16_SINT, DXGI_FORMAT_R16G16B16A16_SNORM,
            DXGI_FORMAT_R16G16B16A16_UINT, DXGI_FORMAT_R16G16B16A16_UNORM,
            DXGI_FORMAT_R16G16_FLOAT, DXGI_FORMAT_R16G16_SINT, DXGI_FORMAT_R16G16_SNORM,
            DXGI_FORMAT_R16G16_UINT, DXGI_FORMAT_R16G16_UNORM, DXGI_FORMAT_R16_FLOAT,
            DXGI_FORMAT_R16_SINT, DXGI_FORMAT_R16_SNORM, DXGI_FORMAT_R16_UINT,
            DXGI_FORMAT_R16_UNORM, DXGI_FORMAT_R32G32B32A32_FLOAT, DXGI_FORMAT_R32G32B32A32_SINT,
            DXGI_FORMAT_R32G32B32A32_UINT, DXGI_FORMAT_R32G32_FLOAT, DXGI_FORMAT_R32G32_SINT,
            DXGI_FORMAT_R32G32_UINT, DXGI_FORMAT_R32_FLOAT, DXGI_FORMAT_R32_SINT,
            DXGI_FORMAT_R32_UINT, DXGI_FORMAT_R8G8B8A8_SINT, DXGI_FORMAT_R8G8B8A8_SNORM,
            DXGI_FORMAT_R8G8B8A8_UINT, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
            DXGI_FORMAT_R8G8_SINT, DXGI_FORMAT_R8G8_SNORM, DXGI_FORMAT_R8G8_UINT,
            DXGI_FORMAT_R8G8_UNORM, DXGI_FORMAT_R8_SINT, DXGI_FORMAT_R8_SNORM, DXGI_FORMAT_R8_UINT,
            DXGI_FORMAT_R8_UNORM, DXGI_FORMAT_R9G9B9E5_SHAREDEXP,
        },
    },
    System::Memory::{MapViewOfFile, OpenFileMappingA, UnmapViewOfFile, FILE_MAP_ALL_ACCESS},
};

use super::{Loader, TextureSource};

pub struct KatangaLoaderContext {
    katanga_file_handle: HANDLE,
    katanga_file_mapping: *mut c_void,
}

impl Loader for KatangaLoaderContext {
    fn load(
        &mut self,
        _instance: &Instance,
        device: &Device,
    ) -> Result<TextureSource, Box<dyn Error>> {
        self.katanga_file_handle =
            unsafe { OpenFileMappingA(FILE_MAP_ALL_ACCESS.0, false, "Local\\KatangaMappedFile")? };
        println!("Handle: {:?}", self.katanga_file_handle);

        self.katanga_file_mapping =
            unsafe { MapViewOfFile(self.katanga_file_handle, FILE_MAP_ALL_ACCESS, 0, 0, 4) };
        if self.katanga_file_mapping.is_null() {
            return Err("Cannot map file!".into());
        }

        let address = unsafe { *(self.katanga_file_mapping as *mut usize) };
        let tex_handle = (address | 0xFFFFFFFF00000000) as vk::HANDLE;
        println!("{:#01x}", tex_handle as usize);

        let tex_info = get_d3d11_texture_info(HANDLE(tex_handle as isize))?;

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
                        .format(map_texture_format(tex_info.format))
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
            let text_descriptor = TextureDescriptor {
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
            };

            let texture = unsafe {
                <wgpu_hal::api::Vulkan as wgpu_hal::Api>::Device::texture_from_raw(
                    raw_image,
                    &text_descriptor,
                    None,
                )
            };

            let texture = unsafe {
                device.create_texture_from_hal::<Vulkan>(
                    texture,
                    &wgpu::TextureDescriptor {
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
                )
            };

            return Ok(TextureSource {
                texture,
                width: tex_info.width,
                height: tex_info.height,
                stereo_mode: crate::loaders::StereoMode::FSBS,
            });
        }

        return Err("Cannot open shared texture!".into());
    }
}

impl Default for KatangaLoaderContext {
    fn default() -> Self {
        Self {
            katanga_file_handle: Default::default(),
            katanga_file_mapping: ptr::null_mut(),
        }
    }
}

impl Drop for KatangaLoaderContext {
    fn drop(&mut self) {
        println!("Dropping KatangaLoaderContext");

        if !self.katanga_file_mapping.is_null()
            && unsafe { bool::from(UnmapViewOfFile(self.katanga_file_mapping)) }
        {
            println!("Unmapped file!");
        }

        if !self.katanga_file_handle.is_invalid()
            && unsafe { bool::from(CloseHandle(self.katanga_file_handle)) }
        {
            println!("Closed handle!");
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

    Ok(D3D11TextureInfoAdapter {
        width: texture_desc.Width,
        height: texture_desc.Height,
        array_size: texture_desc.ArraySize,
        sample_count: texture_desc.SampleDesc.Count,
        mip_levels: texture_desc.MipLevels,
        format: unmap_texture_format(texture_desc.Format),
    })
}

pub fn unmap_texture_format(format: DXGI_FORMAT) -> TextureFormat {
    match format {
        DXGI_FORMAT_R8_UNORM => TextureFormat::R8Unorm,
        DXGI_FORMAT_R8_SNORM => TextureFormat::R8Snorm,
        DXGI_FORMAT_R8_UINT => TextureFormat::R8Uint,
        DXGI_FORMAT_R8_SINT => TextureFormat::R8Sint,
        DXGI_FORMAT_R16_UINT => TextureFormat::R16Uint,
        DXGI_FORMAT_R16_SINT => TextureFormat::R16Sint,
        DXGI_FORMAT_R16_UNORM => TextureFormat::R16Unorm,
        DXGI_FORMAT_R16_SNORM => TextureFormat::R16Snorm,
        DXGI_FORMAT_R16_FLOAT => TextureFormat::R16Float,
        DXGI_FORMAT_R8G8_UNORM => TextureFormat::Rg8Unorm,
        DXGI_FORMAT_R8G8_SNORM => TextureFormat::Rg8Snorm,
        DXGI_FORMAT_R8G8_UINT => TextureFormat::Rg8Uint,
        DXGI_FORMAT_R8G8_SINT => TextureFormat::Rg8Sint,
        DXGI_FORMAT_R16G16_UNORM => TextureFormat::Rg16Unorm,
        DXGI_FORMAT_R16G16_SNORM => TextureFormat::Rg16Snorm,
        DXGI_FORMAT_R32_UINT => TextureFormat::R32Uint,
        DXGI_FORMAT_R32_SINT => TextureFormat::R32Sint,
        DXGI_FORMAT_R32_FLOAT => TextureFormat::R32Float,
        DXGI_FORMAT_R16G16_UINT => TextureFormat::Rg16Uint,
        DXGI_FORMAT_R16G16_SINT => TextureFormat::Rg16Sint,
        DXGI_FORMAT_R16G16_FLOAT => TextureFormat::Rg16Float,
        DXGI_FORMAT_R8G8B8A8_UNORM => TextureFormat::Rgba8Unorm,
        DXGI_FORMAT_R8G8B8A8_UNORM_SRGB => TextureFormat::Rgba8UnormSrgb,
        DXGI_FORMAT_B8G8R8A8_UNORM_SRGB => TextureFormat::Bgra8UnormSrgb,
        DXGI_FORMAT_R8G8B8A8_SNORM => TextureFormat::Rgba8Snorm,
        DXGI_FORMAT_B8G8R8A8_UNORM => TextureFormat::Bgra8Unorm,
        DXGI_FORMAT_R8G8B8A8_UINT => TextureFormat::Rgba8Uint,
        DXGI_FORMAT_R8G8B8A8_SINT => TextureFormat::Rgba8Sint,
        DXGI_FORMAT_R10G10B10A2_UNORM => TextureFormat::Rgb10a2Unorm,
        DXGI_FORMAT_R11G11B10_FLOAT => TextureFormat::Rg11b10Float,
        DXGI_FORMAT_R32G32_UINT => TextureFormat::Rg32Uint,
        DXGI_FORMAT_R32G32_SINT => TextureFormat::Rg32Sint,
        DXGI_FORMAT_R32G32_FLOAT => TextureFormat::Rg32Float,
        DXGI_FORMAT_R16G16B16A16_UINT => TextureFormat::Rgba16Uint,
        DXGI_FORMAT_R16G16B16A16_SINT => TextureFormat::Rgba16Sint,
        DXGI_FORMAT_R16G16B16A16_UNORM => TextureFormat::Rgba16Unorm,
        DXGI_FORMAT_R16G16B16A16_SNORM => TextureFormat::Rgba16Snorm,
        DXGI_FORMAT_R16G16B16A16_FLOAT => TextureFormat::Rgba16Float,
        DXGI_FORMAT_R32G32B32A32_UINT => TextureFormat::Rgba32Uint,
        DXGI_FORMAT_R32G32B32A32_SINT => TextureFormat::Rgba32Sint,
        DXGI_FORMAT_R32G32B32A32_FLOAT => TextureFormat::Rgba32Float,
        DXGI_FORMAT_D32_FLOAT => TextureFormat::Depth32Float,
        DXGI_FORMAT_D32_FLOAT_S8X24_UINT => TextureFormat::Depth32FloatStencil8,
        DXGI_FORMAT_D24_UNORM_S8_UINT => TextureFormat::Depth24UnormStencil8,
        DXGI_FORMAT_R9G9B9E5_SHAREDEXP => TextureFormat::Rgb9e5Ufloat,
        DXGI_FORMAT_BC1_UNORM => TextureFormat::Bc1RgbaUnorm,
        DXGI_FORMAT_BC1_UNORM_SRGB => TextureFormat::Bc1RgbaUnormSrgb,
        DXGI_FORMAT_BC2_UNORM => TextureFormat::Bc2RgbaUnorm,
        DXGI_FORMAT_BC2_UNORM_SRGB => TextureFormat::Bc2RgbaUnormSrgb,
        DXGI_FORMAT_BC3_UNORM => TextureFormat::Bc3RgbaUnorm,
        DXGI_FORMAT_BC3_UNORM_SRGB => TextureFormat::Bc3RgbaUnormSrgb,
        DXGI_FORMAT_BC4_UNORM => TextureFormat::Bc4RUnorm,
        DXGI_FORMAT_BC4_SNORM => TextureFormat::Bc4RSnorm,
        DXGI_FORMAT_BC5_UNORM => TextureFormat::Bc5RgUnorm,
        DXGI_FORMAT_BC5_SNORM => TextureFormat::Bc5RgSnorm,
        DXGI_FORMAT_BC6H_UF16 => TextureFormat::Bc6hRgbUfloat,
        DXGI_FORMAT_BC6H_SF16 => TextureFormat::Bc6hRgbSfloat,
        DXGI_FORMAT_BC7_UNORM => TextureFormat::Bc7RgbaUnorm,
        DXGI_FORMAT_BC7_UNORM_SRGB => TextureFormat::Bc7RgbaUnormSrgb,
        _ => panic!("Unsupported texture format: {:?}", format),
    }
}

pub fn map_texture_format(format: wgpu::TextureFormat) -> vk::Format {
    use ash::vk::Format as F;
    use wgpu::TextureFormat as Tf;
    use wgpu::{AstcBlock, AstcChannel};
    match format {
        Tf::R8Unorm => F::R8_UNORM,
        Tf::R8Snorm => F::R8_SNORM,
        Tf::R8Uint => F::R8_UINT,
        Tf::R8Sint => F::R8_SINT,
        Tf::R16Uint => F::R16_UINT,
        Tf::R16Sint => F::R16_SINT,
        Tf::R16Unorm => F::R16_UNORM,
        Tf::R16Snorm => F::R16_SNORM,
        Tf::R16Float => F::R16_SFLOAT,
        Tf::Rg8Unorm => F::R8G8_UNORM,
        Tf::Rg8Snorm => F::R8G8_SNORM,
        Tf::Rg8Uint => F::R8G8_UINT,
        Tf::Rg8Sint => F::R8G8_SINT,
        Tf::Rg16Unorm => F::R16G16_UNORM,
        Tf::Rg16Snorm => F::R16G16_SNORM,
        Tf::R32Uint => F::R32_UINT,
        Tf::R32Sint => F::R32_SINT,
        Tf::R32Float => F::R32_SFLOAT,
        Tf::Rg16Uint => F::R16G16_UINT,
        Tf::Rg16Sint => F::R16G16_SINT,
        Tf::Rg16Float => F::R16G16_SFLOAT,
        Tf::Rgba8Unorm => F::R8G8B8A8_UNORM,
        Tf::Rgba8UnormSrgb => F::R8G8B8A8_SRGB,
        Tf::Bgra8UnormSrgb => F::B8G8R8A8_SRGB,
        Tf::Rgba8Snorm => F::R8G8B8A8_SNORM,
        Tf::Bgra8Unorm => F::B8G8R8A8_UNORM,
        Tf::Rgba8Uint => F::R8G8B8A8_UINT,
        Tf::Rgba8Sint => F::R8G8B8A8_SINT,
        Tf::Rgb10a2Unorm => F::A2B10G10R10_UNORM_PACK32,
        Tf::Rg11b10Float => F::B10G11R11_UFLOAT_PACK32,
        Tf::Rg32Uint => F::R32G32_UINT,
        Tf::Rg32Sint => F::R32G32_SINT,
        Tf::Rg32Float => F::R32G32_SFLOAT,
        Tf::Rgba16Uint => F::R16G16B16A16_UINT,
        Tf::Rgba16Sint => F::R16G16B16A16_SINT,
        Tf::Rgba16Unorm => F::R16G16B16A16_UNORM,
        Tf::Rgba16Snorm => F::R16G16B16A16_SNORM,
        Tf::Rgba16Float => F::R16G16B16A16_SFLOAT,
        Tf::Rgba32Uint => F::R32G32B32A32_UINT,
        Tf::Rgba32Sint => F::R32G32B32A32_SINT,
        Tf::Rgba32Float => F::R32G32B32A32_SFLOAT,
        Tf::Depth32Float => F::D32_SFLOAT,
        Tf::Depth32FloatStencil8 => F::D32_SFLOAT_S8_UINT,
        Tf::Depth24Plus => F::D32_SFLOAT,
        Tf::Depth24PlusStencil8 => F::D24_UNORM_S8_UINT,
        Tf::Depth24UnormStencil8 => F::D24_UNORM_S8_UINT,
        Tf::Rgb9e5Ufloat => F::E5B9G9R9_UFLOAT_PACK32,
        Tf::Bc1RgbaUnorm => F::BC1_RGBA_UNORM_BLOCK,
        Tf::Bc1RgbaUnormSrgb => F::BC1_RGBA_SRGB_BLOCK,
        Tf::Bc2RgbaUnorm => F::BC2_UNORM_BLOCK,
        Tf::Bc2RgbaUnormSrgb => F::BC2_SRGB_BLOCK,
        Tf::Bc3RgbaUnorm => F::BC3_UNORM_BLOCK,
        Tf::Bc3RgbaUnormSrgb => F::BC3_SRGB_BLOCK,
        Tf::Bc4RUnorm => F::BC4_UNORM_BLOCK,
        Tf::Bc4RSnorm => F::BC4_SNORM_BLOCK,
        Tf::Bc5RgUnorm => F::BC5_UNORM_BLOCK,
        Tf::Bc5RgSnorm => F::BC5_SNORM_BLOCK,
        Tf::Bc6hRgbUfloat => F::BC6H_UFLOAT_BLOCK,
        Tf::Bc6hRgbSfloat => F::BC6H_SFLOAT_BLOCK,
        Tf::Bc7RgbaUnorm => F::BC7_UNORM_BLOCK,
        Tf::Bc7RgbaUnormSrgb => F::BC7_SRGB_BLOCK,
        Tf::Etc2Rgb8Unorm => F::ETC2_R8G8B8_UNORM_BLOCK,
        Tf::Etc2Rgb8UnormSrgb => F::ETC2_R8G8B8_SRGB_BLOCK,
        Tf::Etc2Rgb8A1Unorm => F::ETC2_R8G8B8A1_UNORM_BLOCK,
        Tf::Etc2Rgb8A1UnormSrgb => F::ETC2_R8G8B8A1_SRGB_BLOCK,
        Tf::Etc2Rgba8Unorm => F::ETC2_R8G8B8A8_UNORM_BLOCK,
        Tf::Etc2Rgba8UnormSrgb => F::ETC2_R8G8B8A8_SRGB_BLOCK,
        Tf::EacR11Unorm => F::EAC_R11_UNORM_BLOCK,
        Tf::EacR11Snorm => F::EAC_R11_SNORM_BLOCK,
        Tf::EacRg11Unorm => F::EAC_R11G11_UNORM_BLOCK,
        Tf::EacRg11Snorm => F::EAC_R11G11_SNORM_BLOCK,
        Tf::Astc { block, channel } => match channel {
            AstcChannel::Unorm => match block {
                AstcBlock::B4x4 => F::ASTC_4X4_UNORM_BLOCK,
                AstcBlock::B5x4 => F::ASTC_5X4_UNORM_BLOCK,
                AstcBlock::B5x5 => F::ASTC_5X5_UNORM_BLOCK,
                AstcBlock::B6x5 => F::ASTC_6X5_UNORM_BLOCK,
                AstcBlock::B6x6 => F::ASTC_6X6_UNORM_BLOCK,
                AstcBlock::B8x5 => F::ASTC_8X5_UNORM_BLOCK,
                AstcBlock::B8x6 => F::ASTC_8X6_UNORM_BLOCK,
                AstcBlock::B8x8 => F::ASTC_8X8_UNORM_BLOCK,
                AstcBlock::B10x5 => F::ASTC_10X5_UNORM_BLOCK,
                AstcBlock::B10x6 => F::ASTC_10X6_UNORM_BLOCK,
                AstcBlock::B10x8 => F::ASTC_10X8_UNORM_BLOCK,
                AstcBlock::B10x10 => F::ASTC_10X10_UNORM_BLOCK,
                AstcBlock::B12x10 => F::ASTC_12X10_UNORM_BLOCK,
                AstcBlock::B12x12 => F::ASTC_12X12_UNORM_BLOCK,
            },
            AstcChannel::UnormSrgb => match block {
                AstcBlock::B4x4 => F::ASTC_4X4_SRGB_BLOCK,
                AstcBlock::B5x4 => F::ASTC_5X4_SRGB_BLOCK,
                AstcBlock::B5x5 => F::ASTC_5X5_SRGB_BLOCK,
                AstcBlock::B6x5 => F::ASTC_6X5_SRGB_BLOCK,
                AstcBlock::B6x6 => F::ASTC_6X6_SRGB_BLOCK,
                AstcBlock::B8x5 => F::ASTC_8X5_SRGB_BLOCK,
                AstcBlock::B8x6 => F::ASTC_8X6_SRGB_BLOCK,
                AstcBlock::B8x8 => F::ASTC_8X8_SRGB_BLOCK,
                AstcBlock::B10x5 => F::ASTC_10X5_SRGB_BLOCK,
                AstcBlock::B10x6 => F::ASTC_10X6_SRGB_BLOCK,
                AstcBlock::B10x8 => F::ASTC_10X8_SRGB_BLOCK,
                AstcBlock::B10x10 => F::ASTC_10X10_SRGB_BLOCK,
                AstcBlock::B12x10 => F::ASTC_12X10_SRGB_BLOCK,
                AstcBlock::B12x12 => F::ASTC_12X12_SRGB_BLOCK,
            },
            AstcChannel::Hdr => match block {
                AstcBlock::B4x4 => F::ASTC_4X4_SFLOAT_BLOCK_EXT,
                AstcBlock::B5x4 => F::ASTC_5X4_SFLOAT_BLOCK_EXT,
                AstcBlock::B5x5 => F::ASTC_5X5_SFLOAT_BLOCK_EXT,
                AstcBlock::B6x5 => F::ASTC_6X5_SFLOAT_BLOCK_EXT,
                AstcBlock::B6x6 => F::ASTC_6X6_SFLOAT_BLOCK_EXT,
                AstcBlock::B8x5 => F::ASTC_8X5_SFLOAT_BLOCK_EXT,
                AstcBlock::B8x6 => F::ASTC_8X6_SFLOAT_BLOCK_EXT,
                AstcBlock::B8x8 => F::ASTC_8X8_SFLOAT_BLOCK_EXT,
                AstcBlock::B10x5 => F::ASTC_10X5_SFLOAT_BLOCK_EXT,
                AstcBlock::B10x6 => F::ASTC_10X6_SFLOAT_BLOCK_EXT,
                AstcBlock::B10x8 => F::ASTC_10X8_SFLOAT_BLOCK_EXT,
                AstcBlock::B10x10 => F::ASTC_10X10_SFLOAT_BLOCK_EXT,
                AstcBlock::B12x10 => F::ASTC_12X10_SFLOAT_BLOCK_EXT,
                AstcBlock::B12x12 => F::ASTC_12X12_SFLOAT_BLOCK_EXT,
            },
        },
    }
}
