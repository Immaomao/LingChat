<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    class="heart-liquid"
    :style="glowStyle"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
  >
    <defs>
      <clipPath :id="clipId">
        <path :d="HEART_PATH" />
      </clipPath>
      <linearGradient :id="gradId" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0%" :stop-color="frontTopColor" />
        <stop offset="100%" :stop-color="frontBottomColor" />
      </linearGradient>
    </defs>

    <!-- 心形容器暗色底 -->
    <path :d="HEART_PATH" fill="rgba(255, 255, 255, 0.05)" />

    <!-- 液体：后波（浅色）+ 前波（渐变）双层，心形裁剪；随窗口移动倾斜晃动。
         负面情绪峰值高时由红渐变为灰（颜色在 frame 循环里平滑过渡） -->
    <g :clip-path="`url(#${clipId})`" :transform="`rotate(${tiltDeg} 12 12)`">
      <path :d="backWaveD" :fill="backWaveColor" />
      <path :d="frontWaveD" :fill="`url(#${gradId})`" />
    </g>

    <!-- 描边盖在液体之上：白边，满溢（>100）发光 -->
    <path
      :d="HEART_PATH"
      fill="none"
      stroke="rgba(255, 255, 255, 0.9)"
      stroke-width="1.8"
      stroke-linecap="round"
      stroke-linejoin="round"
    />
  </svg>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, useId, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";

const props = withDefaults(
  defineProps<{
    /** 好感平均值：null=无数据（空杯）；<0 冷色描边；>100 满溢发光（钳到满杯） */
    value: number | null;
    /** 负面情绪峰值（0~100+）：越高液体越灰（≤15 纯红、≥60 全灰，平滑过渡） */
    negative?: number | null;
    size?: number;
    /** 动态波浪开关（高级设置）；关闭后液面静止为平面，液位弹簧保留 */
    wave?: boolean;
  }>(),
  { size: 18, wave: true, negative: null },
);

// Lucide heart 轮廓（24x24）
const HEART_PATH =
  "M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z";

// 同页多颗心共存，clip/渐变 id 必须各自唯一
const uid = useId();
const clipId = `heart-clip-${uid}`;
const gradId = `heart-grad-${uid}`;

// ── 红→灰配色插值（负面情绪驱动） ──
type Rgb = [number, number, number];
const RED_TOP: Rgb = [255, 125, 156];
const RED_BOTTOM: Rgb = [224, 17, 60];
const GRAY_TOP: Rgb = [201, 201, 209];
const GRAY_BOTTOM: Rgb = [85, 85, 94];
const mixRgb = (a: Rgb, b: Rgb, t: number): string =>
  `rgb(${Math.round(a[0] + (b[0] - a[0]) * t)}, ${Math.round(a[1] + (b[1] - a[1]) * t)}, ${Math.round(
    a[2] + (b[2] - a[2]) * t,
  )})`;

/** 灰化程度 0（纯红）~1（全灰），frame 循环里向目标值平滑趋近 */
const grayMix = ref(0);
const frontTopColor = computed(() => mixRgb(RED_TOP, GRAY_TOP, grayMix.value));
const frontBottomColor = computed(() => mixRgb(RED_BOTTOM, GRAY_BOTTOM, grayMix.value));
const backWaveColor = computed(() => {
  const r = Math.round(255 + (170 - 255) * grayMix.value);
  const g = Math.round(122 + (170 - 122) * grayMix.value);
  const b = Math.round(152 + (180 - 152) * grayMix.value);
  return `rgba(${r}, ${g}, ${b}, 0.35)`;
});

const overflow = computed(() => (props.value ?? 0) > 100);
const glowStyle = computed(() => {
  if (!overflow.value) return {};
  const t = grayMix.value;
  const r = Math.round(255 + (150 - 255) * t);
  const g = Math.round(61 + (150 - 61) * t);
  const b = Math.round(113 + (160 - 113) * t);
  return { filter: `drop-shadow(0 0 4px rgba(${r}, ${g}, ${b}, 0.9))` };
});

/** 目标液位 0..1 */
const targetLevel = computed(() => {
  const v = props.value;
  if (v === null) return 0;
  return Math.min(1, Math.max(0, v / 100));
});

// ── 液体物理：液位弹簧阻尼（欠阻尼 → 过冲）+ 晃动能量注入/衰减 + 双层行波 ──
const frontWaveD = ref("");
const backWaveD = ref("");
/** 液面倾斜角（度）：窗口拖动时注入角速度，弹簧回正 */
const tiltDeg = ref(0);

let level = targetLevel.value;
let velocity = 0;
let slosh = 0; // 晃动能量 0..1：液位突变/窗口拖动注入，随时间指数衰减
let phase = 0;
let tilt = 0;
let tiltVel = 0;
let rafId: number | null = null;
let lastT = 0;
let unlistenMove: UnlistenFn | null = null;

const W = 24;

/** 正弦液面 + 向下闭合到底部外的填充路径（左右各外延 4px 防止晃动露边） */
function wavePath(surfaceY: number, amp: number, ph: number, waves: number): string {
  const points: string[] = [];
  for (let x = -4; x <= W + 4; x += 2) {
    const y = surfaceY + Math.sin((x / W) * Math.PI * 2 * waves + ph) * amp;
    points.push(`L ${x} ${y.toFixed(2)}`);
  }
  return `M -4 28 ${points.join(" ")} L ${W + 4} 28 Z`;
}

function frame(t: number) {
  const dt = Math.min(0.05, lastT ? (t - lastT) / 1000 : 0.016);
  lastT = t;

  // 弹簧阻尼积分：刚度/阻尼刻意欠阻尼，液面冲向目标位会轻微过冲再回稳
  const stiffness = 130;
  const damping = 9;
  const accel = stiffness * (targetLevel.value - level) - damping * velocity;
  velocity += accel * dt;
  level += velocity * dt;

  // 弹簧速度本身也带起晃动；相位速度随晃动能量加快（激烈时浪更急）
  slosh = Math.min(1, slosh + Math.abs(velocity) * dt * 1.5);
  slosh *= Math.exp(-2.4 * dt);
  if (props.wave) phase += dt * (2.4 + slosh * 5);

  // 倾斜角弹簧回正（欠阻尼 → 拖窗停下后液面左右摇两下再平）
  tiltVel += (-70 * tilt - 7 * tiltVel) * dt;
  tilt += tiltVel * dt;
  tiltDeg.value = props.wave ? Math.max(-14, Math.min(14, tilt)) : 0;

  // 灰化程度向目标平滑趋近（负面情绪 ≤15 纯红、≥60 全灰）
  const grayTarget = Math.min(1, Math.max(0, ((props.negative ?? 0) - 15) / 45));
  grayMix.value += (grayTarget - grayMix.value) * (1 - Math.exp(-3 * dt));

  // 液面映射到心形内部（心形内容区约 y=2~21.5）：满杯盖过顶部，空杯沉到心尖以下
  const surfaceY = 22.5 - level * 24.5;
  // 波浪关闭时液面为静止平面（振幅 0），液位弹簧照常工作
  const ampFront = props.wave ? 0.9 + slosh * 1.8 : 0;
  frontWaveD.value = wavePath(surfaceY, ampFront, phase, 1.5);
  backWaveD.value = wavePath(surfaceY + 0.6, ampFront * 0.75, phase * 0.8 + 1.9, 1.2);

  rafId = requestAnimationFrame(frame);
}

// 好感突变 → 注入晃动能量（升/降好感时液体「晃一下」）
watch(targetLevel, (next, prev) => {
  slosh = Math.min(1, slosh + Math.abs(next - prev) * 4 + 0.15);
});

onMounted(async () => {
  rafId = requestAnimationFrame(frame);

  // 拖动窗口 → 液体物理：位移距离注入晃动能量，水平速度注入倾斜角速度
  try {
    let lastPos: { x: number; y: number } | null = null;
    unlistenMove = await getCurrentWindow().onMoved(({ payload: pos }) => {
      if (lastPos) {
        const dx = pos.x - lastPos.x;
        const dy = pos.y - lastPos.y;
        slosh = Math.min(1, slosh + Math.hypot(dx, dy) / 260);
        tiltVel = Math.max(-40, Math.min(40, tiltVel + dx * 0.12));
      }
      lastPos = { x: pos.x, y: pos.y };
    });
  } catch {
    /* 非 Tauri 环境（纯 web 预览）无窗口事件，忽略 */
  }
});
onUnmounted(() => {
  if (rafId !== null) cancelAnimationFrame(rafId);
  unlistenMove?.();
});
</script>
