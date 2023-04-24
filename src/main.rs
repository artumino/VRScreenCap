#![windows_subsystem = "windows"]
use vr_screen_cap_core::launch;

pub fn main() {
    if let Err(err) = launch() {
        log::error!("VRScreenCap closed unexpectedly with an error: {}", err);
    }
}
