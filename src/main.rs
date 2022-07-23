use ash::{vk, prelude::VkResult};
use engine::{geometry::{Vertex, Mesh}, flat::make_flat_context, WgpuRunner};
use loaders::Loader;
use wgpu::{TextureDescriptor};
use wgpu_hal::{api::Vulkan, InstanceError};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window, dpi::{PhysicalSize},
};

pub mod loaders;
pub mod engine;
use std::{borrow::Cow, ffi::{CStr}, slice, time::Instant, num::NonZeroU32};

use crate::engine::WgpuLoader;

fn main() {
    env_logger::init();
    let mut wgpu_context = None;
    let mut engine_runner: Option<Box<dyn WgpuRunner>> = None;
    if let Ok(mut xr_context) = engine::vr::enable_xr_runtime() {
        println!("OpenXR OK");
        wgpu_context = xr_context.load_wgpu();
        engine_runner = Some(Box::new(xr_context));
    }

    if wgpu_context.is_none() {
        if let Some(mut flat_context) = make_flat_context() {
            wgpu_context = flat_context.load_wgpu();
            engine_runner = Some(Box::new(flat_context));
        }
        else {
            panic!("No runner available!");
        }
    }
    engine_runner.unwrap().run(&wgpu_context.unwrap());
}