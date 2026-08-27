/**
 * 新手引导 Pinia Store（issue #717，移植自 old-python 分支）
 *
 * 管理教程生命周期：启动、导航、完成、持久化。
 *
 * ## 触发模式
 * 首次启动：localStorage 中无 `lingchat_onboarding_done` 标记 → 触发引导；
 * 完成后写入标记，后续不再弹出。
 *
 * ## 交互流程
 * 每个配置步骤的典型流程：
 *   提示 tooltip → 点击"下一步" → nextAction 打开面板 → isPaused 隐藏教程
 *   → 用户操作面板 → 关闭面板 → waitForField 检测到 → 推进到下一步
 *
 * 普通步骤（无 nextAction）：
 *   提示 → 点击"下一步" → 直接推进
 */
import { defineStore } from 'pinia'
import { watch } from 'vue'
import type { TutorialAction, TutorialState, TutorialStep } from '../../types/tutorial'
import { REGISTERED_PRESETS } from './tutorial-presets'
import { useUIStore } from './ui/ui'

/** localStorage 首次启动完成标记 */
const ONBOARDING_DONE_KEY = 'lingchat_onboarding_done'

/** 自动推进定时器（模块级，避免被 persist 序列化） */
let autoAdvanceTimer: ReturnType<typeof setTimeout> | null = null

export const useTutorialStore = defineStore('tutorial', {
  state: (): TutorialState => ({
    isActive: false,
    isCompleted: false,
    isPaused: false,
    currentPresetId: '',
    currentStepIndex: 0,
    tutorialMode: 'idle',
    firstFrameRendered: false,
  }),

  getters: {
    /** 当前步骤定义 */
    currentStep(state): TutorialStep | null {
      if (!state.isActive || !state.currentPresetId) return null
      const preset = REGISTERED_PRESETS[state.currentPresetId]
      if (!preset) return null
      return preset.steps[state.currentStepIndex] ?? null
    },

    /** 当前预设的总步数 */
    totalSteps(state): number {
      if (!state.currentPresetId) return 0
      const preset = REGISTERED_PRESETS[state.currentPresetId]
      return preset?.steps.length ?? 0
    },

    /** 是否为最后一步 */
    isLastStep(state): boolean {
      const preset = REGISTERED_PRESETS[state.currentPresetId]
      if (!preset) return false
      return state.currentStepIndex >= preset.steps.length - 1
    },

    /** 是否为第一步 */
    isFirstStep(state): boolean {
      return state.currentStepIndex <= 0
    },
  },

  actions: {
    /**
     * 检查是否为首次启动，决定是否展示引导。
     * 返回 true 表示应该展示（首次启动且未完成）。
     */
    checkFirstLaunch(): boolean {
      const done = localStorage.getItem(ONBOARDING_DONE_KEY) === '1'
      if (done) {
        this.tutorialMode = 'idle'
        return false
      }
      this.tutorialMode = 'first-launch'
      return true
    },

    /** 启动指定预设的教程 */
    startPreset(presetId: string, stepIndex = 0) {
      const preset = REGISTERED_PRESETS[presetId]
      if (!preset) {
        console.warn(`[Tutorial] 预设 "${presetId}" 不存在`)
        return
      }
      this.currentPresetId = presetId
      this.currentStepIndex = stepIndex
      this.isActive = true
      this.isPaused = false
      // 执行当前步骤的入口 action（如果有），并设置自动推进定时器
      this._executeCurrentAction()
    },

    /**
     * 下一步（由 TutorialOverlay 在点击按钮或自动推进时调用）
     */
    nextStep() {
      const preset = REGISTERED_PRESETS[this.currentPresetId]
      if (!preset) return
      this._clearAutoAdvance()
      if (this.currentStepIndex < preset.steps.length - 1) {
        this.currentStepIndex++
        this.isPaused = false
        this._executeCurrentAction()
      } else {
        this.complete()
      }
    },

    /** 上一步 */
    prevStep() {
      if (this.currentStepIndex <= 0) return
      this._clearAutoAdvance()
      this.currentStepIndex--
      this.isPaused = false
      this._executeCurrentAction()
    },

    /**
     * 执行当前步骤的 nextAction（由 TutorialOverlay 在点击"下一步"时调用）
     * 返回 true 表示有 nextAction 需要等待
     */
    executeNextAction(): boolean {
      const step = this.currentStep
      if (!step?.nextAction) return false
      this._executeAction(step.nextAction)
      // 如果有 waitForField，暂停教程遮罩
      if (step.waitForField) {
        this.isPaused = true
        return true
      }
      return false
    },

    /** 标记教程第一帧已渲染 */
    markFirstFrameRendered() {
      this.firstFrameRendered = true
    },

    /** 返回一个 Promise，在第一帧渲染完成后 resolve */
    waitForFirstFrame(): Promise<void> {
      if (this.firstFrameRendered) return Promise.resolve()
      return new Promise((resolve) => {
        const unwatch = watch(
          () => this.firstFrameRendered,
          (val) => {
            if (val) {
              unwatch()
              resolve()
            }
          },
        )
      })
    },

    /** 跳过整个教程 */
    skip() {
      this._clearAutoAdvance()
      this.complete()
    },

    /** 标记教程完成 */
    async complete() {
      const uiStore = useUIStore()
      if (uiStore.showSettings) {
        uiStore.toggleSettings(false)
      }

      this.isCompleted = true
      this.isActive = false
      this.isPaused = false

      // 写入首次启动完成标记
      localStorage.setItem(ONBOARDING_DONE_KEY, '1')

      this.currentPresetId = ''
      this.currentStepIndex = 0
    },

    /** 重置教程状态（开发调试用） */
    reset() {
      this._clearAutoAdvance()
      this.isActive = false
      this.isCompleted = false
      this.isPaused = false
      this.currentPresetId = ''
      this.currentStepIndex = 0
      this.tutorialMode = 'idle'
      this.firstFrameRendered = false
    },

    /** 清除首次启动标记（开发调试用） */
    clearDoneFlag() {
      localStorage.removeItem(ONBOARDING_DONE_KEY)
    },

    // ========== 内部方法 ==========

    /** 进入步骤时执行入口 action，并设置自动推进定时器 */
    _executeCurrentAction() {
      const step = this.currentStep
      if (step?.action && step.action.type !== 'none') {
        this._executeAction(step.action)
      }
      // 设置自动推进定时器（如 welcome/complete 步骤）
      if (step?.autoAdvanceMs && step.autoAdvanceMs > 0) {
        this._scheduleAutoAdvance(step.autoAdvanceMs)
      }
    },

    /** 执行指定 action（立即执行，无延迟） */
    _executeAction(action: TutorialAction) {
      const uiStore = useUIStore()
      const { type, payload } = action
      switch (type) {
        case 'openSettings':
          uiStore.toggleSettings(true)
          break
        case 'closeSettings':
          uiStore.toggleSettings(false)
          break
        case 'switchSettingsTab':
          if (payload?.tab) {
            uiStore.setSettingsTab(payload.tab as string)
            uiStore.toggleSettings(true)
          }
          break
        case 'openLlmConfig':
          // 打开设置面板的「高级」tab，并定位到 LLM 配置
          uiStore.toggleSettings(true)
          uiStore.setSettingsTab('advance')
          uiStore.advanceTab = 'llm'
          break
      }
    },

    _scheduleAutoAdvance(ms: number) {
      this._clearAutoAdvance()
      autoAdvanceTimer = setTimeout(() => {
        autoAdvanceTimer = null
        this.nextStep()
      }, ms)
    },

    _clearAutoAdvance() {
      if (autoAdvanceTimer) {
        clearTimeout(autoAdvanceTimer)
        autoAdvanceTimer = null
      }
    },
  },
})
