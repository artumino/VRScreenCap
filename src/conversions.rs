use ash::vk::{self, Format};
use wgpu::{Device, TextureDescriptor, TextureFormat};
use wgpu_hal::api::Vulkan;
use windows::Win32::Graphics::Dxgi::Common::*;

use crate::{macros::auto_map, engine::formats::InternalColorFormat};


#[cfg(target_os = "windows")]
#[cfg_attr(feature = "profiling", profiling::function)]
pub fn vulkan_image_to_texture(
    device: &Device,
    image: vk::Image,
    tex_desc: TextureDescriptor,
    hal_tex_desc: wgpu_hal::TextureDescriptor,
) -> wgpu::Texture {
    let texture = unsafe {
        <wgpu_hal::api::Vulkan as wgpu_hal::Api>::Device::texture_from_raw(
            image,
            &hal_tex_desc,
            None,
        )
    };

    unsafe { device.create_texture_from_hal::<Vulkan>(texture, &tex_desc) }
}

// Color Format Mappings
auto_map!(TextureFormat InternalColorFormat {
    (TextureFormat::R8Unorm, InternalColorFormat::R8Unorm),
    (TextureFormat::R8Snorm, InternalColorFormat::R8Snorm),
    (TextureFormat::R8Uint, InternalColorFormat::R8Uint),
    (TextureFormat::R8Sint, InternalColorFormat::R8Sint),
    (TextureFormat::R16Uint, InternalColorFormat::R16Uint),
    (TextureFormat::R16Sint, InternalColorFormat::R16Sint),
    (TextureFormat::R16Unorm, InternalColorFormat::R16Unorm),
    (TextureFormat::R16Snorm, InternalColorFormat::R16Snorm),
    (TextureFormat::R16Float, InternalColorFormat::R16Float),
    (TextureFormat::Rg8Unorm, InternalColorFormat::Rg8Unorm),
    (TextureFormat::Rg8Snorm, InternalColorFormat::Rg8Snorm),
    (TextureFormat::Rg8Uint, InternalColorFormat::Rg8Uint),
    (TextureFormat::Rg8Sint, InternalColorFormat::Rg8Sint),
    (TextureFormat::Rg16Unorm, InternalColorFormat::Rg16Unorm),
    (TextureFormat::Rg16Snorm, InternalColorFormat::Rg16Snorm),
    (TextureFormat::R32Uint, InternalColorFormat::R32Uint),
    (TextureFormat::R32Sint, InternalColorFormat::R32Sint),
    (TextureFormat::R32Float, InternalColorFormat::R32Float),
    (TextureFormat::Rg16Uint, InternalColorFormat::Rg16Uint),
    (TextureFormat::Rg16Sint, InternalColorFormat::Rg16Sint),
    (TextureFormat::Rg16Float, InternalColorFormat::Rg16Float),
    (TextureFormat::Rgba8Unorm, InternalColorFormat::Rgba8Unorm),
    (TextureFormat::Rgba8Unorm, InternalColorFormat::Rgba8Unorm),
    (TextureFormat::Rgba8UnormSrgb, InternalColorFormat::Rgba8UnormSrgb),
    (TextureFormat::Bgra8UnormSrgb, InternalColorFormat::Bgra8UnormSrgb),
    (TextureFormat::Rgba8Snorm, InternalColorFormat::Rgba8Snorm),
    (TextureFormat::Bgra8Unorm, InternalColorFormat::Bgra8Unorm),
    (TextureFormat::Rgba8Uint, InternalColorFormat::Rgba8Uint),
    (TextureFormat::Rgba8Sint, InternalColorFormat::Rgba8Sint),
    (TextureFormat::Rgb10a2Unorm, InternalColorFormat::Rgb10a2Unorm),
    (TextureFormat::Rg11b10Float, InternalColorFormat::Rg11b10Float),
    (TextureFormat::Rg32Uint, InternalColorFormat::Rg32Uint),
    (TextureFormat::Rg32Sint, InternalColorFormat::Rg32Sint),
    (TextureFormat::Rg32Float, InternalColorFormat::Rg32Float),
    (TextureFormat::Rgba16Uint, InternalColorFormat::Rgba16Uint),
    (TextureFormat::Rgba16Sint, InternalColorFormat::Rgba16Sint),
    (TextureFormat::Rgba16Unorm, InternalColorFormat::Rgba16Unorm),
    (TextureFormat::Rgba16Snorm, InternalColorFormat::Rgba16Snorm),
    (TextureFormat::Rgba16Float, InternalColorFormat::Rgba16Float),
    (TextureFormat::Rgba32Uint, InternalColorFormat::Rgba32Uint),
    (TextureFormat::Rgba32Sint, InternalColorFormat::Rgba32Sint),
    (TextureFormat::Rgba32Float, InternalColorFormat::Rgba32Float),
    (TextureFormat::Depth32Float, InternalColorFormat::Depth32Float),
    (TextureFormat::Depth32FloatStencil8, InternalColorFormat::Depth32FloatStencil8),
    (TextureFormat::Depth24PlusStencil8, InternalColorFormat::Depth24PlusStencil8),
    (TextureFormat::Rgb9e5Ufloat, InternalColorFormat::Rgb9e5Ufloat),
    (TextureFormat::Bc1RgbaUnorm, InternalColorFormat::Bc1RgbaUnorm),
    (TextureFormat::Bc1RgbaUnormSrgb, InternalColorFormat::Bc1RgbaUnormSrgb),
    (TextureFormat::Bc2RgbaUnorm, InternalColorFormat::Bc2RgbaUnorm),
    (TextureFormat::Bc2RgbaUnormSrgb, InternalColorFormat::Bc2RgbaUnormSrgb),
    (TextureFormat::Bc3RgbaUnorm, InternalColorFormat::Bc3RgbaUnorm),
    (TextureFormat::Bc3RgbaUnormSrgb, InternalColorFormat::Bc3RgbaUnormSrgb),
    (TextureFormat::Bc4RUnorm, InternalColorFormat::Bc4RUnorm),
    (TextureFormat::Bc4RSnorm, InternalColorFormat::Bc4RSnorm),
    (TextureFormat::Bc5RgUnorm, InternalColorFormat::Bc5RgUnorm),
    (TextureFormat::Bc5RgSnorm, InternalColorFormat::Bc5RgSnorm),
    (TextureFormat::Bc6hRgbUfloat, InternalColorFormat::Bc6hRgbUfloat),
    (TextureFormat::Bc6hRgbFloat, InternalColorFormat::Bc6hRgbFloat),
    (TextureFormat::Bc7RgbaUnorm, InternalColorFormat::Bc7RgbaUnorm),
    (TextureFormat::Bc7RgbaUnormSrgb, InternalColorFormat::Bc7RgbaUnormSrgb),
    (TextureFormat::Depth16Unorm, InternalColorFormat::Depth16Unorm)
});

auto_map!(InternalColorFormat Format {
    (InternalColorFormat::R8Unorm, ash::vk::Format::R8_UNORM),
    (InternalColorFormat::R8Snorm, ash::vk::Format::R8_SNORM),
    (InternalColorFormat::R8Uint, ash::vk::Format::R8_UINT),
    (InternalColorFormat::R8Sint, ash::vk::Format::R8_SINT),
    (InternalColorFormat::R16Uint, ash::vk::Format::R16_UINT),
    (InternalColorFormat::R16Sint, ash::vk::Format::R16_SINT),
    (InternalColorFormat::R16Unorm, ash::vk::Format::R16_UNORM),
    (InternalColorFormat::R16Snorm, ash::vk::Format::R16_SNORM),
    (InternalColorFormat::R16Float, ash::vk::Format::R16_SFLOAT),
    (InternalColorFormat::Rg8Unorm, ash::vk::Format::R8G8_UNORM),
    (InternalColorFormat::Rg8Snorm, ash::vk::Format::R8G8_SNORM),
    (InternalColorFormat::Rg8Uint, ash::vk::Format::R8G8_UINT),
    (InternalColorFormat::Rg8Sint, ash::vk::Format::R8G8_SINT),
    (InternalColorFormat::Rg16Unorm, ash::vk::Format::R16G16_UNORM),
    (InternalColorFormat::Rg16Snorm, ash::vk::Format::R16G16_SNORM),
    (InternalColorFormat::R32Uint, ash::vk::Format::R32_UINT),
    (InternalColorFormat::R32Sint, ash::vk::Format::R32_SINT),
    (InternalColorFormat::R32Float, ash::vk::Format::R32_SFLOAT),
    (InternalColorFormat::Rg16Uint, ash::vk::Format::R16G16_UINT),
    (InternalColorFormat::Rg16Sint, ash::vk::Format::R16G16_SINT),
    (InternalColorFormat::Rg16Float, ash::vk::Format::R16G16_SFLOAT),
    (InternalColorFormat::Rgba8Unorm, ash::vk::Format::R8G8B8A8_UNORM),
    (InternalColorFormat::Rgba8UnormSrgb, ash::vk::Format::R8G8B8A8_SRGB),
    (InternalColorFormat::Bgra8UnormSrgb, ash::vk::Format::B8G8R8A8_SRGB),
    (InternalColorFormat::Rgba8Snorm, ash::vk::Format::R8G8B8A8_SNORM),
    (InternalColorFormat::Bgra8Unorm, ash::vk::Format::B8G8R8A8_UNORM),
    (InternalColorFormat::Rgba8Uint, ash::vk::Format::R8G8B8A8_UINT),
    (InternalColorFormat::Rgba8Sint, ash::vk::Format::R8G8B8A8_SINT),
    (InternalColorFormat::Rgb10a2Unorm, ash::vk::Format::A2B10G10R10_UNORM_PACK32),
    (InternalColorFormat::Rg11b10Float, ash::vk::Format::B10G11R11_UFLOAT_PACK32),
    (InternalColorFormat::Rg32Uint, ash::vk::Format::R32G32_UINT),
    (InternalColorFormat::Rg32Sint, ash::vk::Format::R32G32_SINT),
    (InternalColorFormat::Rg32Float, ash::vk::Format::R32G32_SFLOAT),
    (InternalColorFormat::Rgba16Uint, ash::vk::Format::R16G16B16A16_UINT),
    (InternalColorFormat::Rgba16Sint, ash::vk::Format::R16G16B16A16_SINT),
    (InternalColorFormat::Rgba16Unorm, ash::vk::Format::R16G16B16A16_UNORM),
    (InternalColorFormat::Rgba16Snorm, ash::vk::Format::R16G16B16A16_SNORM),
    (InternalColorFormat::Rgba16Float, ash::vk::Format::R16G16B16A16_SFLOAT),
    (InternalColorFormat::Rgba32Uint, ash::vk::Format::R32G32B32A32_UINT),
    (InternalColorFormat::Rgba32Sint, ash::vk::Format::R32G32B32A32_SINT),
    (InternalColorFormat::Rgba32Float, ash::vk::Format::R32G32B32A32_SFLOAT),
    (InternalColorFormat::Depth32Float, ash::vk::Format::D32_SFLOAT),
    (InternalColorFormat::Depth32FloatStencil8, ash::vk::Format::D32_SFLOAT_S8_UINT),
    (InternalColorFormat::Depth24Plus, ash::vk::Format::D32_SFLOAT),
    (InternalColorFormat::Depth24PlusStencil8, ash::vk::Format::D24_UNORM_S8_UINT),
    (InternalColorFormat::Depth16Unorm, ash::vk::Format::D16_UNORM),
    (InternalColorFormat::Rgb9e5Ufloat, ash::vk::Format::E5B9G9R9_UFLOAT_PACK32),
    (InternalColorFormat::Bc1RgbaUnorm, ash::vk::Format::BC1_RGBA_UNORM_BLOCK),
    (InternalColorFormat::Bc1RgbaUnormSrgb, ash::vk::Format::BC1_RGBA_SRGB_BLOCK),
    (InternalColorFormat::Bc2RgbaUnorm, ash::vk::Format::BC2_UNORM_BLOCK),
    (InternalColorFormat::Bc2RgbaUnormSrgb, ash::vk::Format::BC2_SRGB_BLOCK),
    (InternalColorFormat::Bc3RgbaUnorm, ash::vk::Format::BC3_UNORM_BLOCK),
    (InternalColorFormat::Bc3RgbaUnormSrgb, ash::vk::Format::BC3_SRGB_BLOCK),
    (InternalColorFormat::Bc4RUnorm, ash::vk::Format::BC4_UNORM_BLOCK),
    (InternalColorFormat::Bc4RSnorm, ash::vk::Format::BC4_SNORM_BLOCK),
    (InternalColorFormat::Bc5RgUnorm, ash::vk::Format::BC5_UNORM_BLOCK),
    (InternalColorFormat::Bc5RgSnorm, ash::vk::Format::BC5_SNORM_BLOCK),
    (InternalColorFormat::Bc6hRgbUfloat, ash::vk::Format::BC6H_UFLOAT_BLOCK),
    (InternalColorFormat::Bc6hRgbFloat, ash::vk::Format::BC6H_SFLOAT_BLOCK),
    (InternalColorFormat::Bc7RgbaUnorm, ash::vk::Format::BC7_UNORM_BLOCK),
    (InternalColorFormat::Bc7RgbaUnormSrgb, ash::vk::Format::BC7_SRGB_BLOCK),
    (InternalColorFormat::Etc2Rgb8Unorm, ash::vk::Format::ETC2_R8G8B8_UNORM_BLOCK),
    (InternalColorFormat::Etc2Rgb8UnormSrgb, ash::vk::Format::ETC2_R8G8B8_SRGB_BLOCK),
    (InternalColorFormat::Etc2Rgb8A1Unorm, ash::vk::Format::ETC2_R8G8B8A1_UNORM_BLOCK),
    (InternalColorFormat::Etc2Rgb8A1UnormSrgb, ash::vk::Format::ETC2_R8G8B8A1_SRGB_BLOCK),
    (InternalColorFormat::Etc2Rgba8Unorm, ash::vk::Format::ETC2_R8G8B8A8_UNORM_BLOCK),
    (InternalColorFormat::Etc2Rgba8UnormSrgb, ash::vk::Format::ETC2_R8G8B8A8_SRGB_BLOCK),
    (InternalColorFormat::EacR11Unorm, ash::vk::Format::EAC_R11_UNORM_BLOCK),
    (InternalColorFormat::EacR11Snorm, ash::vk::Format::EAC_R11_SNORM_BLOCK),
    (InternalColorFormat::EacRg11Unorm, ash::vk::Format::EAC_R11G11_UNORM_BLOCK),
    (InternalColorFormat::EacRg11Snorm, ash::vk::Format::EAC_R11G11_SNORM_BLOCK),
    (InternalColorFormat::Stencil8, ash::vk::Format::S8_UINT)
});

#[cfg(target_os = "windows")]
auto_map!(DXGI_FORMAT InternalColorFormat {
    (DXGI_FORMAT_R8_UNORM, InternalColorFormat::R8Unorm),
    (DXGI_FORMAT_R8_SNORM, InternalColorFormat::R8Snorm),
    (DXGI_FORMAT_R8_UINT, InternalColorFormat::R8Uint),
    (DXGI_FORMAT_R8_SINT, InternalColorFormat::R8Sint),
    (DXGI_FORMAT_R16_UINT, InternalColorFormat::R16Uint),
    (DXGI_FORMAT_R16_SINT, InternalColorFormat::R16Sint),
    (DXGI_FORMAT_R16_UNORM, InternalColorFormat::R16Unorm),
    (DXGI_FORMAT_R16_SNORM, InternalColorFormat::R16Snorm),
    (DXGI_FORMAT_R16_FLOAT, InternalColorFormat::R16Float),
    (DXGI_FORMAT_R8G8_UNORM, InternalColorFormat::Rg8Unorm),
    (DXGI_FORMAT_R8G8_SNORM, InternalColorFormat::Rg8Snorm),
    (DXGI_FORMAT_R8G8_UINT, InternalColorFormat::Rg8Uint),
    (DXGI_FORMAT_R8G8_SINT, InternalColorFormat::Rg8Sint),
    (DXGI_FORMAT_R16G16_UNORM, InternalColorFormat::Rg16Unorm),
    (DXGI_FORMAT_R16G16_SNORM, InternalColorFormat::Rg16Snorm),
    (DXGI_FORMAT_R32_UINT, InternalColorFormat::R32Uint),
    (DXGI_FORMAT_R32_SINT, InternalColorFormat::R32Sint),
    (DXGI_FORMAT_R32_FLOAT, InternalColorFormat::R32Float),
    (DXGI_FORMAT_R16G16_UINT, InternalColorFormat::Rg16Uint),
    (DXGI_FORMAT_R16G16_SINT, InternalColorFormat::Rg16Sint),
    (DXGI_FORMAT_R16G16_FLOAT, InternalColorFormat::Rg16Float),
    (DXGI_FORMAT_R8G8B8A8_UNORM, InternalColorFormat::Rgba8Unorm),
    (DXGI_FORMAT_R8G8B8A8_TYPELESS, InternalColorFormat::Rgba8Unorm),
    (DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, InternalColorFormat::Rgba8UnormSrgb),
    (DXGI_FORMAT_B8G8R8A8_UNORM_SRGB, InternalColorFormat::Bgra8UnormSrgb),
    (DXGI_FORMAT_R8G8B8A8_SNORM, InternalColorFormat::Rgba8Snorm),
    (DXGI_FORMAT_B8G8R8A8_UNORM, InternalColorFormat::Bgra8Unorm),
    (DXGI_FORMAT_R8G8B8A8_UINT, InternalColorFormat::Rgba8Uint),
    (DXGI_FORMAT_R8G8B8A8_SINT, InternalColorFormat::Rgba8Sint),
    (DXGI_FORMAT_R10G10B10A2_UNORM, InternalColorFormat::Rgb10a2Unorm),
    (DXGI_FORMAT_R11G11B10_FLOAT, InternalColorFormat::Rg11b10Float),
    (DXGI_FORMAT_R32G32_UINT, InternalColorFormat::Rg32Uint),
    (DXGI_FORMAT_R32G32_SINT, InternalColorFormat::Rg32Sint),
    (DXGI_FORMAT_R32G32_FLOAT, InternalColorFormat::Rg32Float),
    (DXGI_FORMAT_R16G16B16A16_UINT, InternalColorFormat::Rgba16Uint),
    (DXGI_FORMAT_R16G16B16A16_SINT, InternalColorFormat::Rgba16Sint),
    (DXGI_FORMAT_R16G16B16A16_UNORM, InternalColorFormat::Rgba16Unorm),
    (DXGI_FORMAT_R16G16B16A16_SNORM, InternalColorFormat::Rgba16Snorm),
    (DXGI_FORMAT_R16G16B16A16_FLOAT, InternalColorFormat::Rgba16Float),
    (DXGI_FORMAT_R32G32B32A32_UINT, InternalColorFormat::Rgba32Uint),
    (DXGI_FORMAT_R32G32B32A32_SINT, InternalColorFormat::Rgba32Sint),
    (DXGI_FORMAT_R32G32B32A32_FLOAT, InternalColorFormat::Rgba32Float),
    (DXGI_FORMAT_D32_FLOAT, InternalColorFormat::Depth32Float),
    (DXGI_FORMAT_D32_FLOAT_S8X24_UINT, InternalColorFormat::Depth32FloatStencil8),
    (DXGI_FORMAT_D24_UNORM_S8_UINT, InternalColorFormat::Depth24PlusStencil8),
    (DXGI_FORMAT_R9G9B9E5_SHAREDEXP, InternalColorFormat::Rgb9e5Ufloat),
    (DXGI_FORMAT_BC1_UNORM, InternalColorFormat::Bc1RgbaUnorm),
    (DXGI_FORMAT_BC1_UNORM_SRGB, InternalColorFormat::Bc1RgbaUnormSrgb),
    (DXGI_FORMAT_BC2_UNORM, InternalColorFormat::Bc2RgbaUnorm),
    (DXGI_FORMAT_BC2_UNORM_SRGB, InternalColorFormat::Bc2RgbaUnormSrgb),
    (DXGI_FORMAT_BC3_UNORM, InternalColorFormat::Bc3RgbaUnorm),
    (DXGI_FORMAT_BC3_UNORM_SRGB, InternalColorFormat::Bc3RgbaUnormSrgb),
    (DXGI_FORMAT_BC4_UNORM, InternalColorFormat::Bc4RUnorm),
    (DXGI_FORMAT_BC4_SNORM, InternalColorFormat::Bc4RSnorm),
    (DXGI_FORMAT_BC5_UNORM, InternalColorFormat::Bc5RgUnorm),
    (DXGI_FORMAT_BC5_SNORM, InternalColorFormat::Bc5RgSnorm),
    (DXGI_FORMAT_BC6H_UF16, InternalColorFormat::Bc6hRgbUfloat),
    (DXGI_FORMAT_BC6H_SF16, InternalColorFormat::Bc6hRgbFloat),
    (DXGI_FORMAT_BC7_UNORM, InternalColorFormat::Bc7RgbaUnorm),
    (DXGI_FORMAT_BC7_UNORM_SRGB, InternalColorFormat::Bc7RgbaUnormSrgb),
    (DXGI_FORMAT_D16_UNORM, InternalColorFormat::Depth16Unorm),
    (DXGI_FORMAT_AYUV, InternalColorFormat::Ayuv),
    (DXGI_FORMAT_NV12, InternalColorFormat::Nv12),
    (DXGI_FORMAT_Y410, InternalColorFormat::Y410),
    (DXGI_FORMAT_P010, InternalColorFormat::P010)
});

#[cfg(test)]
mod test {
    use wgpu::TextureFormat;
    use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT};

    use crate::engine::formats::InternalColorFormat;

    #[test]
    fn dxgi_conversion_test() -> anyhow::Result<()> {
        let internal_format = InternalColorFormat::Rgba8Unorm;
        let dxgi_equivalent = DXGI_FORMAT_R8G8B8A8_UNORM;

        let mapped_dxgi: DXGI_FORMAT = internal_format.try_into()?;
        let mapped_internal: InternalColorFormat = dxgi_equivalent.try_into()?;
        
        assert_eq!(mapped_dxgi, dxgi_equivalent);
        assert_eq!(mapped_internal, internal_format);
        
        let error: Result<TextureFormat, _> = InternalColorFormat::Y410.try_into();
        assert!(error.is_err());

        Ok(())
    }
}