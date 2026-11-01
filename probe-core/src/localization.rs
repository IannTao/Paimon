//! Screen-derived task preparation, separate from navigation and input.
use crate::route_import::ImportReport;
use lowe_sift::{match_features, Feature, GrayImage, Sift};
use serde::Serialize;
use std::{sync::OnceLock, time::Instant};

pub const MINI_SIZE: usize = 264;

/// Capability availability is scoped to the calibrated surface reference, not all Teyvat.
pub fn supports_route(route: &ImportReport) -> bool {
    route.map_name == "Teyvat"
        && !route.points.is_empty()
        && route.original.as_object().is_some_and(|m| {
            m.keys()
                .all(|key| ["info", "positions", "config"].contains(&key.as_str()))
        })
        && route.original["info"].as_object().is_some_and(|m| {
            m.keys().all(|key| {
                [
                    "name",
                    "type",
                    "version",
                    "bgi_version",
                    "map_name",
                    "author",
                    "authors",
                    "description",
                ]
                .contains(&key.as_str())
            })
        })
        && route
            .original
            .get("config")
            .is_none_or(|v| v.is_null() || v.as_object().is_some_and(|m| m.is_empty()))
        && route.points.iter().all(|p| {
            (-324.0..580.0).contains(&p.x)
                && (-1220.0..-316.0).contains(&p.y)
                && p.original.as_object().is_some_and(|m| {
                    m.keys().all(|key| {
                        [
                            "id",
                            "x",
                            "y",
                            "type",
                            "move_mode",
                            "action",
                            "action_params",
                        ]
                        .contains(&key.as_str())
                    })
                })
        })
}

pub struct MiniMapFrame<'a> {
    pub captured_at: u64,
    pub rgb: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalizationState {
    Unlocated,
    Located,
}

#[derive(Clone, Debug, Serialize)]
pub struct LocalizationReport {
    pub state: LocalizationState,
    pub position: Option<Position>,
    pub target: Option<Position>,
    pub heading_degrees: Option<f64>,
    pub map_rotation_degrees: Option<f64>,
    pub navigation_allowed: bool,
    pub reason: String,
    pub inliers: usize,
    pub reprojection_error: Option<f64>,
    pub captured_at: Option<u64>,
    pub valid_until: Option<u64>,
}

pub struct LocalizationTask {
    target: Option<Position>,
    map_supported: bool,
    last_capture: u64,
    cached: Option<LocalizationReport>,
    previous_fix: Option<(u64, Position)>,
    jumped: bool,
}
impl LocalizationTask {
    pub fn new(route: &ImportReport) -> Self {
        if route.map_name == "Teyvat" {
            atlas();
        }
        Self {
            target: route.points.first().map(|p| Position { x: p.x, y: p.y }),
            map_supported: supports_route(route),
            last_capture: 0,
            cached: None,
            previous_fix: None,
            jumped: false,
        }
    }
    pub fn observe(&mut self, now: u64, frame: MiniMapFrame<'_>) -> LocalizationReport {
        let started = Instant::now();
        let mut report = if self.jumped {
            self.failure("坐标跳变，已终止可信定位；请重新开始定位准备")
        } else if !self.map_supported {
            self.failure("路线地图未支持或路线无效")
        } else if frame.captured_at > now
            || now - frame.captured_at > 1500
            || frame.captured_at <= self.last_capture
        {
            self.failure("画面过期、重复或时间无效")
        } else {
            self.last_capture = frame.captured_at;
            self.locate(&frame)
        };
        if now
            .saturating_add(started.elapsed().as_millis() as u64)
            .saturating_sub(frame.captured_at)
            > 1500
        {
            report = self.failure("定位计算完成时画面已过期");
        }
        if let Some(position) = report.position {
            if let Some((at, previous)) = self.previous_fix {
                let limit = 20.0 + frame.captured_at.saturating_sub(at) as f64 / 1000.0 * 15.0;
                if (position.x - previous.x).hypot(position.y - previous.y) > limit {
                    self.jumped = true;
                    report = self.failure("坐标跳变，已终止可信定位；请重新开始定位准备");
                }
            }
            if !self.jumped {
                self.previous_fix = Some((frame.captured_at, position));
            }
        }
        self.cached = Some(report.clone());
        report
    }
    pub fn snapshot(&mut self, now: u64) -> LocalizationReport {
        if now < self.last_capture || now.saturating_sub(self.last_capture) > 1500 {
            self.cached = Some(self.failure("画面已过期，不能导航"));
        }
        self.cached
            .clone()
            .unwrap_or_else(|| self.failure("等待真实游戏画面"))
    }
    pub fn invalidate(&mut self) -> LocalizationReport {
        let report = self.failure("画面不可用或定位准备已停止，不能导航");
        self.cached = Some(report.clone());
        report
    }
    fn failure(&self, reason: &str) -> LocalizationReport {
        LocalizationReport {
            state: LocalizationState::Unlocated,
            position: None,
            target: self.target,
            heading_degrees: None,
            map_rotation_degrees: None,
            navigation_allowed: false,
            reason: reason.into(),
            inliers: 0,
            reprojection_error: None,
            captured_at: None,
            valid_until: None,
        }
    }
    fn locate(&self, frame: &MiniMapFrame<'_>) -> LocalizationReport {
        let mut report = self.failure("没有可信地图匹配（仅支持璃月港地表参照范围）");
        if frame.rgb.len() != MINI_SIZE * MINI_SIZE * 3 {
            report.reason = "小地图尺寸无效".into();
            return report;
        }
        let image = GrayImage::new(
            MINI_SIZE,
            MINI_SIZE,
            frame
                .rgb
                .chunks_exact(3)
                .map(|p| (p[0] as f32 * 0.299 + p[1] as f32 * 0.587 + p[2] as f32 * 0.114) / 255.0)
                .collect(),
        )
        .unwrap();
        let mut query = Sift::default().detect_and_compute(&image);
        query.retain(|f| {
            let radius = (f.keypoint.x - 132.0).hypot(f.keypoint.y - 132.0);
            (28.0..110.0).contains(&radius)
        });
        query.sort_by(|a, b| b.keypoint.response.total_cmp(&a.keypoint.response));
        query.truncate(400);
        let atlas = atlas();
        let match_atlas = |features: &[Feature]| {
            let matches = match_features(&query, features, 0.75);
            let pairs: Vec<_> = matches
                .iter()
                .take(80)
                .map(|m| {
                    let q = &query[m.query_index].keypoint;
                    let t = &features[m.train_index].keypoint;
                    ((q.x as f64, q.y as f64), (t.x as f64, t.y as f64))
                })
                .collect();
            match_geometry(&pairs, (132.0, 132.0), 0.25..3.0)
        };
        let matched = match_atlas(atlas).or_else(|| {
            // A recent screen fix narrows descriptor candidates, never substitutes for a new match.
            let (at, previous) = self.previous_fix?;
            if frame.captured_at.saturating_sub(at) > 1500 {
                return None;
            }
            let (cx, cy) = (640.0 - previous.x, -256.0 - previous.y);
            let nearby: Vec<_> = atlas
                .iter()
                .filter(|f| (f.keypoint.x as f64 - cx).hypot(f.keypoint.y as f64 - cy) < 200.0)
                .cloned()
                .collect();
            match_atlas(&nearby)
        });
        let Some((fit, count, error)) = matched else {
            return report;
        };
        let (x, y) = fit.apply(132.0, 132.0);
        let Some(arrow) = player_arrow(frame.rgb) else {
            report.reason = "角色朝向箭头不清晰".into();
            return report;
        };
        report.state = LocalizationState::Located;
        report.position = Some(Position {
            x: 16384.0 - (15744.0 + x),
            y: 12288.0 - (12544.0 + y),
        });
        report.heading_degrees = arrow
            .heading_degrees
            .map(|heading| (heading + fit.b.atan2(fit.a).to_degrees()).rem_euclid(360.0));
        report.map_rotation_degrees = Some(fit.b.atan2(fit.a).to_degrees());
        report.inliers = count;
        report.reprojection_error = Some(error);
        report.captured_at = Some(frame.captured_at);
        report.valid_until = Some(frame.captured_at.saturating_add(1500));
        report.reason = "璃月港地表图像匹配通过；定位结果不直接授权输入".into();
        report
    }
}

pub(crate) fn atlas() -> &'static [Feature] {
    static FEATURES: OnceLock<Vec<Feature>> = OnceLock::new();
    FEATURES.get_or_init(|| {
        let bytes = include_bytes!("../assets/liyue-harbor.gray");
        let image = GrayImage::new(
            1024,
            1024,
            bytes.iter().map(|v| *v as f32 / 255.0).collect(),
        )
        .unwrap();
        Sift::default().detect_and_compute(&image)
    })
}

#[derive(Clone, Copy)]
pub(crate) struct Similarity {
    a: f64,
    b: f64,
    tx: f64,
    ty: f64,
}
impl Similarity {
    pub(crate) fn apply(self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x - self.b * y + self.tx,
            self.b * x + self.a * y + self.ty,
        )
    }
    pub(crate) fn inverse(self, x: f64, y: f64) -> (f64, f64) {
        let d = self.a * self.a + self.b * self.b;
        let (x, y) = (x - self.tx, y - self.ty);
        (
            (self.a * x + self.b * y) / d,
            (-self.b * x + self.a * y) / d,
        )
    }
    pub(crate) fn rotation(self) -> f64 {
        self.b.atan2(self.a).to_degrees()
    }
}
type Pair = ((f64, f64), (f64, f64));

pub(crate) fn match_geometry(
    pairs: &[Pair],
    center: (f64, f64),
    scales: std::ops::Range<f64>,
) -> Option<(Similarity, usize, f64)> {
    let mut candidates = Vec::new();
    for i in 0..pairs.len() {
        for j in i + 1..pairs.len() {
            let ((qx, qy), (tx, ty)) = pairs[i];
            let ((qx2, qy2), (tx2, ty2)) = pairs[j];
            let (dx, dy) = (qx2 - qx, qy2 - qy);
            let d = dx * dx + dy * dy;
            if d < 900.0 {
                continue;
            }
            let a = ((tx2 - tx) * dx + (ty2 - ty) * dy) / d;
            let b = ((ty2 - ty) * dx - (tx2 - tx) * dy) / d;
            if !scales.contains(&a.hypot(b)) {
                continue;
            }
            let fit = Similarity {
                a,
                b,
                tx: tx - a * qx + b * qy,
                ty: ty - b * qx - a * qy,
            };
            let (cx, cy) = fit.apply(center.0, center.1);
            if !(60.0..964.0).contains(&cx) || !(60.0..964.0).contains(&cy) {
                continue;
            }
            let mut error = 0.0;
            let count = pairs
                .iter()
                .filter(|&&((x, y), (u, v))| {
                    let (px, py) = fit.apply(x, y);
                    let e = (px - u).hypot(py - v);
                    if e < 3.0 {
                        error += e * e;
                        true
                    } else {
                        false
                    }
                })
                .count();
            if count >= 8 {
                candidates.push((fit, count, (error / count as f64).sqrt()));
            }
        }
    }
    candidates.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.total_cmp(&b.2)));
    let best = *candidates.first()?;
    let p = best.0.apply(center.0, center.1);
    if candidates.iter().any(|(fit, count, _)| {
        let q = fit.apply(center.0, center.1);
        *count as f64 >= best.1 as f64 * 0.8 && (p.0 - q.0).hypot(p.1 - q.1) > 15.0
    }) {
        return None;
    }
    // The two-point hypothesis selects consensus; fit every inlier to reduce fan-dependent jitter.
    let inliers: Vec<_> = pairs
        .iter()
        .copied()
        .filter(|&((x, y), (u, v))| {
            let (px, py) = best.0.apply(x, y);
            (px - u).hypot(py - v) < 3.0
        })
        .collect();
    let n = inliers.len() as f64;
    let (qx, qy, tx, ty) = inliers
        .iter()
        .fold((0.0, 0.0, 0.0, 0.0), |s, &((x, y), (u, v))| {
            (s.0 + x, s.1 + y, s.2 + u, s.3 + v)
        });
    let (qx, qy, tx, ty) = (qx / n, qy / n, tx / n, ty / n);
    let (a, b, denominator) = inliers
        .iter()
        .fold((0.0, 0.0, 0.0), |s, &((x, y), (u, v))| {
            let (x, y, u, v) = (x - qx, y - qy, u - tx, v - ty);
            (
                s.0 + x * u + y * v,
                s.1 + x * v - y * u,
                s.2 + x * x + y * y,
            )
        });
    let (a, b) = (a / denominator, b / denominator);
    let fit = Similarity {
        a,
        b,
        tx: tx - a * qx + b * qy,
        ty: ty - b * qx - a * qy,
    };
    let (cx, cy) = fit.apply(center.0, center.1);
    if !scales.contains(&fit.a.hypot(fit.b))
        || !(60.0..964.0).contains(&cx)
        || !(60.0..964.0).contains(&cy)
    {
        return None;
    }
    let error = (inliers
        .iter()
        .map(|&((x, y), (u, v))| {
            let (px, py) = fit.apply(x, y);
            (px - u).powi(2) + (py - v).powi(2)
        })
        .sum::<f64>()
        / n)
        .sqrt();
    Some((fit, inliers.len(), error))
}

struct PlayerArrow {
    heading_degrees: Option<f64>,
}

fn player_arrow(rgb: &[u8]) -> Option<PlayerArrow> {
    fn cyan(p: &[u8]) -> bool {
        p[0] < 35 && p[1] > 170 && p[2] > 230
    }
    let model = include_bytes!("../assets/mobile-player-arrow.rgb");
    let mut query = Vec::new();
    let mut reference = Vec::new();
    for y in 0..40 {
        for x in 0..40 {
            let q = &rgb[((y + 112) * MINI_SIZE + x + 112) * 3..][..3];
            let r = &model[(y * 40 + x) * 3..][..3];
            if cyan(q) {
                query.push((x as f64, y as f64, q));
            }
            if cyan(r) {
                reference.push((x as f64, y as f64));
            }
        }
    }
    if !(100..450).contains(&query.len()) {
        return None;
    }
    let qx = query.iter().map(|p| p.0).sum::<f64>() / query.len() as f64;
    let qy = query.iter().map(|p| p.1).sum::<f64>() / query.len() as f64;
    let rx = reference.iter().map(|p| p.0).sum::<f64>() / reference.len() as f64;
    let ry = reference.iter().map(|p| p.1).sum::<f64>() / reference.len() as f64;
    let mut candidates = Vec::new();
    for angle in (0..360).step_by(2) {
        let rotation = (angle as f64 - 95.0).to_radians();
        let (sin, cos) = rotation.sin_cos();
        let mut error = 0.0;
        for &(x, y, color) in &query {
            let dx = x - qx;
            let dy = y - qy;
            let mx = (rx + cos * dx + sin * dy).round() as i32;
            let my = (ry - sin * dx + cos * dy).round() as i32;
            if !(0..40).contains(&mx) || !(0..40).contains(&my) {
                error += 4.0;
                continue;
            }
            let r = &model[((my * 40 + mx) * 3) as usize..][..3];
            if !cyan(r) {
                error += 4.0;
                continue;
            }
            let chroma = |p: &[u8]| (p[1] as f64 - p[0] as f64) / (p[2] as f64 - p[0] as f64);
            error += (chroma(color) - chroma(r)).powi(2) * 25.0;
        }
        candidates.push((error / query.len() as f64, angle as f64));
    }
    candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
    let best = candidates[0];
    if best.0 > 0.8 {
        return None;
    }
    let ambiguous = candidates.iter().any(|&(score, angle)| {
        let delta = (angle - best.1).abs();
        delta.min(360.0 - delta) > 25.0 && score < best.0 + 0.05
    });
    // The cyan silhouette proves the player marker is present; its nearly triangular
    // shape can still leave facing ambiguous. Navigation separately observes the camera fan.
    Some(PlayerArrow {
        heading_degrees: (!ambiguous).then_some(best.1),
    })
}
