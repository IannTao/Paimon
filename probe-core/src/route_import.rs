//! Read-only preparation of original BetterGI routes. This module has no input backend.
use serde::{Deserialize, Serialize};
use serde_json_lenient::Value;
use sha2::{Digest, Sha256};

pub const MAX_BYTES: usize = 2 * 1024 * 1024;
pub const UPSTREAM_COMMIT: &str = "84c7e15e6096eb6c313b1b525ca7b38298971425";

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportStatus {
    Invalid,
    Unsupported,
    Executable,
}

#[derive(Debug, Serialize)]
pub struct Capability {
    pub field: String,
    pub recognized: bool,
    pub executable: bool,
}

#[derive(Debug, Serialize)]
pub struct Point {
    pub index: usize,
    pub x: f64,
    pub y: f64,
    pub point_type: String,
    pub move_mode: String,
    pub action: String,
    pub original: Value,
}

#[derive(Debug, Serialize)]
pub struct Diagnostic {
    pub field: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct ImportReport {
    pub status: ImportStatus,
    pub file_name: String,
    pub sha256: String,
    pub byte_count: usize,
    pub upstream_commit: Option<String>,
    pub name: String,
    pub map_name: String,
    pub points: Vec<Point>,
    pub capabilities: Vec<Capability>,
    pub diagnostics: Vec<Diagnostic>,
    pub original: Value,
}

/// Parses the original bytes and reports readiness; never starts a gathering task.
pub fn import_route(file_name: &str, bytes: &[u8]) -> ImportReport {
    let mut report = ImportReport {
        status: ImportStatus::Invalid,
        file_name: file_name.into(),
        sha256: Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
        byte_count: bytes.len(),
        upstream_commit: None,
        name: file_name.into(),
        map_name: "Teyvat".into(),
        points: vec![],
        capabilities: vec![],
        diagnostics: vec![],
        original: Value::Null,
    };
    if bytes.len() > MAX_BYTES {
        report.error("$", "文件超过 2 MiB");
        return report;
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let original: Value = match serde_json_lenient::from_slice::<UniqueValue>(bytes) {
        Ok(value) => value.0,
        Err(error) => {
            report.error("$", &error.to_string());
            return report;
        }
    };
    report.name = original["info"]["name"]
        .as_str()
        .unwrap_or(file_name)
        .into();
    report.original = original;
    if !report.original.is_object() || !report.original["info"].is_object() {
        report.error("info", "必须提供 info 对象");
        return report;
    }
    if let Some(config) = report.original.get("config") {
        if !config.is_null() && !config.is_object() {
            report.error("config", "必须为对象或 null");
            return report;
        }
    }
    let Some(points) = report.original["positions"].as_array() else {
        report.error("positions", "必须为路径点数组");
        return report;
    };
    if points.is_empty() || points.len() > 10000 {
        report.error("positions", "路径点数量必须为 1–10000");
        return report;
    }
    for (i, p) in points.iter().enumerate() {
        let mut problems = Vec::new();
        if !p.is_object() {
            problems.push((format!("positions[{i}]"), "必须为路径点对象"));
        }
        for key in ["x", "y"] {
            if !p[key].as_f64().is_some_and(f64::is_finite) {
                problems.push((format!("positions[{i}].{key}"), "坐标必须为有限数值"));
            }
        }
        for key in ["type", "move_mode", "action"] {
            if let Some(value) = p.get(key) {
                if !(value.is_string() || key == "action" && value.is_null()) {
                    problems.push((
                        format!("positions[{i}].{key}"),
                        "必须为字符串（action 可为 null）",
                    ));
                }
            }
        }
        for (field, reason) in problems {
            report.diagnostics.push(Diagnostic {
                field,
                reason: format!("第 {} 点：{reason}", i + 1),
            });
        }
    }
    if !report.diagnostics.is_empty() {
        return report;
    }
    if let Some(map) = report.original["info"].get("map_name") {
        if let Some(name) = map.as_str() {
            report.map_name = name.into();
        } else {
            report.error("info.map_name", "地图名称必须为字符串");
            return report;
        }
    }
    report.points = points
        .iter()
        .enumerate()
        .map(|(index, p)| Point {
            index: index + 1,
            x: p["x"].as_f64().unwrap_or(0.0),
            y: p["y"].as_f64().unwrap_or(0.0),
            point_type: p["type"].as_str().unwrap_or("path").into(),
            move_mode: p["move_mode"].as_str().unwrap_or("walk").into(),
            action: p["action"].as_str().unwrap_or("").into(),
            original: p.clone(),
        })
        .collect();
    let local_map = crate::localization::supports_route(&report);
    report.capabilities.push(Capability {
        field: "world_localization".into(),
        recognized: true,
        executable: local_map,
    });
    report.error(
        "world_localization",
        if local_map {
            "仅璃月港地表局部、2480×1116 固定布局已验证；启动时仍须确认实时位置与运行条件"
        } else {
            "路线不在已验证的璃月港地表定位范围或包含未解释的地图配置"
        },
    );
    report.capabilities.push(Capability {
        field: format!("map:{}", report.map_name),
        recognized: report.map_name == "Teyvat",
        executable: local_map,
    });
    if !local_map {
        report.error("info.map_name", "地图范围或配置未验证，不能执行");
    }
    let mut extensions = vec![];
    for key in report.original.as_object().unwrap().keys() {
        if !["info", "positions", "config"].contains(&key.as_str()) {
            extensions.push(key.clone());
        }
    }
    if let Some(config) = report.original["config"].as_object() {
        for key in config.keys() {
            extensions.push(format!("config.{key}"));
        }
    }
    for (i, p) in report.points.iter().enumerate() {
        for key in p.original.as_object().unwrap().keys() {
            if ![
                "id",
                "x",
                "y",
                "type",
                "move_mode",
                "action",
                "action_params",
            ]
            .contains(&key.as_str())
            {
                extensions.push(format!("positions[{i}].{key}"));
            }
        }
        if p.original
            .get("action_params")
            .is_some_and(|v| !v.is_null() && v != "")
            && p.action.is_empty()
        {
            extensions.push(format!("positions[{i}].action_params"));
        }
    }
    for field in extensions {
        report.capabilities.push(Capability {
            field: field.clone(),
            recognized: false,
            executable: false,
        });
        report.error(
            &field,
            "原始扩展/执行配置已保留，其行为尚未支持，整条路线不能执行",
        );
    }
    for i in 0..report.points.len() {
        let p = &report.points[i];
        let requirements = [
            (
                "type",
                p.point_type.clone(),
                ["path", "target", "teleport", "orientation"].contains(&p.point_type.as_str()),
            ),
            (
                "move_mode",
                p.move_mode.clone(),
                ["walk", "run", "dash", "climb", "fly", "jump", "swim"]
                    .contains(&p.move_mode.as_str()),
            ),
            ("action", p.action.clone(), known_action(&p.action)),
        ];
        for (key, value, recognized) in requirements {
            if key == "action" && value.is_empty() {
                continue;
            }
            let field = format!("positions[{i}].{key}");
            let executable = local_map
                && ((key == "type" && ["path", "target", "orientation"].contains(&value.as_str()))
                    || (key == "move_mode" && value == "walk"));
            report.capabilities.push(Capability {
                field: format!("{key}:{value}"),
                recognized,
                executable,
            });
            if !executable {
                report.error(
                    &field,
                    &format!(
                        "第 {} 点：{} {}",
                        i + 1,
                        value,
                        if recognized {
                            "已识别，尚不能执行"
                        } else {
                            "未知行为，不能执行"
                        }
                    ),
                );
            }
        }
    }
    report.capabilities.sort_by(|a, b| a.field.cmp(&b.field));
    report.capabilities.dedup_by(|a, b| a.field == b.field);
    if [
        "1646045d03cfb277e3651f341d0d876f9042b080e623149c217bb50517015dd2",
        "a2e2bb2ebc659f6013126320d52b53a1c68b4126b24c9e74e59270d36931e6fd",
    ]
    .contains(&report.sha256.as_str())
    {
        report.upstream_commit = Some(UPSTREAM_COMMIT.into());
    }
    report.status = if report.capabilities.iter().all(|c| c.executable) {
        ImportStatus::Executable
    } else {
        ImportStatus::Unsupported
    };
    report
}

// Reject duplicate object keys rather than silently replacing a required behavior.
struct UniqueValue(Value);
impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = UniqueValue;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("JSON value with unique object keys")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                serde_json_lenient::Number::from_f64(v)
                    .map(|n| UniqueValue(Value::Number(n)))
                    .ok_or_else(|| E::custom("non-finite number"))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(v) = seq.next_element::<UniqueValue>()? {
                    values.push(v.0);
                }
                Ok(UniqueValue(Value::Array(values)))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = serde_json_lenient::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(serde::de::Error::custom(format!("duplicate key: {key}")));
                    }
                    values.insert(key, map.next_value::<UniqueValue>()?.0);
                }
                Ok(UniqueValue(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

fn known_action(action: &str) -> bool {
    [
        "",
        "stop_flying",
        "force_tp",
        "nahida_collect",
        "pick_around",
        "hydro_collect",
        "electro_collect",
        "anemo_collect",
        "pyro_collect",
        "up_down_grab_leaf",
        "fight",
        "combat_script",
        "log_output",
        "fishing",
        "set_time",
        "use_gadget",
        "mining",
        "linnea_mining",
        "exit_and_relogin",
        "wonderland_cycle",
        "pick_up_collect",
    ]
    .contains(&action)
}

impl ImportReport {
    fn error(&mut self, field: &str, reason: &str) {
        self.diagnostics.push(Diagnostic {
            field: field.into(),
            reason: reason.into(),
        });
    }
}
