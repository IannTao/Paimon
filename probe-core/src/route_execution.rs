//! 原始路线字节驱动的任务入口：传送、地面导航和显式单点拾取验证。
//! 时间使用同一个单调毫秒时钟；输入回执只能证明输入结束，不能证明到点或取得物品。
use crate::{
    localization::{self, LocalizationTask, MiniMapFrame, Position},
    map_screen,
    movement::Conditions,
    navigation::{
        self, NavigationCommand, NavigationScope, NavigationState, NavigationTask, PointEvidence,
    },
    pickup_screen,
    route_import::{import_route, ImportStatus, Point},
};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionScope {
    /// 当前仅执行整条路线的已支持导航语义，不等于完成整条采集。
    WholeRoute,
    /// 保留整个原文件，但只验证首个传送点。
    FirstTeleport,
    /// 从原首点执行到第一个target，确认一个物品后交回下一原索引并结束验证。
    FirstPickup,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionPhase {
    Preparing,
    OpeningMap,
    SelectingAnchor,
    AnchorDetails,
    Loading,
    Navigation,
    PickupSearch,
    PickupConfirm,
    Finished,
}

#[derive(Debug, Serialize)]
pub struct ExecutionReport {
    pub source_sha256: String,
    pub scope: ExecutionScope,
    pub current_index: usize,
    pub last: usize,
    pub state: NavigationState,
    pub phase: ExecutionPhase,
    pub reason: String,
    pub original_route_passed: bool,
    pub teleport_validation_passed: bool,
    pub pickup_target_index: Option<usize>,
    pub pickup_validation_passed: bool,
    pub pickup_evidence: Option<PickupEvidence>,
    pub completed_points: Vec<PointEvidence>,
    pub command_count: u64,
    pub move_count: u64,
    pub captured_at: Option<u64>,
    pub position: Option<Position>,
    pub initial_position: Option<Position>,
    pub input_released: bool,
    pub pending_serial: Option<u64>,
    pub loading_observed: bool,
    pub map_target: Option<MapTargetEvidence>,
}

#[derive(Debug, Serialize)]
pub struct PickupEvidence {
    pub target_index: usize,
    pub original_id: serde_json_lenient::Value,
    pub item: String,
    pub quantity: u32,
    pub prompt_captured_at: u64,
    pub obtained_captured_at: u64,
    pub samples: usize,
}

#[derive(Debug, Serialize)]
pub struct MapTargetEvidence {
    pub x: u32,
    pub y: u32,
    pub inliers: usize,
    pub reprojection_error: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RouteCommand {
    Observe,
    Tap { serial: u64, x: u32, y: u32 },
    Turn { serial: u64, dx: i32 },
    Move { serial: u64, duration_ms: u32 },
    End,
}

/// 两种图像来自同一次截图：小地图保留264×264原像素，界面概览为1240×558 RGB。
/// 导航阶段可省略概览；地图交互和拾取阶段必须提供概览，不能拼接不同时间的画面。
pub struct RouteFrame<'a> {
    pub captured_at: u64,
    pub screen: Option<&'a [u8]>,
    pub minimap: &'a [u8],
}

struct Pending {
    serial: u64,
    created_at: u64,
    released_at: Option<u64>,
    ground_serial: Option<u64>,
}

pub struct RouteTask {
    report: ExecutionReport,
    localization: LocalizationTask,
    points: Vec<Point>,
    started_at: u64,
    last_now: u64,
    last_capture: u64,
    pending: Option<Pending>,
    samples: usize,
    map_target: Option<(u32, u32)>,
    bytes: Vec<u8>,
    zooms: usize,
    missing: usize,
    ground: Option<NavigationTask>,
    teleport_started_at: u64,
    search_turns: usize,
    prompt: Option<(u32, u32)>,
    prompt_at: u64,
    pickup_clicked_at: u64,
}

pub(crate) fn supported_teleport(point: &Point) -> bool {
    point.point_type == "teleport"
        && (point.x - 267.9441).hypot(point.y + 665.0128) <= 2.0
        && point.move_mode == "walk"
        && point.action.is_empty()
        && point
            .original
            .get("action_params")
            .is_none_or(|v| v.is_null() || v == "")
}

impl RouteTask {
    /// 在发出任何游戏输入前检查整份原文件，包括本轮验证范围之外的未知必需行为。
    /// 范围只控制何时结束，不重写字节、删除路径点或改变原坐标与动作参数。
    pub fn prepare(
        name: &str,
        bytes: &[u8],
        scope: ExecutionScope,
        now: u64,
    ) -> Result<Self, String> {
        let route = import_route(name, bytes);
        if route.status == ImportStatus::Invalid
            || !localization::supports_route(&route)
            || !navigation::within_limits(&route.points)
        {
            return Err("路线无效、地图/配置或导航范围未支持".into());
        }
        for p in &route.points {
            if p.point_type == "teleport" {
                if !supported_teleport(p) {
                    return Err(format!("第{}点的传送目标、动作或参数尚未支持", p.index));
                }
            } else if !["path", "target", "orientation"].contains(&p.point_type.as_str())
                || p.move_mode != "walk"
                || !p.action.is_empty()
                || p.original
                    .get("action_params")
                    .is_some_and(|v| !v.is_null() && v != "")
            {
                return Err(format!("第{}点的移动或动作尚未支持", p.index));
            }
        }
        if scope == ExecutionScope::FirstTeleport && route.points[0].point_type != "teleport" {
            return Err("首点不是teleport，不能作为首点传送验证".into());
        }
        // target只提供到达位置；是否存在资源以及是否取得，必须随后由真实画面证明。
        let pickup_target_index = if scope == ExecutionScope::FirstPickup {
            Some(
                route
                    .points
                    .iter()
                    .find(|p| p.point_type == "target")
                    .ok_or("路线没有可验证拾取的target点")?
                    .index,
            )
        } else {
            None
        };
        Ok(Self {
            localization: LocalizationTask::new(&route),
            started_at: now,
            last_now: now,
            last_capture: 0,
            pending: None,
            samples: 0,
            map_target: None,
            bytes: bytes.to_vec(),
            zooms: 0,
            missing: 0,
            ground: None,
            teleport_started_at: now,
            search_turns: 0,
            prompt: None,
            prompt_at: 0,
            pickup_clicked_at: 0,
            report: ExecutionReport {
                source_sha256: route.sha256,
                scope,
                current_index: 1,
                last: route.points.len(),
                state: NavigationState::Observing,
                phase: ExecutionPhase::Preparing,
                reason: "等待真实起始画面".into(),
                original_route_passed: false,
                teleport_validation_passed: false,
                pickup_target_index,
                pickup_validation_passed: false,
                pickup_evidence: None,
                completed_points: vec![],
                command_count: 0,
                move_count: 0,
                captured_at: None,
                position: None,
                initial_position: None,
                input_released: true,
                pending_serial: None,
                loading_observed: false,
                map_target: None,
            },
            points: route.points,
        })
    }
    pub fn report(&self) -> &ExecutionReport {
        &self.report
    }
    pub(crate) fn terminal(&self) -> bool {
        matches!(
            self.report.state,
            NavigationState::Completed | NavigationState::Failed | NavigationState::Stopped
        )
    }
    fn fail(&mut self, reason: &str) -> RouteCommand {
        self.report.state = NavigationState::Failed;
        self.report.reason = reason.into();
        self.report.position = None;
        self.report.captured_at = None;
        self.report.pending_serial = None;
        self.pending = None;
        RouteCommand::End
    }
    fn tap(&mut self, now: u64, x: u32, y: u32, phase: ExecutionPhase) -> RouteCommand {
        if self
            .report
            .captured_at
            .is_none_or(|at| now.saturating_sub(at) > 1500)
        {
            return self.fail("地图识别完成时画面已过期");
        }
        self.last_now = now;
        self.report.command_count += 1;
        let serial = self.report.command_count;
        self.report.input_released = false;
        self.report.pending_serial = Some(serial);
        self.pending = Some(Pending {
            serial,
            created_at: now,
            released_at: None,
            ground_serial: None,
        });
        self.report.state = NavigationState::Running;
        self.report.phase = phase;
        if phase == ExecutionPhase::OpeningMap {
            self.report.position = None;
        }
        self.samples = 0;
        RouteCommand::Tap { serial, x, y }
    }
    pub fn step(
        &mut self,
        now: u64,
        condition: Conditions,
        frame: Option<RouteFrame<'_>>,
    ) -> RouteCommand {
        if self.terminal() {
            return RouteCommand::End;
        }
        if condition != Conditions::Ready {
            let c = self.fail(match condition {
                Conditions::UserStopped => "已主动停止",
                Conditions::WrongForeground => "原神不在前台",
                Conditions::PermissionLost => "权限失效",
                Conditions::Locked => "屏幕锁定",
                Conditions::DisplayChanged => "显示配置改变",
                _ => "画面失效",
            });
            if condition == Conditions::UserStopped {
                self.report.state = NavigationState::Stopped;
            }
            return c;
        }
        if now < self.last_now
            || now.saturating_sub(self.started_at) > 180_000
            || (self.report.phase != ExecutionPhase::Navigation
                && now.saturating_sub(self.teleport_started_at) > 45_000)
        {
            return self.fail("传送时钟无效或超过45秒期限");
        }
        self.last_now = now;
        if matches!(
            self.report.phase,
            ExecutionPhase::PickupSearch | ExecutionPhase::PickupConfirm
        ) && now.saturating_sub(self.teleport_started_at) > 18_000
        {
            return self.fail("拾取搜索超过18秒，未确认取得物品");
        }
        if self.report.phase == ExecutionPhase::PickupConfirm
            && now.saturating_sub(self.pickup_clicked_at) > 5000
        {
            return self.fail("点击后5秒内未确认取得物品；提示消失不算成功");
        }
        if self.report.phase == ExecutionPhase::Preparing
            && now.saturating_sub(self.teleport_started_at) > 5000
        {
            return self.fail("起始可游玩画面定位超时");
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.released_at.is_none() && now.saturating_sub(p.created_at) > 1000)
        {
            return self.fail("传送输入回执超时");
        }
        let Some(frame) = frame else {
            return RouteCommand::Observe;
        };
        if frame.captured_at > now
            || now - frame.captured_at > 1500
            || frame.captured_at <= self.last_capture
        {
            return self.fail("传送画面过期、重复或时间无效");
        }
        // 回执成功后仍不能复用操作前的图像；下一决策必须基于释放之后的新截图。
        if self.pending.as_ref().is_some_and(|p| {
            p.released_at.is_none() || p.released_at.is_some_and(|at| frame.captured_at <= at)
        }) {
            return RouteCommand::Observe;
        }
        self.pending = None;
        self.report.pending_serial = None;
        self.last_capture = frame.captured_at;
        self.report.captured_at = Some(frame.captured_at);
        match self.report.phase {
            ExecutionPhase::Preparing => {
                if self.points[self.report.current_index - 1].point_type != "teleport" {
                    self.report.phase = ExecutionPhase::Navigation;
                    return self.ground_step(now, frame);
                }
                let observed = self.localization.observe(
                    now,
                    MiniMapFrame {
                        captured_at: frame.captured_at,
                        rgb: frame.minimap,
                    },
                );
                self.report.position = observed.position;
                if observed.position.is_none() {
                    self.samples = 0;
                    self.report.reason = observed.reason;
                    return RouteCommand::Observe;
                }
                self.samples += 1;
                if self.samples >= 2 {
                    if self.report.initial_position.is_none() {
                        self.report.initial_position = observed.position;
                    }
                    self.report.reason = "已确认可游玩起始位置，打开游戏地图".into();
                    return self.tap(now, 312, 160, ExecutionPhase::OpeningMap);
                }
            }
            ExecutionPhase::OpeningMap => {
                let Some(screen) = frame.screen else {
                    return self.fail("缺少地图界面画面");
                };
                if !map_screen::map_open(screen) {
                    self.report.reason = "等待已验证的地图界面".into();
                    return RouteCommand::Observe;
                }
                let goal = &self.points[self.report.current_index - 1];
                if let Some((x, y, inliers, error)) = map_screen::target(
                    screen,
                    Position {
                        x: goal.x,
                        y: goal.y,
                    },
                ) {
                    if self
                        .map_target
                        .is_some_and(|p| (p.0 as f64 - x as f64).hypot(p.1 as f64 - y as f64) < 6.0)
                    {
                        self.report.reason = "地图特征和可用锚点已确认，选择锚点".into();
                        self.report.map_target = Some(MapTargetEvidence {
                            x,
                            y,
                            inliers,
                            reprojection_error: error,
                        });
                        return self.tap(now, x, y, ExecutionPhase::SelectingAnchor);
                    }
                    self.map_target = Some((x, y));
                    self.report.reason = "等待第二帧地图目标稳定".into();
                } else {
                    self.map_target = None;
                    if self.zooms >= 4 {
                        return self.fail("地图或可用传送目标未确认，停止而不猜测锚点");
                    }
                    if let Some((x, y)) = map_screen::zoom_button(screen) {
                        self.zooms += 1;
                        self.report.reason = "目标未确认，在已识别地图中有限放大".into();
                        return self.tap(now, x, y, ExecutionPhase::OpeningMap);
                    }
                    return self.fail("地图缩放控件未确认");
                }
            }
            ExecutionPhase::SelectingAnchor | ExecutionPhase::AnchorDetails => {
                let Some(screen) = frame.screen else {
                    return self.fail("缺少锚点选择画面");
                };
                if let Some((x, y)) = map_screen::teleport_button(screen) {
                    self.report.reason = "已识别锚点详情和可用传送按钮，等待加载及重新定位".into();
                    self.localization =
                        LocalizationTask::new(&import_route("selected-route.json", &self.bytes));
                    self.report.position = None;
                    self.missing = 0;
                    return self.tap(now, x, y, ExecutionPhase::Loading);
                }
                if self.report.phase == ExecutionPhase::SelectingAnchor {
                    if let Some((x, y)) = map_screen::anchor_choice(screen) {
                        self.report.reason = "在重叠目标中选择已识别的传送锚点项".into();
                        return self.tap(now, x, y, ExecutionPhase::AnchorDetails);
                    }
                }
                self.missing += 1;
                self.report.reason = "等待可用传送锚点详情；未知或不可用目标不点击".into();
                if self.missing >= 3 {
                    return self.fail("未识别可用传送按钮或锚点，停止传送");
                }
            }
            ExecutionPhase::Loading => {
                let Some(screen) = frame.screen else {
                    return self.fail("缺少传送返回画面");
                };
                if map_screen::loading(screen) {
                    self.report.loading_observed = true;
                    self.samples = 0;
                    self.report.reason = "已观察到加载，等待可游玩画面和目标附近位置".into();
                    return RouteCommand::Observe;
                }
                if !self.report.loading_observed {
                    self.report.reason = "尚未观察到加载，不把点击成功算成传送成功".into();
                    return RouteCommand::Observe;
                }
                let observed = self.localization.observe(
                    now,
                    MiniMapFrame {
                        captured_at: frame.captured_at,
                        rgb: frame.minimap,
                    },
                );
                let Some(position) = observed.position else {
                    self.samples = 0;
                    self.report.reason = observed.reason;
                    return RouteCommand::Observe;
                };
                let point = &self.points[self.report.current_index - 1];
                let distance = (position.x - point.x).hypot(position.y - point.y);
                if distance > 8.0 {
                    self.missing += 1;
                    self.samples = 0;
                    if self.missing >= 3 {
                        return self.fail("加载后实际位置与原传送目标不符");
                    }
                    self.report.reason = "返回画面位置尚未匹配传送目标".into();
                    return RouteCommand::Observe;
                }
                self.samples += 1;
                self.report.position = Some(position);
                if self.samples >= 2 {
                    self.report.completed_points.push(PointEvidence {
                        index: point.index,
                        original_id: point.original["id"].clone(),
                        point_type: point.point_type.clone(),
                        position,
                        distance,
                        camera_error: None,
                        captured_at: frame.captured_at,
                        samples: self.samples,
                    });
                    self.report.teleport_validation_passed = true;
                    self.report.current_index += 1;
                    self.samples = 0;
                    if self.report.scope == ExecutionScope::FirstTeleport
                        || self.report.current_index > self.report.last
                    {
                        self.report.state = NavigationState::Completed;
                        self.report.phase = ExecutionPhase::Finished;
                        self.report.original_route_passed =
                            self.report.scope == ExecutionScope::WholeRoute;
                        self.report.reason =
                            if self.report.scope == ExecutionScope::FirstTeleport {
                                "首点传送验证完成，不代表整条原路线通过"
                            } else {
                                "路线完成"
                            }
                            .into();
                        return RouteCommand::End;
                    }
                    self.prepare_next_point(now);
                    self.report.reason = "传送已重新定位确认，下一帧交给后续路径点执行".into();
                }
            }
            ExecutionPhase::Navigation => return self.ground_step(now, frame),
            ExecutionPhase::PickupSearch => {
                return self.pickup_step(now, frame);
            }
            ExecutionPhase::PickupConfirm => return self.pickup_step(now, frame),
            ExecutionPhase::Finished => return self.fail("路线阶段已结束，不能继续输入"),
        }
        RouteCommand::Observe
    }
    pub fn input_finished(&mut self, serial: u64, now: u64, success: bool, released: bool) {
        // 终态不可被迟到回执重启；平台最终取消是否释放成功还会独立记录。
        if self.terminal() {
            return;
        }
        let Some(p) = self.pending.as_mut() else {
            return;
        };
        if p.serial != serial || p.released_at.is_some() {
            return;
        }
        if !success || !released || now < self.last_now || now.saturating_sub(p.created_at) > 1000 {
            self.fail("传送输入失败、超时或释放未确认");
            return;
        }
        p.released_at = Some(now);
        if let Some(local) = p.ground_serial {
            if let Some(ground) = self.ground.as_mut() {
                ground.input_finished(local, now, success, released);
                if ground.report().state == NavigationState::Failed {
                    self.fail("地面导航输入回执无效");
                    return;
                }
            }
        }
        self.report.input_released = true;
        self.last_now = now;
    }
    fn ground_step(&mut self, now: u64, frame: RouteFrame<'_>) -> RouteCommand {
        if self.ground.is_none() {
            let first = self.report.current_index;
            let mut last = self
                .points
                .iter()
                .skip(first - 1)
                .find(|p| p.point_type == "teleport")
                .map_or(self.points.len(), |p| p.index - 1);
            // 单点拾取复用同一原字节的导航分段，先停在原target处等待拾取证据。
            if let Some(target) = self.report.pickup_target_index {
                last = last.min(target);
            }
            match NavigationTask::prepare(
                "selected-route.json",
                &self.bytes,
                NavigationScope::Segment { first, last },
                now,
            ) {
                Ok(mut task) => {
                    // 普通导航的2.5单位容差不保证进入交互范围；拾取向同一原坐标靠近。
                    if self.report.scope == ExecutionScope::FirstPickup {
                        task.require_pickup_distance();
                    }
                    self.ground = Some(task);
                }
                Err(error) => return self.fail(&error),
            }
        }
        let ground = self.ground.as_mut().unwrap();
        let command = ground.step(
            now,
            Conditions::Ready,
            Some(MiniMapFrame {
                captured_at: frame.captured_at,
                rgb: frame.minimap,
            }),
        );
        let r = ground.report();
        self.report.current_index = r.current_index;
        self.report.reason = r.reason.clone();
        self.report.position = r.position;
        if self.report.initial_position.is_none() {
            self.report.initial_position = r.position;
        }
        self.report.input_released = r.input_released;
        self.report.captured_at = r.captured_at;
        for point in &r.completed_points {
            if !self
                .report
                .completed_points
                .iter()
                .any(|done| done.index == point.index)
            {
                self.report.completed_points.push(point.clone());
            }
        }
        if r.state == NavigationState::Failed {
            let reason = r.reason.clone();
            return self.fail(&reason);
        }
        if r.state == NavigationState::Completed {
            self.ground = None;
            if let Some(target) = self.report.pickup_target_index {
                if self.report.current_index == target + 1 {
                    self.report.current_index = target;
                    self.report.phase = ExecutionPhase::PickupSearch;
                    self.teleport_started_at = now;
                    self.samples = 0;
                    self.localization =
                        LocalizationTask::new(&import_route("selected-route.json", &self.bytes));
                    self.report.reason = "到达target，尚未取得物品".into();
                    return RouteCommand::Observe;
                }
            }
            if self.report.current_index > self.report.last {
                self.report.state = NavigationState::Completed;
                self.report.phase = ExecutionPhase::Finished;
                self.report.original_route_passed = true;
                self.report.reason = "原路线全部受支持点已按画面确认完成；未执行拾取".into();
                return RouteCommand::End;
            }
            self.prepare_next_point(now);
            return RouteCommand::Observe;
        }
        let (local, action) = match command {
            NavigationCommand::Turn { serial, dx } => {
                (serial, RouteCommand::Turn { serial: 0, dx })
            }
            NavigationCommand::Move {
                serial,
                duration_ms,
            } => (
                serial,
                RouteCommand::Move {
                    serial: 0,
                    duration_ms,
                },
            ),
            _ => return RouteCommand::Observe,
        };
        self.report.command_count += 1;
        let serial = self.report.command_count;
        self.report.pending_serial = Some(serial);
        self.report.state = NavigationState::Running;
        self.pending = Some(Pending {
            serial,
            created_at: now,
            released_at: None,
            ground_serial: Some(local),
        });
        match action {
            RouteCommand::Turn { dx, .. } => RouteCommand::Turn { serial, dx },
            RouteCommand::Move { duration_ms, .. } => {
                self.report.move_count += 1;
                RouteCommand::Move {
                    serial,
                    duration_ms,
                }
            }
            _ => unreachable!(),
        }
    }
    fn prepare_next_point(&mut self, now: u64) {
        self.ground = None;
        if self.points[self.report.current_index - 1].point_type != "teleport" {
            self.report.phase = ExecutionPhase::Navigation;
            return;
        }
        self.report.phase = ExecutionPhase::Preparing;
        self.teleport_started_at = now;
        self.zooms = 0;
        self.missing = 0;
        self.samples = 0;
        self.map_target = None;
        self.report.map_target = None;
        self.report.loading_observed = false;
        self.report.position = None;
        self.localization =
            LocalizationTask::new(&import_route("selected-route.json", &self.bytes));
    }

    fn pickup_step(&mut self, now: u64, frame: RouteFrame<'_>) -> RouteCommand {
        let Some(screen) = frame.screen else {
            return self.fail("缺少拾取界面画面");
        };
        let pose = self.localization.observe(
            now,
            MiniMapFrame {
                captured_at: frame.captured_at,
                rgb: frame.minimap,
            },
        );
        let Some(position) = pose.position else {
            // 不把旧位置配上新截图时间发布，也不让失去定位前的半次确认跨帧续用。
            self.report.position = None;
            self.report.captured_at = None;
            self.report.reason = pose.reason;
            self.samples = 0;
            self.prompt = None;
            self.missing += 1;
            if self.missing >= 3 {
                return self.fail("拾取期间无法确认当前位置");
            }
            return RouteCommand::Observe;
        };
        self.missing = 0;
        let point = &self.points[self.report.current_index - 1];
        if (position.x - point.x).hypot(position.y - point.y) > 4.0 {
            return self.fail("拾取期间偏离原target，停止输入");
        }
        self.report.position = Some(position);
        if self.report.phase == ExecutionPhase::PickupConfirm {
            // 搜索阶段已排除旧通知，且外层保证本帧晚于点击释放；这里只累计连续新证据。
            if pickup_screen::obtained(screen) {
                self.samples += 1;
                if self.samples >= 2 {
                    self.report.pickup_evidence = Some(PickupEvidence {
                        target_index: point.index,
                        original_id: point.original["id"].clone(),
                        item: "琉璃百合".into(),
                        quantity: 1,
                        prompt_captured_at: self.prompt_at,
                        obtained_captured_at: frame.captured_at,
                        samples: self.samples,
                    });
                    self.report.pickup_validation_passed = true;
                    // 到点证据与取得证据分别保留；单点验证只交回进度，不冒充整条通过。
                    self.report.current_index += 1;
                    self.report.state = NavigationState::Completed;
                    self.report.phase = ExecutionPhase::Finished;
                    self.report.reason =
                        "首个target取得单个物品已确认；下一点已交回路线进度，未验证整条采集".into();
                    return RouteCommand::End;
                }
            } else {
                self.samples = 0;
            }
            self.report.reason = "等待新出现的获得琉璃百合×1通知与第二帧确认".into();
            return RouteCommand::Observe;
        }
        if pickup_screen::obtained(screen) {
            // 进入范围时已有的获得通知无法归因于本次点击，先等待其消退。
            self.samples = 0;
            self.prompt = None;
            self.report.reason = "已有获得通知，等待消退，不能归因于本次拾取".into();
            return RouteCommand::Observe;
        }
        if let Some(target) = pickup_screen::prompt(screen) {
            // 两帧中图标和完整名称须在同一行稳定出现，避免淡入或列表跳动时误点。
            if self
                .prompt
                .is_some_and(|p| p.0.abs_diff(target.0) <= 6 && p.1.abs_diff(target.1) <= 6)
            {
                self.samples += 1;
            } else {
                self.samples = 1;
                self.prompt = Some(target);
            }
            self.report.reason = "已识别直接拾取图标及完整中文提示，等待新帧复核".into();
            if self.samples >= 2 {
                self.prompt_at = frame.captured_at;
                self.pickup_clicked_at = now;
                return self.tap(now, target.0, target.1, ExecutionPhase::PickupConfirm);
            }
            return RouteCommand::Observe;
        }
        self.samples = 0;
        self.prompt = None;
        if self.search_turns >= 6 {
            return self.fail("六次视角搜索仍无明确拾取提示，不点击未知交互");
        }
        self.search_turns += 1;
        self.report.command_count += 1;
        let serial = self.report.command_count;
        self.report.input_released = false;
        self.report.pending_serial = Some(serial);
        self.pending = Some(Pending {
            serial,
            created_at: now,
            released_at: None,
            ground_serial: None,
        });
        self.report.state = NavigationState::Running;
        self.report.reason = format!("视角搜索{}/6；尚无明确拾取提示", self.search_turns);
        // 视角搜索同样走单帧租约、串行回执及平台看门狗；每次拖动180ms后释放再观察。
        RouteCommand::Turn { serial, dx: 180 }
    }
}
