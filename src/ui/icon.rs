//! The Launchpad icon, rasterised at runtime (no image assets, no decoder dependency).
//!
//! A rounded violet square holding a 2×2 grid of white tiles. 4×4 supersampling gives
//! clean anti-aliased edges at tray sizes (16–32 px) and window-icon sizes.
//!
//! Std-only on purpose: `build.rs` includes this file to embed the same icon into the
//! Windows executable, so it must not depend on any crate.

pub fn render(size: u32) -> Vec<u8> {
    let (top, bottom) = ([0x8a, 0x7b, 0xff], [0x5b, 0x49, 0xf2]);
    const SS: u32 = 4;
    let mut out = vec![0u8; (size * size * 4) as usize];
    for py in 0..size {
        for px in 0..size {
            let mut bg_cov = 0.0f32;
            let mut fg_cov = 0.0f32;
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = (px as f32 + (sx as f32 + 0.5) / SS as f32) / size as f32;
                    let y = (py as f32 + (sy as f32 + 0.5) / SS as f32) / size as f32;
                    if in_rounded_rect(x, y, 0.02, 0.02, 0.98, 0.98, 0.22) {
                        bg_cov += 1.0;
                        if in_glyph(x, y) {
                            fg_cov += 1.0;
                        }
                    }
                }
            }
            let n = (SS * SS) as f32;
            let (bg_a, fg_a) = (bg_cov / n, fg_cov / n);
            let t = py as f32 / size as f32;
            let base = [0, 1, 2].map(|i| top[i] as f32 * (1.0 - t) + bottom[i] as f32 * t);
            let i = ((py * size + px) * 4) as usize;
            if bg_a > 0.0 {
                // White glyph over the gradient (fg coverage is a fraction of bg coverage).
                let mix = fg_a / bg_a;
                for c in 0..3 {
                    out[i + c] = (base[c] * (1.0 - mix) + 255.0 * mix).round() as u8;
                }
                out[i + 3] = (bg_a * 255.0).round() as u8;
            }
        }
    }
    out
}

fn in_rounded_rect(x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> bool {
    if x < x0 || x > x1 || y < y0 || y > y1 {
        return false;
    }
    let cx = x.clamp(x0 + r, x1 - r);
    let cy = y.clamp(y0 + r, y1 - r);
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

/// Four rounded tiles in a 2×2 grid.
fn in_glyph(x: f32, y: f32) -> bool {
    const SPANS: [(f32, f32); 2] = [(0.25, 0.46), (0.54, 0.75)];
    SPANS.iter().any(|&(x0, x1)| SPANS.iter().any(|&(y0, y1)| in_rounded_rect(x, y, x0, y0, x1, y1, 0.055)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_opaque_center_and_transparent_corner() {
        let px = render(32);
        assert_eq!(px.len(), 32 * 32 * 4);
        assert_eq!(px[3], 0, "top-left corner is outside the rounded rect");
        let tile = ((11 * 32 + 11) * 4) as usize;
        assert_eq!(px[tile + 3], 255);
        assert_eq!(px[tile], 255, "inside a glyph tile is white");
    }
}
