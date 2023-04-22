use wgpu::{Device, Instance};

use crate::engine::texture::Texture2D;

#[cfg(target_os = "windows")]
pub mod katanga_loader;

pub struct TextureSource {
    pub texture: Texture2D,
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
    fn load(&mut self, instance: &Instance, device: &Device) -> anyhow::Result<TextureSource>;

    fn is_invalid(&self) -> bool;
}
