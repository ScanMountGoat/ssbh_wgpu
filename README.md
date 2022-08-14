# ssbh_wgpu
ssbh_wgpu is a lightweight and portable Smash Ultimate model renderer built using [WGPU](https://github.com/gfx-rs/wgpu). 

## Usage
Add the following lines to the `Cargo.toml`. This library is still highly experimental. Specify the commit tag or commit the `Cargo.lock` file to version control to avoid any versioning issues.

```
ssbh_wgpu = { git = "https://github.com/ScanMountGoat/ssbh_wgpu" }
nutexb_wgpu = { git = "https://github.com/ScanMountGoat/ssbh_wgpu" }
```

## Overview
- nutexb_wgpu -- texture renderer and library for converting nutexb files to WGPU textures
- nutexb_wgpu_viewer -- simple winit application for viewing nutexb textures
- ssbh_wgpu -- model and animation rendering library. Converts ssbh_data types to WGPU types.
- ssbh_wgpu_test -- windowless program for testing model loading
- ssbh_wgpu_viewer -- simple winit application for viewing models and animations

## Building
With the Rust 1.60 or later toolchain installed, run `cargo build --release`. Don't forget the `--release` since debug builds have very low framerates!

## Credits
ssbh_wgpu is a continuation of the rendering work done for [CrossMod](https://github.com/Ploaj/SSBHLib). Implementations are based on data from testing the output of the emulators [Yuzu](https://yuzu-emu.org/) and [Ryujinx](https://ryujinx.org/) using the [RenderDoc](https://renderdoc.org/) debugger. 
