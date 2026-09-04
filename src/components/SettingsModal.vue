<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { settings, resetSettings, isDark } from '../composables/useSettings';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import KyoIcon from './KyoIcon.vue';
import SettingSlider from './SettingSlider.vue';
import BloubEditor from './BloubEditor.vue';
import type { IconName } from '@/ui/icons';

const emit = defineEmits<{
  (e: 'close'): void;
}>();

type CategoryId = 'general' | 'avatar' | 'gaze' | 'notifications' | 'advanced' | 'about';

const currentCategory = ref<CategoryId>('general');
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

interface Category {
  id: CategoryId;
  label: string;
  icon: IconName;
  keywords: string;
}

const categories: Category[] = [
  { id: 'general', label: 'General', icon: 'gear', keywords: 'ventana idioma tema arranque inicio' },
  { id: 'avatar', label: 'Editor Bloub', icon: 'face', keywords: 'avatar forma color ojos expresion tamaño aspecto' },
  { id: 'gaze', label: 'Comportamiento', icon: 'eye', keywords: 'mirada cursor seguimiento sensibilidad reposo' },
  { id: 'notifications', label: 'Notificaciones', icon: 'bell', keywords: 'alertas orbitas particulas efectos' },
  { id: 'advanced', label: 'Avanzado', icon: 'bolt', keywords: 'rendimiento gpu depuracion reset salir' },
  { id: 'about', label: 'Acerca de', icon: 'info', keywords: 'version licencia creditos zannen kyo' },
];

const filteredCategories = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return categories;
  return categories.filter(
    (c) =>
      c.label.toLowerCase().includes(q) ||
      c.id.includes(q) ||
      c.keywords.includes(q)
  );
});

watch(filteredCategories, (cats) => {
  if (cats.length > 0 && !cats.some((c) => c.id === currentCategory.value)) {
    currentCategory.value = cats[0].id;
  }
});

const dangerTitle = computed(() =>
  dangerActionType.value === 'reset' ? '¿Restablecer configuraciones?' : '¿Cerrar Kyo?'
);

const dangerBody = computed(() =>
  dangerActionType.value === 'reset'
    ? 'Se restauran todos los parámetros visuales, comportamientos y temas a sus valores de fábrica.'
    : 'Kyo se cerrará por completo y dejará de flotar en tu escritorio.'
);

function handleClose() {
  emit('close');
}

function toggleTheme() {
  settings.theme = isDark.value ? 'light' : 'dark';
  triggerSaveFeedback();
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
  <!--
    Sin `backdrop-filter` ni sombras proyectadas: la ventana de Tauri es
    transparente, asi que cada cuadro de un desenfoque a pantalla completa
    obliga al compositor del sistema a releer el escritorio entero. Era la
    causa del tiron general al arrastrar un deslizador.
  -->
  <div class="fixed inset-0 z-50 flex items-center justify-center p-2 select-none">
    <div
      class="kyo-panel relative w-full h-full max-w-5xl rounded-2xl overflow-hidden flex flex-col border contain-render"
      :class="isDark ? 'dark' : ''"
      style="border-color: var(--kyo-border)"
      @click.stop
    >
      <!-- Barra de titulo -->
      <header
        data-tauri-drag-region
        class="h-13 px-5 py-3 flex items-center justify-between border-b kyo-divider cursor-grab active:cursor-grabbing"
        style="background-color: var(--kyo-bg-2)"
      >
        <div class="flex items-center gap-3">
          <span
            class="w-7 h-7 rounded-xl flex items-center justify-center"
            style="background-color: var(--kyo-accent-soft); color: var(--kyo-accent)"
          >
            <KyoIcon name="gear" class="w-4 h-4" />
          </span>
          <h1 class="text-sm font-semibold tracking-wide">Ajustes de Kyo</h1>

          <transition
            enter-active-class="transition-opacity duration-150"
            enter-from-class="opacity-0"
            leave-active-class="transition-opacity duration-150"
            leave-to-class="opacity-0"
          >
            <span
              v-if="isSavedBadgeVisible"
              class="text-[11px] px-2.5 py-0.5 rounded-full font-medium flex items-center gap-1.5 text-emerald-500"
              style="background-color: rgba(16, 185, 129, 0.1)"
            >
              <KyoIcon name="check" :width="2.5" class="w-3 h-3" />
              Guardado automático
            </span>
          </transition>
        </div>

        <div class="flex items-center gap-2">
          <button type="button" class="kyo-chip" title="Alternar modo claro / oscuro" @click="toggleTheme">
            <KyoIcon :name="isDark ? 'moon' : 'sun'" class="w-3.5 h-3.5" />
            <span>{{ isDark ? 'Oscuro' : 'Claro' }}</span>
          </button>

          <button
            type="button"
            class="kyo-chip w-8 h-8 rounded-xl justify-center"
            title="Cerrar ajustes (Esc)"
            @click="handleClose"
          >
            <KyoIcon name="close" :width="2" class="w-4 h-4" />
          </button>
        </div>
      </header>

      <div class="flex-1 flex overflow-hidden">
        <!-- Navegacion lateral -->
        <aside
          class="w-56 flex-shrink-0 border-r kyo-divider flex flex-col justify-between p-3"
          style="background-color: var(--kyo-bg-2)"
        >
          <div class="space-y-1">
            <div class="relative mb-3">
              <input
                v-model="searchQuery"
                type="text"
                placeholder="Buscar ajuste..."
                class="kyo-input w-full pl-8"
              />
              <KyoIcon
                name="search"
                class="w-3.5 h-3.5 absolute left-2.5 top-2.5"
                style="color: var(--kyo-muted)"
              />
            </div>

            <button
              v-for="cat in filteredCategories"
              :key="cat.id"
              type="button"
              class="kyo-nav"
              :class="currentCategory === cat.id ? 'kyo-nav-on' : ''"
              @click="currentCategory = cat.id"
            >
              <KyoIcon :name="cat.icon" class="w-4 h-4" />
              <span>{{ cat.label }}</span>
            </button>

            <p v-if="filteredCategories.length === 0" class="kyo-hint text-center py-4">
              Sin resultados
            </p>
          </div>

          <div class="px-1 pt-3 border-t kyo-divider flex items-center justify-between kyo-hint">
            <span>Kyo v0.1.0</span>
            <span class="flex items-center gap-1.5 text-emerald-500">
              <svg viewBox="0 0 8 8" class="w-2 h-2" aria-hidden="true">
                <circle cx="4" cy="4" r="4" fill="currentColor" />
              </svg>
              En línea
            </span>
          </div>
        </aside>

        <!-- Contenido -->
        <main class="flex-1 overflow-y-auto p-5 custom-scrollbar contain-render">
          <!-- GENERAL -->
          <section v-if="currentCategory === 'general'" class="space-y-4">
            <div>
              <h2 class="text-base font-bold tracking-tight">Configuración general</h2>
              <p class="kyo-hint">Arranque, idioma y preferencias del sistema.</p>
            </div>

            <div class="kyo-card p-4 space-y-4">
              <div class="flex items-center gap-2">
                <KyoIcon name="window" class="w-3.5 h-3.5 text-blue-500" />
                <h3 class="kyo-card-title">Comportamiento de la ventana</h3>
              </div>

              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="kyo-label">Fijar siempre visible</label>
                  <span class="kyo-hint">Mantiene el avatar flotando por encima del resto de ventanas.</span>
                </div>
                <input v-model="settings.alwaysOnTop" type="checkbox" class="kyo-check" @change="triggerSaveFeedback" />
              </div>

              <hr class="kyo-divider" />

              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="kyo-label">Iniciar con el sistema</label>
                  <span class="kyo-hint">Arranca Kyo en segundo plano al encender el equipo.</span>
                </div>
                <input v-model="settings.startWithSystem" type="checkbox" class="kyo-check" @change="triggerSaveFeedback" />
              </div>
            </div>

            <div class="kyo-card p-4 space-y-4">
              <div class="flex items-center gap-2">
                <KyoIcon name="palette" class="w-3.5 h-3.5 text-blue-500" />
                <h3 class="kyo-card-title">Interfaz de usuario</h3>
              </div>

              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="kyo-label">Tema del panel</label>
                  <span class="kyo-hint">Contraste visual del panel de ajustes.</span>
                </div>
                <select v-model="settings.theme" class="kyo-input" @change="triggerSaveFeedback">
                  <option value="dark">Modo oscuro</option>
                  <option value="light">Modo claro</option>
                  <option value="system">Sincronizar con el sistema</option>
                </select>
              </div>

              <hr class="kyo-divider" />

              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="kyo-label">Idioma</label>
                  <span class="kyo-hint">Idioma de expresiones, estados y accesibilidad.</span>
                </div>
                <select v-model="settings.language" class="kyo-input" @change="triggerSaveFeedback">
                  <option value="es">Español</option>
                  <option value="en">English</option>
                  <option value="fr">Français</option>
                </select>
              </div>
            </div>
          </section>

          <!-- EDITOR BLOUB -->
          <section v-else-if="currentCategory === 'avatar'" class="space-y-4">
            <div>
              <h2 class="text-base font-bold tracking-tight">Editor Bloub</h2>
              <p class="kyo-hint">Personalizador nativo del motor: forma, tinta, expresión y ojos, con vista previa en vivo.</p>
            </div>

            <BloubEditor @change="triggerSaveFeedback" />
          </section>

          <!-- COMPORTAMIENTO -->
          <section v-else-if="currentCategory === 'gaze'" class="space-y-4">
            <div>
              <h2 class="text-base font-bold tracking-tight">Comportamiento y mirada</h2>
              <p class="kyo-hint">Reactividad, seguimiento del cursor y física ocular.</p>
            </div>

            <div class="kyo-card p-4 space-y-4">
              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="kyo-label">Seguimiento global multimonitor</label>
                  <span class="kyo-hint">El avatar sigue el cursor fuera de la ventana y entre pantallas.</span>
                </div>
                <input v-model="settings.globalTracking" type="checkbox" class="kyo-check" @change="triggerSaveFeedback" />
              </div>

              <hr class="kyo-divider" />

              <div class="space-y-2">
                <div class="flex items-center justify-between gap-4">
                  <div>
                    <label class="kyo-label">Sensibilidad de giro de mirada</label>
                    <span class="kyo-hint">Velocidad y amplitud angular con la que los ojos apuntan al cursor.</span>
                  </div>
                  <span class="kyo-value">{{ settings.trackingSensitivity.toFixed(1) }}x</span>
                </div>
                <SettingSlider
                  v-model="settings.trackingSensitivity"
                  :min="0.5"
                  :max="2"
                  :step="0.1"
                  label="Sensibilidad de mirada"
                  @commit="triggerSaveFeedback"
                />
              </div>

              <hr class="kyo-divider" />

              <div class="space-y-2">
                <div class="flex items-center justify-between gap-4">
                  <div>
                    <label class="kyo-label">Tiempo antes de reposo orgánico</label>
                    <span class="kyo-hint">Segundos sin mover el ratón antes de volver a la respiración natural.</span>
                  </div>
                  <span class="kyo-value">{{ settings.idleTimeout.toFixed(1) }} s</span>
                </div>
                <SettingSlider
                  v-model="settings.idleTimeout"
                  :min="1"
                  :max="10"
                  :step="0.5"
                  label="Tiempo de reposo"
                  @commit="triggerSaveFeedback"
                />
              </div>
            </div>
          </section>

          <!-- NOTIFICACIONES -->
          <section v-else-if="currentCategory === 'notifications'" class="space-y-4">
            <div>
              <h2 class="text-base font-bold tracking-tight">Notificaciones y efectos</h2>
              <p class="kyo-hint">Alertas visuales y órbitas animadas del avatar.</p>
            </div>

            <div class="kyo-card p-4 space-y-4">
              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="kyo-label">Anillos orbitales animados</label>
                  <span class="kyo-hint">Anillos de cometa y órbitas que giran alrededor del avatar.</span>
                </div>
                <input v-model="settings.orbitEffects" type="checkbox" class="kyo-check" @change="triggerSaveFeedback" />
              </div>

              <hr class="kyo-divider" />

              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="kyo-label">Partículas de estallido</label>
                  <span class="kyo-hint">Partículas de profundidad durante las transiciones de estado.</span>
                </div>
                <input v-model="settings.burstParticles" type="checkbox" class="kyo-check" @change="triggerSaveFeedback" />
              </div>
            </div>
          </section>

          <!-- AVANZADO -->
          <section v-else-if="currentCategory === 'advanced'" class="space-y-4">
            <div>
              <h2 class="text-base font-bold tracking-tight">Avanzado y rendimiento</h2>
              <p class="kyo-hint">Opciones de bajo nivel, diagnóstico y zona de peligro.</p>
            </div>

            <div class="kyo-card p-4 space-y-4">
              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="kyo-label">Aceleración por hardware (GPU)</label>
                  <span class="kyo-hint">Delega la interpolación del motor en la tarjeta gráfica.</span>
                </div>
                <input v-model="settings.hardwareAcceleration" type="checkbox" class="kyo-check" @change="triggerSaveFeedback" />
              </div>
            </div>

            <div class="kyo-card overflow-hidden">
              <button
                type="button"
                class="w-full p-4 flex items-center justify-between text-left"
                @click="isAdvancedAccordionOpen = !isAdvancedAccordionOpen"
              >
                <div>
                  <h3 class="kyo-card-title">Parámetros de depuración</h3>
                  <span class="kyo-hint">Registros y telemetría de eventos internos.</span>
                </div>
                <KyoIcon
                  name="chevron"
                  class="w-4 h-4 transition-transform duration-150"
                  :class="isAdvancedAccordionOpen ? 'rotate-180' : ''"
                  style="color: var(--kyo-muted)"
                />
              </button>
              <div
                v-show="isAdvancedAccordionOpen"
                class="p-4 border-t kyo-divider"
                style="background-color: var(--kyo-bg-3)"
              >
                <div class="flex items-center justify-between gap-4">
                  <div>
                    <label class="kyo-label">Modo de depuración en consola</label>
                    <span class="kyo-hint">Imprime el delta de reloj e índices de muestreo en devtools.</span>
                  </div>
                  <input v-model="settings.debugMode" type="checkbox" class="kyo-check" @change="triggerSaveFeedback" />
                </div>
              </div>
            </div>

            <!-- Zona de peligro -->
            <div class="rounded-2xl p-4 border border-red-500/30 space-y-3" style="background-color: rgba(220, 38, 38, 0.06)">
              <div class="flex items-center gap-2 text-red-500">
                <KyoIcon name="warning" :width="2" class="w-4 h-4" />
                <h3 class="text-xs font-bold uppercase tracking-wider">Zona de peligro</h3>
              </div>
              <p class="text-[11px] text-red-500/80">
                Estas acciones tienen efecto global e inmediato sobre la aplicación.
              </p>

              <div class="flex items-center gap-2 pt-1">
                <button type="button" class="kyo-btn text-red-500" @click="confirmDangerAction('reset')">
                  <KyoIcon name="refresh" class="w-3.5 h-3.5" />
                  Restablecer valores
                </button>
                <button type="button" class="kyo-btn kyo-btn-danger" @click="confirmDangerAction('exit')">
                  <KyoIcon name="power" class="w-3.5 h-3.5" />
                  Cerrar Kyo
                </button>
              </div>
            </div>
          </section>

          <!-- ACERCA DE -->
          <section v-else-if="currentCategory === 'about'" class="space-y-4">
            <div>
              <h2 class="text-base font-bold tracking-tight">Acerca de Kyo</h2>
              <p class="kyo-hint">Versión, licencia y créditos del proyecto.</p>
            </div>

            <div class="kyo-card p-6 text-center space-y-4">
              <span
                class="w-16 h-16 rounded-2xl flex items-center justify-center mx-auto"
                style="background-color: var(--kyo-accent-soft); color: var(--kyo-accent)"
              >
                <KyoIcon name="face" :width="1.5" class="w-9 h-9" />
              </span>
              <div>
                <h3 class="text-base font-bold">Kyo</h3>
                <p class="kyo-hint">Versión 0.1.0 · Tauri v2 + Vue 3 + motor Bloub</p>
              </div>
              <p class="text-xs max-w-md mx-auto leading-relaxed" style="color: var(--kyo-muted)">
                Avatar de escritorio reactivo, flotante y transparente, con seguimiento de mirada
                multipantalla y geometría procedural.
              </p>
              <div class="pt-2 kyo-hint">Licencia MIT · Creado por Zannen</div>
            </div>
          </section>
        </main>
      </div>
    </div>

    <!-- Confirmacion de acciones peligrosas -->
    <div
      v-if="showDangerConfirm"
      class="kyo-panel fixed inset-0 z-50 flex items-center justify-center p-4"
      :class="isDark ? 'dark' : ''"
      style="background-color: rgba(0, 0, 0, 0.55)"
    >
      <div class="kyo-card w-full max-w-sm p-5 space-y-4" style="background-color: var(--kyo-bg)">
        <div class="flex items-center gap-3 text-red-500">
          <KyoIcon name="warning" :width="2" class="w-6 h-6" />
          <h3 class="text-sm font-bold">{{ dangerTitle }}</h3>
        </div>
        <p class="kyo-hint">{{ dangerBody }}</p>
        <div class="flex items-center justify-end gap-2 pt-1">
          <button type="button" class="kyo-btn" @click="showDangerConfirm = false">Cancelar</button>
          <button type="button" class="kyo-btn kyo-btn-danger" @click="executeDangerAction">Confirmar</button>
        </div>
      </div>
    </div>
  </div>
</template>
