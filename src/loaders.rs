use std::error::Error;

use wgpu::{Device, Instance};

pub mod katanga_loader;

pub trait Loader {
    fn load(&mut self, instance: &Instance, device: &Device) -> Result<(), Box<dyn Error>>;
}

