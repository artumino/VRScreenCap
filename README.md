# VRScreenCap
## Description
This is a very simple rust program that uses the Vulkan backend of WPGU combined with OpenXR to show a virtual screen.

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

**ATTENTION**: VRScreenCap doesn't open any window on the desktop, it only appears as a tray icon (and in your VR runtime's dashboard).
