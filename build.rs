use std::io;
use std::env::var;
#[cfg(windows)]  use winres::WindowsResource;

fn main() -> io::Result<()> {
    if var("CARGO_CFG_TARGET_OS")
        .map(|target_os| target_os == "windows")
        .unwrap_or(false)
    {
        #[cfg(windows)] {
            WindowsResource::new()
                // This path can be absolute, or relative to your crate root.
                .set_icon_with_id("assets/icon.ico", "tray-icon")
                .compile()?;
        }
    }
    
    // On Android, we must ensure that we're dynamically linking against the C++ standard library.
    // For more details, see https://github.com/rust-windowing/android-ndk-rs/issues/167
    if var("TARGET")
        .map(|target| target == "aarch64-linux-android")
        .unwrap_or(false)
    {
        println!("cargo:rustc-link-lib=dylib=c++");
    }
    
    Ok(())
}