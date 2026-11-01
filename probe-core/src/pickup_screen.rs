//! Fixed-layout, offline visual vocabulary. Unknown interactions never become taps.
const W: usize = 1240;
const H: usize = 558;

fn ink(pixel: &[u8]) -> bool {
    pixel.iter().all(|v| *v > 185) && pixel.iter().max().unwrap() - pixel.iter().min().unwrap() < 60
}

fn text_score(rgb: &[u8], model: &[u8], x: usize, y: usize, w: usize, h: usize) -> f64 {
    let mut intersection = 0;
    let mut union = 0;
    for row in 0..h {
        for col in 0..w {
            let a = ink(&rgb[((y + row) * W + x + col) * 3..][..3]);
            let b = ink(&model[(row * w + col) * 3..][..3]);
            intersection += usize::from(a && b);
            union += usize::from(a || b);
        }
    }
    intersection as f64 / union.max(1) as f64
}

fn icon_matches(rgb: &[u8], model: &[u8], x: usize, y: usize, w: usize, h: usize) -> bool {
    let mut error = 0u32;
    for row in 0..h {
        for col in 0..w {
            for c in 0..3 {
                error += rgb[((y + row) * W + x + col) * 3 + c]
                    .abs_diff(model[(row * w + col) * 3 + c]) as u32;
            }
        }
    }
    error as usize / (w * h * 3) < 38
}

// Video preparation and runtime JPEGs differ at antialiased stroke edges. Require
// mutual coverage of the whole glyph mask within one overview pixel, not just a prefix.
fn glyph_score(rgb: &[u8], model: &[u8], x: usize, y: usize, w: usize, h: usize) -> f64 {
    let mut hits = 0;
    let mut total = 0;
    for row in 0..h {
        for col in 0..w {
            let actual = ink(&rgb[((y + row) * W + x + col) * 3..][..3]);
            let expected = ink(&model[(row * w + col) * 3..][..3]);
            for (present, in_model) in [(actual, true), (expected, false)] {
                if !present {
                    continue;
                }
                total += 1;
                let nearby = (row.saturating_sub(1)..=(row + 1).min(h - 1)).any(|r| {
                    (col.saturating_sub(1)..=(col + 1).min(w - 1)).any(|c| {
                        ink(if in_model {
                            &model[(r * w + c) * 3..][..3]
                        } else {
                            &rgb[((y + r) * W + x + c) * 3..][..3]
                        })
                    })
                });
                hits += usize::from(nearby);
            }
        }
    }
    hits as f64 / total.max(1) as f64
}

fn obtained_icon(rgb: &[u8], x: usize, y: usize) -> bool {
    let model = include_bytes!("../assets/pickup-obtained-icon.rgb");
    let mut error = 0i32;
    let mut samples = 0;
    for row in 0..25 {
        for col in 0..24 {
            let m = &model[(row * 24 + col) * 3..][..3];
            if m.iter().max().unwrap() - m.iter().min().unwrap() <= 60 {
                continue;
            }
            let a = &rgb[((y + row) * W + x + col) * 3..][..3];
            for channel in [0, 2] {
                error +=
                    ((a[channel] as i32 - a[1] as i32) - (m[channel] as i32 - m[1] as i32)).abs();
                samples += 1;
            }
        }
    }
    samples >= 40 && error / samples < 25
}

pub(crate) fn prompt(rgb: &[u8]) -> Option<(u32, u32)> {
    if rgb.len() != W * H * 3 {
        return None;
    }
    let model = include_bytes!("../assets/pickup-seed-text.rgb");
    for y in 165..375 {
        for x in 765..785 {
            if text_score(rgb, model, x, y, 148, 19) > 0.88
                && icon_matches(
                    rgb,
                    include_bytes!("../assets/pickup-seed-icon.rgb"),
                    x - 31,
                    y - 3,
                    22,
                    26,
                )
            {
                return Some(((x + 60) as u32 * 2, (y + 9) as u32 * 2));
            }
        }
    }
    None
}

pub(crate) fn obtained(rgb: &[u8]) -> bool {
    if rgb.len() != W * H * 3 {
        return false;
    }
    for y in 250..355 {
        for x in 143..156 {
            if text_score(
                rgb,
                include_bytes!("../assets/pickup-obtained-text.rgb"),
                x,
                y,
                88,
                20,
            ) > 0.45
                && glyph_score(
                    rgb,
                    include_bytes!("../assets/pickup-obtained-text.rgb"),
                    x,
                    y,
                    88,
                    20,
                ) > 0.93
                && glyph_score(
                    rgb,
                    include_bytes!("../assets/pickup-obtained-heading.rgb"),
                    x + 5,
                    y - 27,
                    32,
                    18,
                ) > 0.85
                && obtained_icon(rgb, x - 27, y - 1)
            {
                return true;
            }
        }
    }
    false
}
