# Changelog for egui-winit

## \[0.21.0]

- Update tao to 0.17 and glutin_tao to 0.31.
  - [f5f220a4](https://github.com/tauri-apps/egui/commit/f5f220a46c063e70fb276c472764d5be1f286c45) Update tao to 0.17 and glutin_tao to 0.31 on 2023-03-10

## \[0.20.1]

- Update tao to 0.15 and glutin_tao to 0.30.1
  - [6ec685ac](https://github.com/tauri-apps/egui/commit/6ec685ac2ee91b7516ef676afa142c12e4dac661) chore(deps): update tao to 0.15 and glutin_tao to 0.30.1([#7](https://github.com/tauri-apps/egui/pull/7)) on 2022-11-08

## \[0.20.0]

- Update tao to 0.14.
  - [a8fbfed7](https://github.com/tauri-apps/egui/commit/a8fbfed7bc45ba42a1623bcb6487a4301d93e996) setup covector on 2022-09-16

All notable changes to the `egui-winit` integration will be noted in this file.

## Unreleased

## 0.19.0 - 2022-08-20

- MSRV (Minimum Supported Rust Version) is now `1.61.0` ([#1846](https://github.com/emilk/egui/pull/1846)).
- Fixed clipboard on Wayland ([#1613](https://github.com/emilk/egui/pull/1613)).
- Allow deferred render + surface state initialization for Android ([#1634](https://github.com/emilk/egui/pull/1634)).
- Fixed window position persistence ([#1745](https://github.com/emilk/egui/pull/1745)).
- Fixed mouse cursor change on Linux ([#1747](https://github.com/emilk/egui/pull/1747)).
- Use the new `RawInput::has_focus` field to indicate whether the window has the keyboard focus ([#1859](https://github.com/emilk/egui/pull/1859)).

## 0.18.0 - 2022-04-30

- Reexport `egui` crate
- MSRV (Minimum Supported Rust Version) is now `1.60.0` ([#1467](https://github.com/emilk/egui/pull/1467)).
- Added new feature `puffin` to add [`puffin profiler`](https://github.com/EmbarkStudios/puffin) scopes ([#1483](https://github.com/emilk/egui/pull/1483)).
- Renamed the feature `convert_bytemuck` to `bytemuck` ([#1467](https://github.com/emilk/egui/pull/1467)).
- Renamed the feature `serialize` to `serde` ([#1467](https://github.com/emilk/egui/pull/1467)).
- Removed the features `dark-light` and `persistence` ([#1542](https://github.com/emilk/egui/pull/1542)).

## 0.17.0 - 2022-02-22

- Fixed horizontal scrolling direction on Linux.
- Replaced `std::time::Instant` with `instant::Instant` for WebAssembly compatability ([#1023](https://github.com/emilk/egui/pull/1023))
- Automatically detect and apply dark or light mode from system ([#1045](https://github.com/emilk/egui/pull/1045)).
- Fixed `enable_drag` on Windows OS ([#1108](https://github.com/emilk/egui/pull/1108)).
- Shift-scroll will now result in horizontal scrolling on all platforms ([#1136](https://github.com/emilk/egui/pull/1136)).
- Require knowledge about max texture side (e.g. `GL_MAX_TEXTURE_SIZE`)) ([#1154](https://github.com/emilk/egui/pull/1154)).

## 0.16.0 - 2021-12-29

- Added helper `EpiIntegration` ([#871](https://github.com/emilk/egui/pull/871)).
- Fixed shift key getting stuck enabled with the X11 option `shift:both_capslock` enabled ([#849](https://github.com/emilk/egui/pull/849)).
- Removed `State::is_quit_event` and `State::is_quit_shortcut` ([#881](https://github.com/emilk/egui/pull/881)).
- Updated `winit` to 0.26 ([#930](https://github.com/emilk/egui/pull/930)).

## 0.15.0 - 2021-10-24

First stand-alone release. Previously part of `egui_glium`.
