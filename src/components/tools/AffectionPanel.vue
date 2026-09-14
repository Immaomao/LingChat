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

    <Transition
      enter-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
      leave-active-class="transition-all duration-300 cubic-bezier(0.2, 0.8, 0.2, 1)"
      enter-from-class="opacity-0 -translate-y-2"
      leave-to-class="opacity-0 -translate-y-2"
    >
      <div
        v-if="enabled"
        class="box-border flex w-65 flex-col rounded-3xl border border-white/10 bg-[#12121c]/75 p-5 text-white shadow-[0_8px_32px_rgba(0,0,0,0.4)] backdrop-blur-[20px]"
      >
        <!-- 只显示当前对话角色；切换角色时旧内容淡出、新内容淡入 -->
        <Transition name="affection-role" mode="out-in">
          <div :key="role?.roleId ?? 'none'" class="flex flex-col gap-4">
            <div class="flex items-center gap-3">
              <img
                v-if="avatarUrl"
                :src="avatarUrl"
                :alt="role?.roleName ?? ''"
                class="h-10 w-10 shrink-0 rounded-full border border-white/15 object-cover"
              />
              <div
                v-else
                class="h-10 w-10 shrink-0 rounded-full border border-white/10 bg-white/5"
              ></div>
              <div class="min-w-0 truncate text-base font-bold">{{ role?.roleName ?? "—" }}</div>
            </div>

            <template v-if="affection">
              <div v-for="dim in dimensions" :key="dim.key" class="flex items-center gap-2">
                <span class="w-16 shrink-0 truncate text-xs text-white/60">
                  {{ $t(dim.labelKey) }}
                </span>
                <div class="h-1.5 min-w-0 flex-1 overflow-hidden rounded-full bg-white/10">
                  <div
                    class="h-full rounded-full bg-linear-to-r from-[#ff5c8a] to-[#ff9ec7] transition-[width] duration-700"
                    :style="{ width: barWidth(affection[dim.key]) }"
                  ></div>
                </div>
                <span class="w-6 shrink-0 text-right text-xs text-[#ff9ec7] tabular-nums">
                  {{ Math.round(affection[dim.key]) }}
                </span>
              </div>
            </template>
            <div v-else class="py-3 text-center text-sm text-white/40">
              {{ $t("ui.affection.noData") }}
            </div>
          </div>
        </Transition>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Heart } from "lucide-vue-next";
import { convertFileSrc } from "@tauri-apps/api/core";
import Button from "../base/widget/Button.vue";
import { useGameStore } from "../../stores/modules/game";
import { useUIStore } from "@/stores/modules/ui/ui";
import { avatarFolderParams } from "@/composables/role/useRoleAvatar";
import { getAvatarFile } from "@/api/services/character";
import { getAffection } from "@/api/services/affection";

const gameStore = useGameStore();
const uiStore = useUIStore();

// 窄屏默认收起（宽屏默认展开，便于发现入口）
const enabled = ref(!uiStore.isNarrowScreen);

const role = computed(() => gameStore.currentInteractRole);
const affection = computed(() => role.value?.affection ?? null);

const dimensions = [
  { key: "fondness", labelKey: "ui.affection.fondness" },
  { key: "trust", labelKey: "ui.affection.trust" },
  { key: "intimacy", labelKey: "ui.affection.intimacy" },
  { key: "rapport", labelKey: "ui.affection.rapport" },
  { key: "interest", labelKey: "ui.affection.interest" },
  { key: "longing", labelKey: "ui.affection.longing" },
] as const;

const average = computed(() => {
  const a = affection.value;
  if (!a) return null;
  return Math.round((a.fondness + a.trust + a.intimacy + a.rapport + a.interest + a.longing) / 6);
});

const barWidth = (v: number) => `${Math.min(100, Math.max(0, v))}%`;

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

function toggleEnabled() {
  enabled.value = !enabled.value;
}
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
</style>
