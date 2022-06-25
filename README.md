# ssbh_wgpu
ssbh_wgpu is a lightweight and portable Smash Ultimate model renderer built using [WGPU](https://github.com/gfx-rs/wgpu).

## Overview
- nutexb_wgpu -- texture renderer and library for converting nutexb files to WGPU textures
- nutexb_wgpu_viewer -- simple winit application for viewing nutexb textures
- ssbh_wgpu -- model and animation rendering library. Converts ssbh_data types to WGPU types.
- ssbh_wgpu_test -- windowless program for testing model loading
- ssbh_wgpu_viewer -- simple winit application for viewing models and animations

## Building
With the Rust toolchain installed, run `cargo build --release`. Don't forget the `--release` since debug builds have very low framerates!
