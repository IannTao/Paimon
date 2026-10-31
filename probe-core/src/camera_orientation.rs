//! Camera fan motion from two stationary, chronological screen observations.
//! Terrain cancels in the difference; character-arrow direction is never substituted.
pub(crate) fn estimate_turn(before: &[u8], after: &[u8]) -> Option<(f64, f64, f64)> {
    if before.len() != 264 * 264 * 3 || after.len() != before.len() {
        return None;
    }
    let mut delta = [0.0; 360];
    for (angle, value) in delta.iter_mut().enumerate() {
        let (sin, cos) = (angle as f64).to_radians().sin_cos();
        let mut changes = Vec::new();
        for radius in (40..101).step_by(2) {
            let x = (132.0 + radius as f64 * sin).round() as usize;
            let y = (132.0 - radius as f64 * cos).round() as usize;
            let at = (y * 264 + x) * 3;
            let change = (0..3)
                .map(|c| after[at + c] as f64 - before[at + c] as f64)
                .sum::<f64>()
                / 3.0;
            changes.push(change);
        }
        changes.sort_by(f64::total_cmp);
        *value = changes[6..25].iter().sum::<f64>() / 19.0;
    }
    let smooth: Vec<f64> = (0..360)
        .map(|a| (0..5).map(|d| delta[(a + d + 358) % 360]).sum::<f64>() / 5.0)
        .collect();
    fn runs(values: &[f64], sign: f64) -> Vec<(f64, f64, f64)> {
        let active: Vec<bool> = values.iter().map(|v| v * sign > 8.0).collect();
        let mut output = Vec::new();
        for start in 0..360 {
            if !active[start] || active[(start + 359) % 360] {
                continue;
            }
            let mut width = 0;
            while width < 360 && active[(start + width) % 360] {
                width += 1;
            }
            if (6..=85).contains(&width) {
                let strength = (0..width)
                    .map(|d| values[(start + d) % 360] * sign)
                    .sum::<f64>()
                    / width as f64;
                output.push((start as f64, width as f64, strength));
            }
        }
        output
    }
    let positive = runs(&smooth, 1.0);
    let negative = runs(&smooth, -1.0);
    let mut candidates = Vec::new();
    for &(ps, pw, strength_p) in &positive {
        for &(ns, nw, strength_n) in &negative {
            let separation = ((ps + pw / 2.0) - (ns + nw / 2.0) + 180.0).rem_euclid(360.0) - 180.0;
            if (separation.abs() - 90.0).abs() > 12.0 || (pw - nw).abs() > 12.0 {
                continue;
            }
            let direction = separation.signum();
            let width = (pw + nw) / 2.0;
            let after_angle = if direction > 0.0 {
                ps + pw - 45.0
            } else {
                ps + 45.0
            };
            let score = strength_p.min(strength_n);
            candidates.push((score, after_angle.rem_euclid(360.0), direction * width));
        }
    }
    candidates.sort_by(|a, b| b.0.total_cmp(&a.0));
    let best = *candidates.first()?;
    if best.0 < 12.0
        || candidates
            .get(1)
            .is_some_and(|other| other.0 > best.0 * 0.8)
    {
        return None;
    }
    Some((best.1, best.2, (best.0 / 30.0).min(1.0)))
}

pub(crate) fn unchanged(before: &[u8], after: &[u8]) -> bool {
    if before.len() != 264 * 264 * 3 || after.len() != before.len() {
        return false;
    }
    let mut changed = 0;
    for angle in (0..360).step_by(3) {
        let (sin, cos) = (angle as f64).to_radians().sin_cos();
        let mut changes = Vec::new();
        for radius in (40..101).step_by(2) {
            let x = (132.0 + radius as f64 * sin).round() as usize;
            let y = (132.0 - radius as f64 * cos).round() as usize;
            let at = (y * 264 + x) * 3;
            changes.push(
                (0..3)
                    .map(|c| after[at + c] as f64 - before[at + c] as f64)
                    .sum::<f64>()
                    / 3.0,
            );
        }
        changes.sort_by(f64::total_cmp);
        if (changes[6..25].iter().sum::<f64>() / 19.0).abs() > 6.0 {
            changed += 1;
        }
    }
    changed <= 4
}
