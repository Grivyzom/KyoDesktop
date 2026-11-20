import { ref, computed, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { settings, type WindowLayerMode } from './useSettings';

export interface ScreenRect {
  left: number;
  top: number;
  right: number;
  bottom: number;
  width: number;
  height: number;
}

export interface DesktopBounds {
  work_area: ScreenRect;
  screen_size: { width: number; height: number };
  scale_factor: number;
  current_x: number;
  current_y: number;
  window_width: number;
  window_height: number;
}

export interface OpenWindowInfo {
  hwnd: number;
  title: string;
  process_name: string;
}

export interface MovementTick {
  x: number;
  y: number;
  vx: number;
  vy: number;
  progress: number;
  is_moving: boolean;
}

export type AnchorPosition =
  | 'bottom-right'
  | 'bottom-left'
  | 'top-right'
  | 'top-left'
  | 'center'
  | 'bottom-center'
  | 'random';

// Estado reactivo compartido para toda la aplicación
const currentPos = ref<{ x: number; y: number }>({ x: 0, y: 0 });
const velocity = ref<{ vx: number; vy: number }>({ vx: 0, vy: 0 });
const isMoving = ref(false);
const isDragging = ref(false);
const desktopBounds = ref<DesktopBounds | null>(null);
const openWindows = ref<OpenWindowInfo[]>([]);
const isLoadingWindows = ref(false);
const isRoamingActive = ref(false);
const isClickThrough = ref(false);

let roamTimer: ReturnType<typeof setTimeout> | null = null;
let unlistenMoving: UnlistenFn | null = null;
let unlistenMoved: UnlistenFn | null = null;
let unlistenDragStart: UnlistenFn | null = null;
let unlistenDragEnd: UnlistenFn | null = null;
let unlistenLayer: UnlistenFn | null = null;
let unlistenAgentMove: UnlistenFn | null = null;
let unlistenAgentLayer: UnlistenFn | null = null;
let unlistenAgentRoam: UnlistenFn | null = null;
let isInitialized = false;

export function useAvatarMovement() {
  const currentLayer = computed<WindowLayerMode>({
    get: () => settings.windowLayer,
    set: (val) => {
      settings.windowLayer = val;
    },
  });

  async function refreshDesktopBounds(): Promise<DesktopBounds | null> {
    try {
      const bounds = await invoke<DesktopBounds>('get_desktop_bounds');
      desktopBounds.value = bounds;
      currentPos.value = { x: bounds.current_x, y: bounds.current_y };
      return bounds;
    } catch (err) {
      console.error('Error fetching desktop bounds:', err);
      return null;
    }
  }

  async function refreshOpenWindows(): Promise<OpenWindowInfo[]> {
    try {
      isLoadingWindows.value = true;
      const list = await invoke<OpenWindowInfo[]>('get_open_windows');
      openWindows.value = list;
      return list;
    } catch (err) {
      console.error('Error fetching open windows:', err);
      return [];
    } finally {
      isLoadingWindows.value = false;
    }
  }

  async function moveTo(
    targetX: number,
    targetY: number,
    options?: { duration?: number; easing?: string }
  ): Promise<void> {
    try {
      const duration = options?.duration ?? 800;
      const easing = options?.easing ?? 'smooth';
      isMoving.value = true;
      await invoke('animate_avatar_to', {
        targetX: Math.round(targetX),
        targetY: Math.round(targetY),
        durationMs: duration,
        easing,
      });
    } catch (err) {
      console.error('Error animating avatar to coordinates:', err);
      isMoving.value = false;
    }
  }

  async function teleportTo(x: number, y: number): Promise<void> {
    try {
      await invoke('teleport_avatar', {
        x: Math.round(x),
        y: Math.round(y),
      });
      currentPos.value = { x, y };
      velocity.value = { vx: 0, vy: 0 };
      isMoving.value = false;
    } catch (err) {
      console.error('Error teleporting avatar:', err);
    }
  }

  async function moveToAnchor(
    anchor: AnchorPosition,
    options?: { animate?: boolean; duration?: number }
  ): Promise<void> {
    const bounds = desktopBounds.value ?? (await refreshDesktopBounds());
    if (!bounds) return;

    if (anchor === 'random') {
      await triggerSingleRoam();
      return;
    }

    try {
      const animate = options?.animate ?? true;
      const duration = options?.duration ?? 750;
      await invoke('position_window_anchor', {
        anchor,
        animate,
        durationMs: duration,
      });
    } catch (err) {
      console.error(`Error positioning avatar to anchor ${anchor}:`, err);
    }
  }

  async function setLayer(layer: WindowLayerMode, targetHwnd?: number): Promise<string> {
    try {
      const applied = await invoke<string>('set_avatar_layer', {
        layer,
        targetHwnd: targetHwnd ?? null,
      });
      settings.windowLayer = layer;
      settings.alwaysOnTop = layer === 'top';
      return applied;
    } catch (err) {
      console.error(`Error setting avatar layer to ${layer}:`, err);
      return settings.windowLayer;
    }
  }

  async function cycleLayer(): Promise<void> {
    const layers: WindowLayerMode[] = ['top', 'normal', 'bottom'];
    const currentIndex = layers.indexOf(settings.windowLayer);
    const nextLayer = layers[(currentIndex + 1) % layers.length];
    await setLayer(nextLayer);
  }

  async function setClickThrough(enabled: boolean): Promise<void> {
    try {
      await invoke('set_avatar_ignore_cursor', { ignore: enabled });
      isClickThrough.value = enabled;
    } catch (err) {
      console.error('Error toggling click through:', err);
    }
  }

  function getDurationBySpeed(speed: 'slow' | 'normal' | 'fast'): number {
    switch (speed) {
      case 'slow':
        return 2200;
      case 'fast':
        return 600;
      default:
        return 1200;
    }
  }

  async function triggerSingleRoam(): Promise<void> {
    const bounds = desktopBounds.value ?? (await refreshDesktopBounds());
    if (!bounds) return;

    const scale = bounds.scale_factor || 1.0;
    const margin = (28 * scale);
    const winW = bounds.window_width;
    const winH = bounds.window_height;

    let targetX: number;
    let targetY: number;

    if (settings.roamMode === 'edges') {
      const anchors: AnchorPosition[] = [
        'bottom-right',
        'bottom-left',
        'top-right',
        'top-left',
        'bottom-center',
      ];
      const pick = anchors[Math.floor(Math.random() * anchors.length)];
      await moveToAnchor(pick, { animate: true, duration: getDurationBySpeed(settings.roamSpeed) });
      return;
    }

    // Modo 'full' o 'curious': punto aleatorio dentro del área de trabajo
    const minX = bounds.work_area.left + margin;
    const maxX = Math.max(minX, bounds.work_area.right - winW - margin);
    const minY = bounds.work_area.top + margin;
    const maxY = Math.max(minY, bounds.work_area.bottom - winH - margin);

    targetX = Math.round(minX + Math.random() * (maxX - minX));
    targetY = Math.round(minY + Math.random() * (maxY - minY));

    const duration = getDurationBySpeed(settings.roamSpeed);
    await moveTo(targetX, targetY, { duration, easing: 'smooth' });
  }

  function scheduleNextRoam() {
    if (roamTimer) {
      clearTimeout(roamTimer);
      roamTimer = null;
    }

    if (!settings.freeRoamEnabled && !isRoamingActive.value) {
      return;
    }

    // Intervalo configurado con variación aleatoria (+/- 25%) para sensación viva y orgánica
    const baseInterval = Math.max(8, settings.roamIntervalSeconds) * 1000;
    const jitter = (Math.random() * 0.5 - 0.25) * baseInterval;
    const nextDelay = Math.max(4000, baseInterval + jitter);

    roamTimer = setTimeout(async () => {
      if ((settings.freeRoamEnabled || isRoamingActive.value) && !isMoving.value && !isDragging.value) {
        await triggerSingleRoam();
      }
      scheduleNextRoam();
    }, nextDelay);
  }

  function startRoaming() {
    isRoamingActive.value = true;
    settings.freeRoamEnabled = true;
    scheduleNextRoam();
  }

  function stopRoaming() {
    isRoamingActive.value = false;
    settings.freeRoamEnabled = false;
    if (roamTimer) {
      clearTimeout(roamTimer);
      roamTimer = null;
    }
  }

  function toggleRoaming() {
    if (settings.freeRoamEnabled || isRoamingActive.value) {
      stopRoaming();
    } else {
      startRoaming();
    }
  }

  function initMovementBridge() {
    if (isInitialized) return;
    isInitialized = true;

    refreshDesktopBounds();

    // 1. Escuchar progreso de movimiento desde Rust
    listen<MovementTick>('avatar://moving', (event) => {
      const payload = event.payload;
      currentPos.value = { x: payload.x, y: payload.y };
      velocity.value = { vx: payload.vx, vy: payload.vy };
      isMoving.value = payload.is_moving;
    }).then((un) => {
      unlistenMoving = un;
    });

    listen<{ x: number; y: number }>('avatar://moved', (event) => {
      currentPos.value = event.payload;
      velocity.value = { vx: 0, vy: 0 };
      isMoving.value = false;
    }).then((un) => {
      unlistenMoved = un;
    });

    listen('avatar://drag-start', () => {
      isDragging.value = true;
    }).then((un) => {
      unlistenDragStart = un;
    });

    listen('avatar://drag-end', () => {
      isDragging.value = false;
      velocity.value = { vx: 0, vy: 0 };
    }).then((un) => {
      unlistenDragEnd = un;
    });

    // 2. Escuchar cambio de capa desde Rust (menú contextual nativo o bandeja)
    listen<string>('avatar://layer-changed', (event) => {
      const layer = event.payload as WindowLayerMode;
      settings.windowLayer = layer;
      settings.alwaysOnTop = layer === 'top';
    }).then((un) => {
      unlistenLayer = un;
    });

    // 3. Escuchar comandos dirigidos al agente vía IPC/Eventos
    listen<{ x?: number; y?: number; duration?: number; easing?: string; anchor?: AnchorPosition }>(
      'agent://move',
      async (event) => {
        const payload = event.payload;
        if (payload.anchor) {
          await moveToAnchor(payload.anchor, {
            animate: true,
            duration: payload.duration,
          });
        } else if (payload.x !== undefined && payload.y !== undefined) {
          await moveTo(payload.x, payload.y, {
            duration: payload.duration,
            easing: payload.easing,
          });
        }
      }
    ).then((un) => {
      unlistenAgentMove = un;
    });

    listen<{ layer: WindowLayerMode; targetHwnd?: number }>('agent://layer', async (event) => {
      const payload = event.payload;
      if (payload.layer) {
        await setLayer(payload.layer, payload.targetHwnd);
      }
    }).then((un) => {
      unlistenAgentLayer = un;
    });

    listen<{ enabled: boolean }>('agent://roam', (event) => {
      if (event.payload?.enabled) {
        startRoaming();
      } else {
        stopRoaming();
      }
    }).then((un) => {
      unlistenAgentRoam = un;
    });

    // Watcher de cambios en ajustes de deambulación
    watch(
      () => settings.freeRoamEnabled,
      (enabled) => {
        if (enabled) {
          scheduleNextRoam();
        } else if (roamTimer) {
          clearTimeout(roamTimer);
          roamTimer = null;
        }
      },
      { immediate: true }
    );
  }

  function destroyMovementBridge() {
    if (roamTimer) {
      clearTimeout(roamTimer);
      roamTimer = null;
    }
    unlistenMoving?.();
    unlistenMoved?.();
    unlistenDragStart?.();
    unlistenDragEnd?.();
    unlistenLayer?.();
    unlistenAgentMove?.();
    unlistenAgentLayer?.();
    unlistenAgentRoam?.();
    isInitialized = false;
  }

  function setDragging(val: boolean) {
    isDragging.value = val;
  }

  return {
    currentPos,
    velocity,
    isMoving,
    isDragging,
    setDragging,
    currentLayer,
    desktopBounds,
    openWindows,
    isLoadingWindows,
    isRoamingActive: computed(() => settings.freeRoamEnabled || isRoamingActive.value),
    isClickThrough,
    initMovementBridge,
    destroyMovementBridge,
    refreshDesktopBounds,
    refreshOpenWindows,
    moveTo,
    teleportTo,
    moveToAnchor,
    setLayer,
    cycleLayer,
    setClickThrough,
    startRoaming,
    stopRoaming,
    toggleRoaming,
    triggerSingleRoam,
  };
}
