use pixels::{Pixels, SurfaceTexture};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

// Configuration
const WIDTH: u32 = 256; // horizontal axis: offset 0..255
const HEIGHT: u32 = 512; // number of rows in the waterfall (history depth)

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    // h in [0, 360), s, v in [0,1]
    let c = v * s;
    let x = c * (1.0 - (((h / 60.0) % 2.0) - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = match h {
        h if h < 60.0 => (c, x, 0.0),
        h if h < 120.0 => (x, c, 0.0),
        h if h < 180.0 => (0.0, c, x),
        h if h < 240.0 => (0.0, x, c),
        h if h < 300.0 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let (r, g, b) = (r1 + m, g1 + m, b1 + m);
    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

fn byte_to_color(b: u8) -> [u8; 4] {
    let h = (b as f32) / 255.0 * 360.0;
    let (r, g, b) = hsv_to_rgb(h, 1.0, 1.0);
    [r, g, b, 0xFF]
}

fn main() {
    // Shared ring buffer of rows (each row is exactly 256 bytes)
    let rows: Arc<Mutex<VecDeque<[u8; WIDTH as usize]>>> = Arc::new(Mutex::new(VecDeque::with_capacity(HEIGHT as usize)));

    // Spawn background ZeroMQ receiver (PULL)
    {
        let rows_bg = Arc::clone(&rows);
        thread::spawn(move || {
            let ctx = zmq::Context::new();
            let socket = ctx.socket(zmq::PULL).expect("failed to create ZMQ PULL socket");
            socket.bind("tcp://0.0.0.0:1337").expect("failed to bind ZMQ PULL at tcp://0.0.0.0:1337");
            let mut last_sleep = Instant::now();
            loop {
                match socket.recv_bytes(zmq::DONTWAIT) {
                    Ok(bytes) => {
                        if bytes.len() == WIDTH as usize {
                            let mut buf = [0u8; WIDTH as usize];
                            buf.copy_from_slice(&bytes);
                            if let Ok(mut q) = rows_bg.lock() {
                                if q.len() as u32 == HEIGHT {
                                    q.pop_front();
                                }
                                q.push_back(buf);
                            }
                        } else {
                            // Ignore messages with incorrect size
                        }
                    }
                    Err(zmq::Error::EAGAIN) => {
                        // nothing to read now
                    }
                    Err(_) => {
                        // transient error; ignore
                    }
                }
                // Throttle a bit to avoid busy-spin if no messages
                if last_sleep.elapsed() < Duration::from_millis(5) {
                    thread::sleep(Duration::from_millis(1));
                }
                last_sleep = Instant::now();
            }
        });
    }

    // Create winit window and pixels surface
    let event_loop = EventLoop::new();
    let scale: u32 = 2; // present at 512x1024 but logical buffer remains 256x512
    let window = WindowBuilder::new()
        .with_title("ZeroMQ Waterfall (tcp://*:1337)")
        .with_inner_size(LogicalSize::new((WIDTH * scale) as f64, (HEIGHT * scale) as f64))
        .with_min_inner_size(LogicalSize::new((WIDTH) as f64, (HEIGHT) as f64))
        .build(&event_loop)
        .expect("failed to build window");

    let mut pixels = {
        let size = window.inner_size();
        let surface_texture = SurfaceTexture::new(size.width, size.height, &window);
        Pixels::new(WIDTH, HEIGHT, surface_texture).expect("failed to create pixels surface")
    };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll; // continuously redraw
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::KeyboardInput { input, .. } => {
                    if let Some(VirtualKeyCode::Escape) = input.virtual_keycode {
                        *control_flow = ControlFlow::Exit;
                    }
                }
                WindowEvent::Resized(size) => {
                    pixels.resize_surface(size.width, size.height).ok();
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    pixels.resize_surface(new_inner_size.width, new_inner_size.height).ok();
                }
                _ => {}
            },
            Event::MainEventsCleared => {
                // Request a redraw
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
                // Render current buffer
                let frame = pixels.frame_mut();
                // clear to black
                for px in frame.chunks_exact_mut(4) { px.copy_from_slice(&[0, 0, 0, 0xFF]); }

                let snapshot: Vec<[u8; WIDTH as usize]> = {
                    match rows.lock() { Ok(q) => q.iter().cloned().collect(), Err(_) => Vec::new() }
                };
                let rows_len = snapshot.len();
                // draw each row; newest at bottom
                for (ri, row) in snapshot.iter().enumerate() {
                    let y = (HEIGHT as usize - rows_len) + ri; // start from top so rows fill from bottom
                    if y >= HEIGHT as usize { continue; }
                    let base = y as u32 * WIDTH;
                    for (x, b) in row.iter().copied().enumerate() {
                        let idx = (base as usize + x) * 4;
                        let color = byte_to_color(b);
                        frame[idx..idx + 4].copy_from_slice(&color);
                    }
                }
                if let Err(e) = pixels.render() {
                    eprintln!("pixels render error: {e}");
                    *control_flow = ControlFlow::Exit;
                }
            }
            _ => {}
        }
    });
}
