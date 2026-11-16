//! The Yaperino logo, drawn in code so it is sharp at every size and needs no image
//! files: a rounded square with a purple-to-cyan gradient, a white chat bubble, and a
//! "Y" in the bubble. Used for the window icon, the program's icon (build.rs puts it
//! into Yaperino.exe, which is what Windows shows in Start, the taskbar and the Apps
//! list) and `--write-logo` (PNG and ICO files for the repository).
//!
//! Also compiled into build.rs, so it uses nothing from the rest of the app.

#![allow(dead_code)]

const PURPLE: [f32; 3] = [0x7b as f32, 0x3f as f32, 0xe4 as f32];
const CYAN: [f32; 3] = [0x00 as f32, 0xae as f32, 0xef as f32];
const INK: [f32; 3] = [0x4a as f32, 0x1d as f32, 0xa8 as f32];

fn rounded_rect(x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> bool {
    let cx = x.clamp(x0 + r, x1 - r);
    let cy = y.clamp(y0 + r, y1 - r);
    x >= x0 && x <= x1 && y >= y0 && y <= y1 && (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

fn segment_dist(px: f32, py: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
    let t = (((px - a.0) * vx + (py - a.1) * vy) / (vx * vx + vy * vy)).clamp(0.0, 1.0);
    ((px - a.0 - t * vx).powi(2) + (py - a.1 - t * vy).powi(2)).sqrt()
}

fn in_triangle(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let s = |p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)| {
        (p1.0 - p3.0) * (p2.1 - p3.1) - (p2.0 - p3.0) * (p1.1 - p3.1)
    };
    let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

/// The colour (straight RGBA, 0..255) at a point of the unit square, or None outside.
fn sample(x: f32, y: f32) -> Option<[f32; 4]> {
    if !rounded_rect(x, y, 0.03, 0.03, 0.97, 0.97, 0.22) {
        return None;
    }
    // the "Y": two arms and a stem with round ends
    let w = 0.062;
    let fork = (0.50, 0.445);
    let d = segment_dist(x, y, (0.365, 0.30), fork)
        .min(segment_dist(x, y, (0.635, 0.30), fork))
        .min(segment_dist(x, y, fork, (0.50, 0.585)));
    if d <= w {
        return Some([INK[0], INK[1], INK[2], 255.0]);
    }
    // the chat bubble with its tail at the lower left
    let bubble = rounded_rect(x, y, 0.17, 0.19, 0.83, 0.69, 0.17)
        || in_triangle((x, y), (0.27, 0.62), (0.47, 0.62), (0.22, 0.85));
    if bubble {
        return Some([255.0, 255.0, 255.0, 255.0]);
    }
    let t = ((x + y) / 2.0).clamp(0.0, 1.0);
    let c = |i: usize| PURPLE[i] + (CYAN[i] - PURPLE[i]) * t;
    Some([c(0), c(1), c(2), 255.0])
}

/// The logo as RGBA pixels, `size` x `size`, smoothed with 4 x 4 samples per pixel.
pub fn rgba(size: u32) -> Vec<u8> {
    let n = 4;
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let mut acc = [0.0f32; 4];
            for sy in 0..n {
                for sx in 0..n {
                    let x = (px as f32 + (sx as f32 + 0.5) / n as f32) / size as f32;
                    let y = (py as f32 + (sy as f32 + 0.5) / n as f32) / size as f32;
                    if let Some(c) = sample(x, y) {
                        // premultiplied, so edges blend into transparency
                        acc[0] += c[0];
                        acc[1] += c[1];
                        acc[2] += c[2];
                        acc[3] += 1.0;
                    }
                }
            }
            let cover = acc[3] / (n * n) as f32;
            if acc[3] > 0.0 {
                out.extend_from_slice(&[
                    (acc[0] / acc[3]) as u8,
                    (acc[1] / acc[3]) as u8,
                    (acc[2] / acc[3]) as u8,
                    (cover * 255.0).round() as u8,
                ]);
            } else {
                out.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    out
}

/// A Windows .ico with the logo at the given sizes (32-bit images, as Windows reads them).
pub fn ico(sizes: &[u32]) -> Vec<u8> {
    let mut images: Vec<Vec<u8>> = Vec::new();
    for &s in sizes {
        let px = rgba(s);
        let mut d = Vec::new();
        // BITMAPINFOHEADER: height counts the colour rows and the mask rows
        for v in [40u32, s, s * 2] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.extend_from_slice(&1u16.to_le_bytes());
        d.extend_from_slice(&32u16.to_le_bytes());
        for _ in 0..6 {
            d.extend_from_slice(&0u32.to_le_bytes());
        }
        // BGRA rows, bottom row first
        for y in (0..s).rev() {
            for x in 0..s {
                let i = ((y * s + x) * 4) as usize;
                d.extend_from_slice(&[px[i + 2], px[i + 1], px[i], px[i + 3]]);
            }
        }
        // the AND mask (all zero: the alpha channel decides), rows padded to 4 bytes
        let row = (s.div_ceil(32) * 4) as usize;
        d.extend(std::iter::repeat_n(0u8, row * s as usize));
        images.push(d);
    }
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (&s, img) in sizes.iter().zip(&images) {
        let b = if s >= 256 { 0u8 } else { s as u8 };
        out.extend_from_slice(&[b, b, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(img.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += img.len() as u32;
    }
    for img in images {
        out.extend_from_slice(&img);
    }
    out
}

/// The sizes put into the program's icon.
pub const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
