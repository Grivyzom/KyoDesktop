<script setup lang="ts">
import { onMounted, onBeforeUnmount } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import KyoIcon from './KyoIcon.vue';

const props = defineProps<{
  position: { x: number; y: number };
  /** Tema ya resuelto por el padre: el menu no vuelve a calcularlo. */
  dark: boolean;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'open-options'): void;
  (e: 'open-chat'): void;
}>();

async function handleOpenChat() {
  emit('close');
  emit('open-chat');
}

async function handleOptions() {
  emit('close');
  emit('open-options');
}

async function handleQuit() {
  emit('close');
  try {
    await invoke('exit_app');
  } catch (err) {
    console.error('Error exiting app:', err);
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    emit('close');
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown);
});
</script>

<template>
  <div
    class="custom-context-menu kyo-panel fixed z-50 w-[150px] rounded-2xl p-1.5 border"
    :class="props.dark ? 'dark' : ''"
    :style="{
      top: `${props.position.y}px`,
      left: `${props.position.x}px`,
      borderColor: 'var(--kyo-border)',
      backgroundColor: 'var(--kyo-bg)',
    }"
    @click.stop
    @mousedown.stop
  >
    <button type="button" class="kyo-nav" @click="handleOpenChat">
      <KyoIcon name="chat" class="w-3.5 h-3.5" />
      <span>Abrir chat</span>
    </button>

    <button type="button" class="kyo-nav" @click="handleOptions">
      <KyoIcon name="gear" class="w-3.5 h-3.5" />
      <span>Opciones</span>
    </button>

    <div class="my-1 border-t kyo-divider" />

    <button type="button" class="kyo-nav text-red-500" @click="handleQuit">
      <KyoIcon name="power" class="w-3.5 h-3.5" />
      <span>Cerrar Kyo</span>
    </button>
  </div>
</template>
