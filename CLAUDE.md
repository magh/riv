# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

RIV (Rust Image Viewer) is a single-binary CLI that opens one or more images in a native GUI window. It is a thin wrapper around four crates: `clap` (arg parsing), `image` (decode + resize), `winit` (window + event loop + fullscreen + monitor handling), and `softbuffer` (a CPU framebuffer presented to the winit window). All logic lives in `src/main.rs`.

## Commands

```bash
cargo build              # debug build
cargo build --release    # release build -> target/release/riv
cargo run -- <images>    # build and run, e.g. cargo run -- a.jpg b.png
cargo fmt                # format
cargo clippy             # lint (treat warnings as issues to fix)
cargo test               # run tests (none exist yet)
```

Run a single test once tests exist: `cargo test <name>`.

## Architecture

The program is a single `App` struct that implements winit's `ApplicationHandler`. `main` builds the event loop (`ControlFlow::Wait`) and calls `run_app`. Key pieces:

- **Event-driven, not a busy loop.** Work happens in `window_event` (close, keyboard, resize, redraw) and `resumed` (one-time window/context/surface creation). Idle = zero CPU; we only ever paint after calling `request_redraw()` in response to a state change.
- **Rendering is a CPU software framebuffer via softbuffer.** `render()` resizes the surface to the window's physical size, writes `u32` pixels in `0x00RRGGBB`, and `present()`s. There is no GPU path.
- **Resize is cached, not per-frame.** `fit_image` resizes the source with `FilterType::Triangle` (fast bilinear) into `App::scaled`. It only re-runs when the `scaled_key` `(width, height, generation)` changes — `generation` bumps on every image load. Picking a faster filter and skipping redundant resizes is what keeps live window-dragging smooth.
- **Keys are edge-triggered for free.** winit delivers `KeyEvent`; we act only on `state == Pressed && !repeat`. `logical_key.as_ref()` is matched against `Key::Character("f")` etc. (match both cases). No manual debounce bookkeeping.
- **Fullscreen is real.** `toggle_fullscreen` calls `window.set_fullscreen(Some(Fullscreen::Borderless(None)))`; `None` targets the monitor the window is currently on, so multi-monitor works without recreating the window. This replaced the old minifb path that hardcoded `1920x1080` and rebuilt the window.
- **Navigation is circular** (`step_image`) over the `paths` vector; `d` (`delete_image`) removes the file from disk (`fs::remove_file`) and from `paths`, exiting when empty.

## Constraints worth knowing

- softbuffer gives a raw framebuffer only — no text/overlay rendering; status info goes in the window title (`App::title`).
- The window/surface live behind `Rc<Window>` because softbuffer's `Context`/`Surface` both need to hold the window handle. `Surface<D, W>` is generic over display+window handle types — keep them as `SharedWindow = Rc<Window>`.
- `image` is pinned at 0.24; format support comes from its default features (see README for the format list).
