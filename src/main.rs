use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::path::PathBuf;

use image::GenericImageView;
use minifb::{Key, Window, WindowOptions};

#[derive(Parser)]
#[command(name = "riv")]
#[command(about = "Rust Image Viewer - A GUI image viewer")]
#[command(version = "0.1.0")]
struct Args {
    #[arg(help = "Path(s) to the image file(s). Use 'n' and 'p' to navigate, 'd' to delete")]
    image_paths: Vec<PathBuf>,

    #[arg(short, long, help = "Enable verbose output")]
    verbose: bool,

    #[arg(
        short = 'w',
        long,
        help = "Set window width (default: image width or 800)"
    )]
    width: Option<usize>,

    #[arg(
        short = 'H',
        long,
        help = "Set window height (default: image height or 600)"
    )]
    height: Option<usize>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.image_paths.is_empty() {
        eprintln!("Error: At least one image path must be provided");
        std::process::exit(1);
    }

    if args.verbose {
        println!("Loading {} image(s)", args.image_paths.len());
        for path in &args.image_paths {
            println!("  - {}", path.display());
        }
    }

    let viewer = ImageViewer::new(args.width, args.height);
    viewer.display_images(args.image_paths)?;

    Ok(())
}

struct ImageViewer {
    width: Option<usize>,
    height: Option<usize>,
}

impl ImageViewer {
    fn new(width: Option<usize>, height: Option<usize>) -> Self {
        ImageViewer { width, height }
    }

    fn display_images(&self, mut paths: Vec<PathBuf>) -> Result<()> {
        if paths.is_empty() {
            return Err(anyhow::anyhow!("No image paths provided"));
        }

        let mut current_index = 0;
        let mut current_img = self.load_image(&paths[current_index])?;
        let (img_width, img_height) = current_img.dimensions();

        let initial_width = self.width.unwrap_or(img_width as usize);
        let initial_height = self.height.unwrap_or(img_height as usize);

        let window_options = WindowOptions {
            resize: true,
            ..WindowOptions::default()
        };

        let mut window = Window::new(
            &self.get_window_title(&paths[current_index], current_index, paths.len()),
            initial_width,
            initial_height,
            window_options,
        )
        .context("Failed to create window")?;

        window.limit_update_rate(Some(std::time::Duration::from_millis(16)));

        let mut current_width = initial_width;
        let mut current_height = initial_height;
        let mut buffer = self.create_buffer(&current_img, current_width, current_height);
        let mut f_pressed = false;
        let mut n_pressed = false;
        let mut p_pressed = false;
        let mut d_pressed = false;
        let mut is_fullscreen = false;
        let fullscreen_width = 1920; // TODO: Get actual monitor dimensions
        let fullscreen_height = 1080;

        while window.is_open() && !window.is_key_down(Key::Q) && !window.is_key_down(Key::Escape) {
            let f_key_down = window.is_key_down(Key::F);
            let n_key_down = window.is_key_down(Key::N);
            let p_key_down = window.is_key_down(Key::P);
            let d_key_down = window.is_key_down(Key::D);

            // Handle fullscreen toggle
            if f_key_down && !f_pressed {
                is_fullscreen = !is_fullscreen;

                // Create new window with appropriate settings
                if is_fullscreen {
                    let fs_options = WindowOptions {
                        borderless: true,
                        resize: false,
                        ..WindowOptions::default()
                    };

                    window = Window::new(
                        &self.get_window_title(&paths[current_index], current_index, paths.len()),
                        fullscreen_width,
                        fullscreen_height,
                        fs_options,
                    )
                    .context("Failed to create fullscreen window")?;

                    window.set_position(0, 0);
                    current_width = fullscreen_width;
                    current_height = fullscreen_height;
                } else {
                    let win_options = WindowOptions {
                        resize: true,
                        ..WindowOptions::default()
                    };

                    window = Window::new(
                        &self.get_window_title(&paths[current_index], current_index, paths.len()),
                        initial_width,
                        initial_height,
                        win_options,
                    )
                    .context("Failed to create windowed window")?;

                    current_width = initial_width;
                    current_height = initial_height;
                }

                window.limit_update_rate(Some(std::time::Duration::from_millis(16)));
                buffer = self.create_buffer(&current_img, current_width, current_height);
                f_pressed = true;
            } else if !f_key_down {
                f_pressed = false;
            }

            // Handle next image
            if n_key_down && !n_pressed && paths.len() > 1 {
                current_index = (current_index + 1) % paths.len();
                match self.load_image(&paths[current_index]) {
                    Ok(img) => {
                        current_img = img;
                        buffer = self.create_buffer(&current_img, current_width, current_height);
                        window.set_title(&self.get_window_title(
                            &paths[current_index],
                            current_index,
                            paths.len(),
                        ));
                    }
                    Err(e) => eprintln!(
                        "Failed to load image {}: {}",
                        paths[current_index].display(),
                        e
                    ),
                }
                n_pressed = true;
            } else if !n_key_down {
                n_pressed = false;
            }

            // Handle previous image
            if p_key_down && !p_pressed && paths.len() > 1 {
                current_index = if current_index == 0 {
                    paths.len() - 1
                } else {
                    current_index - 1
                };
                match self.load_image(&paths[current_index]) {
                    Ok(img) => {
                        current_img = img;
                        buffer = self.create_buffer(&current_img, current_width, current_height);
                        window.set_title(&self.get_window_title(
                            &paths[current_index],
                            current_index,
                            paths.len(),
                        ));
                    }
                    Err(e) => eprintln!(
                        "Failed to load image {}: {}",
                        paths[current_index].display(),
                        e
                    ),
                }
                p_pressed = true;
            } else if !p_key_down {
                p_pressed = false;
            }

            // Handle delete image
            if d_key_down && !d_pressed {
                if let Err(e) = fs::remove_file(&paths[current_index]) {
                    eprintln!(
                        "Failed to delete image {}: {}",
                        paths[current_index].display(),
                        e
                    );
                } else {
                    println!("Deleted image: {}", paths[current_index].display());

                    // Remove the deleted file from the paths vector
                    paths.remove(current_index);

                    // If no images left, exit the application
                    if paths.is_empty() {
                        break;
                    }

                    // Adjust current_index if it's now out of bounds
                    if current_index >= paths.len() {
                        current_index = 0; // Wrap to first image
                    }

                    // Load the next image
                    match self.load_image(&paths[current_index]) {
                        Ok(img) => {
                            current_img = img;
                            buffer =
                                self.create_buffer(&current_img, current_width, current_height);
                            window.set_title(&self.get_window_title(
                                &paths[current_index],
                                current_index,
                                paths.len(),
                            ));
                        }
                        Err(e) => {
                            eprintln!(
                                "Failed to load next image {}: {}",
                                paths[current_index].display(),
                                e
                            );
                            break;
                        }
                    }
                }
                d_pressed = true;
            } else if !d_key_down {
                d_pressed = false;
            }

            // Handle window resizing
            if !is_fullscreen {
                let (window_width, window_height) = window.get_size();
                if window_width != current_width || window_height != current_height {
                    current_width = window_width;
                    current_height = window_height;
                    buffer = self.create_buffer(&current_img, current_width, current_height);
                }
            }

            window
                .update_with_buffer(&buffer, current_width, current_height)
                .context("Failed to update window buffer")?;
        }

        Ok(())
    }

    fn load_image(&self, path: &PathBuf) -> Result<image::DynamicImage> {
        image::open(path).with_context(|| format!("Failed to open image: {}", path.display()))
    }

    fn get_window_title(&self, path: &std::path::Path, index: usize, total: usize) -> String {
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        if total > 1 {
            format!(
                "RIV - {} ({}/{}) - [f] fullscreen, [n] next, [p] prev, [d] delete",
                filename,
                index + 1,
                total
            )
        } else {
            format!("RIV - {} - [f] fullscreen, [d] delete", filename)
        }
    }

    fn create_buffer(&self, img: &image::DynamicImage, width: usize, height: usize) -> Vec<u32> {
        let (img_width, img_height) = img.dimensions();
        let img_aspect = img_width as f64 / img_height as f64;
        let window_aspect = width as f64 / height as f64;

        let (scaled_width, scaled_height) = if img_aspect > window_aspect {
            // Image is wider than window - fit to width
            let scaled_width = width;
            let scaled_height = (width as f64 / img_aspect) as usize;
            (scaled_width, scaled_height)
        } else {
            // Image is taller than window - fit to height
            let scaled_height = height;
            let scaled_width = (height as f64 * img_aspect) as usize;
            (scaled_width, scaled_height)
        };

        // Create black buffer for letterboxing
        let mut buffer = vec![0u32; width * height]; // Black background

        // Resize image maintaining aspect ratio
        let resized_img = img.resize(
            scaled_width as u32,
            scaled_height as u32,
            image::imageops::FilterType::Lanczos3,
        );

        let resized_rgb = resized_img.to_rgb8();

        // Calculate centering offsets
        let x_offset = (width - scaled_width) / 2;
        let y_offset = (height - scaled_height) / 2;

        // Copy resized image to center of buffer
        for (y, row) in resized_rgb.rows().enumerate() {
            for (x, pixel) in row.enumerate() {
                let buffer_x = x + x_offset;
                let buffer_y = y + y_offset;
                let buffer_index = buffer_y * width + buffer_x;

                if buffer_index < buffer.len() {
                    let r = pixel[0] as u32;
                    let g = pixel[1] as u32;
                    let b = pixel[2] as u32;
                    buffer[buffer_index] = (r << 16) | (g << 8) | b;
                }
            }
        }

        buffer
    }
}
