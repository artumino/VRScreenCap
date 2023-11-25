use std::env::var;
use std::io;
#[cfg(windows)]
use winres::WindowsResource;

fn main() -> io::Result<()> {
    if var("CARGO_CFG_TARGET_OS")
        .map(|target_os| target_os == "windows")
        .unwrap_or(false)
    {
        #[cfg(windows)]
        {
            WindowsResource::new()
                // This path can be absolute, or relative to your crate root.
                .set_icon_with_id("assets/icon.ico", "tray-icon")
                .compile()?;
        }
    }

    Ok(())
}
