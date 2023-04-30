use wgpu::{Device, Instance, Queue};

use crate::engine::texture::{Bound, Texture2D, Unbound};

#[cfg(target_os = "windows")]
pub mod katanga_loader;

#[cfg(any(target_os = "windows"))]
pub mod desktop_duplication_loader;

#[cfg(any(target_os = "windows", target_os = "unix"))]
pub mod captrs_loader;

pub struct TextureSource {
    pub texture: Texture2D<Unbound>,
    pub width: u32,
    pub height: u32,
    pub stereo_mode: Option<StereoMode>,
}

#[allow(unused)]
#[derive(Clone)]
pub enum StereoMode {
    Mono,
    Sbs,
    Tab,
    FullSbs,
    FullTab,
}

impl StereoMode {
    pub fn aspect_ratio_multiplier(&self) -> f32 {
        match self {
            StereoMode::Mono => 1.0,
            StereoMode::Sbs => 1.0,
            StereoMode::Tab => 1.0,
            StereoMode::FullSbs => 0.5,
            StereoMode::FullTab => 2.0,
        }
    }
}

pub trait Loader {
    fn load(&mut self, instance: &Instance, device: &Device) -> anyhow::Result<TextureSource>;
    fn update(
        &mut self,
        instance: &Instance,
        device: &Device,
        queue: &Queue,
        texture: &Texture2D<Bound>,
    ) -> anyhow::Result<()>;
    fn is_invalid(&self) -> bool;
}
