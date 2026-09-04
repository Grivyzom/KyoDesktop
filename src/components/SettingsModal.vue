<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import { settings, resetSettings } from '../composables/useSettings';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const currentCategory = ref<'general' | 'avatar' | 'gaze' | 'notifications' | 'advanced' | 'about'>('general');
const searchQuery = ref('');
const isSavedBadgeVisible = ref(false);
const showDangerConfirm = ref(false);
const dangerActionType = ref<'reset' | 'exit' | null>(null);
const isAdvancedAccordionOpen = ref(false);

let saveTimeout: number | undefined;

function triggerSaveFeedback() {
  isSavedBadgeVisible.value = true;
  if (saveTimeout) clearTimeout(saveTimeout);
  saveTimeout = window.setTimeout(() => {
    isSavedBadgeVisible.value = false;
  }, 2200);
}

const categories = [
  { id: 'general', label: 'General', icon: 'M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z M15 12a3 3 0 11-6 0 3 3 0 016 0z' },
  { id: 'avatar', label: 'Avatar & Aspecto', icon: 'M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z' },
  { id: 'gaze', label: 'Comportamiento', icon: 'M15 12a3 3 0 11-6 0 3 3 0 016 0z M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z' },
  { id: 'notifications', label: 'Notificaciones', icon: 'M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9' },
  { id: 'advanced', label: 'Avanzado & Sistema', icon: 'M13 10V3L4 14h7v7l9-11h-7z' },
  { id: 'about', label: 'Acerca de', icon: 'M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z' },
] as const;

const filteredCategories = computed(() => {
  if (!searchQuery.value.trim()) return categories;
  const q = searchQuery.value.toLowerCase();
  return categories.filter(c =>
    c.label.toLowerCase().includes(q) ||
    c.id.toLowerCase().includes(q)
  );
});

watch(filteredCategories, (cats) => {
  if (cats.length > 0 && !cats.some(c => c.id === currentCategory.value)) {
    currentCategory.value = cats[0].id as any;
  }
});

const shapes = [
  { id: 'cercle', label: 'Círculo' },
  { id: 'squircle', label: 'Squircle' },
  { id: 'galet', label: 'Guijarro' },
  { id: 'goutte', label: 'Gota' },
  { id: 'capsule', label: 'Cápsula' },
  { id: 'nuage', label: 'Nube' },
];

const skinColors = [
  { id: 'encre', hex: '#0a0a0c', label: 'Negro Encre' },
  { id: 'bleu', hex: '#3b93f0', label: 'Azul Eléctrico' },
  { id: 'violet', hex: '#8b5cf6', label: 'Violeta Neón' },
  { id: 'vert', hex: '#3ecf8e', label: 'Verde Esmeralda' },
  { id: 'rouge', hex: '#e8483f', label: 'Rojo Carmín' },
  { id: 'ambre', hex: '#f0b429', label: 'Ámbar Cálido' },
];

const eyeColors = [
  { hex: '#ffffff', label: 'Blanco Puro' },
  { hex: '#f1efe9', label: 'Crema Suave' },
  { hex: '#38bdf8', label: 'Cian Glaciar' },
  { hex: '#f472b6', label: 'Rosa Neón' },
  { hex: '#a3e635', label: 'Lima Eléctrico' },
];

function handleClose() {
  emit('close');
}

function confirmDangerAction(type: 'reset' | 'exit') {
  dangerActionType.value = type;
  showDangerConfirm.value = true;
}

async function executeDangerAction() {
  if (dangerActionType.value === 'reset') {
    resetSettings();
    triggerSaveFeedback();
  } else if (dangerActionType.value === 'exit') {
    try {
      await invoke('exit_app');
    } catch {
      const appWindow = getCurrentWindow();
      await appWindow.close();
    }
  }
  showDangerConfirm.value = false;
  dangerActionType.value = null;
}
</script>

<template>
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-6 bg-black/60 backdrop-blur-md select-none animate-in fade-in duration-200">
    <!-- Modal Container -->
    <div
      class="relative w-full max-w-4xl h-[620px] rounded-3xl overflow-hidden shadow-2xl flex flex-col border transition-colors duration-200"
      :class="settings.theme === 'light' ? 'bg-white text-slate-800 border-slate-200' : 'bg-[#12141a] text-zinc-100 border-zinc-800/80'"
      @click.stop
    >
      <!-- Top Titlebar / Header -->
      <header
        data-tauri-drag-region
        class="h-14 px-6 flex items-center justify-between border-b cursor-grab active:cursor-grabbing select-none"
        :class="settings.theme === 'light' ? 'border-slate-200/80 bg-slate-50/60' : 'border-zinc-800/70 bg-[#161820]/80'"
      >
        <div class="flex items-center gap-3">
          <div class="w-7 h-7 rounded-xl bg-blue-500/10 text-blue-500 flex items-center justify-center font-bold text-sm">
            ⚙️
          </div>
          <h1 class="text-sm font-semibold tracking-wide" :class="settings.theme === 'light' ? 'text-slate-900' : 'text-zinc-100'">
            Ajustes de Kyo Agent
          </h1>
          <!-- Save status indicator -->
          <transition
            enter-active-class="transition duration-200 ease-out"
            enter-from-class="opacity-0 translate-y-1"
            enter-to-class="opacity-100 translate-y-0"
            leave-active-class="transition duration-150 ease-in"
            leave-from-class="opacity-100"
            leave-to-class="opacity-0"
          >
            <span v-if="isSavedBadgeVisible" class="text-xs px-2.5 py-0.5 rounded-full bg-emerald-500/10 text-emerald-500 font-medium flex items-center gap-1.5 border border-emerald-500/20">
              <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M5 13l4 4L19 7" /></svg>
              Guardado automático
            </span>
          </transition>
        </div>

        <!-- Quick actions: Theme toggle and Close -->
        <div class="flex items-center gap-2">
          <!-- Light/Dark quick toggle -->
          <button
            type="button"
            @click="settings.theme = settings.theme === 'dark' ? 'light' : 'dark'; triggerSaveFeedback()"
            class="p-1.5 rounded-lg text-xs font-medium transition-colors"
            :class="settings.theme === 'light' ? 'bg-slate-200/70 text-slate-700 hover:bg-slate-300/70' : 'bg-zinc-800 text-zinc-300 hover:bg-zinc-700'"
            title="Alternar Modo Claro / Oscuro"
          >
            {{ settings.theme === 'dark' ? '🌙 Oscuro' : '☀️ Claro' }}
          </button>

          <!-- Close Modal Button -->
          <button
            type="button"
            @click="handleClose"
            class="w-8 h-8 rounded-xl flex items-center justify-center transition-colors"
            :class="settings.theme === 'light' ? 'hover:bg-slate-200/80 text-slate-500 hover:text-slate-800' : 'hover:bg-zinc-800 text-zinc-400 hover:text-white'"
            title="Cerrar Ajustes (Esc)"
          >
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" /></svg>
          </button>
        </div>
      </header>

      <!-- Main Layout Body -->
      <div class="flex-1 flex overflow-hidden">
        <!-- Left Sidebar Navigation -->
        <aside
          class="w-60 flex-shrink-0 border-r flex flex-col justify-between p-3.5"
          :class="settings.theme === 'light' ? 'border-slate-200/80 bg-slate-50/40' : 'border-zinc-800/70 bg-[#14161e]/50'"
        >
          <div class="space-y-1">
            <!-- Search Bar in sidebar/top -->
            <div class="relative mb-3">
              <input
                v-model="searchQuery"
                type="text"
                placeholder="Buscar ajuste..."
                class="w-full px-3 py-1.5 pl-8 rounded-xl text-xs transition-all border outline-none focus:ring-2 focus:ring-blue-500/40"
                :class="settings.theme === 'light' ? 'bg-white border-slate-200 text-slate-900 placeholder:text-slate-400' : 'bg-zinc-900/90 border-zinc-800 text-zinc-100 placeholder:text-zinc-500'"
              />
              <svg class="w-3.5 h-3.5 absolute left-2.5 top-2.5 text-zinc-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
              </svg>
            </div>

            <!-- Categories Nav List -->
            <button
              v-for="cat in filteredCategories"
              :key="cat.id"
              type="button"
              @click="currentCategory = cat.id as any"
              class="w-full px-3 py-2 rounded-xl text-xs font-medium flex items-center gap-2.5 transition-all text-left"
              :class="[
                currentCategory === cat.id
                  ? (settings.theme === 'light' ? 'bg-blue-50 text-blue-600 font-semibold shadow-sm' : 'bg-blue-600/15 text-blue-400 font-semibold border border-blue-500/20')
                  : (settings.theme === 'light' ? 'text-slate-600 hover:bg-slate-100/70 hover:text-slate-900' : 'text-zinc-400 hover:bg-zinc-800/60 hover:text-zinc-200')
              ]"
            >
              <svg class="w-4 h-4 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" :d="cat.icon" />
              </svg>
              <span>{{ cat.label }}</span>
            </button>

            <!-- No results message -->
            <p v-if="filteredCategories.length === 0" class="text-[11px] text-center py-4" :class="settings.theme === 'light' ? 'text-slate-400' : 'text-zinc-500'">
              Sin resultados
            </p>
          </div>

          <!-- Bottom Footer in Sidebar -->
          <div class="px-2 pt-3 border-t text-[11px] flex items-center justify-between" :class="settings.theme === 'light' ? 'border-slate-200 text-slate-400' : 'border-zinc-800/80 text-zinc-500'">
            <span>Kyo Agent v0.1.0</span>
            <span class="text-emerald-500 font-mono">● En línea</span>
          </div>
        </aside>

        <!-- Right Content Area (Fluid & Scrollable) — custom-scrollbar ensures visible but slim scrollbar only when needed -->
        <main
          class="flex-1 overflow-y-auto p-6 space-y-6 custom-scrollbar contain-render"
          :class="settings.theme === 'light' ? 'bg-slate-100/40' : 'bg-[#12141a]'"
        >
          <!-- CATEGORY: GENERAL -->
          <section v-if="currentCategory === 'general'" class="space-y-5 animate-in fade-in duration-150">
            <div>
              <h2 class="text-lg font-bold tracking-tight">Configuración General</h2>
              <p class="text-xs text-slate-500 dark:text-zinc-400">Personaliza el arranque, el idioma y las preferencias del sistema.</p>
            </div>

            <!-- Card: Ventana y Arranque -->
            <div class="rounded-2xl p-4 border space-y-4 shadow-sm" :class="settings.theme === 'light' ? 'bg-white border-slate-200/80' : 'bg-zinc-900/50 border-zinc-800/80'">
              <h3 class="text-xs font-semibold uppercase tracking-wider text-slate-400 dark:text-zinc-500">Comportamiento de la Ventana</h3>

              <!-- Item: Siempre al frente -->
              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="text-xs font-semibold block">Fijar siempre visible (Always on top)</label>
                  <span class="text-[11px] text-slate-500 dark:text-zinc-400">Mantiene al avatar flotando por encima de todas las demás ventanas.</span>
                </div>
                <input
                  type="checkbox"
                  v-model="settings.alwaysOnTop"
                  @change="triggerSaveFeedback"
                  class="w-5 h-5 accent-blue-600 rounded cursor-pointer"
                />
              </div>

              <hr :class="settings.theme === 'light' ? 'border-slate-100' : 'border-zinc-800/60'" />

              <!-- Item: Iniciar con el sistema -->
              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="text-xs font-semibold block">Iniciar con Windows</label>
                  <span class="text-[11px] text-slate-500 dark:text-zinc-400">Inicia automáticamente el avatar en segundo plano al encender el equipo.</span>
                </div>
                <input
                  type="checkbox"
                  v-model="settings.startWithSystem"
                  @change="triggerSaveFeedback"
                  class="w-5 h-5 accent-blue-600 rounded cursor-pointer"
                />
              </div>
            </div>

            <!-- Card: Idioma y Tema -->
            <div class="rounded-2xl p-4 border space-y-4 shadow-sm" :class="settings.theme === 'light' ? 'bg-white border-slate-200/80' : 'bg-zinc-900/50 border-zinc-800/80'">
              <h3 class="text-xs font-semibold uppercase tracking-wider text-slate-400 dark:text-zinc-500">Interfaz de Usuario</h3>

              <!-- Item: Tema -->
              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="text-xs font-semibold block">Tema del Panel</label>
                  <span class="text-[11px] text-slate-500 dark:text-zinc-400">Escoge el contraste visual que mejor se adapte a tu entorno.</span>
                </div>
                <select
                  v-model="settings.theme"
                  @change="triggerSaveFeedback"
                  class="text-xs px-3 py-1.5 rounded-xl border outline-none"
                  :class="settings.theme === 'light' ? 'bg-slate-50 border-slate-200 text-slate-900' : 'bg-zinc-800 border-zinc-700 text-zinc-100'"
                >
                  <option value="dark">Modo Oscuro</option>
                  <option value="light">Modo Claro</option>
                  <option value="system">Sincronizar con el sistema</option>
                </select>
              </div>

              <hr :class="settings.theme === 'light' ? 'border-slate-100' : 'border-zinc-800/60'" />

              <!-- Item: Idioma -->
              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="text-xs font-semibold block">Idioma del Asistente</label>
                  <span class="text-[11px] text-slate-500 dark:text-zinc-400">Idioma utilizado para las expresiones, estados y accesibilidad.</span>
                </div>
                <select
                  v-model="settings.language"
                  @change="triggerSaveFeedback"
                  class="text-xs px-3 py-1.5 rounded-xl border outline-none"
                  :class="settings.theme === 'light' ? 'bg-slate-50 border-slate-200 text-slate-900' : 'bg-zinc-800 border-zinc-700 text-zinc-100'"
                >
                  <option value="es">Español (Latinoamérica)</option>
                  <option value="en">English (US)</option>
                  <option value="fr">Français</option>
                </select>
              </div>
            </div>
          </section>

          <!-- CATEGORY: AVATAR & ASPECTO -->
          <section v-if="currentCategory === 'avatar'" class="space-y-5 animate-in fade-in duration-150">
            <div>
              <h2 class="text-lg font-bold tracking-tight">Avatar & Aspecto Visual</h2>
              <p class="text-xs text-slate-500 dark:text-zinc-400">Modifica la geometría, el tamaño y los colores de piel y ojos del bot.</p>
            </div>

            <!-- Card: Tamaño del Avatar -->
            <div class="rounded-2xl p-4 border space-y-3 shadow-sm" :class="settings.theme === 'light' ? 'bg-white border-slate-200/80' : 'bg-zinc-900/50 border-zinc-800/80'">
              <div class="flex items-center justify-between">
                <div>
                  <label class="text-xs font-semibold block">Tamaño del Avatar</label>
                  <span class="text-[11px] text-slate-500 dark:text-zinc-400">Escala el avatar en píxeles. Ajusta la proporción en pantalla.</span>
                </div>
                <span class="text-xs font-mono font-bold px-2.5 py-1 rounded-lg bg-blue-500/10 text-blue-500 border border-blue-500/20">
                  {{ settings.avatarSize }} px
                </span>
              </div>
              <input
                type="range"
                min="120"
                max="175"
                step="5"
                v-model.number="settings.avatarSize"
                @input="triggerSaveFeedback"
                class="w-full accent-blue-600 cursor-pointer h-2 bg-slate-200 dark:bg-zinc-800 rounded-lg"
              />
            </div>

            <!-- Card: Forma y Geometría -->
            <div class="rounded-2xl p-4 border space-y-3 shadow-sm" :class="settings.theme === 'light' ? 'bg-white border-slate-200/80' : 'bg-zinc-900/50 border-zinc-800/80'">
              <div>
                <label class="text-xs font-semibold block">Forma Base del Avatar</label>
                <span class="text-[11px] text-slate-500 dark:text-zinc-400">Geometría del cuerpo generada proceduralmente mediante armónicos.</span>
              </div>
              <div class="grid grid-cols-3 gap-2 pt-1">
                <button
                  v-for="s in shapes"
                  :key="s.id"
                  type="button"
                  @click="settings.shape = s.id; triggerSaveFeedback()"
                  class="py-2 px-3 rounded-xl text-xs font-medium border text-center transition-all"
                  :class="[
                    settings.shape === s.id
                      ? 'border-blue-500 bg-blue-500/10 text-blue-500 font-semibold ring-1 ring-blue-500'
                      : (settings.theme === 'light' ? 'border-slate-200 hover:bg-slate-50' : 'border-zinc-800 hover:bg-zinc-800/60 text-zinc-300')
                  ]"
                >
                  {{ s.label }}
                </button>
              </div>
            </div>

            <!-- Card: Color de Ojos -->
            <div class="rounded-2xl p-4 border space-y-3 shadow-sm" :class="settings.theme === 'light' ? 'bg-white border-slate-200/80' : 'bg-zinc-900/50 border-zinc-800/80'">
              <div>
                <label class="text-xs font-semibold block">Color de Ojos (Contraste)</label>
                <span class="text-[11px] text-slate-500 dark:text-zinc-400">Color de fondo nítido aplicado a la mirada y las pupilas del avatar.</span>
              </div>
              <div class="flex items-center gap-3 pt-1">
                <button
                  v-for="e in eyeColors"
                  :key="e.hex"
                  type="button"
                  @click="settings.eyeColor = e.hex; triggerSaveFeedback()"
                  class="w-8 h-8 rounded-full border-2 transition-all flex items-center justify-center relative shadow-sm"
                  :style="{ backgroundColor: e.hex }"
                  :class="settings.eyeColor === e.hex ? 'border-blue-500 scale-110 ring-2 ring-blue-500/40' : 'border-slate-300 dark:border-zinc-700 hover:scale-105'"
                  :title="e.label"
                >
                  <span v-if="settings.eyeColor === e.hex" class="w-2 h-2 rounded-full bg-slate-900"></span>
                </button>
              </div>
            </div>

            <!-- Card: Color del Cuerpo -->
            <div class="rounded-2xl p-4 border space-y-3 shadow-sm" :class="settings.theme === 'light' ? 'bg-white border-slate-200/80' : 'bg-zinc-900/50 border-zinc-800/80'">
              <div>
                <label class="text-xs font-semibold block">Color del Cuerpo (Tinta)</label>
                <span class="text-[11px] text-slate-500 dark:text-zinc-400">Paleta de tintas oficiales del motor morfológico Bloub.</span>
              </div>
              <div class="flex items-center gap-3 pt-1">
                <button
                  v-for="c in skinColors"
                  :key="c.id"
                  type="button"
                  @click="settings.color = c.id; triggerSaveFeedback()"
                  class="w-8 h-8 rounded-full border-2 transition-all flex items-center justify-center shadow-sm"
                  :style="{ backgroundColor: c.hex }"
                  :class="settings.color === c.id ? 'border-blue-500 scale-110 ring-2 ring-blue-500/40' : 'border-slate-300 dark:border-zinc-700 hover:scale-105'"
                  :title="c.label"
                >
                  <span v-if="settings.color === c.id" class="w-2 h-2 rounded-full bg-white"></span>
                </button>
              </div>
            </div>
          </section>

          <!-- CATEGORY: COMPORTAMIENTO & MIRADA -->
          <section v-if="currentCategory === 'gaze'" class="space-y-5 animate-in fade-in duration-150">
            <div>
              <h2 class="text-lg font-bold tracking-tight">Comportamiento & Mirada</h2>
              <p class="text-xs text-slate-500 dark:text-zinc-400">Controla la reactividad, el seguimiento de cursor y la física ocular.</p>
            </div>

            <!-- Card: Seguimiento Global -->
            <div class="rounded-2xl p-4 border space-y-4 shadow-sm" :class="settings.theme === 'light' ? 'bg-white border-slate-200/80' : 'bg-zinc-900/50 border-zinc-800/80'">
              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="text-xs font-semibold block">Seguimiento Global Multimonitor</label>
                  <span class="text-[11px] text-slate-500 dark:text-zinc-400">Permite que el avatar siga tu cursor incluso fuera de la ventana y entre pantallas.</span>
                </div>
                <input
                  type="checkbox"
                  v-model="settings.globalTracking"
                  @change="triggerSaveFeedback"
                  class="w-5 h-5 accent-blue-600 rounded cursor-pointer"
                />
              </div>

              <hr :class="settings.theme === 'light' ? 'border-slate-100' : 'border-zinc-800/60'" />

              <!-- Item: Sensibilidad -->
              <div class="space-y-2">
                <div class="flex items-center justify-between">
                  <div>
                    <label class="text-xs font-semibold block">Sensibilidad de Giro de Mirada</label>
                    <span class="text-[11px] text-slate-500 dark:text-zinc-400">Ajusta la velocidad y amplitud angular con la que los ojos apuntan al cursor.</span>
                  </div>
                  <span class="text-xs font-mono font-bold px-2 py-0.5 rounded bg-blue-500/10 text-blue-500">
                    {{ settings.trackingSensitivity }}x
                  </span>
                </div>
                <input
                  type="range"
                  min="0.5"
                  max="2.0"
                  step="0.1"
                  v-model.number="settings.trackingSensitivity"
                  @input="triggerSaveFeedback"
                  class="w-full accent-blue-600 cursor-pointer h-2 bg-slate-200 dark:bg-zinc-800 rounded-lg"
                />
              </div>

              <hr :class="settings.theme === 'light' ? 'border-slate-100' : 'border-zinc-800/60'" />

              <!-- Item: Tiempo de reposo -->
              <div class="space-y-2">
                <div class="flex items-center justify-between">
                  <div>
                    <label class="text-xs font-semibold block">Tiempo antes de Reposo Orgánico</label>
                    <span class="text-[11px] text-slate-500 dark:text-zinc-400">Segundos de inactividad del ratón antes de que el avatar retome la respiración natural.</span>
                  </div>
                  <span class="text-xs font-mono font-bold px-2 py-0.5 rounded bg-blue-500/10 text-blue-500">
                    {{ settings.idleTimeout }} s
                  </span>
                </div>
                <input
                  type="range"
                  min="1"
                  max="10"
                  step="0.5"
                  v-model.number="settings.idleTimeout"
                  @input="triggerSaveFeedback"
                  class="w-full accent-blue-600 cursor-pointer h-2 bg-slate-200 dark:bg-zinc-800 rounded-lg"
                />
              </div>
            </div>
          </section>

          <!-- CATEGORY: NOTIFICACIONES & EFECTOS -->
          <section v-if="currentCategory === 'notifications'" class="space-y-5 animate-in fade-in duration-150">
            <div>
              <h2 class="text-lg font-bold tracking-tight">Notificaciones & Efectos</h2>
              <p class="text-xs text-slate-500 dark:text-zinc-400">Configura las alertas visuales y las órbitas animadas del avatar.</p>
            </div>

            <!-- Card: Efectos Visuales -->
            <div class="rounded-2xl p-4 border space-y-4 shadow-sm" :class="settings.theme === 'light' ? 'bg-white border-slate-200/80' : 'bg-zinc-900/50 border-zinc-800/80'">
              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="text-xs font-semibold block">Anillos Orbitales Animados</label>
                  <span class="text-[11px] text-slate-500 dark:text-zinc-400">Muestra los anillos de cometa y órbitas que giran alrededor del avatar.</span>
                </div>
                <input
                  type="checkbox"
                  v-model="settings.orbitEffects"
                  @change="triggerSaveFeedback"
                  class="w-5 h-5 accent-blue-600 rounded cursor-pointer"
                />
              </div>

              <hr :class="settings.theme === 'light' ? 'border-slate-100' : 'border-zinc-800/60'" />

              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="text-xs font-semibold block">Partículas de Estallido (Burst Dots)</label>
                  <span class="text-[11px] text-slate-500 dark:text-zinc-400">Proyecta partículas de profundidad durante las transiciones de estado.</span>
                </div>
                <input
                  type="checkbox"
                  v-model="settings.burstParticles"
                  @change="triggerSaveFeedback"
                  class="w-5 h-5 accent-blue-600 rounded cursor-pointer"
                />
              </div>
            </div>
          </section>

          <!-- CATEGORY: AVANZADO & SISTEMA -->
          <section v-if="currentCategory === 'advanced'" class="space-y-5 animate-in fade-in duration-150">
            <div>
              <h2 class="text-lg font-bold tracking-tight">Avanzado & Rendimiento</h2>
              <p class="text-xs text-slate-500 dark:text-zinc-400">Opciones de bajo nivel, diagnósticos y zona de peligro.</p>
            </div>

            <!-- Card: Rendimiento -->
            <div class="rounded-2xl p-4 border space-y-4 shadow-sm" :class="settings.theme === 'light' ? 'bg-white border-slate-200/80' : 'bg-zinc-900/50 border-zinc-800/80'">
              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="text-xs font-semibold block">Aceleración por Hardware (GPU)</label>
                  <span class="text-[11px] text-slate-500 dark:text-zinc-400">Aprovecha la tarjeta gráfica para interpolar las funciones matemáticas a 60-120 FPS.</span>
                </div>
                <input
                  type="checkbox"
                  v-model="settings.hardwareAcceleration"
                  @change="triggerSaveFeedback"
                  class="w-5 h-5 accent-blue-600 rounded cursor-pointer"
                />
              </div>
            </div>

            <!-- Progressive Disclosure: Acordeón Opciones Avanzadas -->
            <div class="rounded-2xl border overflow-hidden shadow-sm" :class="settings.theme === 'light' ? 'bg-white border-slate-200/80' : 'bg-zinc-900/50 border-zinc-800/80'">
              <button
                type="button"
                @click="isAdvancedAccordionOpen = !isAdvancedAccordionOpen"
                class="w-full p-4 flex items-center justify-between text-left transition-colors"
                :class="settings.theme === 'light' ? 'hover:bg-slate-50' : 'hover:bg-zinc-800/40'"
              >
                <div>
                  <h3 class="text-xs font-semibold">Parámetros de Depuración</h3>
                  <span class="text-[11px] text-slate-500 dark:text-zinc-400">Registros y telemetría de eventos internos.</span>
                </div>
                <svg
                  class="w-4 h-4 transition-transform duration-200 text-zinc-400"
                  :class="{ 'rotate-180': isAdvancedAccordionOpen }"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                </svg>
              </button>
              <div v-show="isAdvancedAccordionOpen" class="p-4 border-t space-y-3" :class="settings.theme === 'light' ? 'border-slate-100 bg-slate-50/50' : 'border-zinc-800/60 bg-zinc-950/40'">
                <div class="flex items-center justify-between gap-4">
                  <div>
                    <label class="text-xs font-medium block">Modo de Depuración en Consola</label>
                    <span class="text-[11px] text-slate-500 dark:text-zinc-400">Imprime el tiempo de reloj delta (`dt`) e índices de muestreo en devtools.</span>
                  </div>
                  <input
                    type="checkbox"
                    v-model="settings.debugMode"
                    @change="triggerSaveFeedback"
                    class="w-4 h-4 accent-blue-600 rounded"
                  />
                </div>
              </div>
            </div>

            <!-- DANGER ZONE (Zona de Peligro) -->
            <div class="rounded-2xl p-4 border border-red-500/30 bg-red-500/5 dark:bg-red-950/20 space-y-3">
              <div class="flex items-center gap-2 text-red-500">
                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                </svg>
                <h3 class="text-xs font-bold uppercase tracking-wider">Zona de Peligro</h3>
              </div>
              <p class="text-[11px] text-red-600/80 dark:text-red-400/80">
                Las siguientes acciones tienen efectos globales e inmediatos sobre la aplicación.
              </p>

              <div class="flex items-center gap-3 pt-1">
                <button
                  type="button"
                  @click="confirmDangerAction('reset')"
                  class="px-3 py-1.5 rounded-xl text-xs font-medium text-red-600 dark:text-red-400 border border-red-500/40 hover:bg-red-500/10 transition-colors"
                >
                  Restablecer valores por defecto
                </button>
                <button
                  type="button"
                  @click="confirmDangerAction('exit')"
                  class="px-3 py-1.5 rounded-xl text-xs font-medium text-white bg-red-600 hover:bg-red-700 transition-colors shadow-sm"
                >
                  Cerrar Kyo Agent
                </button>
              </div>
            </div>
          </section>

          <!-- CATEGORY: ACERCA DE -->
          <section v-if="currentCategory === 'about'" class="space-y-5 animate-in fade-in duration-150">
            <div>
              <h2 class="text-lg font-bold tracking-tight">Acerca de Kyo Agent</h2>
              <p class="text-xs text-slate-500 dark:text-zinc-400">Información de versión, licencias y créditos del proyecto.</p>
            </div>

            <div class="rounded-2xl p-6 border text-center space-y-4 shadow-sm" :class="settings.theme === 'light' ? 'bg-white border-slate-200/80' : 'bg-zinc-900/50 border-zinc-800/80'">
              <div class="w-16 h-16 rounded-2xl bg-blue-500/10 border border-blue-500/20 text-3xl flex items-center justify-center mx-auto shadow-inner">
                🤖
              </div>
              <div>
                <h3 class="text-base font-bold">Kyo Agent Desktop Avatar</h3>
                <p class="text-xs text-slate-500 dark:text-zinc-400">Versión 0.1.0 • Tauri v2 + Vue 3 + Bloub Engine</p>
              </div>
              <p class="text-xs text-slate-600 dark:text-zinc-300 max-w-md mx-auto leading-relaxed">
                Avatar de escritorio reactivo, flotante y transparente, inspirado en el diseño procedural de x.ai con seguimiento de mirada multi-pantalla y aceleración nativa.
              </p>
              <div class="pt-2 text-[11px] text-slate-400 dark:text-zinc-500">
                Licencia MIT • Desarrollado por Grivyzom & Claude/Gemini
              </div>
            </div>
          </section>
        </main>
      </div>
    </div>

    <!-- Confirmation Modal for Danger Actions -->
    <div
      v-if="showDangerConfirm"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4 animate-in fade-in duration-150"
    >
      <div
        class="w-full max-w-sm rounded-2xl p-5 border shadow-2xl space-y-4"
        :class="settings.theme === 'light' ? 'bg-white border-slate-200 text-slate-900' : 'bg-[#181a24] border-zinc-700 text-zinc-100'"
      >
        <div class="flex items-center gap-3 text-red-500">
          <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
          <h3 class="text-sm font-bold">
            {{ dangerActionType === 'reset' ? '¿Restablecer configuraciones?' : '¿Cerrar la aplicación?' }}
          </h3>
        </div>
        <p class="text-xs text-slate-500 dark:text-zinc-400 leading-relaxed">
          {{
            dangerActionType === 'reset'
              ? 'Esta acción restaurará todos los parámetros visuales, comportamientos y temas a sus valores de fábrica originales.'
              : 'La aplicación Kyo Agent se cerrará por completo y dejará de flotar en tu escritorio.'
          }}
        </p>
        <div class="flex items-center justify-end gap-2 pt-2">
          <button
            type="button"
            @click="showDangerConfirm = false"
            class="px-3 py-1.5 rounded-xl text-xs font-medium border transition-colors"
            :class="settings.theme === 'light' ? 'border-slate-200 hover:bg-slate-100 text-slate-700' : 'border-zinc-700 hover:bg-zinc-800 text-zinc-300'"
          >
            Cancelar
          </button>
          <button
            type="button"
            @click="executeDangerAction"
            class="px-3 py-1.5 rounded-xl text-xs font-medium text-white bg-red-600 hover:bg-red-700 transition-colors shadow-sm"
          >
            Confirmar
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
