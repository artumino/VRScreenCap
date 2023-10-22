# ![VRScreenCap](RepoAssets/banner.webp)

VRScreenCap is a very simple rust program that uses the Vulkan backend of WPGU combined with OpenXR to show a virtual screen.

## Disclaimer
This was a pet project done in a few days of work, it doesn't offer any customization options and QoL features since It was meant to do just one thing.
The application was also only tested on a limited set of geo-11 games, with an nvidia card, so different configurations could have issues.
One of the underlying components of the application (WGPU) is still in development phase and could have issues on its own.

## Supported Inputs
Right now it only supports full side-by-side 3D feeds coming from [geo-11](https://helixmod.blogspot.com/2022/06/announcing-new-geo-11-3d-driver.html)'s
DX11 games.

## How to Use
- Setup geo-11's files for the game
- In `d3dxdm.ini` set `direct_mode` to `katanga_vr`, then adjust `dm_separation` to your likings (usually 20-30 is the range for VR viewing)
- Run the game
- Start VRScreenCap

The feed should then be visible on a curved screen. In case the video feed freezes, restart VRScreenCap.
Some VR runtimes don't seem to allow for screen recentering, a future update will probably take care of this in-app.

### Recentering
You can recenter the viewer at any point in time by holding one or two hand controller near the HMD for more than 3 seconds.
Doing it with one hand will keep the screen locked to the horizon, while using two hands will allow you to also change the screen's pitch.
Recentering can also be triggered by selecting the proper options from the menu in the icon tray.

**ATTENTION**: VRScreenCap doesn't open any window on the desktop, it only appears as a tray icon (and in your VR runtime's dashboard).


## Launch Parameters

VR-Screen-Cap offers a few configuration launch parameters (all of them are optional):
```
vr-screen-cap.exe [OPTIONS]
  OPTIONS:
    --x-curvature=0.4
    --y-curvature=0.08
    --swap-eyes=true
    --flip-x=false
    --flip-y=false
    --distance=20.0
    --scale=10.0
    --ambient=true
    --no-input=false
    --config-file=<file-path>
```
Where every distance is in meters. The effects of horizontal and vertical curvature are summed together, with a curvature of 1.0 the center of the screen will be bent inwards of about half its size.

A json configuration file can be provided and it will be watched for changes, the structure of the json config is similar to the launch parameters:
```json
{
    "distance" : 20.0,
    "scale": 10.0,
    "x_curvature": 0.4,
    "y_curvature": 0.08
}
```

## Build Info

### Windows
The release version published here were compiled with the following configuration:
```
cargo build --release --no-default-features --features dist
```

### Android Targets
Standalone is still in experimental phase:
1. Install *cargo-apk* `cargo install cargo-apk`
2. Add android targets to your rust installation `rustup target add aarch64-linux-android`
3. Build through `cargo apk build --no-default-features --features dist`

## WMR Users Disclaimer

This application uses Vulkan as its backend. To the best of my knowledge WMR still doesn't support OpenXR Vulkan applications so this one won't work out of the box for you. You can try to run VR Screen Cap through projects like [OpenXR-Vk-D3D12](https://github.com/mbucchia/OpenXR-Vk-D3D12) but I can't guarantee It'll work.
