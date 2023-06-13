# egui

> egui (pronounced "e-gooey") is a simple, fast, and highly portable immediate mode GUI library for Rust.
>
> egui aims to be the easiest-to-use Rust GUI library, and the simplest way to make a web app in Rust.

[![](https://img.shields.io/crates/v/egui.svg)](https://crates.io/crates/egui)
[![Docs.rs](https://docs.rs/egui/badge.svg)](https://docs.rs/egui)

```
[dependencies]
egui = "0.22.0"
```

This repository provides binding for egui to use tao instead. Currently only `glow` backend is supported.

For more information on how to use egui, please check out [egui repository](https://github.com/emilk/egui) for both [simple examples](https://github.com/emilk/egui/tree/master/examples) and [detailed documents](https://docs.rs/egui).

## Who is egui for?

Quoting from egui repository:

> [...] if you are writing something interactive in Rust that needs a simple GUI, egui may be for you.

## Demo

Demo app uses [`eframe_tao`](https://github.com/tauri-apps/egui/tree/master/crates/eframe).

To test the demo app locally, run `cargo run --release -p egui_demo_app`.

The native backend is [`egui_glow_tao`](https://github.com/tauri-apps/egui/tree/master/crates/egui_glow) (using [`glow`](https://crates.io/crates/glow)) and should work out-of-the-box on Mac and Windows, but on Linux you need to first run:

`sudo apt-get install -y libclang-dev libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev`

On Fedora Rawhide you need to run:

`dnf install clang clang-devel clang-tools-extra libxkbcommon-devel pkg-config openssl-devel libxcb-devel gtk3-devel atk fontconfig-devel`

**NOTE**: This is just for the demo app - egui itself is completely platform agnostic!
