<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { LogicalSize, PhysicalPosition } from '@tauri-apps/api/dpi';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import BloubBot from './components/BloubBot.vue';
import SettingsModal from './components/SettingsModal.vue';
import ContextMenu from './components/ContextMenu.vue';
import { settings, isDark } from './composables/useSettings';
import { useKyoBridge } from './composables/useKyoBridge';

const { initBridge } = useKyoBridge();

const appWindow = getCurrentWindow();

const isSettingsOpen = ref(false);
const isContextMenuOpen = ref(false);
const contextMenuPos = ref({ x: 40, y: 70 });
let lastAvatarPosition: PhysicalPosition | null = null;
let unlistenOpenSettings: (() => void) | null = null;
let unlistenRestoreAvatar: (() => void) | null = null;
let unlistenOpenChat: (() => void) | null = null;
let isOpeningChat = false;
const botRef = ref<InstanceType<typeof BloubBot> | null>(null);

function onContextMenu(e: MouseEvent) {
  e.preventDefault();
  if (isSettingsOpen.value) return;
  openDioxusChat();
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
    openDioxusChat();
    return;
  }
  if (e.button === 0 && !isSettingsOpen.value) {
    try {
      await invoke('start_avatar_drag');
    } catch (err) {
      console.error('Failed to drag avatar window:', err);
    }
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

onMounted(async () => {
  initBridge();
  window.addEventListener('keydown', onGlobalKeydown);
  window.addEventListener('contextmenu', onGlobalContextMenu);
  window.addEventListener('mousedown', onWindowMouseDown);

  try {
    await appWindow.show();
    await appWindow.setFocus();
  } catch (err) {
    console.error('Error showing appWindow in onMounted:', err);
  }

  // Escucha el evento emitido por Rust cuando el usuario hace clic en "Opciones"
  // tanto desde el menú contextual del avatar como desde el ícono del tray.
  unlistenOpenSettings = await listen('open-settings', () => {
    openSettings();
  });
  // Escucha la solicitud de apertura de chat desde el tray icon
  unlistenOpenChat = await listen('request-open-chat', () => {
    openDioxusChat();
  });
  // Escucha la restauración del avatar tras cerrar Dioxus Chat o la ventana
  unlistenRestoreAvatar = await listen('restore-avatar', () => {
    isSettingsOpen.value = false;
    isContextMenuOpen.value = false;
    botRef.value?.triggerSpawn();
  });
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onGlobalKeydown);
  window.removeEventListener('contextmenu', onGlobalContextMenu);
  window.removeEventListener('mousedown', onWindowMouseDown);
  unlistenOpenSettings?.();
  unlistenOpenChat?.();
  unlistenRestoreAvatar?.();
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
      class="flex items-center justify-center cursor-grab active:cursor-grabbing select-none"
    >
      <BloubBot
        ref="botRef"
        :paper="settings.eyeColor"
        :eye-color="settings.eyeColor"
        :color="settings.color"
        :shape="settings.shape"
        :expression="settings.expression"
        :size="settings.avatarSize"
        :follow="settings.followCursor"
      />
    </div>

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
