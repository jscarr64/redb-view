# Changelog

## 1.1.0 — 2026-10-14

- Optional `gui` feature (still off by default) now depends on egui 0.36.2 and eframe 0.36.2. The desktop browser uses `App::ui`, `Panel` (`top` / `left`), `ScrollArea::id_salt` / `ComboBox::from_id_salt`, `Context::set_zoom_factor`, and `ThemePreference` (system theme comes from `Context::system_theme`; an unknown OS theme still falls back to light).
- eframe is built with `default-features = false` plus `accesskit`, `default_fonts`, `wgpu`, `wayland`, and `x11`. `wgpu` is the renderer because it has been eframe's default since 0.34 and this viewer does not use OpenGL. `links` and `inspection` stay disabled.
- File dialogs use rfd 0.17.2.
- Minimum supported Rust version is 1.99 (`rust-version` and `rust-toolchain.toml` channel `1.99.0`).

## 1.0.1 — 2026-09-19

Coordinated patch with zenith-float, latex-rust, and hdf5-rust (pure-Rust FOSS family adjacent to Accumath; Accumath itself stays proprietary).

