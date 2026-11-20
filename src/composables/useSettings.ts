import { computed, reactive, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';

export type WindowLayerMode = 'top' | 'normal' | 'bottom' | 'behind_active';
export type MusicDanceStylePreference =
  | 'auto'
  | 'groove'
  | 'chill'
  | 'rock'
  | 'electro'
  | 'urban'
  | 'latin';

export interface AppSettings {
  // General
  theme: 'dark' | 'light' | 'system';
  fontFamily: string;
  language: 'es' | 'en' | 'fr';
  startWithSystem: boolean;
  alwaysOnTop: boolean;
  windowLayer: WindowLayerMode;

  // Desplazamiento y Deambulación libre por el escritorio
  freeRoamEnabled: boolean;
  roamIntervalSeconds: number;
  roamSpeed: 'slow' | 'normal' | 'fast';
  roamMode: 'full' | 'edges' | 'curious';

  // Avatar & Aspecto
  avatarSize: number;
  shape: string;
  color: string;
  eyeColor: string;
  expression: string;

  // Comportamiento & Mirada
  globalTracking: boolean;
  trackingSensitivity: number;
  idleTimeout: number; // seconds before natural breathing
  followCursor: boolean;

  // Notificaciones & Efectos
  showNotificationBadge: boolean;
  orbitEffects: boolean;
  burstParticles: boolean;

  // Volumen & Interacciones (Paridad Web Kyo)
  volumetric: boolean;
  interactivePet: boolean;
  interactiveJelly: boolean;
  interactiveFlinch: boolean;
  autoSleepEnabled: boolean;
  nightInactivitySleepMinutes: number;
  dayInactivitySleepMinutes: number;

  // Música & Detección de Aplicaciones Multimedia (Spotify, SMTC, VLC, etc.)
  musicDetectionEnabled: boolean;
  musicAutoDance: boolean;
  musicDanceStyle: MusicDanceStylePreference;
  musicShowFloatingPill: boolean;
  musicReactOnTrackChange: boolean;
  musicPreventSleep: boolean;

  // Avanzado
  hardwareAcceleration: boolean;
  debugMode: boolean;
}

const STORAGE_KEY = 'kyo_agent_settings_v1';

const defaultSettings: AppSettings = {
  theme: 'system',
  fontFamily: 'default',
  language: 'es',
  startWithSystem: false,
  alwaysOnTop: true,
  windowLayer: 'top',

  freeRoamEnabled: false,
  roamIntervalSeconds: 30,
  roamSpeed: 'normal',
  roamMode: 'full',

  avatarSize: 170,
  shape: 'cercle',
  color: 'encre',
  eyeColor: '#ffffff',
  expression: 'neutre',

  globalTracking: true,
  trackingSensitivity: 1.0,
  idleTimeout: 3.5,
  followCursor: true,

  showNotificationBadge: false,
  orbitEffects: true,
  burstParticles: true,

  volumetric: true,
  interactivePet: true,
  interactiveJelly: true,
  interactiveFlinch: true,
  autoSleepEnabled: true,
  nightInactivitySleepMinutes: 6,
  dayInactivitySleepMinutes: 20,

  musicDetectionEnabled: true,
  musicAutoDance: true,
  musicDanceStyle: 'auto',
  musicShowFloatingPill: true,
  musicReactOnTrackChange: true,
  musicPreventSleep: true,

  hardwareAcceleration: true,
  debugMode: false,
};

function loadSettings(): AppSettings {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved) {
      const parsed = JSON.parse(saved);
      const windowLayer = parsed.windowLayer ?? (parsed.alwaysOnTop === false ? 'normal' : 'top');
      return { ...defaultSettings, ...parsed, windowLayer };
    }
  } catch (e) {
    console.error('Error loading settings:', e);
  }
  return { ...defaultSettings };
}

export const settings = reactive<AppSettings>(loadSettings());

if (typeof window !== 'undefined') {
  // Sincronizar capa de ventana inicial en el backend
  invoke('set_avatar_layer', { layer: settings.windowLayer }).catch(() => {});

  watch(
    () => settings.windowLayer,
    (newLayer) => {
      settings.alwaysOnTop = newLayer === 'top';
      invoke('set_avatar_layer', { layer: newLayer }).catch((err) => {
        console.error('Error setting avatar layer:', err);
      });
    }
  );

  watch(
    () => settings.alwaysOnTop,
    (val) => {
      if (val && settings.windowLayer !== 'top') {
        settings.windowLayer = 'top';
      } else if (!val && settings.windowLayer === 'top') {
        settings.windowLayer = 'normal';
      }
    }
  );
}

// Auto-guardado con 250 ms de rebote: evita bloquear el hilo principal en cada
// tic de deslizador o pulsacion rapida sin dejar de sentirse instantaneo.
let saveDebounce: ReturnType<typeof setTimeout> | null = null;
watch(
  settings,
  (newVal) => {
    if (saveDebounce) clearTimeout(saveDebounce);
    saveDebounce = setTimeout(() => {
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...newVal }));
      } catch (e) {
        console.error('Error saving settings:', e);
      }
    }, 250);
  },
  { deep: true }
);

/**
 * Preferencia del sistema, escuchada una sola vez para todo el proceso.
 *
 * El tema resuelto se expone como un unico booleano: la interfaz aplica la
 * clase `dark` en la raiz del panel y todo lo demas cuelga de variantes CSS
 * estaticas. Antes cada nodo llevaba su propio ternario `theme === 'light'`,
 * asi que un cambio cualquiera de ajustes obligaba a reevaluar decenas de
 * expresiones de clase en el mismo cuadro.
 */
const prefersDark = ref(true);

if (typeof window !== 'undefined' && typeof window.matchMedia === 'function') {
  const query = window.matchMedia('(prefers-color-scheme: dark)');
  prefersDark.value = query.matches;
  query.addEventListener('change', (e) => {
    prefersDark.value = e.matches;
  });
}

export const isDark = computed(() =>
  settings.theme === 'system' ? prefersDark.value : settings.theme === 'dark'
);

export function applyFontFamily(font: string) {
  if (typeof document === 'undefined') return;
  const sanitized = (font || '').replace(/['";\\<>]/g, '').trim();
  const fontValue = (!sanitized || sanitized === 'default' || sanitized === 'Kyo')
    ? "'Kyo', system-ui, -apple-system, sans-serif"
    : `'${sanitized}', 'Kyo', system-ui, -apple-system, sans-serif`;

  document.documentElement.style.setProperty('--kyo-font-family', fontValue);
  if (document.body) {
    document.body.style.setProperty('--kyo-font-family', fontValue);
  }

  // Inyectar o actualizar etiqueta de estilo global para garantizar máxima prioridad
  let styleTag = document.getElementById('kyo-dynamic-font-override') as HTMLStyleElement | null;
  if (!styleTag) {
    styleTag = document.createElement('style');
    styleTag.id = 'kyo-dynamic-font-override';
    document.head.appendChild(styleTag);
  }
  styleTag.textContent = `
    :root, html, body, #app, .kyo-panel {
      --kyo-font-family: ${fontValue} !important;
      font-family: ${fontValue} !important;
    }
    *, *::before, *::after {
      font-family: inherit;
    }
    code, kbd, samp, pre, .kyo-value {
      font-family: ui-monospace, SFMono-Regular, Menlo, monospace !important;
    }
  `;
}

// Inicializar y observar la fuente activa
if (typeof window !== 'undefined') {
  applyFontFamily(settings.fontFamily);

  // Sincronizar con el archivo de configuración en disco (%APPDATA%/kyo/settings.json)
  invoke<string>('get_font_preference')
    .then((savedDiskFont) => {
      if (savedDiskFont && savedDiskFont !== 'Kyo' && settings.fontFamily === 'default') {
        settings.fontFamily = savedDiskFont;
        applyFontFamily(savedDiskFont);
      }
    })
    .catch(() => {});

  watch(
    () => settings.fontFamily,
    (newFont) => {
      applyFontFamily(newFont);
      // Guardar de inmediato en localStorage
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...settings, fontFamily: newFont }));
      } catch (e) {}
      // Guardar en archivo persistente de disco en segundo plano
      try {
        invoke('save_font_preference', { font: newFont }).catch(() => {});
      } catch (e) {}
    }
  );
}

export function resetSettings() {
  Object.assign(settings, defaultSettings);
  applyFontFamily(defaultSettings.fontFamily);
}
