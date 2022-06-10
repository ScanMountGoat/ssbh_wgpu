# ssbh_wgpu
ssbh_wgpu is a lightweight and portable renderer for Smash Ultimate built using [WGPU](https://github.com/gfx-rs/wgpu).

## Overview
- nutexb_wgpu -- texture renderer and library for converting nutexb files to WGPU textures
- ssbh_wgpu -- model and animation rendering library. Converts ssbh_data types to WGPU types.

## Building
With the Rust toolchain installed, run `cargo build --release`. Don't forget the `--release` since debug builds have very low framerates!

## Credits
- Hack Font - [License](https://github.com/source-foundry/Hack/blob/master/LICENSE.md)