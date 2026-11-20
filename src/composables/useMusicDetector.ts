import { ref, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { settings } from './useSettings';
import type { StateId } from '@/bot/states';

export type MusicDanceStyle =
  | 'groove'
  | 'chill'
  | 'rock'
  | 'electro'
  | 'urban'
  | 'latin';

export interface DanceStyleMeta {
  id: MusicDanceStyle;
  stateId: StateId;
  label: string;
  shortLabel: string;
  emoji: string;
  color: string;
  description: string;
}

export const DANCE_STYLE_META: Record<MusicDanceStyle, DanceStyleMeta> = {
  groove: {
    id: 'groove',
    stateId: 'dance',
    label: 'Pop & Groove',
    shortLabel: 'Groove',
    emoji: '🎵',
    color: '#818cf8',
    description: 'Rebote alegre, balanceo rítmico y pasos dinámicos de 4 tiempos.',
  },
  chill: {
    id: 'chill',
    stateId: 'dance-chill',
    label: 'Chill & Indie',
    shortLabel: 'Chill',
    emoji: '🌊',
    color: '#38bdf8',
    description: 'Flotación suave en ocho (Lissajous), mirada soñadora y notas relajadas.',
  },
  rock: {
    id: 'rock',
    stateId: 'dance-rock',
    label: 'Rock & Headbang',
    shortLabel: 'Rock',
    emoji: '🎸',
    color: '#f97316',
    description: 'Headbang intenso al compás de la batería, mirada feroz y destellos eléctricos.',
  },
  electro: {
    id: 'electro',
    stateId: 'dance-electro',
    label: 'Electro & Club',
    shortLabel: 'Electro',
    emoji: '⚡',
    color: '#06b6d4',
    description: 'Pulso de bajo electrónico, anillos neón orbitales y giros sintéticos.',
  },
  urban: {
    id: 'urban',
    stateId: 'dance-urban',
    label: 'Urbano & Flow',
    shortLabel: 'Urbano',
    emoji: '🔥',
    color: '#a855f7',
    description: 'Bounce urbano con inclinación de hombros, mirada confiada y ritmo grave.',
  },
  latin: {
    id: 'latin',
    stateId: 'dance-latin',
    label: 'Ritmo Latino',
    shortLabel: 'Latino',
    emoji: '💃',
    color: '#fb7185',
    description: 'Paso sincopado de salsa/cumbia con cadera lateral y giros festivos.',
  },
};

const STYLE_PATTERNS: Array<{ style: MusicDanceStyle; terms: string[] }> = [
  {
    style: 'chill',
    terms: [
      'lofi', 'lo-fi', 'chill', 'indie', 'dream pop', 'shoegaze', 'ambient',
      'acoustic', 'acústico', 'acustico', 'unplugged', 'piano', 'jazz', 'blues',
      'bossa', 'classical', 'clásica', 'orchestra', 'symphony', 'sonata', 'nocturne',
      'slowed', 'reverb', 'ballad', 'balada', 'relax', 'study', 'sleep', 'soundtrack',
      "her's", 'hers ', 'blue lips', 'mac demarco', 'clairo', 'men i trust',
      'beach house', 'cigarettes after sex', 'tame impala', 'lana del rey', 'joji',
      'billie eilish', 'radiohead', 'mitski', 'phoebe bridgers', 'bon iver', 'laufey',
      'norah jones', 'frank sinatra', 'chet baker', 'hans zimmer', 'ludovico einaudi',
      'chopin', 'debussy', 'beethoven', 'mozart', 'cuco', 'boy pablo', 'vacations',
      'tv girl', 'current joys', 'duster', 'mazzy star', 'the marías', 'the marias',
      'daniel caesar', 'frank ocean', 'sza', 'kali uchis', 'rex orange county',
    ],
  },
  {
    style: 'rock',
    terms: [
      'rock', 'metal', 'punk', 'grunge', 'hardcore', 'heavy', 'thrash', 'emo',
      'riff', 'guitar', 'metallica', 'iron maiden', 'megadeth', 'slayer',
      'black sabbath', 'ozzy', 'ac/dc', 'acdc', 'guns n', 'led zeppelin',
      'pink floyd', 'queen', 'nirvana', 'foo fighters', 'pearl jam', 'soundgarden',
      'alice in chains', 'red hot chili', 'green day', 'blink-182', 'offspring',
      'sum 41', 'my chemical romance', 'paramore', 'fall out boy', 'linkin park',
      'system of a down', 'slipknot', 'korn', 'deftones', 'rammstein', 'avenged',
      'bring me the horizon', 'muse', 'arctic monkeys', 'the strokes', 'oasis',
      'blur', 'the killers', 'franz ferdinand', 'interpol', 'evanescence',
      'three days grace', 'skillet', 'disturbed', 'ghost', 'polyphia', 'soda stereo',
      'heroes del silencio', 'héroes del silencio', 'caifanes', 'molotov',
      'rata blanca', 'cuarteto de nos', 'charly garcia', 'spinetta', 'cerati',
      'enanitos verdes', 'hombres g', 'fito paez', 'los prisioneros', 'los bunkers',
    ],
  },
  {
    style: 'electro',
    terms: [
      'electro', 'edm', 'house', 'techno', 'trance', 'dubstep', 'dnb',
      'drum and bass', 'drum & bass', 'phonk', 'synth', 'synthwave', 'cyberpunk',
      'hardstyle', 'rave', 'club', 'remix', 'vip mix', 'eurobeat', 'nightcore',
      'hyperpop', 'glitch', 'vocaloid', 'daft punk', 'deadmau5', 'skrillex',
      'avicii', 'calvin harris', 'david guetta', 'tiësto', 'tiesto', 'martin garrix',
      'marshmello', 'alan walker', 'kygo', 'zedd', 'porter robinson', 'madeon',
      'flume', 'odesza', 'disclosure', 'fred again', 'peggy gou', 'charlotte de witte',
      'boris brejcha', 'eric prydz', 'fisher', 'gesaffelstein', 'justice', 'kavinsky',
      'aphex twin', 'prodigy', 'chemical brothers', 'fatboy slim', 'pendulum',
      'sub focus', 'kordhell', 'interworld', 'hensonn', 'hatsune miku', 'camellia',
    ],
  },
  {
    style: 'urban',
    terms: [
      'hip hop', 'hip-hop', 'rap', 'trap', 'drill', 'r&b', 'rnb', 'reggaeton',
      'reggaetón', 'dembow', 'perreo', 'freestyle', 'boom bap', 'urbano',
      'bad bunny', 'feid', 'karol g', 'j balvin', 'daddy yankee', 'don omar',
      'ozuna', 'anuel', 'rauw alejandro', 'myke towers', 'jhay', 'mora', 'quevedo',
      'bizarrap', 'bzrp', 'duki', 'ysy a', 'khea', 'trueno', 'wos', 'nicki nicole',
      'tiago pzk', 'lit killah', 'emilia', 'maria becerra', 'young miko',
      'eladio carrion', 'arcangel', 'plan b', 'tego calderon', 'residente',
      'canserbero', 'kendrick lamar', 'drake', 'travis scott', 'kanye', 'eminem',
      'jay-z', '2pac', 'tupac', 'notorious', 'snoop dogg', 'dr. dre', '50 cent',
      'j. cole', 'future', 'metro boomin', 'playboi carti', 'lil uzi', '21 savage',
      'post malone', 'the weeknd', 'asap rocky', 'tyler, the creator', 'central cee',
      'pop smoke', 'juice wrld', 'xxxtentacion',
    ],
  },
  {
    style: 'latin',
    terms: [
      'salsa', 'cumbia', 'bachata', 'merengue', 'tropical', 'vallenato', 'mambo',
      'bolero', 'mariachi', 'ranchera', 'corrido', 'banda', 'norteño', 'regional',
      'flamenco', 'rumba', 'samba', 'pagode', 'sertanejo', 'cuarteto', 'romeo santos',
      'aventura', 'juan luis guerra', 'marc anthony', 'hector lavoe', 'willie colon',
      'ruben blades', 'celia cruz', 'grupo niche', 'joe arroyo', 'gilberto santa rosa',
      'elvis crespo', 'los angeles azules', 'ángeles azules', 'selena', 'americo',
      'rafaga', 'ráfaga', 'damas gratis', 'ke personajes', 'la konga', 'luck ra',
      'rodrigo', 'carlos vives', 'juanes', 'shakira', 'chayanne', 'luis miguel',
      'cristian castro', 'marco antonio solis', 'vicente fernandez', 'christian nodal',
      'peso pluma', 'natanael cano', 'junior h', 'fuerza regida', 'eslabon armado',
      'grupo frontera', 'carin leon', 'banda ms', 'tigres del norte', 'mon laferte',
      'natalia lafourcade', 'rosalia', 'rosalía', 'c. tangana', 'gipsy kings',
    ],
  },
  {
    style: 'groove',
    terms: [
      'pop', 'funk', 'disco', 'soul', 'dance', 'k-pop', 'kpop', 'j-pop', 'jpop',
      'city pop', 'motown', 'groove', 'michael jackson', 'dua lipa', 'bruno mars',
      'silk sonic', 'jamiroquai', 'earth, wind', 'bee gees', 'abba', 'madonna',
      'lady gaga', 'taylor swift', 'ariana grande', 'katy perry', 'rihanna',
      'beyonce', 'beyoncé', 'justin timberlake', 'harry styles', 'charli xcx',
      'sabrina carpenter', 'olivia rodrigo', 'chappell roan', 'coldplay', 'maroon 5',
      'imagine dragons', 'twenty one pilots', 'gorillaz', 'bts', 'blackpink',
      'newjeans', 'twice', 'stray kids', 'le sserafim', 'aespa', 'yoasobi', 'ado',
      'kenshi yonezu', 'fujii kaze', 'vaundy', 'creepy nuts', 'tatsuro yamashita',
      'mariya takeuchi', 'anri', 'miranda!',
    ],
  },
];

const ALL_STYLES: MusicDanceStyle[] = ['groove', 'chill', 'rock', 'electro', 'urban', 'latin'];

export function detectTrackDanceStyle(track: MediaTrackInfo | null): MusicDanceStyle {
  if (settings.musicDanceStyle && settings.musicDanceStyle !== 'auto') {
    return settings.musicDanceStyle;
  }
  if (!track) return 'groove';

  const haystack = [
    track.title ?? '',
    track.artist ?? '',
    track.album ?? '',
    ...(track.genres ?? []),
  ]
    .join(' | ')
    .toLowerCase();

  if (!haystack.trim()) return 'groove';

  for (const group of STYLE_PATTERNS) {
    for (const term of group.terms) {
      if (haystack.includes(term)) {
        return group.style;
      }
    }
  }

  // Si la pista es muy larga (> 5.5 min), tiende a ser progresiva/chill/clásica;
  // de lo contrario, seleccionamos de forma determinista según artista + título
  // para que cada canción tenga su propia personalidad de baile consistente.
  const seedStr = `${track.artist}::${track.title}`.toLowerCase().trim();
  if (!seedStr || seedStr === '::') return 'groove';
  let hash = 2166136261;
  for (let i = 0; i < seedStr.length; i++) {
    hash ^= seedStr.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  const idx = Math.abs(hash) % ALL_STYLES.length;
  return ALL_STYLES[idx] ?? 'groove';
}

export function resolveDanceStateId(style: MusicDanceStyle): StateId {
  return DANCE_STYLE_META[style]?.stateId ?? 'dance';
}

export interface MusicAppInfo {
  id: string;
  name: string;
  process_name: string;
  pid: number;
  hwnd: number;
  window_title: string;
  is_playing: boolean;
  track_title: string | null;
  track_artist: string | null;
}

export interface MediaTrackInfo {
  title: string;
  artist: string;
  album: string;
  genres?: string[];
  source_app_id: string;
  source_app_name: string;
  playback_status: 'playing' | 'paused' | 'stopped' | 'idle' | string;
  is_playing: boolean;
  position_secs: number | null;
  duration_secs: number | null;
  thumbnail_data_uri: string | null;
  is_shuffle: boolean;
  repeat_mode: 'none' | 'track' | 'list' | string;
}

export interface QueueTrackItem {
  id: string;
  title: string;
  artist: string;
  album: string;
  source_app_name: string;
  thumbnail_data_uri: string | null;
  added_at: number;
}

export interface MusicStateSnapshot {
  detected: boolean;
  is_playing: boolean;
  current_track: MediaTrackInfo | null;
  running_apps: MusicAppInfo[];
  up_next: QueueTrackItem[];
  history: QueueTrackItem[];
  sleep_timer_remaining_secs: number | null;
  updated_at: number;
}

export type MediaControlAction = string;

const musicState = ref<MusicStateSnapshot>({
  detected: false,
  is_playing: false,
  current_track: null,
  running_apps: [],
  up_next: [],
  history: [],
  sleep_timer_remaining_secs: null,
  updated_at: 0,
});

const isRefreshingMusic = ref(false);
const trackChangedBanner = ref(false);
let bannerTimeout: ReturnType<typeof setTimeout> | null = null;
let lastTrackKey = '';
let unlistenMusicEvent: UnlistenFn | null = null;
let isInitialized = false;

const trackChangeCallbacks = new Set<(track: MediaTrackInfo) => void>();

function applySnapshot(snapshot: MusicStateSnapshot) {
  const prevTrack = musicState.value.current_track;
  musicState.value = {
    ...snapshot,
    up_next: snapshot.up_next ?? [],
    history: snapshot.history ?? [],
    sleep_timer_remaining_secs: snapshot.sleep_timer_remaining_secs ?? null,
  };

  const cur = snapshot.current_track;
  if (cur && cur.is_playing && cur.title) {
    const newKey = `${cur.source_app_id}::${cur.artist}::${cur.title}`;
    if (lastTrackKey && lastTrackKey !== newKey) {
      if (settings.musicDetectionEnabled && settings.musicReactOnTrackChange) {
        trackChangedBanner.value = true;
        if (bannerTimeout) clearTimeout(bannerTimeout);
        bannerTimeout = setTimeout(() => {
          trackChangedBanner.value = false;
        }, 4200);
        trackChangeCallbacks.forEach((cb) => cb(cur));
      }
    } else if (!prevTrack?.is_playing && cur.is_playing) {
      trackChangedBanner.value = true;
      if (bannerTimeout) clearTimeout(bannerTimeout);
      bannerTimeout = setTimeout(() => {
        trackChangedBanner.value = false;
      }, 3600);
    }
    lastTrackKey = newKey;
  } else if (!cur?.is_playing) {
    trackChangedBanner.value = false;
  }
}

export function formatMmSs(secs: number | null | undefined): string {
  if (!secs || secs < 0) return '00:00';
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
}

export function useMusicDetector() {
  const isMusicDetected = computed(
    () => settings.musicDetectionEnabled && musicState.value.detected
  );

  const isMusicPlaying = computed(
    () => settings.musicDetectionEnabled && musicState.value.is_playing
  );

  const currentTrack = computed<MediaTrackInfo | null>(() => {
    if (!settings.musicDetectionEnabled) return null;
    return musicState.value.current_track;
  });

  const currentDanceStyle = computed<MusicDanceStyle>(() =>
    detectTrackDanceStyle(musicState.value.current_track)
  );

  const currentDanceStyleMeta = computed<DanceStyleMeta>(
    () => DANCE_STYLE_META[currentDanceStyle.value] ?? DANCE_STYLE_META.groove
  );

  const currentDanceStateId = computed<StateId>(() =>
    resolveDanceStateId(currentDanceStyle.value)
  );

  const runningMusicApps = computed<MusicAppInfo[]>(() => {
    if (!settings.musicDetectionEnabled) return [];
    return musicState.value.running_apps;
  });

  const upNextQueue = computed<QueueTrackItem[]>(() => musicState.value.up_next ?? []);
  const recentTracks = computed<QueueTrackItem[]>(() => musicState.value.history ?? []);
  const sleepTimerRemaining = computed<number | null>(
    () => musicState.value.sleep_timer_remaining_secs ?? null
  );

  const primaryAppName = computed<string>(() => {
    if (musicState.value.current_track?.source_app_name) {
      return musicState.value.current_track.source_app_name;
    }
    if (musicState.value.running_apps.length > 0) {
      return musicState.value.running_apps[0].name;
    }
    return 'Spotify';
  });

  const primaryAppId = computed<string>(() => {
    if (musicState.value.current_track?.source_app_id) {
      return musicState.value.current_track.source_app_id;
    }
    if (musicState.value.running_apps.length > 0) {
      return musicState.value.running_apps[0].id;
    }
    return 'spotify';
  });

  async function refreshMusicState(force = true): Promise<MusicStateSnapshot> {
    try {
      isRefreshingMusic.value = true;
      const snap = await invoke<MusicStateSnapshot>('get_music_state', { refresh: force });
      applySnapshot(snap);
      return snap;
    } catch (err) {
      console.error('[useMusicDetector] Error consultando estado musical:', err);
      return musicState.value;
    } finally {
      isRefreshingMusic.value = false;
    }
  }

  async function sendMediaAction(
    action: MediaControlAction,
    targetAppId?: string
  ): Promise<MusicStateSnapshot> {
    try {
      const updated = await invoke<MusicStateSnapshot>('control_music', {
        action,
        targetAppId: targetAppId ?? null,
      });
      applySnapshot(updated);
      return updated;
    } catch (err) {
      console.error(`[useMusicDetector] Error ejecutando acción ${action}:`, err);
      return musicState.value;
    }
  }

  async function togglePlayPause() {
    return sendMediaAction('play_pause');
  }

  async function nextTrack() {
    return sendMediaAction('next');
  }

  async function previousTrack() {
    return sendMediaAction('previous');
  }

  async function toggleShuffle() {
    return sendMediaAction('toggle_shuffle');
  }

  async function cycleRepeat() {
    return sendMediaAction('cycle_repeat');
  }

  async function seekToSecs(secs: number) {
    return sendMediaAction(`seek:${Math.max(0, Math.round(secs))}`);
  }

  async function addToQueue(query: string) {
    if (!query.trim()) return musicState.value;
    return sendMediaAction(`queue_add:${query.trim()}`);
  }

  async function removeFromQueue(id: string) {
    return sendMediaAction(`queue_remove:${id}`);
  }

  async function clearQueue() {
    return sendMediaAction('queue_clear');
  }

  async function playSpotifyQuery(query: string) {
    if (!query.trim()) return musicState.value;
    return sendMediaAction(`play_query:${query.trim()}`);
  }

  async function setSleepTimerMinutes(mins: number) {
    return sendMediaAction(`sleep_timer:${Math.max(0, Math.round(mins))}`);
  }

  async function focusMusicApp(appId?: string) {
    return sendMediaAction('focus_app', appId);
  }

  async function moveNearMusicApp(appId?: string) {
    return sendMediaAction('move_near_app', appId);
  }

  function onTrackChange(cb: (track: MediaTrackInfo) => void): () => void {
    trackChangeCallbacks.add(cb);
    return () => {
      trackChangeCallbacks.delete(cb);
    };
  }

  function initMusicBridge() {
    if (isInitialized) return;
    isInitialized = true;

    refreshMusicState(true);

    listen<MusicStateSnapshot>('music://state-changed', (event) => {
      if (event.payload) {
        applySnapshot(event.payload);
      }
    }).then((un) => {
      unlistenMusicEvent = un;
    });
  }

  function destroyMusicBridge() {
    if (bannerTimeout) {
      clearTimeout(bannerTimeout);
      bannerTimeout = null;
    }
    unlistenMusicEvent?.();
    unlistenMusicEvent = null;
    isInitialized = false;
  }

  return {
    musicState,
    isMusicDetected,
    isMusicPlaying,
    currentTrack,
    currentDanceStyle,
    currentDanceStyleMeta,
    currentDanceStateId,
    runningMusicApps,
    upNextQueue,
    recentTracks,
    sleepTimerRemaining,
    primaryAppName,
    primaryAppId,
    isRefreshingMusic,
    trackChangedBanner,
    initMusicBridge,
    destroyMusicBridge,
    refreshMusicState,
    sendMediaAction,
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
    onTrackChange,
  };
}
