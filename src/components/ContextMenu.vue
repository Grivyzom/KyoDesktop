<script setup lang="ts">
import { onMounted, onBeforeUnmount } from 'vue';
import { invoke } from '@tauri-apps/api/core';

const props = defineProps<{
  position: { x: number; y: number };
  theme: 'light' | 'dark' | 'system';
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
    class="custom-context-menu fixed z-50 w-[140px] rounded-2xl p-1.5 shadow-2xl border select-none transition-all duration-100"
    :class="[
      props.theme === 'light'
        ? 'bg-white text-slate-800 border-slate-200'
        : 'bg-[#181a24] text-zinc-100 border-zinc-700/80'
    ]"
    :style="{
      top: `${props.position.y}px`,
      left: `${props.position.x}px`,
    }"
    @click.stop
    @mousedown.stop
  >
    <!-- Opción: Abrir Chat -->
    <button
      type="button"
      @click="handleOpenChat"
      class="w-full text-left px-2.5 py-1.5 rounded-xl text-xs font-medium transition-colors flex items-center gap-2"
      :class="[
        props.theme === 'light'
          ? 'hover:bg-indigo-50 text-slate-700 hover:text-indigo-600'
          : 'hover:bg-indigo-500/15 text-zinc-200 hover:text-indigo-400'
      ]"
    >
      <span class="text-xs">💬</span>
      <span>Abrir Chat</span>
    </button>

    <!-- Opción: Opciones / Ajustes -->
    <button
      type="button"
      @click="handleOptions"
      class="w-full text-left px-2.5 py-1.5 rounded-xl text-xs font-medium transition-colors flex items-center gap-2"
      :class="[
        props.theme === 'light'
          ? 'hover:bg-blue-50 text-slate-700 hover:text-blue-600'
          : 'hover:bg-blue-500/15 text-zinc-200 hover:text-blue-400'
      ]"
    >
      <span class="text-xs">⚙️</span>
      <span>Opciones</span>
    </button>

    <!-- Separador -->
    <div
      class="my-1 border-t"
      :class="props.theme === 'light' ? 'border-slate-100' : 'border-zinc-800/80'"
    />

    <!-- Opción: Cerrar kyo -->
    <button
      type="button"
      @click="handleQuit"
      class="w-full text-left px-2.5 py-1.5 rounded-xl text-xs font-medium transition-colors flex items-center gap-2 text-red-500 hover:bg-red-500/10 hover:text-red-400"
    >
      <span class="text-xs font-bold">✕</span>
      <span>Cerrar kyo</span>
    </button>
  </div>
</template>
