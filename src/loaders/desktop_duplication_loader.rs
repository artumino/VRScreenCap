use anyhow::{anyhow, Context};
use wgpu::Queue;
use win_desktop_duplication::{
    devices::AdapterFactory,
    outputs::Display,
    DesktopDuplicationApi,
};

use crate::{
    engine::texture::{Bound, Texture2D, Unbound},
};

use super::Loader;

pub struct DesktopDuplicationLoader {
    screen_index: usize,
    output: Display,
    capturer: DesktopDuplicationApi,
    resolution: (u32, u32),
}

impl Loader for DesktopDuplicationLoader {
    #[cfg_attr(feature = "profiling", profiling::function)]
    fn load(
        &mut self,
        _instance: &wgpu::Instance,
        device: &wgpu::Device,
    ) -> anyhow::Result<super::TextureSource> {
        let display_mode = self.output.get_current_display_mode().map_err(|err| {
            anyhow!(
                "Failed to get current display mode for screen {}: {:?}",
                self.screen_index,
                err
            )
        })?;

        self.resolution = (display_mode.width, display_mode.height);
        let width = self.resolution.0;
        let height = self.resolution.1;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(format!("Screen Capture Texture #{}", self.screen_index).as_str()),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            view_formats: &[],
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
        });
        let texture = Texture2D::<Unbound>::from_wgpu(device, texture);

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
        let texture = self
            .capturer
            .acquire_next_frame_now()
            .map_err(|err| anyhow!("Error acquiring desktop duplication frame {:?}", err))?;
        let _texture_desc = texture.desc();
        //let vk_format = unmap_texture_format(texture_desc.format);
        Ok(())
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    fn is_invalid(&self) -> bool {
        let display_mode = self.output.get_current_display_mode();

        if let Ok(display_mode) = display_mode {
            return display_mode.width != self.resolution.0
                || display_mode.height != self.resolution.1;
        }

        return true;
    }
}

impl DesktopDuplicationLoader {
    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn _new(screen_index: usize) -> anyhow::Result<Self> {
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
            capturer: DesktopDuplicationApi::new(adapter, output).map_err(|err| {
                anyhow!(
                    "Failed to access desktop duplication api with error {:?}",
                    err
                )
            })?,
            resolution: (0, 0),
        })
    }
}
