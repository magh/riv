# RIV - Rust Image Viewer

A simple GUI image viewer written in Rust that displays images in a native window.

## Features

- Displays images in a native GUI window
- Multi-image support with navigation controls
- Supports common image formats (PNG, JPEG, GIF, BMP, etc.)
- Automatic window sizing based on image dimensions
- Custom window width and height options
- Fullscreen toggle functionality
- Simple keyboard controls for navigation

## Installation

Build from source:

```bash
cargo build --release
```

The binary will be available at `target/release/riv`.

## Usage

Basic usage:
```bash
# Single image
riv path/to/image.jpg

# Multiple images with navigation
riv image1.jpg image2.png image3.gif
```

Options:
```bash
riv --help
```

```
Rust Image Viewer - A GUI image viewer

Usage: riv [OPTIONS] <IMAGE_PATHS>...

Arguments:
  <IMAGE_PATHS>...  Path(s) to the image file(s). Use 'n' and 'p' to navigate, 'd' to delete

Options:
  -v, --verbose          Enable verbose output
  -w, --width <WIDTH>    Set window width (default: image width or 800)
  -H, --height <HEIGHT>  Set window height (default: image height or 600)
  -h, --help             Print help
  -V, --version          Print version
```

## Controls

- `q` or `Esc`: Close the window and quit
- `f`: Toggle fullscreen mode
- `n`: Next image (when multiple images loaded)
- `p`: Previous image (when multiple images loaded)
- `d`: Delete current image and move to next (exits if no images remain)
- Close button: Close the window and quit
- Window resizing: Drag window edges to resize (image scales automatically)

## Examples

View a single image:
```bash
riv photo.jpg
```

View multiple images with navigation:
```bash
riv photo1.jpg photo2.png photo3.gif
riv *.jpg *.png  # Using shell globbing
```

View with custom window size:
```bash
riv --width 1024 --height 768 photo.jpg
```

Verbose output:
```bash
riv --verbose photo1.jpg photo2.jpg
```

## Supported Image Formats

- JPEG
- PNG
- GIF
- BMP
- ICO
- TIFF
- WebP
- AVIF
- PNM
- DDS
- TGA
- OpenEXR
- farbfeld
