import { invoke } from "@tauri-apps/api/core";
import type { AffectionVector } from "@/stores/modules/game/state";

/** 角色情感状态：六维数值 + 负面情绪标签全集（对应 Rust AffectionState，flatten 序列化） */
export interface AffectionState extends AffectionVector {
  mood_tags: string[];
}

/** affection:changed 事件负载（snake_case，对应 Rust AffectionChangedPayload） */
export interface AffectionChangedPayload {
  role_id: number;
  /** 本次实际发生变化的维度增量（键名为维度序列化键） */
  deltas: Record<string, number>;
  /** 调整后的完整六维数值 */
  values: AffectionVector;
  /** 六维平均（徽章显示的总好感） */
  average: number;
  /** 上帝 Agent 给出的调整理由 */
  reason: string;
  /** 调整后的负面情绪标签全集（含未变化的；空数组 = 已安抚消除） */
  mood_tags: string[];
}

/**
 * 全量查询所有已加载角色的当前情感状态（role_id 字符串键 → 六维数值 + 情绪标签）。
 * 好感度变更由后端主动广播 affection:changed，本命令只用于初始化/兜底刷新。
 */
export const getAffection = async (): Promise<Record<string, AffectionState>> => {
  return invoke<Record<string, AffectionState>>("get_affection");
};
