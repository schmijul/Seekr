# Seekr

`Seekr` is a local-first desktop search app built with Tauri (Rust backend + React frontend).

This milestone initializes the project scaffold and verifies the app builds and starts locally.

## Milestone 1 status

- Project scaffolded with Tauri v2 + React + TypeScript.
- Frontend build is passing (`npm run build`).
- Rust toolchain updated to stable (`rustc 1.94.1`, `cargo 1.94.1`).

## Linux prerequisite note

Tauri on Linux requires WebKitGTK development libraries. In this environment, Rust/Tauri compilation is blocked by missing `javascriptcoregtk-4.1` (`.pc`) from system packages.
