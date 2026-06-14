use std::fs;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::rc::Rc;

use anyhow::{Context as _, Result};
use clap::Parser;
use image::{DynamicImage, GenericImageView};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

/// Cap for the default window size when no explicit dimensions are given, so
/// large images do not spawn windows bigger than the screen.
const MAX_DEFAULT_WIDTH: u32 = 1280;
const MAX_DEFAULT_HEIGHT: u32 = 800;

#[derive(Parser)]
#[command(name = "riv")]
#[command(about = "Rust Image Viewer - A GUI image viewer")]
#[command(version = "0.1.0")]
struct Args {
    #[arg(help = "Path(s) to the image file(s). Use 'n' and 'p' to navigate, 'd' to delete")]
    image_paths: Vec<PathBuf>,

    #[arg(short, long, help = "Enable verbose output")]
    verbose: bool,

    #[arg(short = 'w', long, help = "Set initial window width")]
    width: Option<u32>,

    #[arg(short = 'H', long, help = "Set initial window height")]
    height: Option<u32>,
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

    let event_loop = EventLoop::new().context("Failed to create event loop")?;
    // Event-driven: the loop sleeps until something happens instead of busy
    // redrawing, so an idle viewer uses no CPU.
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::new(args).context("Failed to load the first image")?;
    event_loop.run_app(&mut app).context("Event loop error")?;

    Ok(())
}

/// A copy of the source image resized to fit the current window, kept around so
/// we only run the (relatively expensive) resize when the window size or the
/// image actually changes — not on every frame.
struct Scaled {
    pixels: Vec<u32>,
    width: u32,
    height: u32,
}

type SharedWindow = Rc<Window>;

struct App {
    paths: Vec<PathBuf>,
    index: usize,
    image: DynamicImage,
    /// Bumped whenever a new image is loaded, used to invalidate `scaled`.
    generation: u64,
    requested_width: Option<u32>,
    requested_height: Option<u32>,
    fullscreen: bool,

    window: Option<SharedWindow>,
    context: Option<softbuffer::Context<SharedWindow>>,
    surface: Option<softbuffer::Surface<SharedWindow, SharedWindow>>,

    scaled: Option<Scaled>,
    /// (window_width, window_height, generation) the cached `scaled` was built for.
    scaled_key: Option<(u32, u32, u64)>,
}

impl App {
    fn new(args: Args) -> Result<Self> {
        let image = open_image(&args.image_paths[0])?;
        Ok(App {
            paths: args.image_paths,
            index: 0,
            image,
            generation: 0,
            requested_width: args.width,
            requested_height: args.height,
            fullscreen: false,
            window: None,
            context: None,
            surface: None,
            scaled: None,
            scaled_key: None,
        })
    }

    fn title(&self) -> String {
        let filename = self.paths[self.index]
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        if self.paths.len() > 1 {
            format!(
                "RIV - {} ({}/{}) - [f] fullscreen, [n] next, [p] prev, [d] delete",
                filename,
                self.index + 1,
                self.paths.len()
            )
        } else {
            format!("RIV - {} - [f] fullscreen, [d] delete", filename)
        }
    }

    fn handle_key(&mut self, key: Key, event_loop: &ActiveEventLoop) {
        match key.as_ref() {
            Key::Named(NamedKey::Escape) | Key::Character("q") | Key::Character("Q") => {
                event_loop.exit();
            }
            Key::Character("f") | Key::Character("F") => self.toggle_fullscreen(),
            Key::Character("n") | Key::Character("N") => self.step_image(1),
            Key::Character("p") | Key::Character("P") => self.step_image(-1),
            Key::Character("d") | Key::Character("D") => self.delete_image(event_loop),
            _ => {}
        }
    }

    fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        if let Some(window) = &self.window {
            // `Borderless(None)` fullscreens on the monitor the window is
            // currently on, so multi-monitor setups behave correctly without
            // recreating the window.
            let mode = self.fullscreen.then_some(Fullscreen::Borderless(None));
            window.set_fullscreen(mode);
            window.request_redraw();
        }
    }

    fn step_image(&mut self, delta: isize) {
        if self.paths.len() < 2 {
            return;
        }
        let len = self.paths.len() as isize;
        self.index = (((self.index as isize + delta) % len + len) % len) as usize;
        self.show_current();
    }

    fn delete_image(&mut self, event_loop: &ActiveEventLoop) {
        let path = self.paths[self.index].clone();
        if let Err(e) = fs::remove_file(&path) {
            eprintln!("Failed to delete image {}: {}", path.display(), e);
            return;
        }
        println!("Deleted image: {}", path.display());

        self.paths.remove(self.index);
        if self.paths.is_empty() {
            event_loop.exit();
            return;
        }
        if self.index >= self.paths.len() {
            self.index = 0;
        }
        if !self.show_current() {
            // Could not load any remaining image; nothing left to show.
            event_loop.exit();
        }
    }

    /// Load `self.index` and refresh the window. Returns whether the load
    /// succeeded; on failure the previously shown image is kept.
    fn show_current(&mut self) -> bool {
        match open_image(&self.paths[self.index]) {
            Ok(img) => {
                self.image = img;
                self.generation += 1;
                if let Some(window) = &self.window {
                    window.set_title(&self.title());
                    window.request_redraw();
                }
                true
            }
            Err(e) => {
                eprintln!(
                    "Failed to load image {}: {}",
                    self.paths[self.index].display(),
                    e
                );
                false
            }
        }
    }

    fn render(&mut self) {
        let (Some(window), Some(surface)) = (self.window.as_ref(), self.surface.as_mut()) else {
            return;
        };

        let size = window.inner_size();
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return; // Minimized or zero-sized; nothing to draw.
        };

        if let Err(e) = surface.resize(width, height) {
            eprintln!("Failed to resize surface: {e}");
            return;
        }
        let (w, h) = (width.get(), height.get());

        // Recompute the fitted image only when the window size or image changed.
        if self.scaled_key != Some((w, h, self.generation)) {
            self.scaled = Some(fit_image(&self.image, w, h));
            self.scaled_key = Some((w, h, self.generation));
        }
        let scaled = self.scaled.as_ref().unwrap();

        let mut buffer = match surface.buffer_mut() {
            Ok(buffer) => buffer,
            Err(e) => {
                eprintln!("Failed to acquire buffer: {e}");
                return;
            }
        };

        // Black background, then blit the image centered (letterboxing).
        buffer.fill(0);
        let x_offset = (w - scaled.width) / 2;
        let y_offset = (h - scaled.height) / 2;
        for row in 0..scaled.height {
            let dst = ((y_offset + row) * w + x_offset) as usize;
            let src = (row * scaled.width) as usize;
            let len = scaled.width as usize;
            buffer[dst..dst + len].copy_from_slice(&scaled.pixels[src..src + len]);
        }

        if let Err(e) = buffer.present() {
            eprintln!("Failed to present buffer: {e}");
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let (img_w, img_h) = self.image.dimensions();
        let (win_w, win_h) =
            initial_size(img_w, img_h, self.requested_width, self.requested_height);

        let attributes = Window::default_attributes()
            .with_title(self.title())
            .with_inner_size(LogicalSize::new(win_w, win_h));

        let window = match event_loop.create_window(attributes) {
            Ok(window) => Rc::new(window),
            Err(e) => {
                eprintln!("Failed to create window: {e}");
                event_loop.exit();
                return;
            }
        };

        let context = match softbuffer::Context::new(window.clone()) {
            Ok(context) => context,
            Err(e) => {
                eprintln!("Failed to create graphics context: {e}");
                event_loop.exit();
                return;
            }
        };
        let surface = match softbuffer::Surface::new(&context, window.clone()) {
            Ok(surface) => surface,
            Err(e) => {
                eprintln!("Failed to create drawing surface: {e}");
                event_loop.exit();
                return;
            }
        };

        window.request_redraw();
        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            // Act on the initial press only; `repeat` filters out the OS
            // key-repeat stream so a held key fires once.
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && !event.repeat =>
            {
                self.handle_key(event.logical_key, event_loop);
            }
            WindowEvent::Resized(_) => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => self.render(),
            _ => {}
        }
    }
}

fn open_image(path: &PathBuf) -> Result<DynamicImage> {
    image::open(path).with_context(|| format!("Failed to open image: {}", path.display()))
}

/// Resize `img` to fit within `win_w` x `win_h`, preserving aspect ratio, and
/// pack the result into a softbuffer-format pixel buffer (`0x00RRGGBB`).
fn fit_image(img: &DynamicImage, win_w: u32, win_h: u32) -> Scaled {
    // Triangle (bilinear) is far cheaper than Lanczos3 while staying smooth,
    // which keeps live window resizing responsive.
    let resized = img.resize(win_w, win_h, image::imageops::FilterType::Triangle);
    let (width, height) = resized.dimensions();
    let rgb = resized.to_rgb8();

    let mut pixels = Vec::with_capacity((width * height) as usize);
    for pixel in rgb.pixels() {
        pixels.push((pixel[0] as u32) << 16 | (pixel[1] as u32) << 8 | pixel[2] as u32);
    }

    Scaled {
        pixels,
        width,
        height,
    }
}

/// Pick the initial window size: honor explicit dimensions, otherwise use the
/// image size scaled down to fit within the default cap (never upscaled).
fn initial_size(img_w: u32, img_h: u32, req_w: Option<u32>, req_h: Option<u32>) -> (u32, u32) {
    match (req_w, req_h) {
        (Some(w), Some(h)) => (w.max(1), h.max(1)),
        (Some(w), None) => (w.max(1), img_h.max(1)),
        (None, Some(h)) => (img_w.max(1), h.max(1)),
        (None, None) => {
            let img_w = img_w.max(1);
            let img_h = img_h.max(1);
            let scale = (MAX_DEFAULT_WIDTH as f64 / img_w as f64)
                .min(MAX_DEFAULT_HEIGHT as f64 / img_h as f64)
                .min(1.0);
            (
                ((img_w as f64 * scale).round() as u32).max(1),
                ((img_h as f64 * scale).round() as u32).max(1),
            )
        }
    }
}
