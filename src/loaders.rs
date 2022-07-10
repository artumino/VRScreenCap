use std::error::Error;

pub mod katanga_loader;

pub trait Loader {
    fn load(&mut self) -> Result<(), Box<dyn Error>>;
}

