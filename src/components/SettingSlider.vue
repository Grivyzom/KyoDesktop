<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue';

/**
 * Deslizador desacoplado del estado global.
 *
 * Un `v-model` directo sobre un `<input type="range">` emite una mutacion por
 * cada evento `input` del navegador — hasta 200 por segundo con un raton de
 * alto sondeo. Cada mutacion invalida el arbol reactivo completo del panel y
 * dispara un repintado, que en una ventana Tauri transparente se compone
 * contra el escritorio: de ahi el tiron de todo el sistema.
 *
 * Aqui el valor visible es local (`draft`) y la mutacion al exterior se agrupa
 * en un unico `requestAnimationFrame`: como maximo una escritura por cuadro,
 * sincronizada con el compositor.
 */
const props = defineProps<{
  modelValue: number;
  min: number;
  max: number;
  step: number;
  label?: string;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: number): void;
  /** El usuario solto el deslizador: momento util para persistir o avisar. */
  (e: 'commit', value: number): void;
}>();

const draft = ref(props.modelValue);

let dragging = false;
let frame = 0;
let pending = props.modelValue;

// Cambios venidos de fuera (reset de fabrica, otro control) solo se aceptan si
// el usuario no esta arrastrando: si no, el pulgar saltaria bajo el dedo.
watch(
  () => props.modelValue,
  (value) => {
    if (!dragging) draft.value = value;
  }
);

function flush() {
  frame = 0;
  if (pending !== props.modelValue) emit('update:modelValue', pending);
}

function onInput(event: Event) {
  dragging = true;
  const value = Number((event.target as HTMLInputElement).value);
  draft.value = value;
  pending = value;
  if (!frame) frame = requestAnimationFrame(flush);
}

function onChange() {
  dragging = false;
  if (frame) {
    cancelAnimationFrame(frame);
    frame = 0;
  }
  if (pending !== props.modelValue) emit('update:modelValue', pending);
  emit('commit', pending);
}

onBeforeUnmount(() => {
  if (frame) cancelAnimationFrame(frame);
});
</script>

<template>
  <input
    type="range"
    class="kyo-range"
    :min="props.min"
    :max="props.max"
    :step="props.step"
    :value="draft"
    :aria-label="props.label"
    @input="onInput"
    @change="onChange"
  />
</template>
