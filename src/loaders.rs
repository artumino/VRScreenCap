pub mod katanga_loader;

pub trait Loader {
    fn load(&self) -> bool;
}

