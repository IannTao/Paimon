//! Original-file task entry. Progress requires fresh post-release screen evidence.
use crate::{
    camera_orientation,
    localization::{self, LocalizationTask, MiniMapFrame, Position},
    movement::Conditions,
    route_import::{import_route, ImportStatus, Point},
};
use serde::Serialize;
use serde_json_lenient::Value;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NavigationCommand {
    Observe,
    Turn { serial: u64, dx: i32 },
    Move { serial: u64, duration_ms: u32 },
    End,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NavigationScope {
    WholeRoute,
    Segment { first: usize, last: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigationState {
    Observing,
    Running,
    Completed,
    Stopped,
    Failed,
}

#[derive(Debug, Serialize)]
pub struct PointEvidence {
    pub index: usize,
    pub original_id: Value,
    pub point_type: String,
    pub position: Position,
    pub distance: f64,
    pub camera_error: Option<f64>,
    pub captured_at: u64,
    pub samples: usize,
}

#[derive(Debug, Serialize)]
pub struct NavigationReport {
    pub source_sha256: String,
    pub scope: NavigationScope,
    pub first: usize,
    pub last: usize,
    pub current_index: usize,
    pub state: NavigationState,
    pub reason: String,
    pub original_route_passed: bool,
    pub segment_passed: bool,
    pub completed_points: Vec<PointEvidence>,
    pub position: Option<Position>,
    pub character_heading_degrees: Option<f64>,
    pub camera_heading_degrees: Option<f64>,
    pub camera_confidence: Option<f64>,
    pub captured_at: Option<u64>,
    pub command_count: u64,
    pub move_count: u64,
    pub pending_serial: Option<u64>,
    pub input_released: bool,
}

struct Pending {
    serial: u64,
    action: PendingAction,
    created_at: u64,
    finished_at: Option<u64>,
    before: Vec<u8>,
    position: Position,
    camera: Option<f64>,
}
enum PendingAction {
    Turn(i32),
    Move { distance: f64 },
}

pub struct NavigationTask {
    report: NavigationReport,
    localization: LocalizationTask,
    points: Vec<Point>,
    last_frame: Vec<u8>,
    pending: Option<Pending>,
    observations: usize,
    settled: usize,
    previous_position: Option<Position>,
    camera_gain: Option<f64>,
    started_at: u64,
    last_now: u64,
    missing: usize,
    commands_this_point: u64,
    stalled_moves: usize,
    leg_start: Option<Position>,
}

impl NavigationTask {
    pub fn prepare(
        name: &str,
        bytes: &[u8],
        scope: NavigationScope,
        now: u64,
    ) -> Result<Self, String> {
        let route = import_route(name, bytes);
        if route.status == ImportStatus::Invalid {
            return Err("原文件格式无效".into());
        }
        let (first, last) = match scope {
            NavigationScope::WholeRoute => (1, route.points.len()),
            NavigationScope::Segment { first, last } => (first, last),
        };
        if first == 0 || last < first || last > route.points.len() {
            return Err("分段序号无效".into());
        }
        if !localization::supports_route(&route) {
            return Err("地图、区域或定位配置未验证".into());
        }
        for p in &route.points[first - 1..last] {
            if !["path", "target", "orientation"].contains(&p.point_type.as_str()) {
                return Err(format!("第{}点：{} 尚未支持", p.index, p.point_type));
            }
            if p.move_mode != "walk"
                || !p.action.is_empty()
                || p.original
                    .get("action_params")
                    .is_some_and(|v| !v.is_null() && v != "")
            {
                return Err(format!("第{}点的移动模式或动作未支持", p.index));
            }
        }
        let localization = LocalizationTask::new(&route);
        let report = NavigationReport {
            source_sha256: route.sha256,
            scope,
            first,
            last,
            current_index: first,
            state: NavigationState::Observing,
            reason: "等待起点画面".into(),
            original_route_passed: false,
            segment_passed: false,
            completed_points: vec![],
            position: None,
            character_heading_degrees: None,
            camera_heading_degrees: None,
            camera_confidence: None,
            captured_at: None,
            command_count: 0,
            move_count: 0,
            pending_serial: None,
            input_released: true,
        };
        Ok(Self {
            report,
            localization,
            points: route
                .points
                .into_iter()
                .skip(first - 1)
                .take(last - first + 1)
                .collect(),
            last_frame: vec![],
            pending: None,
            observations: 0,
            settled: 0,
            previous_position: None,
            camera_gain: None,
            started_at: now,
            last_now: now,
            missing: 0,
            commands_this_point: 0,
            stalled_moves: 0,
            leg_start: None,
        })
    }
    pub fn report(&self) -> &NavigationReport {
        &self.report
    }
    fn terminal(&self) -> bool {
        matches!(
            self.report.state,
            NavigationState::Completed | NavigationState::Stopped | NavigationState::Failed
        )
    }
    fn fail(&mut self, reason: &str) -> NavigationCommand {
        self.report.state = NavigationState::Failed;
        self.report.reason = reason.into();
        self.report.position = None;
        self.report.camera_heading_degrees = None;
        NavigationCommand::End
    }
    pub fn step(
        &mut self,
        now: u64,
        condition: Conditions,
        frame: Option<MiniMapFrame<'_>>,
    ) -> NavigationCommand {
        if self.terminal() {
            return NavigationCommand::End;
        }
        if condition != Conditions::Ready {
            let reason = match condition {
                Conditions::UserStopped => "已主动停止",
                Conditions::PermissionLost => "权限失效",
                Conditions::ScreenLost => "画面获取失效",
                Conditions::WrongForeground => "原神不在前台",
                Conditions::DisplayChanged => "屏幕配置改变",
                Conditions::Locked => "屏幕已锁定",
                Conditions::Ready => unreachable!(),
            };
            self.fail(reason);
            if condition == Conditions::UserStopped {
                self.report.state = NavigationState::Stopped;
            }
            return NavigationCommand::End;
        }
        if now < self.last_now {
            return self.fail("时钟顺序无效");
        }
        self.last_now = now;
        if now.saturating_sub(self.started_at) > 120_000 {
            return self.fail("导航任务达到120秒期限");
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.finished_at.is_none() && now.saturating_sub(p.created_at) > 1000)
        {
            return self.fail("输入回执超时，必须释放输入");
        }
        if self
            .report
            .captured_at
            .is_some_and(|at| now.saturating_sub(at) > 1500)
            && frame.as_ref().is_none_or(|incoming| {
                incoming.captured_at <= self.report.captured_at.unwrap()
                    || incoming.captured_at > now
                    || now.saturating_sub(incoming.captured_at) > 1500
            })
        {
            return self.fail("定位画面过期");
        }
        if self.observations == 0 && now.saturating_sub(self.started_at) > 5000 {
            return self.fail("起点定位超时");
        }
        let Some(frame) = frame else {
            return NavigationCommand::Observe;
        };
        if self.pending.as_ref().is_some_and(|p| {
            p.finished_at.is_none() || p.finished_at.is_some_and(|at| frame.captured_at <= at)
        }) {
            return NavigationCommand::Observe;
        }
        let rgb = frame.rgb;
        let captured_at = frame.captured_at;
        let observed = self.localization.observe(now, frame);
        self.report.position = observed.position;
        self.report.character_heading_degrees = observed.heading_degrees;
        let Some(position) = observed.position else {
            self.report.camera_heading_degrees = None;
            self.report.reason = observed.reason.clone();
            self.missing += 1;
            if self.missing >= 3
                || observed.reason.contains("跳变")
                || observed.reason.contains("时间")
                || observed.reason.contains("过期")
            {
                return self.fail("定位失效，停止导航");
            }
            return NavigationCommand::Observe;
        };
        self.missing = 0;
        if self.observations == 0
            && self.points[0].point_type != "orientation"
            && distance(
                position,
                Position {
                    x: self.points[0].x,
                    y: self.points[0].y,
                },
            ) > 30.0
        {
            return self.fail("当前位置不满足已确认的起点条件");
        }
        if observed
            .map_rotation_degrees
            .is_none_or(|angle| angle.abs() > 5.0)
        {
            return self.fail("小地图未锁定方向或布局发生改变");
        }
        self.observations += 1;
        if self.leg_start.is_none() {
            self.leg_start = Some(position);
        }
        self.report.captured_at = Some(captured_at);
        if let Some(pending) = self.pending.take() {
            self.report.pending_serial = None;
            match pending.action {
                PendingAction::Turn(dx) => {
                    if distance(position, pending.position) > 1.5 {
                        return self.fail("视角操作期间位置改变，不能继续校准");
                    }
                    if let Some((angle, change, confidence)) =
                        camera_orientation::estimate_turn(&pending.before, rgb)
                    {
                        let gain = change / dx as f64;
                        if !(0.03..2.0).contains(&gain.abs()) {
                            return self.fail("视角响应不在校验范围");
                        }
                        self.camera_gain = Some(gain);
                        self.report.camera_heading_degrees = Some(angle);
                        self.report.camera_confidence = Some(confidence);
                    } else {
                        self.report.camera_heading_degrees = None;
                    }
                }
                PendingAction::Move {
                    distance: before_distance,
                } => {
                    let goal = &self.points[self.report.current_index - self.report.first];
                    let remaining = distance(
                        position,
                        Position {
                            x: goal.x,
                            y: goal.y,
                        },
                    );
                    if before_distance - remaining < 0.5 {
                        self.stalled_moves += 1;
                    } else {
                        self.stalled_moves = 0;
                    }
                    if self.stalled_moves >= 4 {
                        return self.fail("移动无进展，停止导航");
                    }
                    if distance(position, pending.position) > 1.0
                        && pending.camera.is_some_and(|camera| {
                            difference(bearing(pending.position, position), camera).abs() > 45.0
                        })
                    {
                        return self.fail("实际移动方向与视角反馈不一致");
                    }
                    self.report.camera_heading_degrees = if distance(position, pending.position)
                        < 0.7
                        && camera_orientation::unchanged(&pending.before, rgb)
                    {
                        pending.camera
                    } else {
                        None
                    };
                }
            }
        } else if self.report.camera_heading_degrees.is_some()
            && !camera_orientation::unchanged(&self.last_frame, rgb)
        {
            self.report.camera_heading_degrees = None;
        }
        self.last_frame = rgb.to_vec();
        let offset = self.report.current_index - self.report.first;
        let point = &self.points[offset];
        let goal = Position {
            x: point.x,
            y: point.y,
        };
        let remaining = distance(position, goal);
        let orientation = point.point_type == "orientation";
        if !orientation && distance_to_leg(position, self.leg_start.unwrap(), goal) > 12.0 {
            return self.fail("当前位置偏离当前路线段");
        }
        if orientation && remaining < 2.0 {
            return self.fail("朝向点与当前位置过近，无法确认方位");
        }
        let desired = bearing(position, goal);
        let camera_error = self
            .report
            .camera_heading_degrees
            .map(|angle| difference(desired, angle));
        let within = if orientation {
            camera_error.is_some_and(|error| error.abs() <= 12.0)
        } else {
            remaining
                <= if point.point_type == "target" {
                    2.5
                } else {
                    5.0
                }
        };
        let stable = point.point_type != "target"
            || self
                .previous_position
                .is_none_or(|previous| distance(previous, position) <= 1.5);
        self.previous_position = Some(position);
        if within && stable {
            self.settled += 1;
        } else {
            self.settled = 0;
        }
        if self.settled >= 2 {
            self.report.completed_points.push(PointEvidence {
                index: point.index,
                original_id: point.original["id"].clone(),
                point_type: point.point_type.clone(),
                position,
                distance: remaining,
                camera_error: if orientation { camera_error } else { None },
                captured_at,
                samples: self.settled,
            });
            self.settled = 0;
            self.commands_this_point = 0;
            self.stalled_moves = 0;
            self.leg_start = Some(goal);
            self.report.current_index += 1;
            if self.report.current_index > self.report.last {
                self.report.state = NavigationState::Completed;
                self.report.original_route_passed =
                    matches!(self.report.scope, NavigationScope::WholeRoute);
                self.report.segment_passed =
                    matches!(self.report.scope, NavigationScope::Segment { .. });
                self.report.reason = if self.report.original_route_passed {
                    "地面路线已完成"
                } else {
                    "导航分段验证完成，不代表原路线通过"
                }
                .into();
                return NavigationCommand::End;
            }
            self.report.state = NavigationState::Running;
            self.report.reason = "当前点已按画面确认，等待下一点新证据".into();
            return NavigationCommand::Observe;
        }
        if within || self.observations < 2 {
            return NavigationCommand::Observe;
        }
        if self.report.camera_heading_degrees.is_none() {
            return self.turn(now, 160, position, rgb);
        }
        if camera_error.is_some_and(|error| error.abs() > 12.0) {
            let delta = camera_error.unwrap().clamp(-55.0, 55.0);
            let dx = (delta / self.camera_gain.unwrap())
                .round()
                .clamp(-240.0, 240.0) as i32;
            return self.turn(now, dx, position, rgb);
        }
        if !orientation {
            let tolerance = if point.point_type == "target" {
                2.5
            } else {
                5.0
            };
            let duration_ms = ((remaining - tolerance).max(0.0) * 45.0).clamp(120.0, 400.0) as u32;
            return self.advance(now, duration_ms, remaining, position, rgb);
        }
        NavigationCommand::Observe
    }
    fn turn(&mut self, now: u64, dx: i32, position: Position, rgb: &[u8]) -> NavigationCommand {
        if self.commands_this_point >= 40 {
            return self.fail("当前点操作次数达到上限");
        }
        self.commands_this_point += 1;
        self.report.command_count += 1;
        let serial = self.report.command_count;
        self.report.pending_serial = Some(serial);
        self.report.input_released = false;
        self.pending = Some(Pending {
            serial,
            action: PendingAction::Turn(dx),
            created_at: now,
            finished_at: None,
            before: rgb.to_vec(),
            position,
            camera: self.report.camera_heading_degrees,
        });
        self.report.reason = "等待视角操作释放和新画面确认".into();
        self.report.state = NavigationState::Running;
        NavigationCommand::Turn { serial, dx }
    }
    fn advance(
        &mut self,
        now: u64,
        duration_ms: u32,
        remaining: f64,
        position: Position,
        rgb: &[u8],
    ) -> NavigationCommand {
        if self.commands_this_point >= 40 {
            return self.fail("当前点操作次数达到上限");
        }
        self.commands_this_point += 1;
        self.report.command_count += 1;
        self.report.move_count += 1;
        let serial = self.report.command_count;
        self.report.pending_serial = Some(serial);
        self.report.input_released = false;
        self.report.state = NavigationState::Running;
        self.pending = Some(Pending {
            serial,
            action: PendingAction::Move {
                distance: remaining,
            },
            created_at: now,
            finished_at: None,
            before: rgb.to_vec(),
            position,
            camera: self.report.camera_heading_degrees,
        });
        self.report.reason = "短时移动中，必须释放后用新画面判断进度".into();
        self.report.camera_heading_degrees = None;
        NavigationCommand::Move {
            serial,
            duration_ms,
        }
    }
    pub fn input_finished(&mut self, serial: u64, now: u64, success: bool, released: bool) {
        if self.terminal() {
            return;
        }
        if let Some(pending) = self.pending.as_mut() {
            if pending.serial != serial || pending.finished_at.is_some() {
                return;
            }
            if !success || !released || now < pending.created_at {
                self.fail("输入失败、释放未确认或回执时间无效");
                return;
            }
            pending.finished_at = Some(now);
            self.report.input_released = true;
        }
    }
}

fn distance(a: Position, b: Position) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}
fn bearing(from: Position, to: Position) -> f64 {
    (-(to.x - from.x))
        .atan2(to.y - from.y)
        .to_degrees()
        .rem_euclid(360.0)
}
fn difference(to: f64, from: f64) -> f64 {
    (to - from + 180.0).rem_euclid(360.0) - 180.0
}
fn distance_to_leg(p: Position, a: Position, b: Position) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let length = dx * dx + dy * dy;
    if length < 0.01 {
        return distance(p, a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / length).clamp(0.0, 1.0);
    distance(
        p,
        Position {
            x: a.x + t * dx,
            y: a.y + t * dy,
        },
    )
}
