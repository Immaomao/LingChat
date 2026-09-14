import { invoke } from "@tauri-apps/api/core";
import type { AffectionVector } from "@/stores/modules/game/state";

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
}

/**
 * 全量查询所有已加载角色的当前好感度（role_id 字符串键 → 六维数值）。
 * 好感度变更由后端主动广播 affection:changed，本命令只用于初始化/兜底刷新。
 */
export const getAffection = async (): Promise<Record<string, AffectionVector>> => {
  return invoke<Record<string, AffectionVector>>("get_affection");
};
