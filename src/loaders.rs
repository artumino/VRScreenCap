use std::error::Error;

use wgpu::{Device, Instance};
use clap::Parser;

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

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ScreenParamsUniform {
    x_curvature: f32,
    y_curvature: f32,
    eye_offset: f32,
    y_offset: f32,
    x_offset: f32,
}

#[derive(Parser)]
pub struct ScreenParams {
    // Maximum depth at the center in meters, default: 4.0, usage: --x-curvature=4.0
    #[clap(short, long, value_parser, default_value_t = 4.0)]
    pub x_curvature: f32,
    // Maximum depth at the center in meters, default: 0.8, usage: --y-curvature=0.8
    #[clap(long, value_parser, default_value_t = 0.8)]
    pub y_curvature: f32,
    // default: true, usage: --swap-eyes=true (geo11 has them swapped, might be GPU dependent)
    #[clap(long, value_parser, default_value_t = true)]
    pub swap_eyes: bool,
    // default: false, usage: --flip-x=false
    #[clap(long, value_parser, default_value_t = false)]
    pub flip_x: bool,
    // default: false, usage: --flip-y=false
    #[clap(long, value_parser, default_value_t = false)]
    pub flip_y: bool,
    // Distance from user in meters -Z forward, default: 20.0, usage: --distance=20.0
    #[clap(short, long, value_parser, default_value_t = 20.0)]
    pub distance: f32,
    // Screen scaling factor (a screen with aspect ratio 1:1 with scaling factor 1.0 would be 2m wide), default: 10.0, usage: --scale=10.0
    #[clap(short, long, value_parser, default_value_t = 10.0)]
    pub scale: f32
}

impl ScreenParams {
    pub fn uniform(&self) -> ScreenParamsUniform {
        ScreenParamsUniform {
            x_curvature: self.x_curvature,
            y_curvature: self.y_curvature,
            eye_offset: match self.swap_eyes { 
                true => 1.0,
                _ => 0.0
            },
            y_offset: match self.flip_y { 
                true => 1.0,
                _ => 0.0
            },
            x_offset: match self.flip_x { 
                true => 1.0,
                _ => 0.0
            },
        }
    }
}

impl Default for ScreenParams {
    fn default() -> Self {
        Self {
            x_curvature: 4.0,
            y_curvature: 0.8,
            swap_eyes: true,
            flip_x: false,
            flip_y: false,
            distance: 20.0,
            scale: 10.0
        }
    }
}
