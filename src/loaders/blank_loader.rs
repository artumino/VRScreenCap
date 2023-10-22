use crate::engine::texture::{Bound, Texture2D, Unbound};

use super::Loader;

#[derive(Default)]
pub struct BlankLoader;

impl Loader for BlankLoader {
    #[cfg_attr(feature = "profiling", profiling::function)]
    fn load(
        &mut self,
        _instance: &wgpu::Instance,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> anyhow::Result<super::TextureSource> {
        //Load blank texture
        let blank_texture = Texture2D::<Unbound>::from_bytes(
            device,
            queue,
            include_bytes!("../../assets/blank_grey.png"),
            "Blank",
            None,
        )?;

        Ok(super::TextureSource {
            texture: blank_texture,
            width: 1,
            height: 1,
            stereo_mode: None,
        })
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    fn update(
        &mut self,
        _instance: &wgpu::Instance,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _texture: &Texture2D<Bound>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    #[cfg_attr(feature = "profiling", profiling::function)]
    fn is_invalid(&self) -> bool {
        false
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
