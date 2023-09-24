use windows::Win32::Foundation::HANDLE;

use anyhow::{anyhow, Context};
use wgpu::Queue;
use win_desktop_duplication::{
    devices::AdapterFactory, outputs::Display, texture::ColorFormat, DesktopDuplicationApi,
};
use windows::core::ComInterface;
use windows::Win32::Graphics::Dxgi::IDXGIResource;

use crate::{
    engine::{
        formats::InternalColorFormat,
        texture::{Bound, Texture2D},
    },
    macros::auto_map,
    utils::external_texture::{ExternalApi, ExternalTextureInfo},
};

use super::Loader;

pub struct DesktopDuplicationLoader {
    screen_index: usize,
    output: Display,
    capturer: DesktopDuplicationApi,
    current_handle: Option<HANDLE>,
    resolution: Option<(u32, u32)>,
    invalid: bool,
}

impl Loader for DesktopDuplicationLoader {
    #[cfg_attr(feature = "profiling", profiling::function)]
    fn load(
        &mut self,
        _instance: &wgpu::Instance,
        device: &wgpu::Device,
        _queue: &Queue,
    ) -> anyhow::Result<super::TextureSource> {
        let display_mode = self.output.get_current_display_mode().map_err(|err| {
            anyhow!(
                "Failed to get current display mode for screen {}: {:?}",
                self.screen_index,
                err
            )
        })?;

        let resolution = (display_mode.width, display_mode.height);
        let width = resolution.0;
        let height = resolution.1;

        let d3d_texture = self
            .capturer
            .acquire_next_frame_now()
            .map_err(|err| anyhow!("Error acquiring desktop duplication frame {:?}", err))?;

        let texture_desc = d3d_texture.desc();
        let resource: IDXGIResource = d3d_texture.as_raw_ref().cast()?;
        let handle = unsafe { resource.GetSharedHandle() }?;

        self.current_handle = Some(handle);

        let external_texture_info = ExternalTextureInfo {
            external_api: ExternalApi::D3D11,
            width: texture_desc.width,
            height: texture_desc.height,
            array_size: 1u32,
            sample_count: 1u32,
            mip_levels: 1u32,
            format: texture_desc.format.try_into()?,
            actual_handle: handle.0 as usize,
        };

        let screen_format = external_texture_info.format;
        let screen_norm_format = screen_format.to_norm();
        let view_formats = if screen_norm_format != screen_format {
            Some(screen_norm_format)
        } else {
            None
        };

        let texture = external_texture_info
            .map_as_wgpu_texture(
                format!("DD Screen Capture Texture #{}", self.screen_index).as_str(),
                device,
                view_formats,
            )
            .context("Cannot map desktop duplication output to WGPU texture")?;

        self.resolution = Some(resolution);
        Ok(super::TextureSource {
            texture,
            width,
            height,
            stereo_mode: None,
        })
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    fn update(
        &mut self,
        _instance: &wgpu::Instance,
        _device: &wgpu::Device,
        _queue: &Queue,
        _texture: &Texture2D<Bound>,
    ) -> anyhow::Result<()> {
        let d3d_texture = self
            .capturer
            .acquire_next_frame_now()
            .map_err(|err| anyhow!("Error acquiring desktop duplication frame {:?}", err))?;
        let resource: IDXGIResource = d3d_texture.as_raw_ref().cast()?;
        let handle = unsafe { resource.GetSharedHandle() }?;

        if let Some(current_handle) = self.current_handle {
            if current_handle == handle {
                return Ok(());
            }
        }

        self.invalid = true;
        Ok(())
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    fn is_invalid(&self) -> bool {
        let display_mode = self.output.get_current_display_mode();

        if self.resolution.is_none() {
            return true;
        }

        let (width, height) = self.resolution.unwrap();
        if let Ok(display_mode) = display_mode {
            return display_mode.width != width || display_mode.height != height || self.invalid;
        }

        true
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    fn encode_pre_pass(
        &self,
        _encoder: &mut wgpu::CommandEncoder,
        _texture: &Texture2D<Bound>,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

impl DesktopDuplicationLoader {
    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn new(screen_index: usize) -> anyhow::Result<Self> {
        win_desktop_duplication::set_process_dpi_awareness();
        win_desktop_duplication::co_init();

        let adapter = AdapterFactory::new()
            .get_adapter_by_idx(0)
            .context("Failed to get adapter")?;
        let output = adapter
            .get_display_by_idx(screen_index as u32)
            .context("Failed to get display")?;
        Ok(Self {
            screen_index,
            output: output.clone(),
            current_handle: None,
            capturer: DesktopDuplicationApi::new(adapter, output).map_err(|err| {
                anyhow!(
                    "Failed to access desktop duplication api with error {:?}",
                    err
                )
            })?,
            resolution: None,
            invalid: false,
        })
    }
}

#[cfg(target_os = "windows")]
auto_map!(InternalColorFormat ColorFormat {
    (InternalColorFormat::Rgba8Unorm, ColorFormat::ARGB8UNorm),
    (InternalColorFormat::Bgra8Unorm, ColorFormat::BGRA8UNorm), //typo in the library
    (InternalColorFormat::Ayuv, ColorFormat::AYUV),
    (InternalColorFormat::R8Unorm, ColorFormat::YUV444),
    (InternalColorFormat::R16Unorm, ColorFormat::YUV444_10bit),
    (InternalColorFormat::Nv12, ColorFormat::NV12),
    (InternalColorFormat::Rgba16Float, ColorFormat::ARGB16Float),
    (InternalColorFormat::Rgb10a2Unorm, ColorFormat::ARGB10UNorm),
    (InternalColorFormat::Y410, ColorFormat::Y410),
    (InternalColorFormat::P010, ColorFormat::YUV420_10bit)
});
