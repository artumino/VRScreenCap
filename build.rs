use std::io;
#[cfg(windows)] use winres::WindowsResource;

fn main() -> io::Result<()> {
    #[cfg(windows)] {
        WindowsResource::new()
            // This path can be absolute, or relative to your crate root.
            .set_icon_with_id("assets/icon.ico", "tray-icon")
            .compile()?;
    }
    Ok(())
}