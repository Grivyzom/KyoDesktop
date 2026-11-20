<script setup lang="ts">
import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { LogicalSize, PhysicalPosition } from '@tauri-apps/api/dpi';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import BloubBot from './components/BloubBot.vue';
import SettingsModal from './components/SettingsModal.vue';
import ContextMenu from './components/ContextMenu.vue';
import MusicFloatingPill from './components/MusicFloatingPill.vue';
import { settings, isDark } from './composables/useSettings';
import { useKyoBridge } from './composables/useKyoBridge';
import { useAvatarMovement } from './composables/useAvatarMovement';
import { useMusicDetector } from './composables/useMusicDetector';
import { isDanceState, type StateId } from './bot/states';

const { initBridge } = useKyoBridge();
const {
  initMovementBridge,
  destroyMovementBridge,
  isMoving,
  isDragging,
  setDragging,
  velocity,
} = useAvatarMovement();
const {
  isMusicDetected,
  isMusicPlaying,
  currentDanceStateId,
  initMusicBridge,
  destroyMusicBridge,
  onTrackChange,
} = useMusicDetector();

const isMusicDancing = computed(
  () =>
    settings.musicDetectionEnabled &&
    settings.musicAutoDance &&
    isMusicPlaying.value &&
    !isDragging.value &&
    !isMoving.value
);

const appWindow = getCurrentWindow();

const isSettingsOpen = ref(false);
const isContextMenuOpen = ref(false);
const contextMenuPos = ref({ x: 40, y: 70 });
const botState = ref<StateId>('idle');
const musicPillZone = ref<'bottom' | 'top' | 'none'>('none');

const avatarEffectiveSize = computed(() => {
  if (
    settings.musicShowFloatingPill &&
    isMusicDetected.value &&
    musicPillZone.value !== 'none' &&
    !isDragging.value
  ) {
    return Math.min(settings.avatarSize, 152);
  }
  return settings.avatarSize;
});

const avatarOffsetClass = computed(() => {
  if (!settings.musicShowFloatingPill || !isMusicDetected.value || isDragging.value) {
    return '';
  }
  if (musicPillZone.value === 'bottom') {
    return '-translate-y-4';
  }
  if (musicPillZone.value === 'top') {
    return 'translate-y-3.5';
  }
  return '';
});
let lastAvatarPosition: PhysicalPosition | null = null;
let unlistenOpenSettings: (() => void) | null = null;
let unlistenRestoreAvatar: (() => void) | null = null;
let unlistenOpenChat: (() => void) | null = null;
let unlistenAgentState: (() => void) | null = null;
let unlistenTrackChange: (() => void) | null = null;
let isOpeningChat = false;
const botRef = ref<InstanceType<typeof BloubBot> | null>(null);

function syncMusicDanceState() {
  if (isDragging.value || isMoving.value) {
    if (isDanceState(botState.value)) {
      botState.value = 'idle';
      botRef.value?.stopDanceImmediately?.();
    }
    return;
  }

  if (isMusicDancing.value) {
    const targetDance = currentDanceStateId.value;
    if (
      botState.value === 'idle' ||
      botState.value === 'sleep' ||
      botState.value === 'deepsleep' ||
      (isDanceState(botState.value) && botState.value !== targetDance)
    ) {
      botState.value = targetDance;
      botRef.value?.setState?.(targetDance);
    }
  } else if (isDanceState(botState.value)) {
    botState.value = 'idle';
    botRef.value?.setState?.('idle');
  }
}

function triggerManualDance() {
  if (isDragging.value || isMoving.value) return;
  const targetDance = currentDanceStateId.value;
  botState.value = targetDance;
  botRef.value?.setState?.(targetDance);
  if (!isMusicDancing.value) {
    setTimeout(() => {
      if (isDanceState(botState.value) && !isMusicDancing.value) {
        botState.value = 'idle';
        botRef.value?.setState?.('idle');
      }
    }, 6000);
  }
}

watch(
  [
    isMusicPlaying,
    currentDanceStateId,
    isDragging,
    isMoving,
    () => settings.musicAutoDance,
    () => settings.musicDetectionEnabled,
    () => settings.musicDanceStyle,
  ],
  () => {
    syncMusicDanceState();
  }
);

async function onContextMenu(e: MouseEvent) {
  e.preventDefault();
  if (isSettingsOpen.value) return;
  try {
    await invoke('show_context_menu');
  } catch (err) {
    console.error('Error opening context menu:', err);
    openDioxusChat();
  }
}

function closeContextMenu() {
  isContextMenuOpen.value = false;
}

async function openSettings() {
  closeContextMenu();
  if (isSettingsOpen.value) return;
  try {
    lastAvatarPosition = await appWindow.outerPosition();
    // Comando nativo de Rust para expandir la ventana a 920x640 y centrarla en pantalla
    await invoke('set_window_mode', { mode: 'settings' });
  } catch (err) {
    console.error('Error expanding window for settings via Rust command:', err);
    try {
      await appWindow.setResizable(true);
      await appWindow.setSize(new LogicalSize(920, 640));
      await appWindow.center();
    } catch (fallbackErr) {
      console.error('Fallback setSize failed:', fallbackErr);
    }
  }
  isSettingsOpen.value = true;
}

async function openDioxusChat() {
  if (isOpeningChat) return;
  isOpeningChat = true;
  closeContextMenu();
  try {
    // Disparar animación de colapso a singularidad hacia modo ventana
    if (botRef.value?.triggerCollapse) {
      await botRef.value.triggerCollapse();
    }
    await invoke('open_dioxus_chat');
  } catch (err) {
    console.error('Error opening Dioxus chat:', err);
  } finally {
    setTimeout(() => {
      isOpeningChat = false;
    }, 600);
  }
}

async function closeSettings() {
  isSettingsOpen.value = false;
  try {
    // Reducir la ventana a su tamaño de avatar (220x220) y restaurar su posición previa
    await invoke('set_window_mode', { mode: 'avatar' });
    if (lastAvatarPosition) {
      await appWindow.setPosition(lastAvatarPosition);
    }
  } catch (err) {
    console.error('Error restoring avatar window:', err);
  }
}

async function startDrag(e: MouseEvent) {
  if (isContextMenuOpen.value) {
    closeContextMenu();
    return;
  }
  // Doble clic abre Dioxus Chat directamente
  if (e.detail === 2) {
    setDragging(false);
    openDioxusChat();
    return;
  }
  if (e.button === 0 && !isSettingsOpen.value) {
    // Detener baile y limpiar notas musicales antes de mover la ventana transparente
    setDragging(true);
    botRef.value?.stopDanceImmediately?.();
    if (isDanceState(botState.value)) {
      botState.value = 'idle';
    }
    try {
      await invoke('start_avatar_drag');
    } catch (err) {
      setDragging(false);
      console.error('Failed to drag avatar window:', err);
    }
  }
}

function onWindowMouseUp() {
  if (isDragging.value) {
    setDragging(false);
  }
}

function onGlobalKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    if (isContextMenuOpen.value) {
      closeContextMenu();
      return;
    }
    if (isSettingsOpen.value) {
      closeSettings();
    }
  }
}

function onGlobalContextMenu(e: MouseEvent) {
  // Previene el menú por defecto de WebView2/Edge en toda la ventana
  e.preventDefault();
}

function onWindowMouseDown(e: MouseEvent) {
  if (isContextMenuOpen.value && !(e.target as HTMLElement)?.closest?.('.custom-context-menu')) {
    closeContextMenu();
  }
}

// Bandera para detectar desmontaje antes de que las promesas de listen resuelvan (H5)
let isMounted = true;

onMounted(async () => {
  initBridge();
  initMovementBridge();
  initMusicBridge();
  unlistenTrackChange = onTrackChange(() => {
    if (isSettingsOpen.value || !settings.musicDetectionEnabled || isDragging.value || isMoving.value) return;
    if (settings.musicReactOnTrackChange) {
      botState.value = 'joy';
      botRef.value?.setState?.('joy');
      setTimeout(() => {
        const next: StateId = isMusicDancing.value ? currentDanceStateId.value : 'idle';
        botState.value = next;
        botRef.value?.setState?.(next);
      }, 1100);
    }
  });
  window.addEventListener('keydown', onGlobalKeydown);
  window.addEventListener('contextmenu', onGlobalContextMenu);
  window.addEventListener('mousedown', onWindowMouseDown);
  window.addEventListener('mouseup', onWindowMouseUp);

  try {
    await appWindow.show();
  } catch (err) {
    console.error('Error showing appWindow in onMounted:', err);
  }

  // Escucha el evento emitido por Rust cuando el usuario hace clic en "Opciones"
  // tanto desde el menú contextual del avatar como desde el ícono del tray.
  listen('open-settings', () => {
    openSettings();
  }).then(fn => {
    if (isMounted) unlistenOpenSettings = fn;
    else fn();
  });
  // Escucha la solicitud de apertura de chat desde el tray icon
  listen('request-open-chat', () => {
    openDioxusChat();
  }).then(fn => {
    if (isMounted) unlistenOpenChat = fn;
    else fn();
  });
  // Escucha la restauración del avatar tras cerrar Dioxus Chat o la ventana
  listen('restore-avatar', () => {
    isSettingsOpen.value = false;
    isContextMenuOpen.value = false;
    setDragging(false);
    botRef.value?.triggerSpawn();
    botState.value = 'greet';
    setTimeout(() => {
      if (botState.value === 'greet') {
        const next: StateId = isMusicDancing.value ? currentDanceStateId.value : 'idle';
        botState.value = next;
        botRef.value?.setState?.(next);
      }
    }, 1250);
  }).then(fn => {
    if (isMounted) unlistenRestoreAvatar = fn;
    else fn();
  });

  // Escucha eventos de cambio de estado del agente (D5 §7)
  listen<string>('agent://state', (event) => {
    const raw = event.payload;
    if (raw && botRef.value?.setState) {
      const resolvedState: StateId =
        raw === 'dance' ? currentDanceStateId.value : (raw as StateId);
      botState.value = resolvedState;
      botRef.value.setState(resolvedState);
    }
  }).then(fn => {
    if (isMounted) unlistenAgentState = fn;
    else fn();
  });

  // Saludo amigable de bienvenida tras montar el avatar (D5 §2)
  setTimeout(() => {
    if (!isSettingsOpen.value && (botState.value === 'idle' || isDanceState(botState.value))) {
      botState.value = 'greet';
      setTimeout(() => {
        if (botState.value === 'greet') {
          const next: StateId = isMusicDancing.value ? currentDanceStateId.value : 'idle';
          botState.value = next;
          botRef.value?.setState?.(next);
        }
      }, 1250);
    }
  }, 900);
});

onBeforeUnmount(() => {
  isMounted = false;
  destroyMovementBridge();
  destroyMusicBridge();
  unlistenTrackChange?.();
  window.removeEventListener('keydown', onGlobalKeydown);
  window.removeEventListener('contextmenu', onGlobalContextMenu);
  window.removeEventListener('mousedown', onWindowMouseDown);
  window.removeEventListener('mouseup', onWindowMouseUp);
  unlistenOpenSettings?.();
  unlistenOpenChat?.();
  unlistenRestoreAvatar?.();
  unlistenAgentState?.();
});
</script>

<template>
  <main
    @mousedown="startDrag"
    @contextmenu.prevent="onContextMenu"
    class="w-full h-full flex items-center justify-center select-none overflow-hidden relative"
    :class="[
      isSettingsOpen ? '' : 'cursor-grab active:cursor-grabbing',
      isDark ? 'dark' : ''
    ]"
    style="background: transparent;"
  >
    <!-- Vista del Avatar -->
    <div
      v-if="!isSettingsOpen"
      class="flex items-center justify-center cursor-grab active:cursor-grabbing select-none transition-transform duration-200 ease-out"
      :class="avatarOffsetClass"
    >
      <BloubBot
        ref="botRef"
        v-model:state="botState"
        :paper="settings.eyeColor"
        :eye-color="settings.eyeColor"
        :color="settings.color"
        :shape="settings.shape"
        :expression="settings.expression"
        :size="avatarEffectiveSize"
        :follow="settings.followCursor"
        :volumetric="settings.volumetric"
        :interactive-pet="settings.interactivePet"
        :interactive-jelly="settings.interactiveJelly"
        :interactive-flinch="settings.interactiveFlinch"
        :auto-sleep-enabled="settings.autoSleepEnabled"
        :night-inactivity-sleep-minutes="settings.nightInactivitySleepMinutes"
        :day-inactivity-sleep-minutes="settings.dayInactivitySleepMinutes"
        :is-moving="isMoving"
        :is-dragging="isDragging"
        :movement-velocity="velocity"
        :music-dancing="isMusicDancing"
        :dance-state="currentDanceStateId"
      />
    </div>

    <!-- Mini-reproductor y píldora flotante de música cuando Kyo detecta Spotify / SMTC -->
    <MusicFloatingPill
      v-if="!isSettingsOpen && settings.musicShowFloatingPill"
      :dark="isDark"
      :is-dragging="isDragging"
      @dance="triggerManualDance"
      @dock-change="(z) => (musicPillZone = z)"
    />

    <!-- Menú Contextual con contenedor sólido y tema visual -->
    <ContextMenu
      v-if="isContextMenuOpen && !isSettingsOpen"
      :position="contextMenuPos"
      :dark="isDark"
      @close="closeContextMenu"
      @open-chat="openDioxusChat"
      @open-options="openSettings"
    />

    <!-- Modal de Ajustes de Escritorio -->
    <SettingsModal
      v-if="isSettingsOpen"
      @close="closeSettings"
    />
  </main>
</template>
