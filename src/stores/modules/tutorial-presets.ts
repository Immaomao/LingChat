/**
 * 新手引导预设注册表（issue #717，移植自 old-python 分支）
 *
 * 集中管理所有教程预设，方便扩展。
 * 调用 registerTutorialPreset() 即可注册新的教程流程。
 */
import type { TutorialPreset } from '../../types/tutorial'

/**
 * 已注册的教程预设（key = presetId）
 */
export const REGISTERED_PRESETS: Record<string, TutorialPreset> = {}

/**
 * 注册一个教程预设
 */
export function registerTutorialPreset(preset: TutorialPreset): void {
  REGISTERED_PRESETS[preset.id] = preset
}

// ============================================================
// 首次新手引导预设：「onboarding」
//
// 流程设计（用户要求简化版）：
//   欢迎 → 配置 LLM（打开设置面板 → 用户完成配置后关闭面板 → 结束回主界面）
//
// 关键字段：
//   - nextAction：点击"下一步"时触发的操作（不是进入步骤时）
//   - waitForField：隐藏教程后，等待某 store 字段变 false 才推进到下一步
// ============================================================

registerTutorialPreset({
  id: 'onboarding',
  name: '新手引导',
  steps: [
    {
      id: 'welcome',
      title: 'welcome',
      content: 'welcomeContent',
      tooltipPlacement: 'center',
      skippable: true,
      allowBack: false,
    },
    {
      id: 'llm-config-info',
      title: 'llmConfigTitle',
      content: 'llmConfigContent',
      tooltipPlacement: 'bottom',
      nextAction: { type: 'openLlmConfig' },
      waitForField: 'showSettings',
      skippable: true,
      allowBack: true,
    },
    {
      id: 'complete',
      title: 'complete',
      content: 'completeContent',
      tooltipPlacement: 'center',
      skippable: false,
      allowBack: true,
      autoAdvanceMs: 4000,
    },
  ],
})
