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
        <stop offset="0%" stop-color="#ff7d9c" />
        <stop offset="100%" stop-color="#e0113c" />
      </linearGradient>
    </defs>

    <!-- 心形容器暗色底 -->
    <path :d="HEART_PATH" fill="rgba(255, 255, 255, 0.05)" />

    <!-- 红色液体：后波（浅色）+ 前波（渐变）双层，心形裁剪 -->
    <g :clip-path="`url(#${clipId})`">
      <path :d="backWaveD" fill="rgba(255, 122, 152, 0.35)" />
      <path :d="frontWaveD" :fill="`url(#${gradId})`" />
    </g>

    <!-- 描边盖在液体之上：负好感冷色，满溢（>100）发光 -->
    <path
      :d="HEART_PATH"
      fill="none"
      :stroke="strokeColor"
      stroke-width="1.8"
      stroke-linecap="round"
      stroke-linejoin="round"
    />
  </svg>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, useId, watch } from "vue";

const props = withDefaults(
  defineProps<{
    /** 好感平均值：null=无数据（空杯）；<0 冷色描边；>100 满溢发光（钳到满杯） */
    value: number | null;
    size?: number;
  }>(),
  { size: 18 },
);

// Lucide heart 轮廓（24x24）
const HEART_PATH =
  "M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z";

// 同页多颗心共存，clip/渐变 id 必须各自唯一
const uid = useId();
const clipId = `heart-clip-${uid}`;
const gradId = `heart-grad-${uid}`;

const overflow = computed(() => (props.value ?? 0) > 100);
const strokeColor = computed(() => ((props.value ?? 0) < 0 ? "#7fc4ff" : "#ff8fc0"));
const glowStyle = computed(() =>
  overflow.value ? { filter: "drop-shadow(0 0 4px rgba(255, 61, 113, 0.9))" } : {},
);

/** 目标液位 0..1 */
const targetLevel = computed(() => {
  const v = props.value;
  if (v === null) return 0;
  return Math.min(1, Math.max(0, v / 100));
});

// ── 液体物理：液位弹簧阻尼（欠阻尼 → 过冲）+ 晃动能量注入/衰减 + 双层行波 ──
const frontWaveD = ref("");
const backWaveD = ref("");

let level = targetLevel.value;
let velocity = 0;
let slosh = 0; // 晃动能量 0..1：液位突变/弹簧速度注入，随时间指数衰减
let phase = 0;
let rafId: number | null = null;
let lastT = 0;

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
  phase += dt * (1.6 + slosh * 5);

  // 满杯时液面抬到心形顶上方（全满），空杯沉到底部外
  const surfaceY = 26 - level * 30;
  const ampFront = 0.45 + slosh * 1.7;
  frontWaveD.value = wavePath(surfaceY, ampFront, phase, 1.5);
  backWaveD.value = wavePath(surfaceY + 0.6, ampFront * 0.75, phase * 0.8 + 1.9, 1.2);

  rafId = requestAnimationFrame(frame);
}

// 好感突变 → 注入晃动能量（升/降好感时液体「晃一下」）
watch(targetLevel, (next, prev) => {
  slosh = Math.min(1, slosh + Math.abs(next - prev) * 4 + 0.15);
});

onMounted(() => {
  rafId = requestAnimationFrame(frame);
});
onUnmounted(() => {
  if (rafId !== null) cancelAnimationFrame(rafId);
});
</script>
