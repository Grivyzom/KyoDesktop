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

const appWindow = getCurrentWindow();

const isSettingsOpen = ref(false);
const isContextMenuOpen = ref(false);
const contextMenuPos = ref({ x: 40, y: 70 });
let lastAvatarPosition: PhysicalPosition | null = null;
let unlistenOpenSettings: (() => void) | null = null;

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
  closeContextMenu();
  try {
    await invoke('open_dioxus_chat');
  } catch (err) {
    console.error('Error opening Dioxus chat:', err);
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
  // Clic derecho (e.button === 2) abre inmediatamente Dioxus Chat
  if (e.button === 2) {
    e.preventDefault();
    openDioxusChat();
    return;
  }
  if (isContextMenuOpen.value) {
    closeContextMenu();
    return;
  }
  // Si es un doble clic (detail === 2), abrir Dioxus chat y evitar capturar el arrastre
  if (e.detail === 2) {
    openDioxusChat();
    return;
  }
  if (e.button === 0 && !isSettingsOpen.value) {
    try {
      await appWindow.startDragging();
    } catch (err) {
      console.error('Failed to drag window:', err);
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
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onGlobalKeydown);
  window.removeEventListener('contextmenu', onGlobalContextMenu);
  window.removeEventListener('mousedown', onWindowMouseDown);
  unlistenOpenSettings?.();
});
</script>

<template>
  <main
    data-tauri-drag-region
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
      data-tauri-drag-region
      @dblclick="openDioxusChat"
      @contextmenu.prevent="onContextMenu"
      class="flex items-center justify-center cursor-grab active:cursor-grabbing select-none"
    >
      <BloubBot
        :paper="settings.eyeColor"
        :eye-color="settings.eyeColor"
        :color="settings.color"
        :shape="settings.shape"
        :expression="settings.expression"
        :size="settings.avatarSize"
        :follow="settings.followCursor"
        data-tauri-drag-region
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
