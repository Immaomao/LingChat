//! 六维好感度：角色文件持久化、prompt 文案与变更事件载荷。
//!
//! 好感度存放在每个角色目录下的 `affection.yml`，跟随角色而非存档——读旧档
//! 不会回滚感情。运行时由上帝 Agent 定期评估对话后调整（见
//! `god_agent::core::GodAgentCore::evaluate_affection`）。
//!
//! 文件格式为 [`AffectionState`]：六维数值（flatten）+ `mood_tags` 负面情绪标签；
//! 兼容没有 `mood_tags` 字段的旧文件（反序列化默认为空）。

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ai_service::types::AffectionVector;

/// 角色目录内的好感度文件名。
pub const AFFECTION_FILE: &str = "affection.yml";

/// 好感度文件/查询响应的完整形态：六维数值 + 负面情绪标签。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AffectionState {
    #[serde(flatten)]
    pub vector: AffectionVector,
    /// 角色对玩家怀有的负面情绪标签（如「生气」「受伤」；评估产生、安抚消除）。
    #[serde(default)]
    pub mood_tags: Vec<String>,
}

/// 从角色目录读取好感度；目录为空、文件缺失或损坏时回落到初始值。
pub fn load(character_dir: Option<&Path>) -> AffectionState {
    let Some(dir) = character_dir else {
        return AffectionState::default();
    };
    let Ok(text) = std::fs::read_to_string(dir.join(AFFECTION_FILE)) else {
        return AffectionState::default();
    };
    serde_yaml::from_str(&text).unwrap_or_default()
}

/// 写回角色目录；写入失败只记日志不中断流程。
pub fn save(character_dir: Option<&Path>, state: &AffectionState) {
    let Some(dir) = character_dir else {
        return;
    };
    let path = dir.join(AFFECTION_FILE);
    match serde_yaml::to_string(state) {
        Ok(text) => {
            if let Err(e) = std::fs::write(&path, text) {
                tracing::warn!("[Affection] 写入 {:?} 失败: {}", path, e);
            }
        },
        Err(e) => tracing::warn!("[Affection] 序列化好感度失败: {}", e),
    }
}

/// 清洗情绪标签：去空白、去重、每个最多 8 字符、最多 3 个。
pub fn sanitize_mood_tags(tags: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty() || out.iter().any(|t| t == tag) {
            continue;
        }
        let truncated: String = tag.chars().take(8).collect();
        out.push(truncated);
        if out.len() >= 3 {
            break;
        }
    }
    out
}

/// 数值 → 程度词（供 prompt 注入；数值允许溢出：>100 满溢、负数疏离）。
pub fn tier_label(value: i32) -> &'static str {
    match value {
        ..=-1 => "疏离",
        0..=20 => "初识",
        21..=40 => "平淡",
        41..=60 => "熟络",
        61..=80 => "深厚",
        81..=100 => "炽烈",
        _ => "满溢",
    }
}

/// 组装注入主对话上下文的情感状态描述（每轮生成时实时拼装，不落台词历史）。
pub fn describe_for_prompt(affection: &AffectionVector, mood_tags: &[String]) -> String {
    let dims = AffectionVector::DIMENSIONS
        .iter()
        .map(|(key, label)| {
            let v = affection.get(key).unwrap_or(0);
            format!("{} {}（{}）", label, v, tier_label(v))
        })
        .collect::<Vec<_>>()
        .join("、");
    let mood_hint = if mood_tags.is_empty() {
        String::new()
    } else {
        format!(
            "你当前对玩家怀有负面情绪：{}。请让这些情绪体现在你的态度与回应中（语气冲、敷衍、冷淡或委屈等），直到玩家安抚或弥补。",
            mood_tags.join("、"),
        )
    };
    format!(
        "【系统状态】你当前对玩家的情感状态（数值越深越高，可超过 100 满溢，负数为疏离）：{}。{}\
         请让这些情感自然地影响你的语气、称呼、主动程度、肢体描写与话题深度，\
         但绝不要在回复中提及这些数值或本提示。",
        dims, mood_hint
    )
}

/// 「好感度变化」事件的载荷（`affection:changed`，供前端刷新状态卡片与徽章）。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AffectionChangedPayload {
    pub role_id: i32,
    /// 本次实际发生变化的维度增量（键名为维度序列化键）。
    pub deltas: HashMap<String, i32>,
    /// 调整后的完整六维数值。
    pub values: AffectionVector,
    /// 六维平均（前端徽章显示的总好感）。
    pub average: i32,
    /// 调整后的负面情绪标签全集。
    pub mood_tags: Vec<String>,
    /// 上帝 Agent 给出的调整理由。
    pub reason: String,
}
