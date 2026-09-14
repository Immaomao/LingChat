//! 上帝 Agent 核心：决策逻辑、prompt 构建、发言者选择。

use anyhow::{Result, anyhow};

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::god_agent::config::GodAgentConfig;
use crate::ai_service::god_agent::tools;
use crate::ai_service::llm::{LlmSlot, slot_snapshot};
use crate::ai_service::types::{AffectionVector, GameLine, LlmMessage};

// ============================================================
// NpcAffectionView
// ============================================================

/// 在场 NPC 的好感度评估输入视图（评估前从 GameStatus 快照，避免持锁跨 await）。
pub struct NpcAffectionView {
    pub role_id: i32,
    pub name: String,
    pub subtitle: String,
    pub info: String,
    pub current: AffectionVector,
}

// ============================================================
// GodAgentCore
// ============================================================

pub struct GodAgentCore {
    /// LLM 槽位（支持运行时热切换）。
    pub llm: LlmSlot,
    pub config: GodAgentConfig,
}

impl GodAgentCore {
    pub fn new(llm: LlmSlot, config: GodAgentConfig) -> Self {
        Self { llm, config }
    }

    // ============================================================
    // 激活判断
    // ============================================================

    /// 判断上帝 Agent 是否应在当前场景下激活。
    ///
    /// 条件：
    /// - 自由对话模式（`script_status.is_none()`）
    /// - 在场角色数 > 1（含玩家，即玩家 + 至少 1 个 NPC）
    pub fn should_activate(&self, gs: &GameStatus) -> bool {
        gs.script_status.is_none() && gs.present_role_ids.len() > 1
    }

    // ============================================================
    // Prompt 构建
    // ============================================================

    /// 构建上帝 Agent 的决策 prompt。
    ///
    /// 参考 `MemoryBuilder` 的格式化模式，将最近 N 条台词按角色分组呈现，
    /// 同时附上每个在场 NPC 的角色信息。
    fn build_decision_prompt(
        &self,
        lines: &[GameLine],
        npc_ids: &[i32],
        current_speaker: Option<i32>,
        gs: &GameStatus,
    ) -> Vec<LlmMessage> {
        // --- 角色信息 ---
        let mut role_info_block = String::from("【当前在场的非玩家角色列表】\n");
        for &rid in npc_ids {
            let name = gs
                .role_manager
                .get_loaded(rid)
                .and_then(|r| r.display_name.clone())
                .unwrap_or_else(|| format!("角色{}", rid));
            let subtitle = gs
                .role_manager
                .get_loaded(rid)
                .and_then(|r| r.settings.ai_subtitle.clone())
                .unwrap_or_default();
            let info = gs
                .role_manager
                .get_loaded(rid)
                .and_then(|r| r.settings.info.clone())
                .unwrap_or_default();
            role_info_block.push_str(&format!(
                "- role_id={}: {}\n  简介: {}\n  设定: {}\n",
                rid,
                name,
                if subtitle.is_empty() {
                    "无"
                } else {
                    &subtitle
                },
                if info.is_empty() { "无" } else { &info },
            ));
        }

        // --- 最近对话 ---
        let mut dialog_block = String::from("【最近对话记录（由旧到新）】\n");
        if lines.is_empty() {
            dialog_block.push_str("（无对话记录）\n");
        } else {
            for line in lines {
                let name = line.base.display_name.as_deref().unwrap_or("未知");
                let sid = line.base.sender_role_id.unwrap_or(-1);
                let emotion = line
                    .base
                    .original_emotion
                    .as_deref()
                    .filter(|v| !v.is_empty())
                    .map(|v| format!("【{}】", v))
                    .unwrap_or_default();
                let content = &line.base.content;
                dialog_block.push_str(&format!(
                    "[role_id={}] {}: {}{}\n",
                    sid, name, emotion, content
                ));
            }
        }

        // --- 当前发言者提示 ---
        let current_hint = match current_speaker {
            Some(0) => "当前发言者是「玩家」。请选择下一个发言的 NPC 角色。\n".to_string(),
            Some(rid) => {
                let name = gs
                    .role_manager
                    .get_loaded(rid)
                    .and_then(|r| r.display_name.clone())
                    .unwrap_or_else(|| format!("角色{}", rid));
                format!(
                    "当前发言者是「{}」(role_id={})，刚刚说完话。请判断：\n- 如果对话应该继续（比如另一个角色有强烈反应或话题未完），选择下一个发言的 NPC\n- 如果应该交还给玩家，选择 role_id=0\n",
                    name, rid
                )
            },
            None => String::new(),
        };

        let system_prompt = format!(
            "你是一个多人对话的导演（上帝视角）。你的任务是：根据当前场景中的角色列表和最近的对话历史，\
             判断下一个应该发言的角色。\n\
             \n\
             {}\n\
             {}\n\
             {}\n\
             请调用 select_next_speaker 工具来选择下一个发言者。",
            role_info_block, dialog_block, current_hint,
        );

        vec![LlmMessage::system(system_prompt)]
    }

    pub async fn decide_next_speaker(
        &self,
        gs: &GameStatus,
        current_speaker: Option<i32>,
    ) -> Result<(i32, String)> {
        let npc_ids: Vec<i32> = gs
            .present_role_ids
            .iter()
            .filter(|&&id| id != 0)
            .copied()
            .collect();

        if npc_ids.len() <= 1 {
            return Ok((npc_ids.first().copied().unwrap_or(0), "single_npc".into()));
        }

        let window = self.config.recent_window;
        let lines: Vec<GameLine> = gs
            .line_list
            .iter()
            .rev()
            .take(window)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        let messages = self.build_decision_prompt(&lines, &npc_ids, current_speaker, gs);

        let tools = vec![tools::select_next_speaker_tool()];
        let llm = slot_snapshot(&self.llm)
            .await
            .ok_or_else(|| anyhow!("上帝Agent LLM 未配置"))?;

        let response = llm
            .complete_with_tools(&messages, &tools, Some("auto"))
            .await
            .map_err(|e| anyhow!("LLM 调用失败: {}", e))?; // 增加具体错误

        // 更详细的错误信息
        if let Some(ref tool_calls) = response.tool_calls {
            if let Some(tc) = tool_calls.first() {
                if let Some(result) = tools::parse_speaker_selection(tc) {
                    if result.0 == 0 || gs.present_role_ids.contains(&result.0) {
                        return Ok(result);
                    }
                    tracing::warn!("上帝Agent 选择了不在场的角色 {}，忽略", result.0);
                    return Err(anyhow!(
                        "上帝Agent 选择了不在场的角色 {}，在场角色: {:?}",
                        result.0,
                        gs.present_role_ids
                    ));
                }
                // 解析失败
                return Err(anyhow!(
                    "解析 tool_call 失败: {:?}, available roles: {:?}",
                    tc,
                    npc_ids
                ));
            }
            // tool_calls 不为空但第一个元素不存在（理论上不可能）
            return Err(anyhow!("tool_calls 为空数组"));
        }

        // LLM 没有返回 tool_calls
        let content_info = response
            .content
            .as_ref()
            .map(|c| format!("，返回文本: {}", c))
            .unwrap_or_default();

        Err(anyhow!(
            "上帝Agent 未调用工具{}，可用角色: {:?}",
            content_info,
            npc_ids
        ))
    }

    // ============================================================
    // 好感度定期评估
    // ============================================================

    /// 构建好感度评估 prompt：在场角色当前情感状态 + 最近对话 + 评估原则。
    fn build_affection_prompt(
        &self,
        lines: &[GameLine],
        npcs: &[NpcAffectionView],
    ) -> Vec<LlmMessage> {
        let mut npc_block = String::from("【在场角色与当前情感状态】\n");
        for npc in npcs {
            let dims = AffectionVector::DIMENSIONS
                .iter()
                .map(|(key, label)| format!("{} {}", label, npc.current.get(key).unwrap_or(0)))
                .collect::<Vec<_>>()
                .join("、");
            npc_block.push_str(&format!(
                "- role_id={}: {}\n  简介: {}\n  设定: {}\n  当前情感（0~100）: {}\n",
                npc.role_id,
                npc.name,
                if npc.subtitle.is_empty() {
                    "无"
                } else {
                    &npc.subtitle
                },
                if npc.info.is_empty() { "无" } else { &npc.info },
                dims,
            ));
        }

        let mut dialog_block = String::from("【最近对话记录（由旧到新）】\n");
        if lines.is_empty() {
            dialog_block.push_str("（无对话记录）\n");
        } else {
            for line in lines {
                let name = line.base.display_name.as_deref().unwrap_or("未知");
                let content = &line.base.content;
                dialog_block.push_str(&format!("{}: {}\n", name, content));
            }
        }

        let system_prompt = format!(
            "你是一个情感观察员（上帝视角）。请阅读最近对话，评估这段对话对在场角色情感状态的影响。\n\
             \n\
             情感维度含义：\n\
             - 好感 fondness：整体喜欢程度，影响语气甜度\n\
             - 信赖 trust：愿意倾诉与说真心话的程度\n\
             - 亲密 intimacy：对肢体接触/近距离互动的接受度\n\
             - 默契 rapport：接梗、理解言外之意的程度\n\
             - 兴趣 interest：对玩家话题的好奇与主动程度\n\
             - 思念 longing：分别时的挂念强度（即将分别、久别时增加，重逢或相处愉快时回落）\n\
             \n\
             {}\n\
             {}\n\
             评估原则：日常正面互动 +1~+2，明显打动/冒犯 ±3，非常深刻或严重伤害 ±4~±5；\
             没有受到这段对话影响的维度不要调整；变化要符合角色性格，保守为主、宁少勿多。\n\
             请对每个有情感变化的在场角色调用一次 update_affection 工具。",
            npc_block, dialog_block,
        );

        vec![LlmMessage::system(system_prompt)]
    }

    /// 评估最近对话对在场 NPC 好感度的影响，返回解析后的调整列表（未应用）。
    ///
    /// 与 `decide_next_speaker` 独立：单 NPC 时也会真正调用 LLM。
    pub async fn evaluate_affection(
        &self,
        lines: &[GameLine],
        npcs: &[NpcAffectionView],
    ) -> Result<Vec<tools::AffectionAdjustment>> {
        if npcs.is_empty() {
            return Ok(Vec::new());
        }

        let messages = self.build_affection_prompt(lines, npcs);
        let tools = vec![tools::update_affection_tool()];
        let llm = slot_snapshot(&self.llm)
            .await
            .ok_or_else(|| anyhow!("上帝Agent LLM 未配置"))?;

        let response = llm
            .complete_with_tools(&messages, &tools, Some("auto"))
            .await
            .map_err(|e| anyhow!("LLM 调用失败: {}", e))?;

        let mut adjustments = Vec::new();
        let Some(tool_calls) = response.tool_calls else {
            tracing::warn!("上帝Agent 好感度评估未返回 tool_calls");
            return Ok(adjustments);
        };
        for tc in &tool_calls {
            if tc.function.name != "update_affection" {
                continue;
            }
            match tools::parse_affection_update(tc) {
                Some(adj) if npcs.iter().any(|n| n.role_id == adj.role_id) => {
                    adjustments.push(adj)
                },
                Some(adj) => {
                    tracing::warn!("上帝Agent 评估了不在场的角色 {}，忽略", adj.role_id)
                },
                None => tracing::warn!("解析 update_affection tool_call 失败: {:?}", tc),
            }
        }
        Ok(adjustments)
    }
}
