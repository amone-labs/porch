//! Rotate the existing template icon; no board reads or image decoding per tick.
use std::time::Duration;
use tauri::{image::Image, AppHandle};

pub fn start(app: &AppHandle, tray_id: &'static str) -> tauri::Result<()> {
    let original = Image::from_bytes(include_bytes!("../icons/tray.png"))?;
    let frames: Vec<_> = (0..80).map(|frame| rotate(&original, frame as f64 * std::f64::consts::TAU / 80.0)).collect();
    let app = app.clone();
    std::thread::spawn(move || {
        let mut frame = 0;
        let mut animated = false;
        loop {
            let running = crate::SUMMARY_ACTIVITY.running();
            if let Some(tray) = app.tray_by_id(tray_id) {
                if running {
                    // A plain set_icon resets macOS template rendering to false.
                    // Replace image and template mode in one main-thread operation.
                    let _ = tray.set_icon_with_as_template(Some(frames[frame].clone()), true);
                    frame = (frame + 1) % frames.len();
                    animated = true;
                } else if animated {
                    let _ = tray.set_icon_with_as_template(Some(original.clone()), true);
                    frame = 0;
                    animated = false;
                }
            } else {
                break;
            }
            std::thread::sleep(Duration::from_millis(if running { 50 } else { 200 }));
        }
    });
    Ok(())
}

/// Template icons use alpha only. Bilinear sampling keeps the small dots smooth.
fn rotate(source: &Image<'_>, angle: f64) -> Image<'static> {
    let (w, h) = (source.width() as usize, source.height() as usize);
    let mut pixels = vec![0; w * h * 4];
    let (sin, cos) = angle.sin_cos();
    let (cx, cy) = ((w as f64 - 1.0) / 2.0, (h as f64 - 1.0) / 2.0);
    for y in 0..h {
        for x in 0..w {
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            let sx = cos * dx + sin * dy + cx;
            let sy = -sin * dx + cos * dy + cy;
            let (ix, iy) = (sx.floor() as isize, sy.floor() as isize);
            let (fx, fy) = (sx - sx.floor(), sy - sy.floor());
            let mut alpha = 0.0;
            for (ox, wx) in [(0, 1.0 - fx), (1, fx)] {
                for (oy, wy) in [(0, 1.0 - fy), (1, fy)] {
                    let (px, py) = (ix + ox, iy + oy);
                    if px >= 0 && py >= 0 && px < w as isize && py < h as isize {
                        alpha += source.rgba()[(py as usize * w + px as usize) * 4 + 3] as f64 * wx * wy;
                    }
                }
            }
            pixels[(y * w + x) * 4 + 3] = alpha.round() as u8;
        }
    }
    Image::new_owned(pixels, w as u32, h as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_keeps_canvas_and_moves_alpha_clockwise() {
        let mut rgba = vec![0; 3 * 3 * 4];
        rgba[7] = 255; // top middle
        let source = Image::new_owned(rgba, 3, 3);
        let identity = rotate(&source, 0.0);
        assert_eq!(identity.rgba(), source.rgba());
        let rotated = rotate(&source, std::f64::consts::FRAC_PI_2);
        assert_eq!((rotated.width(), rotated.height()), (3, 3));
        assert_eq!(rotated.rgba()[23], 255); // middle right
        assert_eq!(rotated.rgba()[7], 0);
        assert_eq!(rotated.rgba().as_chunks::<4>().0.iter().map(|p| p[3] as u32).sum::<u32>(), 255);
    }
}
