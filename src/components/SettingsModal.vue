<script setup lang="ts">
import { computed, ref, watch, onMounted, onBeforeUnmount } from 'vue';
import { settings, resetSettings, isDark, applyFontFamily } from '../composables/useSettings';
import { useKyoBridge } from '../composables/useKyoBridge';
import {
  validateServerUrl,
  parseAndValidateJwt,
  type JwtValidationResult,
  type UrlValidationResult,
} from '../lib/mcpSecurity';
import { DEFAULT_MCP_SERVER_URL } from '../lib/sidecarBridge';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import KyoIcon from './KyoIcon.vue';
import SettingSlider from './SettingSlider.vue';
import BloubEditor from './BloubEditor.vue';
import { useAvatarMovement } from '../composables/useAvatarMovement';
import {
  useMusicDetector,
  formatMmSs,
  DANCE_STYLE_META,
} from '../composables/useMusicDetector';
import type { IconName } from '@/ui/icons';

const {
  openWindows,
  isLoadingWindows,
  refreshOpenWindows,
  moveToAnchor,
  triggerSingleRoam,
  setLayer,
} = useAvatarMovement();

const {
  isMusicDetected,
  isMusicPlaying,
  currentTrack,
  currentDanceStyleMeta,
  runningMusicApps,
  upNextQueue,
  recentTracks,
  sleepTimerRemaining,
  primaryAppName,
  isRefreshingMusic,
  refreshMusicState,
  togglePlayPause,
  nextTrack,
  previousTrack,
  toggleShuffle,
  cycleRepeat,
  seekToSecs,
  addToQueue,
  removeFromQueue,
  clearQueue,
  playSpotifyQuery,
  setSleepTimerMinutes,
  focusMusicApp,
  moveNearMusicApp,
} = useMusicDetector();

const newQueueInput = ref('');
const activeQueueTab = ref<'up_next' | 'recent'>('up_next');

function handleAddQueueItem() {
  const q = newQueueInput.value.trim();
  if (!q) return;
  addToQueue(q);
  newQueueInput.value = '';
}

const emit = defineEmits<{
  (e: 'close'): void;
}>();

type CategoryId = 'general' | 'avatar' | 'music' | 'gaze' | 'notifications' | 'kyo' | 'advanced' | 'about';

const currentCategory = ref<CategoryId>('general');
const searchQuery = ref('');
const isSavedBadgeVisible = ref(false);
const showDangerConfirm = ref(false);
const dangerActionType = ref<'reset' | 'exit' | null>(null);
const isAdvancedAccordionOpen = ref(false);

const {
  sidecarPort,
  status: kyoStatus,
  healthData,
  serverUrl,
  jwtToken,
  latencyMs,
  activeApiBase,
  isConnected,
  lastProbeResult,
  tokenValidation,
  isVerifying,
  isLoggingIn,
  hasRefreshToken,
  accessExpiresAt,
  authMethod,
  accountInfo,
  availableProfiles,
  activeProfileId,
  activeProfile,
  setActiveProfile,
  initBridge,
  refreshHealth,
  saveConnectionSettings,
  loginWithBrowser,
  logoutSession,
} = useKyoBridge();

const showToken = ref(false);
const showManualToken = ref(false);
const serverUrlDraft = ref(serverUrl.value || DEFAULT_MCP_SERVER_URL);
const tokenDraft = ref('');
const isSavingConnection = ref(false);
const saveFeedback = ref<{ success: boolean; message: string } | null>(null);
const systemFonts = ref<string[]>([]);
const isLoadingFonts = ref(false);

const liveUrlValidation = computed<UrlValidationResult>(() =>
  validateServerUrl(serverUrlDraft.value)
);

const liveJwtValidation = computed<JwtValidationResult>(() => {
  if (!tokenDraft.value) {
    return { valid: false, isExpired: false };
  }
  return parseAndValidateJwt(tokenDraft.value);
});

async function loadSystemFonts() {
  try {
    isLoadingFonts.value = true;
    const fonts = await invoke<string[]>('get_system_fonts');
    if (Array.isArray(fonts) && fonts.length > 0) {
      systemFonts.value = fonts;
    }
  } catch (err) {
    console.error('Error cargando fuentes del sistema:', err);
  } finally {
    isLoadingFonts.value = false;
  }
}

const fontSearchQuery = ref('');
const isFontListOpen = ref(false);

const allFontOptions = computed(() => {
  const list = ['Kyo', ...systemFonts.value.filter(f => f !== 'Kyo')];
  const q = fontSearchQuery.value.trim().toLowerCase();
  if (!q) return list;
  return list.filter(f => f.toLowerCase().includes(q));
});

function selectFont(font: string) {
  settings.fontFamily = font;
  applyFontFamily(font);
  isFontListOpen.value = false;
  try {
    invoke('save_font_preference', { font });
  } catch (e) {}
  triggerSaveFeedback();
}

onMounted(() => {
  initBridge();
  tokenDraft.value = jwtToken.value;
  serverUrlDraft.value = serverUrl.value || DEFAULT_MCP_SERVER_URL;
  loadSystemFonts();
});

watch(jwtToken, (newVal) => {
  tokenDraft.value = newVal;
});

watch(serverUrl, (newVal) => {
  if (newVal) serverUrlDraft.value = newVal;
});

function handleResetUrl() {
  serverUrlDraft.value = DEFAULT_MCP_SERVER_URL;
}

async function handleSaveAndVerifyConnection() {
  isSavingConnection.value = true;
  saveFeedback.value = null;

  try {
    const res = await saveConnectionSettings(serverUrlDraft.value, tokenDraft.value);
    saveFeedback.value = {
      success: res.success,
      message: res.message,
    };
    if (res.success) {
      triggerSaveFeedback();
    }
  } catch (err: unknown) {
    saveFeedback.value = {
      success: false,
      message: (err as Error)?.message || 'Error inesperado al validar la conexión',
    };
  } finally {
    isSavingConnection.value = false;
  }
}

async function handleTestConnectionNow() {
  await refreshHealth(serverUrlDraft.value, tokenDraft.value);
}

async function handleOAuthLogin() {
  saveFeedback.value = null;
  const res = await loginWithBrowser(serverUrlDraft.value);
  saveFeedback.value = {
    success: res.success,
    message: res.message,
  };
  if (res.success) {
    triggerSaveFeedback();
  }
}

async function handleOAuthLogout() {
  saveFeedback.value = null;
  await logoutSession();
  tokenDraft.value = '';
  saveFeedback.value = {
    success: true,
    message: 'Sesión cerrada en Kyō Desktop.',
  };
}

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
  { id: 'general', label: 'General', icon: 'gear', keywords: 'ventana idioma tema arranque inicio tipografia fuente font letra' },
  { id: 'avatar', label: 'Editor Bloub', icon: 'face', keywords: 'avatar forma color ojos expresion tamaño aspecto mcp conexion web' },
  { id: 'music', label: 'Música & Apps', icon: 'music', keywords: 'musica spotify vlc youtube apple reproduccion bailar cancion pista smtc audio' },
  { id: 'gaze', label: 'Comportamiento', icon: 'eye', keywords: 'mirada cursor seguimiento sensibilidad reposo' },
  { id: 'notifications', label: 'Notificaciones', icon: 'bell', keywords: 'alertas orbitas particulas efectos' },
  { id: 'kyo', label: 'Conexión Kyo & MCP', icon: 'sparkles', keywords: 'sidecar cloud kyo api token jwt conexion servidor offline backend mcp url link' },
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

onBeforeUnmount(() => {
  if (saveTimeout) clearTimeout(saveTimeout);
});
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

            <!-- Capas de ventanas y posición en el escritorio -->
            <div class="kyo-card p-4 space-y-4">
              <div class="flex items-center gap-2">
                <KyoIcon name="layers" class="w-3.5 h-3.5 text-blue-500" />
                <h3 class="kyo-card-title">Capa de ventana en el escritorio (Z-Order)</h3>
              </div>
              <p class="text-xs text-neutral-400">
                Define cómo interactúa el avatar con las demás ventanas y aplicaciones abiertas en tu pantalla.
              </p>

              <!-- Selector visual de capas -->
              <div class="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
                <!-- Siempre visible (Topmost) -->
                <button
                  type="button"
                  class="flex items-start gap-2.5 p-3 rounded-xl border text-left transition-all"
                  :class="settings.windowLayer === 'top' ? 'border-blue-500/80 bg-blue-500/10 text-white' : 'border-neutral-800 bg-neutral-900/50 hover:border-neutral-700 text-neutral-300'"
                  @click="settings.windowLayer = 'top'; triggerSaveFeedback()"
                >
                  <span class="text-base leading-none mt-0.5 text-blue-400">▲</span>
                  <div>
                    <div class="text-xs font-semibold">Siempre visible (Topmost)</div>
                    <div class="text-[11px] text-neutral-400 leading-tight mt-0.5">Flota por encima de todas las ventanas de tu escritorio.</div>
                  </div>
                </button>

                <!-- Entre ventanas (Normal) -->
                <button
                  type="button"
                  class="flex items-start gap-2.5 p-3 rounded-xl border text-left transition-all"
                  :class="settings.windowLayer === 'normal' ? 'border-blue-500/80 bg-blue-500/10 text-white' : 'border-neutral-800 bg-neutral-900/50 hover:border-neutral-700 text-neutral-300'"
                  @click="settings.windowLayer = 'normal'; triggerSaveFeedback()"
                >
                  <span class="text-base leading-none mt-0.5 text-emerald-400">■</span>
                  <div>
                    <div class="text-xs font-semibold">Entre ventanas abiertas (Normal)</div>
                    <div class="text-[11px] text-neutral-400 leading-tight mt-0.5">Se intercala en la pila de ventanas normales del sistema.</div>
                  </div>
                </button>

                <!-- Fondo de escritorio (Bottom) -->
                <button
                  type="button"
                  class="flex items-start gap-2.5 p-3 rounded-xl border text-left transition-all"
                  :class="settings.windowLayer === 'bottom' ? 'border-blue-500/80 bg-blue-500/10 text-white' : 'border-neutral-800 bg-neutral-900/50 hover:border-neutral-700 text-neutral-300'"
                  @click="settings.windowLayer = 'bottom'; triggerSaveFeedback()"
                >
                  <span class="text-base leading-none mt-0.5 text-amber-400">▼</span>
                  <div>
                    <div class="text-xs font-semibold">Fondo de escritorio (Desktop)</div>
                    <div class="text-[11px] text-neutral-400 leading-tight mt-0.5">Vive detrás de las ventanas abiertas, sobre tu wallpaper.</div>
                  </div>
                </button>

                <!-- Detrás de la ventana activa -->
                <button
                  type="button"
                  class="flex items-start gap-2.5 p-3 rounded-xl border text-left transition-all"
                  :class="settings.windowLayer === 'behind_active' ? 'border-blue-500/80 bg-blue-500/10 text-white' : 'border-neutral-800 bg-neutral-900/50 hover:border-neutral-700 text-neutral-300'"
                  @click="settings.windowLayer = 'behind_active'; triggerSaveFeedback()"
                >
                  <span class="text-base leading-none mt-0.5 text-purple-400">👁</span>
                  <div>
                    <div class="text-xs font-semibold">Detrás de ventana activa</div>
                    <div class="text-[11px] text-neutral-400 leading-tight mt-0.5">Se sitúa inmediatamente bajo la ventana que estás usando.</div>
                  </div>
                </button>
              </div>

              <!-- Ventanas abiertas específicas -->
              <div class="pt-2 border-t kyo-divider">
                <div class="flex items-center justify-between mb-2">
                  <span class="text-xs font-medium text-neutral-300">Colocar detrás de una ventana específica</span>
                  <button
                    type="button"
                    class="text-xs px-2.5 py-1 rounded-lg bg-neutral-800 hover:bg-neutral-700 text-neutral-300 flex items-center gap-1.5 transition-colors"
                    :disabled="isLoadingWindows"
                    @click="refreshOpenWindows"
                  >
                    <KyoIcon name="refresh" class="w-3 h-3" :class="isLoadingWindows ? 'animate-spin' : ''" />
                    <span>Escanear ventanas</span>
                  </button>
                </div>

                <div v-if="openWindows.length > 0" class="max-h-36 overflow-y-auto space-y-1 pr-1 custom-scrollbar">
                  <div
                    v-for="win in openWindows"
                    :key="win.hwnd"
                    class="flex items-center justify-between p-2 rounded-lg bg-neutral-950/40 hover:bg-neutral-800/60 border border-neutral-800/50 text-xs text-neutral-300"
                  >
                    <div class="truncate max-w-[280px]">
                      <span class="font-medium text-neutral-200">{{ win.title }}</span>
                      <span v-if="win.process_name" class="text-[10px] text-neutral-500 ml-1.5">({{ win.process_name }})</span>
                    </div>
                    <button
                      type="button"
                      class="px-2 py-0.5 text-[11px] rounded bg-blue-600/20 text-blue-400 hover:bg-blue-600/30 transition-colors"
                      @click="setLayer('behind_active', win.hwnd); triggerSaveFeedback()"
                    >
                      Enviar detrás
                    </button>
                  </div>
                </div>
                <div v-else-if="!isLoadingWindows" class="text-[11px] text-neutral-500 italic">
                  Haz clic en "Escanear ventanas" para detectar las aplicaciones abiertas en tu escritorio.
                </div>
              </div>
            </div>

            <!-- Desplazamiento y Deambulación libre en X e Y -->
            <div class="kyo-card p-4 space-y-4">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-2">
                  <KyoIcon name="move" class="w-3.5 h-3.5 text-blue-500" />
                  <h3 class="kyo-card-title">Desplazamiento libre en X e Y (Free Roam)</h3>
                </div>
                <input
                  v-model="settings.freeRoamEnabled"
                  type="checkbox"
                  class="kyo-check"
                  @change="triggerSaveFeedback"
                />
              </div>

              <p class="text-xs text-neutral-400">
                Permite al avatar flotar y recorrer de manera autónoma el escritorio en coordenadas X e Y cuando está ocioso.
              </p>

              <div v-if="settings.freeRoamEnabled" class="space-y-3.5 pt-2 border-t kyo-divider">
                <div class="flex items-center justify-between gap-4">
                  <div>
                    <label class="kyo-label">Área de deambulación</label>
                    <span class="kyo-hint">Dónde se desplaza libremente por la pantalla.</span>
                  </div>
                  <select v-model="settings.roamMode" class="kyo-input w-44" @change="triggerSaveFeedback">
                    <option value="full">Todo el escritorio</option>
                    <option value="edges">Perímetro y bordes</option>
                  </select>
                </div>

                <div class="flex items-center justify-between gap-4">
                  <div>
                    <label class="kyo-label">Velocidad de vuelo</label>
                    <span class="kyo-hint">Aceleración e inercia al cambiar de coordenadas.</span>
                  </div>
                  <select v-model="settings.roamSpeed" class="kyo-input w-44" @change="triggerSaveFeedback">
                    <option value="slow">Flotación suave (Lenta)</option>
                    <option value="normal">Desplazamiento natural</option>
                    <option value="fast">Rápida y ágil</option>
                  </select>
                </div>

                <div>
                  <div class="flex items-center justify-between text-xs mb-1.5">
                    <label class="kyo-label mb-0">Frecuencia entre paseos</label>
                    <span class="text-neutral-400 font-mono">{{ settings.roamIntervalSeconds }}s</span>
                  </div>
                  <input
                    v-model.number="settings.roamIntervalSeconds"
                    type="range"
                    min="10"
                    max="120"
                    step="5"
                    class="w-full accent-blue-500"
                    @change="triggerSaveFeedback"
                  />
                  <div class="flex justify-between text-[10px] text-neutral-500 mt-0.5">
                    <span>Constante (10s)</span>
                    <span>Espaciado (120s)</span>
                  </div>
                </div>
              </div>

              <!-- Botones de prueba y anclaje rápido -->
              <div class="pt-2 border-t kyo-divider">
                <label class="kyo-label mb-2 block">Acciones de desplazamiento rápido</label>
                <div class="flex flex-wrap gap-2">
                  <button
                    type="button"
                    class="px-2.5 py-1 text-xs rounded-lg bg-neutral-800 hover:bg-neutral-700 text-neutral-300 transition-colors"
                    @click="moveToAnchor('center')"
                  >
                    ◎ Ir al centro
                  </button>
                  <button
                    type="button"
                    class="px-2.5 py-1 text-xs rounded-lg bg-neutral-800 hover:bg-neutral-700 text-neutral-300 transition-colors"
                    @click="moveToAnchor('bottom-right')"
                  >
                    ↘ Esquina inferior derecha
                  </button>
                  <button
                    type="button"
                    class="px-2.5 py-1 text-xs rounded-lg bg-neutral-800 hover:bg-neutral-700 text-neutral-300 transition-colors"
                    @click="moveToAnchor('bottom-left')"
                  >
                    ↙ Esquina inferior izquierda
                  </button>
                  <button
                    type="button"
                    class="px-2.5 py-1 text-xs rounded-lg bg-blue-600/30 hover:bg-blue-600/40 text-blue-300 border border-blue-500/40 transition-colors"
                    @click="triggerSingleRoam"
                  >
                    ✦ Paseo aleatorio ahora
                  </button>
                </div>
              </div>
            </div>

            <!-- Inicio con el sistema -->
            <div class="kyo-card p-4 space-y-4">
              <div class="flex items-center gap-2">
                <KyoIcon name="window" class="w-3.5 h-3.5 text-blue-500" />
                <h3 class="kyo-card-title">Inicio del sistema</h3>
              </div>

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

              <hr class="kyo-divider" />

              <div class="space-y-3">
                <div class="flex items-center justify-between gap-4">
                  <div>
                    <label class="kyo-label">Tipografía de la aplicación</label>
                    <span class="kyo-hint">Previsualiza el texto de ejemplo al costado derecho de cada fuente en la lista.</span>
                  </div>
                  <div class="flex items-center gap-2">
                    <span class="kyo-chip text-[11px] py-1 px-2.5">
                      Activa: {{ (settings.fontFamily === 'default' || settings.fontFamily === 'Kyo') ? 'Kyo (Oficial)' : settings.fontFamily }}
                    </span>
                    <button
                      v-if="settings.fontFamily !== 'default' && settings.fontFamily !== 'Kyo'"
                      type="button"
                      class="kyo-chip text-[11px] py-1 px-2.5 text-blue-500 hover:underline cursor-pointer"
                      title="Restablecer a fuente Kyo"
                      @click="selectFont('Kyo')"
                    >
                      Restablecer a Kyo
                    </button>
                  </div>
                </div>

                <!-- Buscador de fuentes -->
                <div class="relative">
                  <input
                    v-model="fontSearchQuery"
                    type="text"
                    placeholder="Buscar entre las fuentes de tu equipo..."
                    class="kyo-input w-full pl-8 pr-8 text-xs py-1.5 cursor-pointer"
                    @focus="isFontListOpen = true"
                    @click="isFontListOpen = true"
                  />
                  <KyoIcon
                    name="search"
                    class="w-3.5 h-3.5 absolute left-2.5 top-2.5"
                    style="color: var(--kyo-muted)"
                  />
                  <button
                    type="button"
                    class="absolute right-2 top-2 p-0.5 text-zinc-400 hover:text-zinc-200 cursor-pointer"
                    @click="isFontListOpen = !isFontListOpen"
                  >
                    <svg class="w-3.5 h-3.5 transition-transform" :class="{ 'rotate-180': isFontListOpen }" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <polyline points="6 9 12 15 18 9" />
                    </svg>
                  </button>
                </div>

                <!-- Listado con texto de muestra al costado derecho (Solo cuando está desplegado) -->
                <div v-if="isFontListOpen" class="rounded-xl border kyo-divider overflow-hidden bg-black/5 dark:bg-zinc-900/60 shadow-lg">
                  <div class="max-h-56 overflow-y-auto custom-scrollbar divide-y kyo-divider" style="max-height: 220px;">
                    <button
                      v-for="font in allFontOptions"
                      :key="font"
                      type="button"
                      class="w-full px-3 py-2.5 text-left flex items-center justify-between gap-4 transition-colors hover:bg-black/5 dark:hover:bg-white/5 cursor-pointer"
                      :class="(settings.fontFamily === font || (settings.fontFamily === 'default' && font === 'Kyo')) ? 'bg-blue-500/10 text-blue-500 dark:text-blue-400 font-semibold' : 'text-zinc-700 dark:text-zinc-300'"
                      @click="selectFont(font)"
                    >
                      <!-- Lado izquierdo: Nombre de la fuente e indicador de selección -->
                      <div class="flex items-center gap-2 min-w-[150px] shrink-0">
                        <KyoIcon
                          v-if="settings.fontFamily === font || (settings.fontFamily === 'default' && font === 'Kyo')"
                          name="check"
                          :width="2.5"
                          class="w-3.5 h-3.5 text-blue-500"
                        />
                        <span v-else class="w-3.5" />
                        <span class="text-xs truncate font-medium">{{ font }}</span>
                        <span v-if="font === 'Kyo'" class="text-[9px] px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-500 font-normal">
                          Oficial
                        </span>
                      </div>

                      <!-- Costado derecho del listado: Texto de muestra con la tipografía aplicada -->
                      <div
                        class="text-xs truncate text-right opacity-85 hover:opacity-100 flex-1"
                        :style="{ fontFamily: font === 'Kyo' ? 'Kyo, system-ui, sans-serif' : `'${font}', Kyo, system-ui, sans-serif` }"
                      >
                        El veloz murciélago 0123456789 (Aa Bb Gg)
                      </div>
                    </button>

                    <div v-if="allFontOptions.length === 0" class="p-4 text-center kyo-hint text-xs">
                      No se encontraron fuentes con el término "{{ fontSearchQuery }}"
                    </div>
                  </div>
                </div>

                <!-- Caja de Vista Previa Resumida -->
                <div
                  class="p-3.5 rounded-xl border border-dashed kyo-divider bg-black/5 dark:bg-white/5 transition-all"
                  :style="{ fontFamily: (settings.fontFamily === 'default' || settings.fontFamily === 'Kyo') ? 'Kyo, system-ui, sans-serif' : `'${settings.fontFamily}', Kyo, system-ui, sans-serif` }"
                >
                  <div class="flex items-center justify-between mb-1.5 opacity-70">
                    <span class="font-semibold text-[10px] uppercase tracking-wider">
                      Vista previa activa: {{ (settings.fontFamily === 'default' || settings.fontFamily === 'Kyo') ? 'Kyo (Oficial de la App)' : settings.fontFamily }}
                    </span>
                    <span class="text-[10px] opacity-60">
                      {{ (settings.fontFamily === 'default' || settings.fontFamily === 'Kyo') ? 'Autoinstalada en Windows' : 'Fuente de tu PC' }}
                    </span>
                  </div>
                  <p class="text-sm font-semibold leading-snug">
                    El veloz murciélago hindú comía feliz cardillo y kiwi. 0123456789
                  </p>
                  <p class="text-[11px] opacity-70 mt-1">
                    The quick brown fox jumps over the lazy dog. Aa Bb Cc Gg Jj Qq Ww Zz
                  </p>
                </div>
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

            <!-- Vinculación del Avatar con el Servidor MCP -->
            <div class="kyo-card p-4 space-y-3">
              <div class="flex items-center justify-between gap-4">
                <div class="flex items-center gap-3">
                  <span
                    class="w-3 h-3 rounded-full shrink-0"
                    :class="
                      kyoStatus === 'connected_local' || (kyoStatus === 'connected_remote' && tokenValidation.valid)
                        ? 'bg-emerald-400 animate-pulse shadow-sm shadow-emerald-500/50'
                        : kyoStatus === 'auth_error' || tokenValidation.isExpired
                        ? 'bg-amber-400 shadow-sm shadow-amber-500/50'
                        : 'bg-zinc-500'
                    "
                  />
                  <div>
                    <h3 class="kyo-card-title flex items-center flex-wrap gap-2">
                      Conexión MCP del Avatar
                      <span
                        class="text-[10px] font-mono px-2 py-0.5 rounded-full uppercase tracking-wider"
                        :class="
                          kyoStatus === 'connected_local' || (kyoStatus === 'connected_remote' && tokenValidation.valid)
                            ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20'
                            : kyoStatus === 'auth_error' || tokenValidation.isExpired
                            ? 'bg-amber-500/10 text-amber-400 border border-amber-500/20'
                            : 'bg-zinc-500/10 text-zinc-400 border border-zinc-500/20'
                        "
                      >
                        {{
                          kyoStatus === 'connected_local'
                            ? 'Local Sidecar'
                            : kyoStatus === 'connected_remote'
                            ? 'Remoto Autenticado'
                            : kyoStatus === 'auth_error' || tokenValidation.isExpired
                            ? 'Token Requerido'
                            : 'Desconectado'
                        }}
                      </span>
                      <span
                        v-if="accountInfo?.username"
                        class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-indigo-500/10 text-indigo-300 border border-indigo-500/20 flex items-center gap-1"
                      >
                        @{{ accountInfo.username }}
                      </span>
                      <span
                        v-if="activeProfile?.name"
                        class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-purple-500/10 text-purple-300 border border-purple-500/20 flex items-center gap-1"
                      >
                        <span>{{ activeProfile.icon || '🗄️' }}</span>
                        {{ activeProfile.name }}
                      </span>
                    </h3>
                    <p class="kyo-hint">
                      {{
                        kyoStatus === 'connected_local' || (kyoStatus === 'connected_remote' && tokenValidation.valid)
                          ? `Avatar vinculado a @${accountInfo?.username || 'usuario'} · Perfil: ${activeProfile?.name || 'Predeterminado'} (${latencyMs ?? 0} ms).`
                          : kyoStatus === 'auth_error' || tokenValidation.isExpired
                          ? 'El token no es válido o expiró. Configura tus credenciales para reactivar las tools de IA.'
                          : 'Configura el link del servidor y token MCP para habilitar las capacidades del avatar.'
                      }}
                    </p>
                  </div>
                </div>

                <button
                  type="button"
                  class="kyo-btn text-xs shrink-0"
                  @click="currentCategory = 'kyo'"
                >
                  <KyoIcon name="sparkles" class="w-3.5 h-3.5 text-indigo-400" />
                  Configurar Conexión & Token
                </button>
              </div>
            </div>
          </section>

          <!-- MÚSICA & APPS -->
          <section v-else-if="currentCategory === 'music'" class="space-y-4">
            <div>
              <h2 class="text-base font-bold tracking-tight">Música y Aplicaciones Multimedia</h2>
              <p class="kyo-hint">
                Detección universal en el PC (Spotify, Apple Music, VLC, YouTube Music, SMTC) e interacciones en vivo de Kyo.
              </p>
            </div>

            <!-- Tarjeta 1: Estado en Vivo & Reproductor Detectado -->
            <div class="kyo-card p-4 space-y-4">
              <div class="flex items-center justify-between gap-4">
                <div class="flex items-center gap-2.5">
                  <span
                    class="w-8 h-8 rounded-xl flex items-center justify-center shrink-0"
                    :class="
                      isMusicPlaying
                        ? 'bg-emerald-500/15 text-emerald-400 border border-emerald-500/30'
                        : isMusicDetected
                        ? 'bg-indigo-500/15 text-indigo-400 border border-indigo-500/30'
                        : 'bg-zinc-800/70 text-zinc-400 border border-zinc-700/50'
                    "
                  >
                    <KyoIcon name="music" class="w-4 h-4" />
                  </span>
                  <div>
                    <div class="flex items-center gap-2">
                      <h3 class="kyo-card-title">Monitor Multimedia de Windows</h3>
                      <span
                        class="text-[10px] font-mono px-2 py-0.5 rounded-full uppercase tracking-wider flex items-center gap-1.5"
                        :class="
                          isMusicPlaying
                            ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20'
                            : isMusicDetected
                            ? 'bg-amber-500/10 text-amber-400 border border-amber-500/20'
                            : 'bg-zinc-500/10 text-zinc-400 border border-zinc-500/20'
                        "
                      >
                        <span
                          class="w-1.5 h-1.5 rounded-full"
                          :class="
                            isMusicPlaying
                              ? 'bg-emerald-400 animate-pulse'
                              : isMusicDetected
                              ? 'bg-amber-400'
                              : 'bg-zinc-500'
                          "
                        />
                        {{
                          isMusicPlaying
                            ? `Sonando en ${primaryAppName}`
                            : isMusicDetected
                            ? `${primaryAppName} detectado (Pausa)`
                            : 'Sin música activa'
                        }}
                      </span>
                    </div>
                    <span class="kyo-hint">
                      Sincronizado mediante Windows SMTC y escaneo nativo de procesos/ventanas sin requerir API Keys.
                    </span>
                  </div>
                </div>

                <button
                  type="button"
                  class="kyo-btn text-xs shrink-0"
                  :disabled="isRefreshingMusic"
                  @click="refreshMusicState(true)"
                >
                  <KyoIcon name="refresh" class="w-3.5 h-3.5" :class="{ 'animate-spin': isRefreshingMusic }" />
                  Actualizar
                </button>
              </div>

              <!-- Pista actual y Controles de Reproducción Estilo Spotify -->
              <div
                v-if="currentTrack || isMusicDetected"
                class="p-4 rounded-2xl border border-[#282828] bg-[#121212] text-white space-y-3 shadow-md"
                :style="
                  isMusicPlaying
                    ? 'border-color: rgba(29, 185, 84, 0.4); background: radial-gradient(120% 90% at 10% 0%, rgba(29, 185, 84, 0.16) 0%, #121212 65%)'
                    : ''
                "
              >
                <div class="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4">
                  <div class="flex items-center gap-3.5 min-w-0 flex-1">
                    <!-- Carátula del Álbum en Vivo -->
                    <div class="w-13 h-13 rounded-xl bg-[#282828] border border-white/10 overflow-hidden flex items-center justify-center shrink-0 shadow">
                      <img
                        v-if="currentTrack?.thumbnail_data_uri"
                        :src="currentTrack.thumbnail_data_uri"
                        alt=""
                        class="w-full h-full object-cover"
                      />
                      <KyoIcon v-else name="music" class="w-6 h-6 text-[#1ED760]" />
                    </div>

                    <div class="min-w-0 flex-1">
                      <div class="flex items-center gap-2">
                        <span class="text-sm font-bold text-white truncate">
                          {{ currentTrack?.title || `${primaryAppName} abierto en el PC` }}
                        </span>
                        <span
                          class="px-2 py-0.5 text-[10px] rounded-full bg-[#1ED760]/15 text-[#1ED760] border border-[#1ED760]/30 font-mono shrink-0"
                        >
                          {{ currentTrack?.source_app_name || primaryAppName }}
                        </span>
                      </div>
                      <div class="text-xs text-[#b3b3b3] truncate mt-0.5">
                        {{
                          currentTrack?.artist
                            ? `${currentTrack.artist}${currentTrack.album ? ` · ${currentTrack.album}` : ''}`
                            : isMusicPlaying
                            ? 'Reproduciendo audio en vivo'
                            : 'En pausa'
                        }}
                      </div>
                    </div>
                  </div>

                  <!-- Botonera de Controles Estilo Spotify -->
                  <div class="flex items-center gap-2 shrink-0">
                    <button
                      type="button"
                      class="px-2 py-1.5 rounded-lg text-xs transition-colors cursor-pointer"
                      :class="currentTrack?.is_shuffle ? 'text-[#1ED760] bg-[#1ED760]/15' : 'text-[#b3b3b3] hover:text-white'"
                      title="Alternar modo aleatorio (Shuffle)"
                      @click="toggleShuffle()"
                    >
                      🔀
                    </button>
                    <button
                      type="button"
                      class="p-2 rounded-full text-[#b3b3b3] hover:text-white hover:bg-white/10 transition-colors cursor-pointer"
                      title="Pista anterior"
                      @click="previousTrack()"
                    >
                      <KyoIcon name="skipPrev" class="w-4 h-4" />
                    </button>
                    <button
                      type="button"
                      class="w-9 h-9 rounded-full bg-[#1ED760] hover:bg-[#3be477] text-black flex items-center justify-center shadow-md transition-transform active:scale-95 cursor-pointer"
                      :title="isMusicPlaying ? 'Pausar reproducción' : 'Reanudar reproducción'"
                      @click="togglePlayPause()"
                    >
                      <KyoIcon :name="isMusicPlaying ? 'pause' : 'play'" class="w-4 h-4" />
                    </button>
                    <button
                      type="button"
                      class="p-2 rounded-full text-[#b3b3b3] hover:text-white hover:bg-white/10 transition-colors cursor-pointer"
                      title="Siguiente pista"
                      @click="nextTrack()"
                    >
                      <KyoIcon name="skipNext" class="w-4 h-4" />
                    </button>
                    <button
                      type="button"
                      class="px-2 py-1.5 rounded-lg text-xs transition-colors cursor-pointer"
                      :class="currentTrack?.repeat_mode && currentTrack.repeat_mode !== 'none' ? 'text-[#1ED760] bg-[#1ED760]/15' : 'text-[#b3b3b3] hover:text-white'"
                      title="Alternar repetición (Repeat)"
                      @click="cycleRepeat()"
                    >
                      🔁 {{ currentTrack?.repeat_mode === 'track' ? '1' : '' }}
                    </button>
                    <button
                      type="button"
                      class="kyo-btn px-2.5 py-1.5 text-xs ml-1"
                      title="Traer ventana de música al frente"
                      @click="focusMusicApp()"
                    >
                      <KyoIcon name="window" class="w-3.5 h-3.5" />
                    </button>
                    <button
                      type="button"
                      class="kyo-btn px-2.5 py-1.5 text-xs text-indigo-300"
                      title="Mover el avatar de Kyo junto a la ventana de música"
                      @click="moveNearMusicApp(); handleClose()"
                    >
                      <KyoIcon name="move" class="w-3.5 h-3.5" />
                    </button>
                  </div>
                </div>

                <!-- Barra de progreso interactiva (Seek) -->
                <div v-if="(currentTrack?.duration_secs ?? 0) > 0" class="space-y-1 pt-1">
                  <input
                    type="range"
                    :min="0"
                    :max="currentTrack?.duration_secs ?? 100"
                    :value="currentTrack?.position_secs ?? 0"
                    class="w-full h-1.5 accent-[#1ED760] cursor-pointer"
                    @change="(e) => seekToSecs(Number((e.target as HTMLInputElement).value))"
                  />
                  <div class="flex justify-between text-[10px] font-mono text-[#a7a7a7]">
                    <span>{{ formatMmSs(currentTrack?.position_secs) }}</span>
                    <span>{{ formatMmSs(currentTrack?.duration_secs) }}</span>
                  </div>
                </div>
              </div>

              <!-- Fila de Reproducción (A continuación + Recientes) y Temporizador Inteligente -->
              <div class="pt-3 border-t kyo-divider space-y-3">
                <div class="flex flex-wrap items-center justify-between gap-2">
                  <div class="flex items-center gap-2">
                    <button
                      type="button"
                      class="px-3 py-1 rounded-lg text-xs font-semibold transition-colors cursor-pointer"
                      :class="activeQueueTab === 'up_next' ? 'bg-[#1ED760]/20 text-[#1ED760] border border-[#1ED760]/40' : 'bg-zinc-800/70 text-zinc-400'"
                      @click="activeQueueTab = 'up_next'"
                    >
                      Fila de reproducción ({{ upNextQueue.length }})
                    </button>
                    <button
                      type="button"
                      class="px-3 py-1 rounded-lg text-xs font-semibold transition-colors cursor-pointer"
                      :class="activeQueueTab === 'recent' ? 'bg-[#1ED760]/20 text-[#1ED760] border border-[#1ED760]/40' : 'bg-zinc-800/70 text-zinc-400'"
                      @click="activeQueueTab = 'recent'"
                    >
                      Escuchado recientemente ({{ recentTracks.length }})
                    </button>
                  </div>

                  <!-- Temporizador Musical (Sleep Timer) -->
                  <div class="flex items-center gap-1.5 text-xs">
                    <span class="text-[11px] text-zinc-400">⏱ Pausa automática:</span>
                    <button
                      type="button"
                      class="px-2 py-0.5 rounded-full text-[10px] font-mono border cursor-pointer"
                      :class="!sleepTimerRemaining ? 'bg-[#1ED760]/20 text-[#1ED760] border-[#1ED760]/40' : 'border-zinc-700 text-zinc-400'"
                      @click="setSleepTimerMinutes(0)"
                    >
                      Off
                    </button>
                    <button
                      v-for="m in [15, 30, 45, 60]"
                      :key="m"
                      type="button"
                      class="px-2 py-0.5 rounded-full text-[10px] font-mono border border-zinc-700 text-zinc-300 hover:border-[#1ED760] hover:text-[#1ED760] cursor-pointer"
                      @click="setSleepTimerMinutes(m)"
                    >
                      {{ m }}m
                    </button>
                    <span v-if="sleepTimerRemaining" class="text-[11px] font-mono text-[#1ED760] font-semibold ml-1">
                      ({{ formatMmSs(sleepTimerRemaining) }})
                    </span>
                  </div>
                </div>

                <!-- Buscador / Agregador a la Fila -->
                <div v-if="activeQueueTab === 'up_next'" class="space-y-2">
                  <div class="flex items-center gap-2">
                    <input
                      v-model="newQueueInput"
                      type="text"
                      placeholder="Añadir canción o artista a tu fila (ej. Daft Punk - Veridis Quo)..."
                      class="kyo-input flex-1 text-xs"
                      @keydown.enter="handleAddQueueItem"
                    />
                    <button
                      type="button"
                      class="px-3 py-1.5 rounded-xl bg-[#1ED760] hover:bg-[#3be477] text-black font-bold text-xs cursor-pointer transition-colors"
                      @click="handleAddQueueItem"
                    >
                      + Añadir a la fila
                    </button>
                    <button
                      v-if="newQueueInput.trim()"
                      type="button"
                      class="kyo-btn text-xs"
                      @click="playSpotifyQuery(newQueueInput); newQueueInput = ''"
                    >
                      ▶ Buscar en Spotify
                    </button>
                  </div>

                  <div v-if="upNextQueue.length > 0" class="max-h-40 overflow-y-auto space-y-1.5 pr-1 custom-scrollbar">
                    <div
                      v-for="(item, idx) in upNextQueue"
                      :key="item.id"
                      class="flex items-center justify-between p-2 rounded-xl bg-zinc-900/60 border border-zinc-800/80 text-xs"
                    >
                      <div class="flex items-center gap-2.5 min-w-0">
                        <span class="text-[11px] font-mono text-[#1ED760] w-4">{{ idx + 1 }}</span>
                        <div class="min-w-0">
                          <div class="font-semibold text-zinc-100 truncate">{{ item.title }}</div>
                          <div class="text-[10.5px] text-zinc-400 truncate">{{ item.artist }}</div>
                        </div>
                      </div>
                      <div class="flex items-center gap-1.5 shrink-0">
                        <button
                          type="button"
                          class="px-2 py-1 rounded bg-[#1ED760]/15 text-[#1ED760] hover:bg-[#1ED760]/25 text-[10px] font-semibold cursor-pointer"
                          @click="playSpotifyQuery(`${item.artist !== 'Spotify' ? item.artist + ' ' : ''}${item.title}`)"
                        >
                          ▶ Reproducir
                        </button>
                        <button
                          type="button"
                          class="px-2 py-1 rounded bg-zinc-800 text-zinc-400 hover:text-rose-400 text-[10px] cursor-pointer"
                          @click="removeFromQueue(item.id)"
                        >
                          Quitar
                        </button>
                      </div>
                    </div>
                    <div class="flex justify-end pt-1">
                      <button
                        type="button"
                        class="text-[11px] text-zinc-400 hover:text-rose-400 cursor-pointer"
                        @click="clearQueue()"
                      >
                        Vaciar fila de reproducción
                      </button>
                    </div>
                  </div>
                  <div v-else class="text-xs text-zinc-500 italic py-1">
                    Tu fila está vacía. Escribe cualquier canción arriba para programar tu siguiente pista en Spotify.
                  </div>
                </div>

                <!-- Historial de canciones recientes -->
                <div v-else class="space-y-1.5 max-h-44 overflow-y-auto pr-1 custom-scrollbar">
                  <div
                    v-for="(item, idx) in recentTracks"
                    :key="item.id"
                    class="flex items-center justify-between p-2 rounded-xl bg-zinc-900/60 border border-zinc-800/80 text-xs"
                  >
                    <div class="flex items-center gap-2.5 min-w-0">
                      <span class="text-[11px] font-mono text-zinc-500 w-4">{{ idx + 1 }}</span>
                      <img
                        v-if="item.thumbnail_data_uri"
                        :src="item.thumbnail_data_uri"
                        alt=""
                        class="w-7 h-7 rounded object-cover shrink-0"
                      />
                      <div class="min-w-0">
                        <div class="font-semibold text-zinc-100 truncate">{{ item.title }}</div>
                        <div class="text-[10.5px] text-zinc-400 truncate">{{ item.artist }} · {{ item.source_app_name }}</div>
                      </div>
                    </div>
                    <div class="flex items-center gap-1.5 shrink-0">
                      <button
                        type="button"
                        class="px-2 py-1 rounded bg-zinc-800 hover:bg-zinc-700 text-zinc-300 text-[10px] cursor-pointer"
                        @click="addToQueue(`${item.artist ? item.artist + ' - ' : ''}${item.title}`)"
                      >
                        + Fila
                      </button>
                      <button
                        type="button"
                        class="px-2 py-1 rounded bg-[#1ED760]/15 text-[#1ED760] hover:bg-[#1ED760]/25 text-[10px] font-semibold cursor-pointer"
                        @click="playSpotifyQuery(`${item.artist} ${item.title}`)"
                      >
                        ▶ Spotify
                      </button>
                    </div>
                  </div>
                  <div v-if="recentTracks.length === 0" class="text-xs text-zinc-500 italic py-1">
                    Las canciones que escuches en tu PC se guardarán automáticamente en este historial.
                  </div>
                </div>
              </div>

              <!-- Lista de Apps de Música detectadas en ejecución -->
              <div v-if="runningMusicApps.length > 0" class="pt-2 border-t kyo-divider space-y-2">
                <span class="text-xs font-medium text-neutral-300 block">
                  Aplicaciones multimedia detectadas en ejecución ({{ runningMusicApps.length }})
                </span>
                <div class="grid grid-cols-1 sm:grid-cols-2 gap-2">
                  <div
                    v-for="app in runningMusicApps"
                    :key="`${app.id}-${app.pid}`"
                    class="p-2.5 rounded-xl border border-zinc-800/80 bg-zinc-950/40 flex items-center justify-between gap-2 text-xs"
                  >
                    <div class="min-w-0">
                      <div class="flex items-center gap-1.5">
                        <span
                          class="w-2 h-2 rounded-full shrink-0"
                          :class="app.is_playing ? 'bg-emerald-400 animate-pulse' : 'bg-zinc-500'"
                        />
                        <span class="font-semibold text-zinc-200 truncate">{{ app.name }}</span>
                        <span class="text-[10px] font-mono text-zinc-500">({{ app.process_name }})</span>
                      </div>
                      <div class="text-[11px] text-zinc-400 truncate mt-0.5">
                        {{
                          app.track_title
                            ? `${app.track_artist ? `${app.track_artist} — ` : ''}${app.track_title}`
                            : app.window_title
                        }}
                      </div>
                    </div>
                    <div class="flex items-center gap-1 shrink-0">
                      <button
                        v-if="app.hwnd !== 0"
                        type="button"
                        class="px-2 py-1 rounded bg-zinc-800 hover:bg-zinc-700 text-[10px] text-zinc-300 transition-colors"
                        @click="focusMusicApp(app.id)"
                      >
                        Enfocar
                      </button>
                      <button
                        v-if="app.hwnd !== 0"
                        type="button"
                        class="px-2 py-1 rounded bg-indigo-600/20 hover:bg-indigo-600/30 text-[10px] text-indigo-300 transition-colors"
                        title="Mover a Kyo hacia esta ventana"
                        @click="moveNearMusicApp(app.id); handleClose()"
                      >
                        Ir allí
                      </button>
                    </div>
                  </div>
                </div>
              </div>

              <div v-else class="text-xs text-zinc-400 italic py-1">
                Abre Spotify, Apple Music, VLC, AIMP, Foobar2000 o YouTube Music en tu navegador para que Kyo lo detecte automáticamente.
              </div>
            </div>

            <!-- Tarjeta 2: Preferencias de Interacción y Reacción Musical de Kyo -->
            <div class="kyo-card p-4 space-y-4">
              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="kyo-label">Detectar música y aplicaciones como Spotify</label>
                  <span class="kyo-hint">Permite a Kyo saber si estás escuchando música en tu PC y qué pista suena.</span>
                </div>
                <input
                  v-model="settings.musicDetectionEnabled"
                  type="checkbox"
                  class="kyo-check"
                  @change="triggerSaveFeedback"
                />
              </div>

              <hr class="kyo-divider" />

              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="kyo-label">Bailar automáticamente con la música</label>
                  <span class="kyo-hint">
                    Kyo adapta su coreografía automáticamente según el género y estilo de la canción (Pop/Groove, Chill/Indie, Rock, Electro, Urbano, Latino).
                  </span>
                </div>
                <input
                  v-model="settings.musicAutoDance"
                  type="checkbox"
                  class="kyo-check"
                  :disabled="!settings.musicDetectionEnabled"
                  @change="triggerSaveFeedback"
                />
              </div>

              <hr class="kyo-divider" />

              <div class="flex items-center justify-between gap-4">
                <div>
                  <div class="flex items-center gap-2">
                    <label class="kyo-label mb-0">Estilo de animación de baile</label>
                    <span
                      class="text-[10px] px-2 py-0.5 rounded-full font-semibold"
                      :style="{
                        backgroundColor: `${currentDanceStyleMeta.color}20`,
                        color: currentDanceStyleMeta.color,
                      }"
                    >
                      {{ currentDanceStyleMeta.emoji }} Activo: {{ currentDanceStyleMeta.label }}
                    </span>
                  </div>
                  <span class="kyo-hint">
                    {{ currentDanceStyleMeta.description }}
                  </span>
                </div>
                <select
                  v-model="settings.musicDanceStyle"
                  class="kyo-input w-52"
                  :disabled="!settings.musicDetectionEnabled"
                  @change="triggerSaveFeedback"
                >
                  <option value="auto">✨ Automático por canción</option>
                  <option
                    v-for="meta in Object.values(DANCE_STYLE_META)"
                    :key="meta.id"
                    :value="meta.id"
                  >
                    {{ meta.emoji }} {{ meta.label }}
                  </option>
                </select>
              </div>

              <hr class="kyo-divider" />

              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="kyo-label">Mostrar mini-píldora interactiva sobre el avatar</label>
                  <span class="kyo-hint">
                    Muestra el título de la canción actual bajo Kyo con controles rápidos (Anterior, Play/Pausa, Siguiente) al pasar el ratón.
                  </span>
                </div>
                <input
                  v-model="settings.musicShowFloatingPill"
                  type="checkbox"
                  class="kyo-check"
                  :disabled="!settings.musicDetectionEnabled"
                  @change="triggerSaveFeedback"
                />
              </div>

              <hr class="kyo-divider" />

              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="kyo-label">Reaccionar al cambiar de canción</label>
                  <span class="kyo-hint">Kyo expresa alegría (<code class="font-mono text-indigo-400">joy</code>) brevemente cuando comienza una nueva pista.</span>
                </div>
                <input
                  v-model="settings.musicReactOnTrackChange"
                  type="checkbox"
                  class="kyo-check"
                  :disabled="!settings.musicDetectionEnabled"
                  @change="triggerSaveFeedback"
                />
              </div>

              <hr class="kyo-divider" />

              <div class="flex items-center justify-between gap-4">
                <div>
                  <label class="kyo-label">Mantener despierto mientras suena música</label>
                  <span class="kyo-hint">Evita que Kyo entre en sueño automático por inactividad si estás escuchando música.</span>
                </div>
                <input
                  v-model="settings.musicPreventSleep"
                  type="checkbox"
                  class="kyo-check"
                  :disabled="!settings.musicDetectionEnabled"
                  @change="triggerSaveFeedback"
                />
              </div>
            </div>
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

          <!-- CONEXIÓN KYO & MCP -->
          <section v-else-if="currentCategory === 'kyo'" class="space-y-4">
            <div>
              <h2 class="text-base font-bold tracking-tight">Conexión Kyo & Servidor MCP</h2>
              <p class="kyo-hint">Configuración del enlace del servidor web, validación de token JWT y diagnóstico del protocolo MCP.</p>
            </div>

            <!-- Tarjeta 1: Estado y Diagnóstico de Conectividad -->
            <div class="kyo-card p-4 space-y-4">
              <div class="flex items-center justify-between gap-4">
                <div>
                  <div class="flex items-center gap-2">
                    <label class="kyo-label">Estado de Conexión</label>
                    <span
                      class="px-2 py-0.5 rounded-full text-[10px] font-medium tracking-wide flex items-center gap-1.5"
                      :class="
                        kyoStatus === 'connected_local' || kyoStatus === 'connected_remote'
                          ? 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20'
                          : kyoStatus === 'fallback_cloud'
                          ? 'bg-amber-500/10 text-amber-400 border border-amber-500/20'
                          : kyoStatus === 'auth_error'
                          ? 'bg-rose-500/10 text-rose-400 border border-rose-500/20'
                          : kyoStatus === 'offline'
                          ? 'bg-rose-500/10 text-rose-400 border border-rose-500/20'
                          : 'bg-zinc-500/10 text-zinc-400 border border-zinc-500/20'
                      "
                    >
                      <span
                        class="w-1.5 h-1.5 rounded-full"
                        :class="
                          kyoStatus === 'connected_local' || kyoStatus === 'connected_remote'
                            ? 'bg-emerald-400 animate-pulse'
                            : kyoStatus === 'fallback_cloud'
                            ? 'bg-amber-400'
                            : kyoStatus === 'auth_error'
                            ? 'bg-rose-400 animate-pulse'
                            : kyoStatus === 'offline'
                            ? 'bg-rose-400'
                            : 'bg-indigo-400 animate-pulse'
                        "
                      ></span>
                      {{
                        kyoStatus === 'connected_local'
                          ? 'Local Sidecar (127.0.0.1)'
                          : kyoStatus === 'connected_remote'
                          ? 'Remoto Conectado & Autenticado'
                          : kyoStatus === 'fallback_cloud'
                          ? 'Servidor Remoto (Acceso Anónimo)'
                          : kyoStatus === 'auth_error'
                          ? 'Token Rechazado por Servidor (401)'
                          : kyoStatus === 'offline'
                          ? 'Servidor Inaccesible'
                          : 'Detectando conexión...'
                      }}
                    </span>
                  </div>
                  <span class="kyo-hint">
                    Endpoint activo: <code class="font-mono text-indigo-400">{{ activeApiBase }}</code>
                  </span>
                </div>
                <button
                  type="button"
                  class="kyo-btn text-xs"
                  :disabled="isVerifying"
                  @click="handleTestConnectionNow"
                >
                  <KyoIcon name="refresh" class="w-3.5 h-3.5" :class="{ 'animate-spin': isVerifying }" />
                  {{ isVerifying ? 'Comprobando...' : 'Probar Conexión' }}
                </button>
              </div>

              <hr class="kyo-divider" />

              <div class="grid grid-cols-3 gap-3 text-xs font-mono">
                <div class="p-2.5 rounded-xl border border-zinc-700/50 bg-zinc-900/40">
                  <span class="text-zinc-500 block text-[10px] uppercase">Puerto Local</span>
                  <span class="text-zinc-200 font-semibold">{{ sidecarPort ? `TCP :${sidecarPort}` : 'Inactivo' }}</span>
                </div>
                <div class="p-2.5 rounded-xl border border-zinc-700/50 bg-zinc-900/40">
                  <span class="text-zinc-500 block text-[10px] uppercase">Latencia Roundtrip</span>
                  <span class="text-zinc-200 font-semibold">{{ latencyMs !== null ? `${latencyMs} ms` : '-- ms' }}</span>
                </div>
                <div class="p-2.5 rounded-xl border border-zinc-700/50 bg-zinc-900/40">
                  <span class="text-zinc-500 block text-[10px] uppercase">Respuesta Servidor</span>
                  <span class="text-zinc-200 font-semibold truncate">
                    {{
                      lastProbeResult?.httpCode
                        ? `HTTP ${lastProbeResult.httpCode}`
                        : healthData?.status === 'ok'
                        ? 'HTTP 200 (OK)'
                        : 'En espera'
                    }}
                  </span>
                </div>
              </div>

              <!-- Mensaje de Diagnóstico Detallado -->
              <div
                v-if="lastProbeResult?.message"
                class="p-2.5 rounded-xl text-xs flex items-center gap-2 border font-mono"
                :class="
                  lastProbeResult.reachable && (lastProbeResult.authenticated || !jwtToken)
                    ? 'bg-emerald-500/10 border-emerald-500/20 text-emerald-300'
                    : lastProbeResult.status === 'invalid_token'
                    ? 'bg-amber-500/10 border-amber-500/20 text-amber-300'
                    : 'bg-rose-500/10 border-rose-500/20 text-rose-300'
                "
              >
                <KyoIcon
                  :name="lastProbeResult.reachable ? (lastProbeResult.authenticated ? 'check' : 'info') : 'warning'"
                  class="w-4 h-4 shrink-0"
                />
                <span class="text-[11px] leading-tight">{{ lastProbeResult.message }}</span>
              </div>
            </div>

            <!-- Tarjeta 2: Cuenta de Usuario Conectada -->
            <div
              v-if="accountInfo?.username || isConnected"
              class="kyo-card p-4 space-y-3 border-indigo-500/30 bg-gradient-to-br from-indigo-950/20 to-zinc-900/60"
            >
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-2">
                  <KyoIcon name="user" class="w-4 h-4 text-indigo-400" />
                  <h3 class="kyo-card-title">Cuenta de Usuario Conectada</h3>
                </div>
                <span
                  v-if="accountInfo?.role"
                  class="px-2 py-0.5 rounded-full text-[10px] font-mono uppercase tracking-wider bg-indigo-500/10 text-indigo-300 border border-indigo-500/20"
                >
                  Rol: {{ accountInfo.role }}
                </span>
              </div>

              <div class="flex items-center gap-3.5 p-3 rounded-xl bg-zinc-900/80 border border-zinc-700/50">
                <!-- Avatar / Inicial -->
                <div
                  class="w-10 h-10 rounded-xl bg-indigo-600/20 border border-indigo-500/30 flex items-center justify-center text-indigo-300 font-bold text-base shrink-0 shadow-sm"
                >
                  {{ (accountInfo?.name || accountInfo?.username || 'K')[0].toUpperCase() }}
                </div>

                <div class="flex-1 min-w-0">
                  <div class="flex items-center gap-2">
                    <span class="text-sm font-semibold text-zinc-100 truncate">
                      {{ accountInfo?.name || accountInfo?.username || 'Usuario Kyō' }}
                    </span>
                    <span
                      v-if="accountInfo?.username"
                      class="text-xs text-indigo-400 font-mono"
                    >
                      @{{ accountInfo.username }}
                    </span>
                  </div>
                  <div class="flex items-center gap-2 text-[11px] text-zinc-400 truncate mt-0.5">
                    <span v-if="accountInfo?.email">{{ accountInfo.email }}</span>
                    <span
                      v-if="accountInfo?.emailVerified"
                      class="inline-flex items-center gap-1 text-[10px] text-emerald-400 font-medium"
                    >
                      <KyoIcon name="check" class="w-3 h-3" />
                      Verificado
                    </span>
                  </div>
                </div>

                <div v-if="accountInfo?.id" class="text-right shrink-0">
                  <span class="text-[10px] text-zinc-500 block uppercase font-mono">ID Cuenta</span>
                  <span class="text-xs font-mono text-zinc-300 font-semibold">{{ accountInfo.id }}</span>
                </div>
              </div>
            </div>

            <!-- Tarjeta 3: Perfiles de Kyō Web (Multi-Perfil / Bóvedas de Trabajo) -->
            <div class="kyo-card p-4 space-y-3.5">
              <div class="flex items-center justify-between">
                <div>
                  <div class="flex items-center gap-2">
                    <KyoIcon name="folder" class="w-4 h-4 text-purple-400" />
                    <h3 class="kyo-card-title">Perfil Activo de Kyō Web (Multi-Perfil)</h3>
                  </div>
                  <span class="kyo-hint">
                    Nuestra plataforma web permite tener múltiples perfiles/bóvedas dentro de una misma cuenta. Selecciona el perfil con el que trabajará Kyō:
                  </span>
                </div>

                <span
                  v-if="activeProfile?.name"
                  class="px-2.5 py-1 rounded-full text-xs font-medium bg-purple-500/10 text-purple-300 border border-purple-500/20 font-mono shrink-0 flex items-center gap-1.5"
                >
                  <span class="w-1.5 h-1.5 rounded-full bg-purple-400 animate-pulse" />
                  Activo: {{ activeProfile.name }}
                </span>
              </div>

              <!-- Listado interactivo de perfiles disponibles -->
              <div v-if="availableProfiles.length > 0" class="grid grid-cols-1 sm:grid-cols-2 gap-2.5 pt-1">
                <button
                  v-for="prof in availableProfiles"
                  :key="prof.id"
                  type="button"
                  class="p-3 rounded-xl border text-left transition-all flex items-start justify-between gap-3 cursor-pointer group"
                  :class="
                    (activeProfileId === prof.id || (!activeProfileId && availableProfiles[0]?.id === prof.id))
                      ? 'bg-purple-950/30 border-purple-500/60 shadow-sm shadow-purple-500/10 ring-1 ring-purple-500/30'
                      : 'bg-zinc-900/50 border-zinc-800 hover:border-zinc-700 hover:bg-zinc-900/80'
                  "
                  @click="setActiveProfile(prof.id)"
                >
                  <div class="flex items-start gap-2.5 min-w-0">
                    <span class="text-xl shrink-0 p-1 rounded-lg bg-zinc-800/80 border border-zinc-700/40">
                      {{ prof.icon || '🗄️' }}
                    </span>
                    <div class="min-w-0">
                      <div class="flex items-center gap-1.5">
                        <span class="text-xs font-semibold text-zinc-100 truncate group-hover:text-purple-300 transition-colors">
                          {{ prof.name }}
                        </span>
                      </div>
                      <p v-if="prof.description" class="text-[11px] text-zinc-400 truncate mt-0.5">
                        {{ prof.description }}
                      </p>
                      <div class="flex items-center gap-2 mt-1 text-[10px] text-zinc-500 font-mono">
                        <span v-if="prof.fileCount !== undefined">{{ prof.fileCount }} recursos</span>
                        <span v-if="prof.provider">vía {{ prof.provider }}</span>
                      </div>
                    </div>
                  </div>

                  <span
                    v-if="activeProfileId === prof.id || (!activeProfileId && availableProfiles[0]?.id === prof.id)"
                    class="px-2 py-0.5 rounded text-[10px] font-semibold bg-purple-500/20 text-purple-300 border border-purple-500/30 shrink-0"
                  >
                    ✓ ACTIVO
                  </span>
                  <span
                    v-else
                    class="opacity-0 group-hover:opacity-100 px-2 py-0.5 rounded text-[10px] font-medium bg-zinc-800 text-zinc-300 border border-zinc-700 shrink-0 transition-opacity"
                  >
                    Seleccionar
                  </span>
                </button>
              </div>

              <div
                v-else
                class="p-3 rounded-xl border border-zinc-800/80 bg-zinc-900/30 text-xs text-zinc-400 flex items-center justify-between"
              >
                <span class="text-[11px]">
                  {{ isConnected ? 'Buscando perfiles asociados a tu cuenta...' : 'Conéctate con tu token para cargar tus perfiles de la web.' }}
                </span>
                <button
                  v-if="isConnected"
                  type="button"
                  class="kyo-chip text-[10px] py-1 px-2"
                  @click="handleTestConnectionNow"
                >
                  Sincronizar
                </button>
              </div>
            </div>

            <!-- Tarjeta 4: Link / URL de Conexión del Servidor MCP -->
            <div class="kyo-card p-4 space-y-3">
              <div class="flex items-center justify-between">
                <div>
                  <label class="kyo-label">Link de Conexión MCP / Aplicación Web</label>
                  <span class="kyo-hint">
                    URL base del servidor remoto que aloja los endpoints MCP y el motor de inferencia.
                  </span>
                </div>
                <button
                  v-if="serverUrlDraft !== 'https://kyo.broco.dev'"
                  type="button"
                  class="kyo-chip text-[11px] py-1 px-2 text-zinc-400 hover:text-zinc-200"
                  title="Restablecer a https://kyo.broco.dev"
                  @click="handleResetUrl"
                >
                  Restablecer Oficial
                </button>
              </div>

              <div class="space-y-1.5">
                <div class="relative flex items-center">
                  <span class="absolute left-3 text-zinc-500 pointer-events-none">
                    <KyoIcon name="link" class="w-3.5 h-3.5" />
                  </span>
                  <input
                    v-model="serverUrlDraft"
                    type="url"
                    placeholder="https://kyo.broco.dev"
                    class="w-full text-xs font-mono pl-9 pr-3 py-2 rounded-xl bg-zinc-900 border text-zinc-100 placeholder-zinc-500 focus:outline-none focus:ring-1 transition-colors"
                    :class="
                      liveUrlValidation.valid
                        ? 'border-zinc-700/70 focus:border-indigo-500 focus:ring-indigo-500/30'
                        : 'border-rose-500/80 focus:border-rose-500 focus:ring-rose-500/30'
                    "
                  />
                </div>

                <p v-if="!liveUrlValidation.valid" class="text-[11px] text-rose-400 font-mono">
                  {{ liveUrlValidation.error }}
                </p>
              </div>
            </div>

            <!-- Tarjeta 3: Inicio de Sesión OAuth 2.1 (Navegador) y Token Manual -->
            <div class="kyo-card p-4 space-y-4">
              <div class="flex items-start justify-between gap-3">
                <div>
                  <div class="flex items-center gap-2">
                    <label class="kyo-label">Sesión de Cuenta Kyō (OAuth 2.1 + PKCE)</label>
                    <span
                      v-if="authMethod === 'oauth' && jwtToken"
                      class="px-2 py-0.5 rounded-full text-[10px] font-medium bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 font-mono"
                    >
                      OAuth Activo{{ hasRefreshToken ? ' · Auto-refresh' : '' }}
                    </span>
                    <span
                      v-else-if="liveJwtValidation.valid"
                      class="px-2 py-0.5 rounded-full text-[10px] font-medium bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 font-mono"
                    >
                      ✓ {{ tokenDraft.trim().startsWith('kyo_mcp_') ? 'Token MCP' : tokenDraft.trim().startsWith('kyo_link_') ? 'Token Proyecto' : 'Token Válido' }}
                    </span>
                  </div>
                  <span class="kyo-hint">
                    Inicia sesión directamente en tu navegador sin copiar ni pegar tokens. Renovación automática de sesión integrada.
                  </span>
                </div>
              </div>

              <!-- Botones principales de OAuth -->
              <div class="flex flex-wrap items-center gap-2.5">
                <button
                  type="button"
                  class="kyo-btn text-xs font-semibold px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white shadow-md shadow-indigo-600/20"
                  :disabled="isLoggingIn || !liveUrlValidation.valid"
                  @click="handleOAuthLogin"
                >
                  <KyoIcon name="sparkles" class="w-3.5 h-3.5" :class="{ 'animate-spin': isLoggingIn }" />
                  {{ isLoggingIn ? 'Esperando autorización en navegador...' : jwtToken ? 'Reconectar con Navegador' : 'Iniciar sesión con Kyō' }}
                </button>

                <button
                  v-if="jwtToken"
                  type="button"
                  class="kyo-btn text-xs text-rose-300 border-rose-500/30 hover:bg-rose-500/10"
                  @click="handleOAuthLogout"
                >
                  <KyoIcon name="power" class="w-3.5 h-3.5" />
                  Cerrar sesión
                </button>

                <button
                  type="button"
                  class="kyo-chip text-[11px] py-1.5 px-2.5 text-zinc-400 hover:text-zinc-200 ml-auto"
                  @click="showManualToken = !showManualToken"
                >
                  {{ showManualToken ? 'Ocultar token manual' : 'Usar token manual (kyo_mcp_ / JWT)' }}
                </button>
              </div>

              <div v-if="accessExpiresAt && jwtToken" class="text-[11px] font-mono text-zinc-400">
                Vigencia del access token: <span class="text-zinc-200">{{ new Date(accessExpiresAt).toLocaleString() }}</span>
              </div>

              <!-- Sección avanzada colapsable para Token manual -->
              <div v-if="showManualToken" class="space-y-3 pt-3 border-t border-zinc-800/80">
                <div class="flex items-center justify-between">
                  <span class="text-xs font-medium text-zinc-300">Token manual (kyo_mcp_…, kyo_link_… o JWT)</span>
                  <span
                    v-if="liveJwtValidation.isExpired"
                    class="px-2 py-0.5 rounded-full text-[10px] font-medium bg-rose-500/10 text-rose-400 border border-rose-500/20 font-mono"
                  >
                    ✗ Token Expirado
                  </span>
                  <span
                    v-else-if="tokenDraft && !liveJwtValidation.valid"
                    class="px-2 py-0.5 rounded-full text-[10px] font-medium bg-amber-500/10 text-amber-400 border border-amber-500/20 font-mono"
                  >
                    ! Formato No Válido
                  </span>
                </div>

              <div class="space-y-2">
                <div class="relative flex items-center">
                  <span class="absolute left-3 text-zinc-500 pointer-events-none">
                    <KyoIcon name="key" class="w-3.5 h-3.5" />
                  </span>
                  <input
                    v-model="tokenDraft"
                    :type="showToken ? 'text' : 'password'"
                    placeholder="Pega aquí tu Token Key (kyo_mcp_..., kyo_link_... o eyJhbGci...)"
                    class="w-full text-xs font-mono pl-9 pr-10 py-2 rounded-xl bg-zinc-900 border text-zinc-100 placeholder-zinc-500 focus:outline-none focus:ring-1 transition-colors"
                    :class="
                      !tokenDraft
                        ? 'border-zinc-700/70 focus:border-indigo-500 focus:ring-indigo-500/30'
                        : liveJwtValidation.valid
                        ? 'border-emerald-500/50 focus:border-emerald-500 focus:ring-emerald-500/20'
                        : liveJwtValidation.isExpired
                        ? 'border-rose-500/80 focus:border-rose-500 focus:ring-rose-500/20'
                        : 'border-amber-500/70 focus:border-amber-500 focus:ring-amber-500/20'
                    "
                  />
                  <button
                    type="button"
                    class="absolute right-2.5 text-zinc-400 hover:text-zinc-200 p-1 text-xs"
                    :title="showToken ? 'Ocultar token' : 'Mostrar token'"
                    @click="showToken = !showToken"
                  >
                    <KyoIcon :name="showToken ? 'eye' : 'close'" class="w-3.5 h-3.5" />
                  </button>
                </div>

                <!-- Caja de Análisis de Seguridad del Token -->
                <div
                  v-if="tokenDraft"
                  class="p-3 rounded-xl border text-xs font-mono space-y-1.5 transition-all"
                  :class="
                    liveJwtValidation.valid
                      ? 'bg-zinc-900/60 border-zinc-700/60 text-zinc-300'
                      : liveJwtValidation.isExpired
                      ? 'bg-rose-950/20 border-rose-500/30 text-rose-300'
                      : 'bg-amber-950/20 border-amber-500/30 text-amber-300'
                  "
                >
                  <div class="flex items-center justify-between text-[11px]">
                    <span class="text-zinc-400">Estado de Expiración:</span>
                    <span
                      class="font-semibold"
                      :class="liveJwtValidation.valid ? 'text-emerald-400' : 'text-rose-400'"
                    >
                      {{
                        liveJwtValidation.isExpired
                          ? `Expirado (${liveJwtValidation.expiresAtFormatted})`
                          : liveJwtValidation.remainingDays !== undefined
                          ? `Vigente • Expira en ${liveJwtValidation.remainingDays} días (${liveJwtValidation.expiresAtFormatted})`
                          : 'Sin fecha de expiración'
                      }}
                    </span>
                  </div>

                  <div v-if="liveJwtValidation.claims?.userId" class="flex items-center justify-between text-[11px]">
                    <span class="text-zinc-400">ID Usuario / Sujeto:</span>
                    <span class="text-zinc-200 font-semibold">{{ liveJwtValidation.claims.userId }}</span>
                  </div>

                  <div v-if="liveJwtValidation.claims?.role" class="flex items-center justify-between text-[11px]">
                    <span class="text-zinc-400">Rol de Autorización:</span>
                    <span class="text-indigo-300 uppercase tracking-wider font-semibold">{{ liveJwtValidation.claims.role }}</span>
                  </div>

                  <div v-if="liveJwtValidation.error && !liveJwtValidation.isExpired" class="text-rose-400 text-[11px]">
                    {{ liveJwtValidation.error }}
                  </div>
                </div>

                <div v-else class="text-[11px] text-zinc-500 font-mono py-0.5">
                  Sin token configurado (acceso anónimo / solo lectura local).
                </div>
              </div>

              <div class="flex items-center justify-end pt-1">
                <button
                  type="button"
                  class="kyo-btn text-xs font-semibold px-4 py-2 bg-zinc-800 hover:bg-zinc-700 text-zinc-100"
                  :disabled="isSavingConnection || !liveUrlValidation.valid"
                  @click="handleSaveAndVerifyConnection"
                >
                  <KyoIcon name="check" class="w-3.5 h-3.5" :class="{ 'animate-spin': isSavingConnection }" />
                  {{ isSavingConnection ? 'Verificando...' : 'Guardar Token Manual' }}
                </button>
              </div>
              </div>

              <!-- Mensaje de Feedback de Guardado -->
              <div
                v-if="saveFeedback"
                class="p-2.5 rounded-xl text-xs font-mono flex items-center gap-2 border"
                :class="
                  saveFeedback.success
                    ? 'bg-emerald-500/10 border-emerald-500/20 text-emerald-300'
                    : 'bg-rose-500/10 border-rose-500/20 text-rose-300'
                "
              >
                <KyoIcon :name="saveFeedback.success ? 'check' : 'warning'" class="w-3.5 h-3.5 shrink-0" />
                <span>{{ saveFeedback.message }}</span>
              </div>

              <!-- Pie informativo -->
              <div class="flex items-center justify-between pt-2 border-t border-zinc-800">
                <span class="text-[11px] text-zinc-500 font-mono">
                  Persistencia segura cifrada en <code class="text-zinc-400">session.json</code>
                </span>
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
