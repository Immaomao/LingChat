<template>
  <div class="flex flex-col gap-3">
    <Button
      type="nav"
      :class="[
        'flex items-center gap-2 px-4 py-2 transition-colors',
        enabled ? 'text-[#ff8fc0]' : 'text-white',
      ]"
      @click="toggleEnabled"
      v-show="!uiStore.showSettings"
    >
      <Heart :size="18" />
      <h3 class="m-0 hidden text-lg font-bold xl:block">
        {{ $t("ui.affection.title") }}
        <span v-if="average !== null" class="ml-1 text-sm font-normal tabular-nums opacity-80">
          {{ average }}
        </span>
      </h3>
    </Button>

    <!-- 日程式弹窗：全屏遮罩 + 居中窗口 -->
    <Teleport to="body">
      <Transition
        enter-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
        leave-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
        enter-from-class="opacity-0"
        leave-to-class="opacity-0"
      >
        <div
          v-if="enabled"
          class="fixed inset-0 z-[1100] flex items-center justify-center bg-black/50 backdrop-blur-sm"
          @click.self="close"
        >
          <Transition
            enter-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
            leave-active-class="transition-all duration-200 cubic-bezier(0.6, -0.28, 0.74, 0.05)"
            enter-from-class="opacity-0 scale-95 translate-y-2"
            leave-to-class="opacity-0 scale-95 translate-y-2"
          >
            <div
              v-if="enabled"
              class="relative flex max-h-[85dvh] flex-col overflow-hidden rounded-3xl border border-white/10 shadow-[0_8px_32px_rgba(0,0,0,0.4)]"
              :class="uiStore.isNarrowScreen ? 'w-[95vw]' : 'w-130'"
            >
              <!-- Header bar -->
              <div
                class="flex shrink-0 items-center justify-between border-b border-white/10 bg-[#12121c]/90 px-5 py-3 backdrop-blur-xl"
              >
                <div class="flex items-center gap-2">
                  <Heart :size="20" class="text-[#ff8fc0]" />
                  <h3 class="text-base font-semibold text-white">
                    {{ $t("ui.affection.title") }}
                  </h3>
                </div>
                <button
                  class="rounded-full p-2 text-white/50 transition-colors hover:bg-white/10 hover:text-white"
                  @click="close"
                >
                  <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M6 18L18 6M6 6l12 12"
                    />
                  </svg>
                </button>
              </div>

              <!-- Content area with glass styling -->
              <div
                class="min-h-0 flex-1 scrollbar-thin [scrollbar-color:var(--accent-color)_transparent] overflow-y-auto rounded-b-3xl bg-[#12121c]/75 p-5 text-white backdrop-blur-[20px]"
              >
                <!-- 只显示当前对话角色；切换角色时旧内容淡出、新内容淡入 -->
                <Transition name="affection-role" mode="out-in">
                  <div :key="role?.roleId ?? 'none'">
                    <template v-if="affection">
                      <!-- 角色头部：头像 + 名字 + 平均值大数字 + 档位徽章 -->
                      <div class="flex items-center gap-3">
                        <img
                          v-if="avatarUrl"
                          :src="avatarUrl"
                          :alt="role?.roleName ?? ''"
                          class="h-12 w-12 shrink-0 rounded-full border border-white/15 object-cover"
                        />
                        <div
                          v-else
                          class="h-12 w-12 shrink-0 rounded-full border border-white/10 bg-white/5"
                        ></div>
                        <div class="min-w-0 flex-1">
                          <div class="truncate text-lg font-bold">
                            {{ role?.roleName ?? "—" }}
                          </div>
                          <div class="text-xs text-white/50">
                            {{ $t("ui.affection.average") }}
                          </div>
                        </div>
                        <div class="flex shrink-0 flex-col items-end gap-1.5">
                          <span
                            class="text-3xl leading-none font-bold tabular-nums"
                            :style="{ color: tierColor }"
                          >
                            {{ average }}
                          </span>
                          <span
                            class="affection-tier-badge"
                            :class="{ 'affection-tier-pulse': tier === 'overflow' }"
                            :style="tierBadgeStyle"
                          >
                            {{ $t(`ui.affection.tier.${tier}`) }}
                          </span>
                        </div>
                      </div>

                      <!-- 距下一档进度 -->
                      <div
                        v-if="nextTierInfo"
                        class="mt-2 flex items-center justify-end gap-2 text-xs text-white/50"
                      >
                        <div class="h-1 w-20 overflow-hidden rounded-full bg-white/10">
                          <div
                            class="h-full rounded-full transition-[width] duration-700"
                            :style="{ width: `${nextTierInfo.progress}%`, background: tierColor }"
                          ></div>
                        </div>
                        <span>
                          {{
                            $t("ui.affection.nextTier", {
                              tier: $t(`ui.affection.tier.${nextTierInfo.key}`),
                              points: nextTierInfo.points,
                            })
                          }}
                        </span>
                      </div>
                      <div
                        v-else-if="tier === 'overflow'"
                        class="mt-2 text-right text-xs"
                        :style="{ color: tierColor }"
                      >
                        {{ $t("ui.affection.maxTier") }}
                      </div>

                      <!-- 六维雷达图 -->
                      <svg class="mt-1 block w-full" viewBox="0 0 400 360">
                        <defs>
                          <linearGradient
                            id="affection-gradient"
                            x1="0%"
                            y1="0%"
                            x2="100%"
                            y2="100%"
                          >
                            <stop offset="0%" stop-color="#ff5c8a" />
                            <stop offset="100%" stop-color="#ff9ec7" />
                          </linearGradient>
                        </defs>

                        <!-- 参考环（25/50/75 虚线 + 100 满刻度淡色实线） -->
                        <polygon
                          v-for="ring in [0.25, 0.5, 0.75]"
                          :key="ring"
                          class="fill-none stroke-white/10"
                          stroke-dasharray="4 4"
                          :points="ringPoints(ring)"
                        />
                        <polygon class="fill-none stroke-white/15" :points="ringPoints(1)" />
                        <!-- 轴线 -->
                        <line
                          v-for="i in 6"
                          :key="`axis-${i}`"
                          class="stroke-white/10"
                          :x1="CX"
                          :y1="CY"
                          :x2="pointAt(i - 1, R).x"
                          :y2="pointAt(i - 1, R).y"
                        />

                        <!-- 数据多边形 -->
                        <polygon
                          class="affection-data-polygon"
                          :points="dataPolygonPoints"
                          fill="url(#affection-gradient)"
                        />

                        <!-- 顶点圆点（>100 满溢：外层脉冲高亮光晕；<0 疏离：冷色） -->
                        <g v-for="(p, i) in dataPoints" :key="`vertex-${i}`">
                          <circle
                            v-if="displayValues[i] > 100"
                            :cx="p.x"
                            :cy="p.y"
                            r="6"
                            class="affection-vertex-halo"
                          />
                          <circle
                            :cx="p.x"
                            :cy="p.y"
                            r="3.5"
                            :class="vertexFillClass(displayValues[i])"
                          />
                          <!-- 加宽 hover 热区 -->
                          <circle
                            :cx="p.x"
                            :cy="p.y"
                            r="12"
                            class="cursor-pointer fill-transparent"
                            @mouseenter="hoveredDim = dimensions[i].key"
                            @mouseleave="hoveredDim = null"
                          />
                        </g>

                        <!-- 轴端标注：维度名 + 真实数值（可溢出/为负） -->
                        <text
                          v-for="(label, i) in axisLabels"
                          :key="`label-${i}`"
                          :x="label.x"
                          :y="label.y"
                          :text-anchor="label.anchor"
                          class="cursor-default fill-white/60 text-[11px]"
                          @mouseenter="hoveredDim = dimensions[i].key"
                          @mouseleave="hoveredDim = null"
                        >
                          {{ $t(`ui.affection.${dimensions[i].key}`) }}
                          <tspan
                            :x="label.x"
                            dy="14"
                            class="text-[12px] font-bold"
                            :class="valueFillClass(displayValues[i])"
                            :style="valueGlowStyle(displayValues[i])"
                          >
                            {{ Math.round(displayValues[i]) }}
                          </tspan>
                        </text>
                      </svg>

                      <!-- hover 顶点/标注时显示维度说明 -->
                      <div class="h-4 text-center text-xs text-white/50">
                        <template v-if="hoveredDim">
                          {{ $t(`ui.affection.${hoveredDim}`) }}：{{
                            $t(`ui.affection.dimDesc.${hoveredDim}`)
                          }}
                        </template>
                      </div>

                      <!-- 最近一次评估变化 -->
                      <div
                        v-if="recentChange"
                        class="mt-3 rounded-2xl border border-white/10 bg-white/5 p-3"
                      >
                        <div class="mb-1.5 text-xs font-semibold text-white/60">
                          {{ $t("ui.affection.recentChange") }}
                        </div>
                        <div class="flex flex-wrap gap-1.5">
                          <span
                            v-for="entry in recentChange.entries"
                            :key="entry.key"
                            class="affection-delta-chip"
                            :class="entry.delta > 0 ? 'affection-delta-up' : 'affection-delta-down'"
                          >
                            {{ $t(`ui.affection.${entry.key}`) }}
                            {{ entry.delta > 0 ? "+" : "" }}{{ entry.delta }}
                          </span>
                        </div>
                        <div
                          v-if="recentChange.reason"
                          class="mt-1.5 text-xs leading-relaxed text-white/50"
                        >
                          {{ recentChange.reason }}
                        </div>
                      </div>
                    </template>

                    <div v-else class="py-10 text-center text-sm text-white/40">
                      {{ $t("ui.affection.noData") }}
                    </div>

                    <!-- 好感度介绍（可折叠） -->
                    <div class="mt-4 border-t border-white/10 pt-3">
                      <button
                        class="flex w-full cursor-pointer items-center justify-between border-none bg-transparent p-0 text-sm text-white/60 transition-colors hover:text-white"
                        @click="introOpen = !introOpen"
                      >
                        <span>{{ $t("ui.affection.introTitle") }}</span>
                        <ChevronDown
                          :size="14"
                          class="transition-transform duration-200"
                          :class="{ 'rotate-180': introOpen }"
                        />
                      </button>
                      <Transition
                        enter-active-class="transition-all duration-200 ease-out"
                        leave-active-class="transition-all duration-150 ease-in"
                        enter-from-class="opacity-0 -translate-y-1"
                        leave-to-class="opacity-0 -translate-y-1"
                      >
                        <div
                          v-if="introOpen"
                          class="mt-2 flex flex-col gap-1.5 text-xs leading-relaxed text-white/50"
                        >
                          <div v-for="dim in dimensions" :key="`desc-${dim.key}`">
                            · <span class="text-white/70">{{ $t(`ui.affection.${dim.key}`) }}</span
                            >：{{ $t(`ui.affection.dimDesc.${dim.key}`) }}
                          </div>
                          <div class="mt-1">{{ $t("ui.affection.introEval") }}</div>
                          <div>{{ $t("ui.affection.introPersist") }}</div>
                          <div>{{ $t("ui.affection.introOverflow") }}</div>
                        </div>
                      </Transition>
                    </div>
                  </div>
                </Transition>
              </div>
            </div>
          </Transition>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { Heart, ChevronDown } from "lucide-vue-next";
import { convertFileSrc } from "@tauri-apps/api/core";
import Button from "../base/widget/Button.vue";
import { useGameStore } from "../../stores/modules/game";
import { useUIStore } from "@/stores/modules/ui/ui";
import { avatarFolderParams } from "@/composables/role/useRoleAvatar";
import { getAvatarFile } from "@/api/services/character";
import { getAffection } from "@/api/services/affection";
import type { AffectionVector } from "@/stores/modules/game/state";

const gameStore = useGameStore();
const uiStore = useUIStore();

const enabled = ref(false);
const introOpen = ref(false);

function toggleEnabled() {
  enabled.value = !enabled.value;
}
function close() {
  enabled.value = false;
}

const role = computed(() => gameStore.currentInteractRole);
const affection = computed(() => role.value?.affection ?? null);

type DimKey = keyof AffectionVector;
const dimensions: { key: DimKey }[] = [
  { key: "fondness" },
  { key: "trust" },
  { key: "intimacy" },
  { key: "rapport" },
  { key: "interest" },
  { key: "longing" },
];

// ── 平均值与档位 ─────────────────────────────────────
const average = computed(() => {
  const a = affection.value;
  if (!a) return null;
  return Math.round((a.fondness + a.trust + a.intimacy + a.rapport + a.interest + a.longing) / 6);
});

const TIER_ORDER = [
  "estranged",
  "acquainted",
  "plain",
  "familiar",
  "deep",
  "blazing",
  "overflow",
] as const;
type TierKey = (typeof TIER_ORDER)[number];
/** 各档位下限：疏离 <0 / 初识 0-20 / 平淡 21-40 / 熟络 41-60 / 深厚 61-80 / 炽烈 81-100 / 满溢 >100 */
const TIER_FLOOR: Record<TierKey, number> = {
  estranged: Number.NEGATIVE_INFINITY,
  acquainted: 0,
  plain: 21,
  familiar: 41,
  deep: 61,
  blazing: 81,
  overflow: 101,
};
const TIER_COLORS: Record<TierKey, string> = {
  estranged: "#7fc4ff",
  acquainted: "#8ec5ff",
  plain: "#b8c4d6",
  familiar: "#ffd479",
  deep: "#ff9ec7",
  blazing: "#ff5c8a",
  overflow: "#ff3d71",
};

const tier = computed<TierKey | null>(() => {
  const v = average.value;
  if (v === null) return null;
  if (v < 0) return "estranged";
  if (v <= 20) return "acquainted";
  if (v <= 40) return "plain";
  if (v <= 60) return "familiar";
  if (v <= 80) return "deep";
  if (v <= 100) return "blazing";
  return "overflow";
});
const tierColor = computed(() => (tier.value ? TIER_COLORS[tier.value] : "#ff9ec7"));
const tierBadgeStyle = computed(() => {
  const c = tierColor.value;
  return { color: c, borderColor: `${c}80`, background: `${c}1f` };
});

/** 距下一档的点数与本档内进度（满溢/无数据时为 null） */
const nextTierInfo = computed((): { key: TierKey; points: number; progress: number } | null => {
  const v = average.value;
  const t = tier.value;
  if (v === null || t === null || t === "overflow") return null;
  const nextKey = TIER_ORDER[TIER_ORDER.indexOf(t) + 1];
  const need = TIER_FLOOR[nextKey];
  // 疏离档以当前负值为进度起点，其余档以本档下限为起点
  const start = t === "estranged" ? v : TIER_FLOOR[t];
  const progress = Math.min(100, Math.max(0, ((v - start) / (need - start)) * 100));
  return { key: nextKey, points: Math.max(0, need - v), progress };
});

// ── 雷达图几何（100 满刻度；溢出/负值仅钳渲染半径，数值照实显示） ──
const CX = 200;
const CY = 180;
const R = 115;
const LABEL_R = 143;

const angleFor = (i: number) => -Math.PI / 2 + (i * Math.PI * 2) / 6;
const pointAt = (i: number, radius: number) => ({
  x: CX + radius * Math.cos(angleFor(i)),
  y: CY + radius * Math.sin(angleFor(i)),
});
const ringPoints = (fraction: number) =>
  Array.from({ length: 6 }, (_, i) => {
    const p = pointAt(i, R * fraction);
    return `${p.x.toFixed(1)},${p.y.toFixed(1)}`;
  }).join(" ");

const clampRadius = (v: number) => Math.min(100, Math.max(0, v));

/** 展示用数值（tween 动画的当前帧），真实数值可能超出 0~100 */
const displayValues = ref<number[]>([0, 0, 0, 0, 0, 0]);
const dataPoints = computed(() =>
  displayValues.value.map((v, i) => pointAt(i, (clampRadius(v) / 100) * R)),
);
const dataPolygonPoints = computed(() =>
  dataPoints.value.map((p) => `${p.x.toFixed(1)},${p.y.toFixed(1)}`).join(" "),
);

const axisLabels = computed(() =>
  dimensions.map((_, i) => {
    const p = pointAt(i, LABEL_R);
    const cos = Math.cos(angleFor(i));
    return {
      x: Number(p.x.toFixed(1)),
      y: Number(p.y.toFixed(1)),
      anchor:
        Math.abs(cos) < 0.3 ? ("middle" as const) : cos > 0 ? ("start" as const) : ("end" as const),
    };
  }),
);

const vertexFillClass = (v: number) =>
  v > 100 ? "fill-[#ffd7e8]" : v < 0 ? "fill-[#7fc4ff]" : "fill-[#ff9ec7]";
const valueFillClass = vertexFillClass;
const valueGlowStyle = (v: number) =>
  v > 100 ? "filter: drop-shadow(0 0 4px rgba(255, 92, 138, 0.9))" : undefined;

// ── 数值 tween：变化时 600ms 缓动过渡；切换角色时直接吸附 ──
const targetValues = computed<number[]>(() => {
  const a = affection.value;
  if (!a) return [0, 0, 0, 0, 0, 0];
  return [a.fondness, a.trust, a.intimacy, a.rapport, a.interest, a.longing];
});

let rafId: number | null = null;
let lastRoleId: number | null = null;

function cancelTween() {
  if (rafId !== null) {
    cancelAnimationFrame(rafId);
    rafId = null;
  }
}

function tweenTo(target: number[]) {
  cancelTween();
  const from = [...displayValues.value];
  const t0 = performance.now();
  const duration = 600;
  const step = (t: number) => {
    const p = Math.min(1, (t - t0) / duration);
    const eased = 1 - Math.pow(1 - p, 3);
    displayValues.value = from.map((f, i) => f + (target[i] - f) * eased);
    rafId = p < 1 ? requestAnimationFrame(step) : null;
  };
  rafId = requestAnimationFrame(step);
}

watch(
  [() => role.value?.roleId ?? null, targetValues],
  ([rid, target]) => {
    if (rid !== lastRoleId) {
      lastRoleId = rid;
      cancelTween();
      displayValues.value = [...target];
    } else {
      tweenTo(target);
    }
  },
  { immediate: true },
);

onUnmounted(cancelTween);

// ── 顶点/标注 hover：显示维度说明 ──
const hoveredDim = ref<DimKey | null>(null);

// ── 最近一次评估变化（仅展示当前角色的） ──
const recentChange = computed(() => {
  const c = gameStore.lastAffectionChange;
  const r = role.value;
  if (!c || !r || c.roleId !== r.roleId) return null;
  const entries = dimensions
    .filter((d) => typeof c.deltas[d.key] === "number" && c.deltas[d.key] !== 0)
    .map((d) => ({ key: d.key, delta: c.deltas[d.key] }));
  return { entries, reason: c.reason };
});

// 头像解析：复用 useRoleAvatar 的归一化纯函数，情绪固定「头像」（getAvatarFile 内部处理）
const avatarUrl = ref("");
let resolveAvatarId = 0;

watch(
  () =>
    role.value
      ? ([role.value.roleId, role.value.character_folder, role.value.clothesName] as const)
      : null,
  async () => {
    const r = role.value;
    if (!r) {
      avatarUrl.value = "";
      return;
    }
    const currentId = ++resolveAvatarId;
    try {
      const { characterFolder, clothesName } = avatarFolderParams(r);
      const path = await getAvatarFile(characterFolder, clothesName);
      if (currentId === resolveAvatarId) avatarUrl.value = convertFileSrc(path);
    } catch {
      if (currentId === resolveAvatarId) avatarUrl.value = "";
    }
  },
  { immediate: true },
);

watch(
  () => uiStore.showSettings,
  (show) => {
    if (show) enabled.value = false;
  },
);

// 展开时若当前角色还没有好感度数据（如 character:switch 临时建档的角色），
// 兜底全量拉一次（init / select_character 数据一般已携带）
watch(enabled, async (v) => {
  if (!v) return;
  if (gameStore.currentInteractRole?.affection) return;
  try {
    const all = await getAffection();
    for (const [roleId, values] of Object.entries(all)) {
      const r = gameStore.gameRoles[Number(roleId)];
      if (r) r.affection = values;
    }
  } catch (e) {
    console.warn("[Affection] 兜底拉取好感度失败:", e);
  }
});
</script>

<style scoped>
/* 切换角色：旧内容轻微上浮淡出，新内容自下淡入 */
.affection-role-enter-active,
.affection-role-leave-active {
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}
.affection-role-enter-from {
  opacity: 0;
  transform: translateY(4px);
}
.affection-role-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

/* 数据多边形：渐变填充 + 粉色描边 */
.affection-data-polygon {
  fill-opacity: 0.35;
  stroke: #ff5c8a;
  stroke-width: 2;
  stroke-linejoin: round;
}

/* 满溢顶点（>100，钳在满刻度边缘）的发光脉冲光晕 */
.affection-vertex-halo {
  fill: rgba(255, 92, 138, 0.35);
  transform-box: fill-box;
  transform-origin: center;
  animation: affection-vertex-pulse 1.6s ease-in-out infinite;
}
@keyframes affection-vertex-pulse {
  0%,
  100% {
    opacity: 0.4;
    transform: scale(1);
  }
  50% {
    opacity: 1;
    transform: scale(1.7);
  }
}

/* 档位徽章：pill，颜色由档位内联样式决定；满溢档额外呼吸光晕 */
.affection-tier-badge {
  padding: 1px 8px;
  border-radius: 9999px;
  font-size: 11px;
  line-height: 1.5;
  border: 1px solid;
  white-space: nowrap;
}
.affection-tier-pulse {
  animation: affection-tier-glow 1.6s ease-in-out infinite;
}
@keyframes affection-tier-glow {
  0%,
  100% {
    box-shadow: 0 0 2px rgba(255, 61, 113, 0.3);
  }
  50% {
    box-shadow: 0 0 10px rgba(255, 61, 113, 0.7);
  }
}

/* 最近变化的维度增量小芯片：正粉负蓝 */
.affection-delta-chip {
  padding: 1px 8px;
  border-radius: 9999px;
  font-size: 11px;
  line-height: 1.5;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.12);
  white-space: nowrap;
}
.affection-delta-up {
  color: #ff9ec7;
}
.affection-delta-down {
  color: #7fc4ff;
}
</style>
