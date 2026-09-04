import { computed, reactive, ref, watch } from 'vue';

export interface AppSettings {
  // General
  theme: 'dark' | 'light' | 'system';
  language: 'es' | 'en' | 'fr';
  startWithSystem: boolean;
  alwaysOnTop: boolean;

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

  // Avanzado
  hardwareAcceleration: boolean;
  debugMode: boolean;
}

const STORAGE_KEY = 'kyo_agent_settings_v1';

const defaultSettings: AppSettings = {
  theme: 'dark',
  language: 'es',
  startWithSystem: false,
  alwaysOnTop: true,

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

  hardwareAcceleration: true,
  debugMode: false,
};

function loadSettings(): AppSettings {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved) {
      return { ...defaultSettings, ...JSON.parse(saved) };
    }
  } catch (e) {
    console.error('Error loading settings:', e);
  }
  return { ...defaultSettings };
}

export const settings = reactive<AppSettings>(loadSettings());

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

export function resetSettings() {
  Object.assign(settings, defaultSettings);
}
