use std::time::Duration;

use anyhow::anyhow;
use wgpu::Queue;

use crate::engine::texture::{Bound, Texture2D, Unbound};

use super::Loader;

// This is a loader to capture the desktop using the captrs crate.
// It is currently not really performance friendly since the texture gets copied to system memory and then back to the GPU.
pub struct CaptrLoader {
    capturer: captrs::Capturer,
    screen_index: usize,
    geometry: (u32, u32),
}

impl Loader for CaptrLoader {
    #[cfg_attr(feature = "profiling", profiling::function)]
    fn load(
        &mut self,
        _instance: &wgpu::Instance,
        device: &wgpu::Device,
        _queue: &Queue,
    ) -> anyhow::Result<super::TextureSource> {
        self.geometry = self.capturer.geometry();
        let width = self.geometry.0;
        let height = self.geometry.1;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(format!("CRS Screen Capture Texture #{}", self.screen_index).as_str()),
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
        queue: &Queue,
        texture: &Texture2D<Bound>,
    ) -> anyhow::Result<()> {
        let capture_result = self.capturer.capture_store_frame();

        if let Err(err) = capture_result {
            match err {
                captrs::CaptureError::Timeout => {
                    return Ok(());
                }
                _ => {
                    return Err(anyhow!("Failed to capture frame with error {:?}", err));
                }
            }
        }

        if let Some(frame) = self.capturer.get_stored_frame() {
            // FIXME: captrs returns a BGRA8 struct, if this has alignement bytes the following code will collect garbage data
            let data =
                unsafe { std::slice::from_raw_parts(frame.as_ptr() as *const u8, frame.len() * 4) };
            queue.write_texture(
                texture.texture.as_image_copy(),
                data,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * self.geometry.0),
                    rows_per_image: Some(self.geometry.1),
                },
                texture.texture.size(),
            );
            Ok(())
        } else {
            Err(anyhow!("Failed to get stored frame"))
        }
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    fn is_invalid(&self) -> bool {
        let geometry = self.capturer.geometry();
        geometry.0 != self.geometry.0 || geometry.1 != self.geometry.1
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

impl CaptrLoader {
    #[cfg_attr(feature = "profiling", profiling::function)]
    pub fn new(screen_index: usize) -> anyhow::Result<Self> {
        let capturer = captrs::Capturer::new_with_timeout(screen_index, Duration::from_nanos(0))
            .map_err(|err| anyhow!(err))?;
        Ok(Self {
            screen_index,
            capturer,
            geometry: (0, 0),
        })
    }
}
