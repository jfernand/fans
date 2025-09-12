use std::collections::VecDeque;
use std::error::Error;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use piston_window::{EventLoop, PistonWindow, WindowSettings};
use plotters::prelude::*;
use plotters_piston::{draw_piston_window, PistonBackend};

fn read_ec_io() -> std::io::Result<Vec<u8>> {
    let f = File::open("/sys/kernel/debug/ec/ec0/io")?;
    let mut buf = vec![0u8; 256];
    f.read_at(&mut buf, 0)?;
    Ok(buf)
}

fn val_to_color(v: u8) -> HSLColor {
    // Map 0..255 to hue 0..360 for a rainbow-like palette
    let h = (v as f64) / 255.0 * 360.0;
    HSLColor(h, 1.0, 0.5)
}

fn main() -> Result<(), Box<dyn Error>> {
    // Waterfall buffer: last N rows, each row is 256 bytes (one per offset)
    const MAX_ROWS: usize = 512; // visible history depth
    let rows: Arc<Mutex<VecDeque<Vec<u8>>>> = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_ROWS)));

    // Spawn a background sampler that loops forever every 100ms
    {
        let rows_bg = Arc::clone(&rows);
        thread::spawn(move || {
            loop {
                if let Ok(buf) = read_ec_io() {
                    if let Ok(mut q) = rows_bg.lock() {
                        if q.len() == MAX_ROWS {
                            q.pop_front();
                        }
                        q.push_back(buf);
                    }
                }
                thread::sleep(Duration::from_millis(100));
            }
        });
    }

    // Create the window first
    let (width, height) = (2048u32, 800u32);
    let mut window: PistonWindow = WindowSettings::new(
        "EC I/O Waterfall (100ms sampling)",
        [width, height],
    )
    .exit_on_esc(true)
    .build()
    .map_err(|e| format!("Failed to create window: {}", e))?;

    // Target ~10 frames per second (~100ms per frame)
    window.set_max_fps(10);

    // Event/render loop
    let rows_for_draw = Arc::clone(&rows);
    draw_piston_window(&mut window, move |b: PistonBackend| {
        // Draw the waterfall: x = offset (0..255), y = time (older at top, newest at bottom)
        let root = b.into_drawing_area();
        root.fill(&WHITE)?;

        // Build a chart without mesh for performance; coordinate space is in cell units
        let mut chart = ChartBuilder::on(&root)
            .caption("EC I/O Waterfall", ("sans-serif", 24))
            .margin(10)
            .x_label_area_size(20)
            .y_label_area_size(30)
            .build_cartesian_2d(0..256i32, 0..MAX_ROWS as i32)?;

        // Optionally draw light axes without grid to keep it clean
        chart
            .configure_mesh()
            .disable_mesh()
            .x_desc("Offset (0..255)")
            .y_desc("Time (older -> newer)")
            .axis_desc_style(("sans-serif", 14))
            .draw()?;

        // Snapshot rows under lock to minimize lock duration during drawing
        let rows_snapshot: Vec<Vec<u8>> = match rows_for_draw.lock() {
            Ok(q) => q.iter().cloned().collect(),
            Err(_) => Vec::new(),
        };

        let current_rows = rows_snapshot.len();
        if current_rows > 0 {
            chart.draw_series(
                rows_snapshot.iter().enumerate().flat_map(|(ri, row)| {
                    // Map logical row index to display y so that newest is at bottom
                    let y = (MAX_ROWS - current_rows) as i32 + ri as i32;
                    row.iter().enumerate().map(move |(x, &v)| {
                        let color = val_to_color(v);
                        Rectangle::new([(x as i32, y), (x as i32 + 1, y + 1)], color.filled())
                    })
                }),
            )?;
        }

        root.present()?;
        loop{}
        Ok(())
    });

    Ok(())
}