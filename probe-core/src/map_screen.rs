//! Verified fixed-layout map controls and atlas-backed target projection.
use crate::localization::{atlas, match_geometry, Position};
use lowe_sift::{match_features, GrayImage, Sift};
pub(crate) const WIDTH: usize = 1240;
pub(crate) const HEIGHT: usize = 558;

fn find(
    rgb: &[u8],
    model: &[u8],
    width: usize,
    height: usize,
    bounds: (usize, usize, usize, usize),
    foreground_only: bool,
) -> Option<(u32, u32)> {
    if rgb.len() != WIDTH * HEIGHT * 3 || model.len() != width * height * 3 {
        return None;
    }
    let mut best = (f64::MAX, 0, 0);
    for y in (bounds.1..bounds.3.min(HEIGHT - height)).step_by(2) {
        for x in (bounds.0..bounds.2.min(WIDTH - width)).step_by(2) {
            let mut error = 0.0;
            let mut n = 0;
            for row in (0..height).step_by(if foreground_only && width < 32 { 1 } else { 2 }) {
                for col in (0..width).step_by(if foreground_only && width < 32 { 1 } else { 2 }) {
                    if foreground_only
                        && !model[(row * width + col) * 3..][..3]
                            .iter()
                            .all(|v| *v > 210)
                    {
                        continue;
                    }
                    for c in 0..3 {
                        error += (rgb[((y + row) * WIDTH + x + col) * 3 + c] as f64
                            - model[(row * width + col) * 3 + c] as f64)
                            .abs();
                        n += 1;
                    }
                }
            }
            let score = error / n as f64;
            if score < best.0 {
                best = (score, x, y);
            }
        }
    }
    let coarse = best;
    for y in coarse.2.saturating_sub(2)..=(coarse.2 + 2).min(HEIGHT - height) {
        for x in coarse.1.saturating_sub(2)..=(coarse.1 + 2).min(WIDTH - width) {
            let mut error = 0.0;
            let mut n = 0;
            for row in (0..height).step_by(if foreground_only && width < 32 { 1 } else { 2 }) {
                for col in (0..width).step_by(if foreground_only && width < 32 { 1 } else { 2 }) {
                    if foreground_only
                        && !model[(row * width + col) * 3..][..3]
                            .iter()
                            .all(|v| *v > 210)
                    {
                        continue;
                    }
                    for c in 0..3 {
                        error += (rgb[((y + row) * WIDTH + x + col) * 3 + c] as f64
                            - model[(row * width + col) * 3 + c] as f64)
                            .abs();
                        n += 1;
                    }
                }
            }
            let score = error / n as f64;
            if score < best.0 {
                best = (score, x, y);
            }
        }
    }
    if foreground_only && width > 32 {
        let dark = (0..height)
            .step_by(2)
            .flat_map(|r| (0..width).step_by(2).map(move |c| (r, c)))
            .filter(|(r, c)| {
                rgb[((best.2 + r) * WIDTH + best.1 + c) * 3..][..3]
                    .iter()
                    .all(|v| *v < 160)
            })
            .count();
        if dark * 2 < width.div_ceil(2) * height.div_ceil(2) {
            return None;
        }
    }
    (best.0 < 18.0).then_some((
        (best.1 + width / 2) as u32 * 2,
        (best.2 + height / 2) as u32 * 2,
    ))
}
pub(crate) fn map_open(rgb: &[u8]) -> bool {
    find(
        rgb,
        include_bytes!("../assets/map-close.rgb"),
        36,
        36,
        (1117, 5, 1134, 22),
        false,
    )
    .is_some()
        && zoom_button(rgb).is_some()
}
pub(crate) fn zoom_button(rgb: &[u8]) -> Option<(u32, u32)> {
    find(
        rgb,
        include_bytes!("../assets/map-zoom-in.rgb"),
        16,
        20,
        (64, 201, 77, 214),
        true,
    )
}
pub(crate) fn anchor_choice(rgb: &[u8]) -> Option<(u32, u32)> {
    find(
        rgb,
        include_bytes!("../assets/map-anchor-choice.rgb"),
        77,
        26,
        (780, 240, 1080, 470),
        true,
    )
}
pub(crate) fn teleport_button(rgb: &[u8]) -> Option<(u32, u32)> {
    find(
        rgb,
        include_bytes!("../assets/map-anchor-header.rgb"),
        79,
        25,
        (906, 2, 923, 19),
        false,
    )?;
    find(
        rgb,
        include_bytes!("../assets/map-teleport-button.rgb"),
        60,
        28,
        (1006, 487, 1023, 504),
        false,
    )
}
pub(crate) fn loading(rgb: &[u8]) -> bool {
    if rgb.len() != WIDTH * HEIGHT * 3 {
        return false;
    }
    let mut blank = 0;
    let mut total = 0;
    for y in (100..480).step_by(8) {
        for x in (200..1000).step_by(8) {
            let p = &rgb[(y * WIDTH + x) * 3..][..3];
            let max = *p.iter().max().unwrap();
            let min = *p.iter().min().unwrap();
            if p.iter().all(|v| *v < 25)
                || p.iter().all(|v| *v > 235)
                || (max < 65 && max - min < 12)
            {
                blank += 1;
            }
            total += 1;
        }
    }
    blank as f64 / total as f64 > 0.9
}
pub(crate) fn target(rgb: &[u8], goal: Position) -> Option<(u32, u32, usize, f64)> {
    if rgb.len() != WIDTH * HEIGHT * 3 {
        return None;
    }
    let (left, top, width, height) = (400, 80, 440, 380);
    let mut gray = Vec::with_capacity(width * height);
    for y in top..top + height {
        for x in left..left + width {
            let p = &rgb[(y * WIDTH + x) * 3..];
            gray.push((p[0] as f32 * 0.299 + p[1] as f32 * 0.587 + p[2] as f32 * 0.114) / 255.0);
        }
    }
    let image = GrayImage::new(width, height, gray).ok()?;
    let mut query = Sift::default().detect_and_compute(&image);
    query.sort_by(|a, b| b.keypoint.response.total_cmp(&a.keypoint.response));
    query.truncate(500);
    let atlas = atlas();
    let matches = match_features(&query, atlas, 0.75);
    let pairs: Vec<_> = matches
        .iter()
        .take(80)
        .map(|m| {
            let q = &query[m.query_index].keypoint;
            let t = &atlas[m.train_index].keypoint;
            ((q.x as f64, q.y as f64), (t.x as f64, t.y as f64))
        })
        .collect();
    let (fit, inliers, error) = match_geometry(&pairs, (220.0, 190.0), 0.5..12.0)?;
    if fit.rotation().abs() > 3.0 {
        return None;
    }
    let (x, y) = fit.inverse(640.0 - goal.x, -256.0 - goal.y);
    let (x, y) = (x + left as f64, y + top as f64);
    if !(200.0..1080.0).contains(&x) || !(80.0..510.0).contains(&y) {
        return None;
    }
    // Match the unlocked cyan/white marker near the projected game coordinate.
    let model = include_bytes!("../assets/map-waypoint.rgb");
    let mut best = (f64::MAX, 0, 0);
    for cy in y as i32 - 3..=y as i32 + 11 {
        for cx in x as i32 - 7..=x as i32 + 7 {
            let mut cost = 0.0;
            let mut n = 0;
            let mut blue = 0;
            let mut blue_matches = 0;
            for row in 0..34 {
                for col in 0..24 {
                    let m = &model[(row * 24 + col) * 3..][..3];
                    if (row >= 16 && m.iter().all(|v| *v > 210))
                        || (m[2] as i32 - m[0] as i32 > 30 && m[2] > 150 && m[1] > 90)
                    {
                        let at = (((cy + row as i32 - 17) as usize) * WIDTH
                            + (cx + col as i32 - 12) as usize)
                            * 3;
                        if m[2] as i32 - m[0] as i32 > 30 && m[2] > 150 && m[1] > 90 {
                            blue += 1;
                            if rgb[at + 2] as i32 - rgb[at] as i32 > 30
                                && rgb[at + 2] > 150
                                && rgb[at + 1] > 90
                            {
                                blue_matches += 1;
                            }
                        }
                        for c in 0..3 {
                            cost += (rgb[at + c] as f64 - m[c] as f64).abs();
                            n += 1;
                        }
                    }
                }
            }
            let cost = cost / n as f64;
            if cost < best.0 && blue_matches as f64 / blue as f64 >= 0.55 {
                best = (cost, cx, cy);
            }
        }
    }
    (best.0 < 40.0).then_some((best.1 as u32 * 2, (best.2 as u32 - 4) * 2, inliers, error))
}
