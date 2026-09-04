import { reactive, watch } from 'vue';

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

// Auto-save with 250ms debounce: avoids blocking the main thread on every
// slider tick or rapid keypress while still feeling instant to the user.
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

export function resetSettings() {
  Object.assign(settings, defaultSettings);
}
