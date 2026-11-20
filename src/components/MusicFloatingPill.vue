<script setup lang="ts">
import { ref, computed, watch, onBeforeUnmount } from 'vue';
import KyoIcon from './KyoIcon.vue';
import { useMusicDetector } from '../composables/useMusicDetector';

const props = defineProps<{
  dark: boolean;
  isDragging?: boolean;
}>();

const emit = defineEmits<{
  (e: 'dance'): void;
  (e: 'dock-change', zone: 'bottom' | 'top' | 'none'): void;
}>();

const {
  isMusicDetected,
  isMusicPlaying,
  currentTrack,
  primaryAppName,
  primaryAppId,
  trackChangedBanner,
  togglePlayPause,
  nextTrack,
  previousTrack,
  focusMusicApp,
} = useMusicDetector();

const isHovered = ref(false);
const isMinimized = ref(false);
const isClosed = ref(false);
const customPos = ref<{ x: number; y: number } | null>(null);
const isDraggingPill = ref(false);
const dockTop = ref(false);

let dragStartMouse = { x: 0, y: 0 };
let dragStartPill = { x: 14, y: 184 };
let movedDuringDrag = false;

const isPillVisible = computed(
  () =>
    !props.isDragging &&
    isMusicDetected.value &&
    !isClosed.value &&
    (isMusicPlaying.value || isHovered.value || trackChangedBanner.value || isMinimized.value)
);

const activeDockZone = computed<'bottom' | 'top' | 'none'>(() => {
  if (!isPillVisible.value) return 'none';
  if (customPos.value) {
    return customPos.value.y < 100 ? 'top' : 'bottom';
  }
  return dockTop.value ? 'top' : 'bottom';
});

watch(
  activeDockZone,
  (zone) => {
    emit('dock-change', zone);
  },
  { immediate: true }
);

function startPillDrag(e: PointerEvent) {
  e.stopPropagation();
  e.preventDefault();
  isDraggingPill.value = true;
  movedDuringDrag = false;
  dragStartMouse = { x: e.clientX, y: e.clientY };
  dragStartPill = customPos.value
    ? { ...customPos.value }
    : { x: 14, y: dockTop.value ? 4 : 184 };

  window.addEventListener('pointermove', onPillPointerMove);
  window.addEventListener('pointerup', onPillPointerUp);
}

function onPillPointerMove(e: PointerEvent) {
  if (!isDraggingPill.value) return;
  const dx = e.clientX - dragStartMouse.x;
  const dy = e.clientY - dragStartMouse.y;
  if (Math.hypot(dx, dy) > 3) {
    movedDuringDrag = true;
  }
  const maxW = isMinimized.value ? 100 : 24;
  const nx = Math.max(4, Math.min(maxW, dragStartPill.x + dx));
  const ny = Math.max(3, Math.min(186, dragStartPill.y + dy));
  customPos.value = { x: nx, y: ny };
}

function onPillPointerUp() {
  if (!isDraggingPill.value) return;
  isDraggingPill.value = false;
  window.removeEventListener('pointermove', onPillPointerMove);
  window.removeEventListener('pointerup', onPillPointerUp);

  if (!movedDuringDrag) {
    customPos.value = null;
    dockTop.value = !dockTop.value;
  }
}

onBeforeUnmount(() => {
  window.removeEventListener('pointermove', onPillPointerMove);
  window.removeEventListener('pointerup', onPillPointerUp);
});

watch(
  () => props.isDragging,
  (dragging) => {
    if (dragging) {
      isHovered.value = false;
    }
  }
);

const containerStyle = computed(() => {
  if (customPos.value) {
    return {
      left: `${customPos.value.x}px`,
      top: `${customPos.value.y}px`,
      transform: 'none',
    };
  }
  return undefined;
});

const containerPositionClass = computed(() => {
  if (customPos.value) {
    return 'absolute z-40 select-none';
  }
  return dockTop.value
    ? 'absolute top-1 left-1/2 -translate-x-1/2 z-40 select-none'
    : 'absolute bottom-1 left-1/2 -translate-x-1/2 z-40 select-none';
});

const accentColor = computed(() => {
  const id = primaryAppId.value;
  if (id.includes('spotify')) return '#1ED760';
  if (id.includes('youtube') || id.includes('apple')) return '#FA243C';
  if (id === 'vlc') return '#FF8800';
  if (id.includes('soundcloud')) return '#FF5500';
  return '#1ED760';
});

const displayTitle = computed(() => {
  if (currentTrack.value?.title) {
    return currentTrack.value.title;
  }
  return primaryAppName.value;
});

const displayArtist = computed(() => {
  if (currentTrack.value?.artist) {
    return currentTrack.value.artist;
  }
  return isMusicPlaying.value ? primaryAppName.value : 'En pausa';
});

const progressPercent = computed(() => {
  const pos = currentTrack.value?.position_secs ?? 0;
  const dur = currentTrack.value?.duration_secs ?? 0;
  if (!dur || dur <= 0) return 0;
  return Math.min(100, Math.max(0, (pos / dur) * 100));
});
</script>

<template>
  <!-- Micro-botón glassmorphic de restauración cuando está cerrado -->
  <button
    v-if="!props.isDragging && isMusicDetected && isClosed"
    type="button"
    class="kyo-glass-restore absolute bottom-1.5 right-2 z-40 w-5.5 h-5.5 rounded-full flex items-center justify-center hover:scale-110 transition-all cursor-pointer"
    :class="props.dark ? 'is-dark' : 'is-light'"
    :style="{ color: accentColor, borderColor: `${accentColor}55` }"
    :title="`Mostrar reproductor ${primaryAppName}`"
    @mousedown.stop
    @click.stop="isClosed = false"
  >
    <KyoIcon name="music" class="w-2.5 h-2.5" />
  </button>

  <transition
    enter-active-class="transition-all duration-200 ease-out"
    enter-from-class="opacity-0 translate-y-1.5 scale-95"
    enter-to-class="opacity-100 translate-y-0 scale-100"
    :leave-active-class="props.isDragging ? 'transition-none duration-0' : 'transition-all duration-150 ease-in'"
    leave-from-class="opacity-100 translate-y-0 scale-100"
    leave-to-class="opacity-0 translate-y-1.5 scale-95"
  >
    <div
      v-if="isPillVisible"
      :class="[containerPositionClass, isMinimized ? 'w-[122px]' : 'w-[196px]']"
      :style="containerStyle"
      @mouseenter="isHovered = true"
      @mouseleave="isHovered = false"
      @mousedown.stop
      @dblclick.stop
    >
      <!-- Cápsula Glassmorphic Ultra-Compacta de 1 sola línea (30px de alto: nunca corta al avatar) -->
      <div
        class="kyo-glass-capsule relative h-[30px] rounded-full px-1.5 flex items-center justify-between gap-1.5 overflow-hidden transition-all duration-200"
        :class="[
          props.dark ? 'is-dark' : 'is-light',
          isMusicPlaying ? 'is-playing' : ''
        ]"
        :style="
          isMusicPlaying
            ? {
                '--pill-accent': accentColor,
                borderColor: props.dark ? `${accentColor}48` : `${accentColor}55`,
              }
            : { '--pill-accent': accentColor }
        "
      >
        <!-- Resplandor ambiental difuso del color de la app (Glass Glow) -->
        <span
          v-if="isMusicPlaying"
          class="pointer-events-none absolute -left-3 -top-3 w-12 h-12 rounded-full opacity-25 blur-md"
          :style="{ backgroundColor: accentColor }"
        />

        <!-- 1. Disco / Carátula Circular (21x21px) -->
        <button
          type="button"
          class="relative z-10 w-[21px] h-[21px] rounded-full flex items-center justify-center shrink-0 cursor-pointer transition-transform active:scale-90 overflow-hidden border"
          :style="{
            backgroundColor: `${accentColor}20`,
            borderColor: `${accentColor}66`,
            color: accentColor,
          }"
          :title="`${primaryAppName} — Clic para hacer bailar a Kyo`"
          @click="emit('dance')"
        >
          <img
            v-if="currentTrack?.thumbnail_data_uri"
            :src="currentTrack.thumbnail_data_uri"
            alt=""
            class="w-full h-full object-cover"
          />
          <div v-else-if="isMusicPlaying" class="flex items-end gap-[1.5px] h-2.5">
            <span class="w-[2px] rounded-full animate-eq-1" :style="{ backgroundColor: accentColor }" />
            <span class="w-[2px] rounded-full animate-eq-2" :style="{ backgroundColor: accentColor }" />
            <span class="w-[2px] rounded-full animate-eq-3" :style="{ backgroundColor: accentColor }" />
          </div>
          <KyoIcon v-else name="music" class="w-2.5 h-2.5" />
        </button>

        <!-- 2A. MODO MINIMIZADO: Solo Play/Pause + Button Group -->
        <template v-if="isMinimized">
          <button
            type="button"
            class="relative z-10 w-5 h-5 rounded-full flex items-center justify-center shrink-0 text-black cursor-pointer shadow-sm transition-transform active:scale-90"
            :style="{ backgroundColor: accentColor }"
            :title="isMusicPlaying ? 'Pausar' : 'Reproducir'"
            @click="togglePlayPause()"
          >
            <KyoIcon :name="isMusicPlaying ? 'pause' : 'play'" class="w-2.5 h-2.5" />
          </button>

          <div class="kyo-glass-btngroup relative z-10 inline-flex items-center rounded-full overflow-hidden shrink-0">
            <button
              type="button"
              class="kyo-glass-winbtn cursor-grab active:cursor-grabbing"
              title="Mover (arrastrar o clic arriba/abajo)"
              @pointerdown="startPillDrag"
            >
              <KyoIcon name="move" class="w-2.5 h-2.5" />
            </button>
            <button
              type="button"
              class="kyo-glass-winbtn cursor-pointer"
              title="Expandir reproductor"
              @click.stop="isMinimized = false"
            >
              <span class="text-[8.5px] leading-none font-bold">□</span>
            </button>
            <button
              type="button"
              class="kyo-glass-winbtn is-close cursor-pointer"
              title="Cerrar"
              @click.stop="isClosed = true"
            >
              <KyoIcon name="close" class="w-2.5 h-2.5" />
            </button>
          </div>
        </template>

        <!-- 2B. MODO NORMAL EN HOVER: Controles de transporte + Button Group en la misma línea (0px de salto vertical) -->
        <template v-else-if="isHovered">
          <div class="relative z-10 flex items-center gap-0.5">
            <button
              type="button"
              class="kyo-glass-ctl"
              title="Pista anterior"
              @click="previousTrack()"
            >
              <KyoIcon name="skipPrev" class="w-2.5 h-2.5" />
            </button>

            <button
              type="button"
              class="w-5 h-5 rounded-full flex items-center justify-center text-black cursor-pointer shadow-sm transition-transform hover:scale-105 active:scale-95"
              :style="{ backgroundColor: accentColor }"
              :title="isMusicPlaying ? 'Pausar' : 'Reproducir'"
              @click="togglePlayPause()"
            >
              <KyoIcon :name="isMusicPlaying ? 'pause' : 'play'" class="w-2.5 h-2.5" />
            </button>

            <button
              type="button"
              class="kyo-glass-ctl"
              title="Siguiente pista"
              @click="nextTrack()"
            >
              <KyoIcon name="skipNext" class="w-2.5 h-2.5" />
            </button>

            <button
              type="button"
              class="kyo-glass-ctl"
              :title="`Abrir ${primaryAppName}`"
              @click="focusMusicApp()"
            >
              <KyoIcon name="window" class="w-2.5 h-2.5" />
            </button>
          </div>

          <!-- Button Group Glassmorphic: Mover | Minimizar | Cerrar -->
          <div class="kyo-glass-btngroup relative z-10 inline-flex items-center rounded-full overflow-hidden shrink-0">
            <button
              type="button"
              class="kyo-glass-winbtn cursor-grab active:cursor-grabbing"
              title="Arrastra para mover o clic para cambiar arriba/abajo"
              @pointerdown="startPillDrag"
            >
              <KyoIcon name="move" class="w-2.5 h-2.5" />
            </button>
            <button
              type="button"
              class="kyo-glass-winbtn cursor-pointer"
              title="Minimizar reproductor"
              @click.stop="isMinimized = true"
            >
              <span class="text-[9px] leading-none font-bold">−</span>
            </button>
            <button
              type="button"
              class="kyo-glass-winbtn is-close cursor-pointer"
              title="Cerrar reproductor"
              @click.stop="isClosed = true"
            >
              <KyoIcon name="close" class="w-2.5 h-2.5" />
            </button>
          </div>
        </template>

        <!-- 2C. MODO NORMAL EN REPOSO: Jerarquía Título + Artista + Botón Play/Pause rápido -->
        <template v-else>
          <button
            type="button"
            class="relative z-10 flex-1 min-w-0 text-left cursor-pointer py-0.5"
            :title="`${displayTitle} — ${displayArtist} (${primaryAppName})`"
            @click="focusMusicApp()"
          >
            <div class="text-[10.5px] font-semibold leading-[1.15] truncate tracking-tight">
              {{ displayTitle }}
            </div>
            <div class="text-[8.5px] leading-[1.1] truncate opacity-65 font-medium">
              {{ displayArtist }}
            </div>
          </button>

          <button
            type="button"
            class="relative z-10 w-5 h-5 rounded-full flex items-center justify-center shrink-0 cursor-pointer transition-transform hover:scale-105 active:scale-90"
            :style="{
              backgroundColor: isMusicPlaying ? `${accentColor}24` : 'rgba(255,255,255,0.08)',
              color: isMusicPlaying ? accentColor : ' currentColor',
            }"
            :title="isMusicPlaying ? 'Pausar' : 'Reproducir'"
            @click="togglePlayPause()"
          >
            <KyoIcon :name="isMusicPlaying ? 'pause' : 'play'" class="w-2.5 h-2.5" />
          </button>
        </template>

        <!-- Línea de progreso luminosa de 1.5px integrada en el borde inferior del cristal -->
        <div
          v-if="(currentTrack?.duration_secs ?? 0) > 0"
          class="pointer-events-none absolute bottom-0 left-3 right-3 h-[1.5px] rounded-full overflow-hidden bg-white/10"
        >
          <div
            class="h-full rounded-full transition-all duration-300"
            :style="{ width: `${progressPercent}%`, backgroundColor: accentColor }"
          />
        </div>
      </div>
    </div>
  </transition>
</template>

<style scoped>
.kyo-glass-capsule {
  backdrop-filter: blur(18px) saturate(190%);
  -webkit-backdrop-filter: blur(18px) saturate(190%);
}

.kyo-glass-capsule.is-dark {
  background: linear-gradient(
    135deg,
    rgba(22, 25, 34, 0.62) 0%,
    rgba(12, 14, 20, 0.74) 100%
  );
  border: 1px solid rgba(255, 255, 255, 0.14);
  color: #f4f4f5;
  box-shadow:
    0 8px 20px -6px rgba(0, 0, 0, 0.6),
    inset 0 1px 0 0 rgba(255, 255, 255, 0.18);
}

.kyo-glass-capsule.is-dark.is-playing {
  box-shadow:
    0 8px 22px -6px rgba(0, 0, 0, 0.65),
    0 0 14px -4px rgba(30, 215, 96, 0.25),
    inset 0 1px 0 0 rgba(255, 255, 255, 0.2);
}

.kyo-glass-capsule.is-light {
  background: linear-gradient(
    135deg,
    rgba(255, 255, 255, 0.78) 0%,
    rgba(241, 245, 249, 0.68) 100%
  );
  border: 1px solid rgba(15, 23, 42, 0.12);
  color: #0f172a;
  box-shadow:
    0 6px 18px -4px rgba(15, 23, 42, 0.15),
    inset 0 1px 0 0 rgba(255, 255, 255, 0.85);
}

.kyo-glass-restore {
  backdrop-filter: blur(14px) saturate(180%);
  -webkit-backdrop-filter: blur(14px) saturate(180%);
  border-width: 1px;
  border-style: solid;
}
.kyo-glass-restore.is-dark {
  background: rgba(16, 18, 26, 0.68);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5), inset 0 1px 0 rgba(255, 255, 255, 0.16);
}
.kyo-glass-restore.is-light {
  background: rgba(255, 255, 255, 0.78);
  box-shadow: 0 4px 12px rgba(15, 23, 42, 0.12);
}

.kyo-glass-ctl {
  width: 19px;
  height: 19px;
  border-radius: 9999px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  opacity: 0.82;
  cursor: pointer;
  transition: all 0.12s ease;
}
.kyo-glass-ctl:hover {
  opacity: 1;
  background-color: rgba(255, 255, 255, 0.14);
}

.kyo-glass-btngroup {
  background-color: rgba(255, 255, 255, 0.07);
  border: 1px solid rgba(255, 255, 255, 0.12);
}
.is-light .kyo-glass-btngroup {
  background-color: rgba(15, 23, 42, 0.06);
  border-color: rgba(15, 23, 42, 0.12);
}

.kyo-glass-winbtn {
  width: 17px;
  height: 18px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  opacity: 0.75;
  border-right: 1px solid rgba(255, 255, 255, 0.08);
  transition: all 0.12s ease;
}
.is-light .kyo-glass-winbtn {
  border-right-color: rgba(15, 23, 42, 0.08);
}
.kyo-glass-winbtn:last-child {
  border-right: none;
}
.kyo-glass-winbtn:hover {
  opacity: 1;
  background-color: rgba(255, 255, 255, 0.16);
}
.kyo-glass-winbtn.is-close:hover {
  color: #fda4af;
  background-color: rgba(244, 63, 94, 0.32);
}

@keyframes eqBar1 {
  0%, 100% { height: 3px; }
  50% { height: 9px; }
}
@keyframes eqBar2 {
  0%, 100% { height: 9px; }
  35% { height: 3px; }
  70% { height: 7px; }
}
@keyframes eqBar3 {
  0%, 100% { height: 5px; }
  45% { height: 10px; }
  80% { height: 3px; }
}
.animate-eq-1 {
  animation: eqBar1 0.65s ease-in-out infinite;
}
.animate-eq-2 {
  animation: eqBar2 0.55s ease-in-out infinite;
}
.animate-eq-3 {
  animation: eqBar3 0.75s ease-in-out infinite;
}
</style>
