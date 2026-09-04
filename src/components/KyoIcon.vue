<script setup lang="ts">
import { computed } from 'vue';
import { ICONS, type IconName } from '@/ui/icons';

const props = withDefaults(
  defineProps<{
    name: IconName;
    /** Grosor del trazo. 1.8 para navegacion, 2 o mas para acciones. */
    width?: number;
  }>(),
  { width: 1.8 }
);

const paths = computed<readonly string[]>(() => ICONS[props.name]);
</script>

<template>
  <svg
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    aria-hidden="true"
    focusable="false"
    class="shrink-0"
  >
    <path
      v-for="(d, i) in paths"
      :key="i"
      :d="d"
      :stroke-width="props.width"
      stroke-linecap="round"
      stroke-linejoin="round"
    />
  </svg>
</template>
