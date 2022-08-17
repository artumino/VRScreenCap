use std::error::Error;
use wgpu::{Device, Instance};

pub mod katanga_loader;

pub struct TextureSource {
    pub texture: wgpu::Texture,
    pub width: u32,
    pub height: u32,
    pub stereo_mode: StereoMode,
}

#[allow(unused)]
pub enum StereoMode {
    Mono,
    Sbs,
    Tab,
    FullSbs,
    FullTab,
}

pub trait Loader {
    fn load(
        &mut self,
        instance: &Instance,
        device: &Device,
    ) -> Result<TextureSource, Box<dyn Error>>;

    fn is_invalid(&self) -> bool;
}