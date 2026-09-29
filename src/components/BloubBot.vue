<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, triggerRef, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { NOTIF_BLUE } from '@/bot/decor'
import { BotEngine, type BotFrame } from '@/bot/engine'
import { clamp } from '@/bot/math'
import { t } from '@/i18n'
import { lookTarget, TURN_TIME, type GazeScript } from '@/ui/gaze'
import {
  DEFAULT_EXPRESSION,
  EXPRESSION_BY_ID
} from '@/bot/expressions'
import {
  COLOR_BY_ID,
  DEFAULT_COLOR,
  DEFAULT_SHAPE,
  SHAPE_BY_ID,
  mixHex
} from '@/bot/skins'
import { blockAt, defaultCycle, offsetOf, type Block } from '@/bot/cycles'
import { DEMI_VIEWBOX, RAYON } from '@/bot/repere'
import { followsCursor, type StateId } from '@/bot/states'

const props = withDefaults(
  defineProps<{
    size?: number
    /** identifiant de forme du personnalisateur */
    shape?: string
    /** identifiant de couleur du personnalisateur */
    color?: string
    /** identifiant d'expression de repos du personnalisateur */
    expression?: string
    /** couleur du fond, utilisee pour la brume de profondeur des particules */
    paper?: string
    /** couleur de fond des yeux */
    eyeColor?: string
    /**
     * Fige le rendu a cette date (en secondes depuis le debut de l'etat).
     */
    frozenAt?: number
    /**
     * Montage joue par le lecteur : une suite d'etats, chacun tenu la duree de son bloc.
     */
    cycle?: Block[]
    /**
     * Le regard suit le pointeur.
     */
    follow?: boolean
    /**
     * Regard scripte de l'arrivee.
     */
    gaze?: GazeScript | null
    /** Rendu volumétrique 3D, ombres et reflets */
    volumetric?: boolean
    /** Interactions tactiles de caresses et ronronnement */
    interactivePet?: boolean
    /** Déformation élastique masse-ressort gélatine */
    interactiveJelly?: boolean
    /** Réflexe de sursaut aux passages rapides du curseur */
    interactiveFlinch?: boolean
    /** Mise en veille automatique par inactivité */
    autoSleepEnabled?: boolean
    /** Minutes d'inactivité avant sommeil de nuit (22h-06h) */
    nightInactivitySleepMinutes?: number
    /** Minutes d'inactivité avant sommeil de jour */
    dayInactivitySleepMinutes?: number
  }>(),
  {
    size: 320,
    shape: DEFAULT_SHAPE,
    color: DEFAULT_COLOR,
    expression: DEFAULT_EXPRESSION,
    paper: '#ffffff',
    eyeColor: '#ffffff',
    frozenAt: undefined,
    cycle: () => defaultCycle().blocks,
    follow: false,
    gaze: null,
    volumetric: true,
    interactivePet: true,
    interactiveJelly: true,
    interactiveFlinch: true,
    autoSleepEnabled: true,
    nightInactivitySleepMinutes: 6,
    dayInactivitySleepMinutes: 20
  }
)

const emit = defineEmits<{
  (e: 'pet'): void
  (e: 'flinch'): void
  (e: 'wakeUp'): void
  (e: 'sleep'): void
}>()

const block = defineModel<number>('block', { default: 0 })
const state = defineModel<StateId>('state', { default: 'idle' })
const playing = defineModel<boolean>('playing', { default: false })
const elapsed = defineModel<number>('elapsed', { default: 0 })

const R = RAYON
const VB = DEMI_VIEWBOX

const shapeRadii = computed(() => SHAPE_BY_ID.get(props.shape)?.radii ?? null)
const ink = computed(() => COLOR_BY_ID.get(props.color)?.hex ?? '#0a0a0c')
const expression = computed(() => EXPRESSION_BY_ID.get(props.expression) ?? null)
const eyeFill = computed(() => props.eyeColor ?? props.paper ?? '#ffffff')

const engine = new BotEngine(R, state.value, shapeRadii.value, expression.value)
const frame = shallowRef<BotFrame>(engine.sample(props.frozenAt ?? 0))

const uid = Math.random().toString(36).slice(2, 8)

let raf = 0
let nextAt = Infinity
let last = 0
let clock = 0
let blockStart = 0
let lastSampleClock = -Infinity
let lastActivityClock = 0

// Références DOM pour écritures directes (zéro saccade, bypass réactivité Vue)
const rotatorEl = ref<HTMLDivElement | null>(null)
const groundEl = ref<HTMLDivElement | null>(null)
const lightEl = ref<SVGRadialGradientElement | null>(null)
const shadeEl = ref<SVGRadialGradientElement | null>(null)
const svg = ref<SVGSVGElement | null>(null)

// Cache des valeurs DOM pour éviter les écritures redondantes
let lastRotatorTransform = ''
let lastGroundTransform = ''
let lastGroundOpacity = -1
let lastLightCx = -9999
let lastLightCy = -9999

/* ------------------------------------------------------- Cycle & Montage */

function apply(i: number, from = 0) {
  const b = props.cycle[i]
  if (!b) {
    nextAt = Infinity
    return
  }
  blockStart = clock - from
  elapsed.value = from
  state.value = b.state
  engine.setState(b.state, clock)
  nextAt = playing.value ? blockStart + b.duration : Infinity
}

function goToBlock(i: number) {
  block.value = i
  apply(i)
}

let pendingOffset = 0

function seek(index: number, offset = 0) {
  if (block.value === index) {
    apply(index, offset)
    return
  }
  pendingOffset = offset
  block.value = index
}

let dernierBloc = -1

function rendAt(t: number) {
  const blocs = props.cycle
  if (!blocs.length) return
  const { index } = blockAt(blocs, t)
  if (index !== dernierBloc) {
    const b = blocs[index]!
    state.value = b.state
    if (index < dernierBloc) engine.reset(b.state, offsetOf(blocs, index))
    else engine.setState(b.state, offsetOf(blocs, index))
    dernierBloc = index
  }
  frame.value = engine.sample(t)
  triggerRef(frame)
}

function setState(s: StateId) {
  state.value = s
  engine.setState(s, clock)
  if (props.frozenAt !== undefined) {
    redrawFrozen()
  }
}

/* ------------------------------------------------------- Horloge circadienne (Nuit 22h-06h) */

function checkIsNight(): boolean {
  const hour = new Date().getHours()
  return hour >= 22 || hour < 6
}

const isNight = ref(checkIsNight())
let nightTimer: ReturnType<typeof setInterval> | null = null

const isNightHaloVisible = computed(() => {
  return (
    props.volumetric &&
    props.size >= 56 &&
    props.frozenAt === undefined &&
    isNight.value &&
    state.value !== 'sleep' &&
    state.value !== 'deepsleep'
  )
})

const haloStyle = computed(() => {
  const haloSize = props.size * 1.35
  const offset = -props.size * 0.175
  return {
    width: `${haloSize}px`,
    height: `${haloSize}px`,
    left: `${offset}px`,
    top: `${offset}px`,
    borderRadius: '50%',
    background: 'radial-gradient(closest-side, rgba(129, 140, 248, 0.22), rgba(129, 140, 248, 0))'
  }
})

const groundStyle = computed(() => {
  const groundW = props.size * 0.56
  const groundH = props.size * 0.09
  const left = props.size * 0.22
  const top = props.size * 0.81
  return {
    width: `${groundW}px`,
    height: `${groundH}px`,
    left: `${left}px`,
    top: `${top}px`,
    borderRadius: '50%',
    background: 'radial-gradient(closest-side, rgba(0, 0, 0, 0.26), rgba(0, 0, 0, 0))'
  }
})

/* ------------------------------------------------------- Physique du regard & curseur 360° */

let aiming = false
let globalDx = 0
let globalDy = 0
let lastCursorMove = 0
let inFlightCursor = false
const CURSOR_POLL = 0.04
let lastCursorPoll = -Infinity

// Vitesse et anticipation sacadique
let pointerSpeed = 0
let pointerVx = 0
let pointerVy = 0
let lastPollDx = 0
let lastPollDy = 0
let lastPollTime = 0

// Lissage exponentiel de la direction de regard
let curNx = 0
let curNy = 0
let prevCurNy = 0
let proximity = 0

// Gelée masse-ressort (k=580, c=28)
let jellyPos = 0
let jellyVel = 0

// Caresses et ronronnement
let petLean = 0
let petIntensity = 0
let petReversals = 0
let lastPetDir = 0
let lastPetTime = 0
let lastPetX = 0

// Sursaut
let lastFlinchTime = -Infinity
let flinchTimer: ReturnType<typeof setTimeout> | null = null

// Réveil
let isWakingUp = false
let wakeupTimer: ReturnType<typeof setTimeout> | null = null

async function trackGlobalCursor() {
  if (inFlightCursor) return
  inFlightCursor = true
  try {
    const [dx, dy] = await invoke<[number, number]>('get_cursor_relative')
    const nowTime = performance.now() / 1000
    const dt = lastPollTime ? nowTime - lastPollTime : CURSOR_POLL

    if (dt >= 0.005 && dt <= 0.35) {
      const moved = Math.hypot(dx - lastPollDx, dy - lastPollDy)
      const instSpeed = moved / dt
      const instVx = (dx - lastPollDx) / dt
      const instVy = (dy - lastPollDy) / dt

      pointerSpeed = 0.35 * pointerSpeed + 0.65 * instSpeed
      pointerVx = 0.6 * instVx + 0.4 * pointerVx
      pointerVy = 0.6 * instVy + 0.4 * pointerVy

      if (moved > 1.0) {
        lastCursorMove = clock
        lastActivityClock = clock

        // Si l'avatar dort et qu'il y a activité significative, le réveiller
        if ((state.value === 'sleep' || state.value === 'deepsleep') && !isWakingUp) {
          triggerWakeup()
        }

        // Détection de sursaut (flinch) : vitesse > 1150 px/s à proximité
        if (
          props.interactiveFlinch &&
          pointerSpeed > 1150 &&
          Math.hypot(dx, dy) < Math.max(props.size * 1.35, 110) &&
          clock - lastFlinchTime > 1.8
        ) {
          triggerFlinch()
        }
      }
    }

    lastPollDx = dx
    lastPollDy = dy
    lastPollTime = nowTime
    globalDx = dx
    globalDy = dy
  } catch {
    // Si hors environnement Tauri natif
  } finally {
    inFlightCursor = false
  }
}

function triggerFlinch() {
  lastFlinchTime = clock
  jellyVel -= 5.4
  const surpris = EXPRESSION_BY_ID.get('surpris') ?? null
  if (surpris) {
    engine.setExpression(surpris, clock)
  }
  emit('flinch')
  if (flinchTimer) clearTimeout(flinchTimer)
  flinchTimer = setTimeout(() => {
    engine.setExpression(expression.value, clock)
  }, 450)
}

function triggerWakeup() {
  if (isWakingUp) return
  isWakingUp = true
  state.value = 'wakeup'
  engine.setState('wakeup', clock)
  emit('wakeUp')
  if (wakeupTimer) clearTimeout(wakeupTimer)
  wakeupTimer = setTimeout(() => {
    isWakingUp = false
    state.value = 'idle'
    engine.setState('idle', clock)
  }, 1350)
}

function triggerPetReaction() {
  emit('pet')
  if (state.value === 'sleep' || state.value === 'deepsleep') {
    state.value = 'deepsleep'
    engine.setState('deepsleep', clock)
    setTimeout(() => {
      if (state.value === 'deepsleep') {
        state.value = 'sleep'
        engine.setState('sleep', clock)
      }
    }, 2400)
  } else {
    state.value = 'cuddle'
    engine.setState('cuddle', clock)
    setTimeout(() => {
      if (state.value === 'cuddle') {
        state.value = 'idle'
        engine.setState('idle', clock)
      }
    }, 1400)
  }
}

function handlePointerMoveOnAvatar(e: PointerEvent) {
  lastCursorMove = clock
  lastActivityClock = clock

  if (props.interactivePet) {
    const rect = svg.value?.getBoundingClientRect()
    if (rect) {
      const px = e.clientX - (rect.left + rect.width / 2)
      const py = e.clientY - (rect.top + rect.height / 2)
      const dist = Math.hypot(px, py)
      const nowTime = performance.now() / 1000

      if (py < 0 && dist < props.size * 0.48) {
        if (nowTime - lastPetTime > 0.65) {
          petReversals = 0
          lastPetX = px
        }
        const deltaX = px - lastPetX
        if (Math.abs(deltaX) > 5) {
          const dir = Math.sign(deltaX)
          petLean = clamp(petLean * 0.6 + dir * 0.4, -1, 1)
          if (dir !== lastPetDir && lastPetDir !== 0) {
            petReversals++
            petIntensity = Math.min(petIntensity + 0.34, 1)
            if (petReversals >= 3) {
              triggerPetReaction()
              petReversals = 0
            }
          }
          lastPetDir = dir
          lastPetX = px
          lastPetTime = nowTime
        }
      }
    }
  }
}

function release() {
  if (!aiming) return
  engine.setLook(null, clock, TURN_TIME)
  aiming = false
}

function updateGazeAndPhysics(dt: number) {
  const canFollow = followsCursor(state.value)

  if (!canFollow || !svg.value || !props.follow) {
    if (aiming) {
      // Revenir doucement au repos
      const k = 8
      const factor = 1 - Math.exp(-k * dt)
      curNx += (0 - curNx) * factor
      curNy += (0 - curNy) * factor
      proximity += (0 - proximity) * factor
      if (Math.hypot(curNx, curNy) <= 0.005) {
        release()
      } else {
        engine.setLook(
          lookTarget({
            nx: curNx,
            ny: curNy,
            tour: 1,
            pointer: false
          }),
          clock - 1 / 60,
          1 / 60
        )
      }
    }
    return
  }

  if (!aiming) {
    lastCursorMove = clock
  }

  if (clock - lastCursorPoll >= CURSOR_POLL) {
    lastCursorPoll = clock
    trackGlobalCursor()
  }

  const screenW = window.screen.availWidth || 1920
  const screenH = window.screen.availHeight || 1080
  const maxReach = Math.min(Math.max(screenW, screenH) * 0.4, 380)

  const dist = Math.hypot(globalDx, globalDy)
  const curve = dist > 0.001 ? Math.sin(Math.min(dist / maxReach, 1) * (Math.PI / 2)) : 0
  let rawX = dist > 0.001 ? (globalDx / dist) * curve : 0
  let rawY = dist > 0.001 ? (globalDy / dist) * curve : 0

  // Anticipation sacadique
  if (pointerSpeed > 80) {
    const leadFactor = Math.min(pointerSpeed * 0.00007, 0.12)
    rawX += (pointerVx / (pointerSpeed || 1)) * leadFactor
    rawY += (pointerVy / (pointerSpeed || 1)) * leadFactor
  }

  // Désattention contemplative
  const idleSec = clock - lastCursorMove
  let focus = 1
  let targetX = rawX
  let targetY = rawY
  let hasDeriva = false

  if (idleSec > 2) {
    focus = Math.max(0.25, 1 - (idleSec - 2) * 0.4)
    targetX = rawX * focus
    targetY = rawY * focus - (1 - focus) * 0.10
    hasDeriva = focus < 0.65
  }

  // Proximité
  const thresh = Math.max(props.size * 0.75, 45)
  const targetProx = Math.max(0, 1 - dist / thresh) * focus

  // Lissage exponentiel temporel indépendant du taux de rafraîchissement
  const kSmooth = isNight.value ? 16 : 24
  const factor = 1 - Math.exp(-kSmooth * dt)

  prevCurNy = curNy
  curNx += (targetX - curNx) * factor
  curNy += (targetY - curNy) * factor
  proximity += (targetProx - proximity) * factor

  // Physique masse-ressort gélatine (D4 §3)
  if (props.interactiveJelly) {
    const deltaNy = curNy - prevCurNy
    const targetJ = clamp(-deltaNy * 0.42, -0.15, 0.15)
    const f = (targetJ - jellyPos) * 580 - jellyVel * 28
    jellyVel += f * dt
    jellyPos += jellyVel * dt
    jellyPos = clamp(jellyPos, -0.35, 0.35)
  }

  // Décroissance des caresses et du ronronnement (D4 §2)
  if (props.interactivePet) {
    petIntensity = Math.max(0, petIntensity - 2.2 * dt)
    petLean *= Math.max(0, 1 - 3 * dt)
  }

  // Transmission au moteur
  engine.setLook(
    lookTarget({
      nx: curNx,
      ny: curNy,
      tour: state.value === 'dance' ? 0.8 : 1,
      pointer: !hasDeriva
    }),
    clock - 1 / 60,
    1 / 60
  )
  aiming = true
}

/* ------------------------------------------------- Regard scripté d'arrivée */

let gazeSince = 0
let scripted = false
const SCRIPT_MORPH = 1 / 60

function scriptedGaze(run: GazeScript) {
  engine.setLook(run(clock - gazeSince), clock, SCRIPT_MORPH)
}

watch(
  () => props.gaze,
  (run) => {
    if (run) {
      gazeSince = clock
      scripted = true
      engine.setLook(run(0), clock - SCRIPT_MORPH, SCRIPT_MORPH)
      return
    }
    if (!scripted) return
    engine.setLook(null, clock)
    scripted = false
  },
  { immediate: true }
)

/* ------------------------------------------------- Boucle de rendu (rAF Throttled) */

function tick(ms: number) {
  raf = requestAnimationFrame(tick)

  const dt = last ? Math.min((ms - last) / 1000, 0.064) : 0
  last = ms
  clock += dt

  if (playing.value) {
    if (clock >= nextAt && props.cycle.length) {
      goToBlock((block.value + 1) % props.cycle.length)
    } else {
      elapsed.value = clock - blockStart
    }
  }

  if (props.follow) {
    updateGazeAndPhysics(dt)
  } else if (props.gaze) {
    scriptedGaze(props.gaze)
  }

  // Cadencement adaptatif (D2 §10)
  const isReducedMotion =
    window.matchMedia('(prefers-reduced-motion: reduce)').matches ||
    document.documentElement.classList.contains('reduce-motion')

  const targetFps = isReducedMotion ? 12 : (clock - lastCursorMove < 1.0 ? 60 : 30)
  const sampleInterval = 1 / targetFps

  if (clock - lastSampleClock >= sampleInterval) {
    lastSampleClock = clock
    frame.value = engine.sample(clock)
    triggerRef(frame)
  }

  // Écritures directes sur le DOM
  writeDirectDomUpdates()
}

function writeDirectDomUpdates() {
  const body = frame.value.body || { cx: 0, cy: 0, sx: 1, sy: 1 }

  // 1. Dégradés volumétriques de lumière et d'ombre (D2 §4)
  if (props.volumetric && lightEl.value && shadeEl.value) {
    const lx = Number((body.cx + (-0.34 - curNx * 0.26) * R).toFixed(1))
    const ly = Number((body.cy + (-0.42 - curNy * 0.26) * R).toFixed(1))
    if (lx !== lastLightCx || ly !== lastLightCy) {
      lastLightCx = lx
      lastLightCy = ly
      const sLx = String(lx)
      const sLy = String(ly)
      lightEl.value.setAttribute('cx', sLx)
      lightEl.value.setAttribute('cy', sLy)
      shadeEl.value.setAttribute('cx', sLx)
      shadeEl.value.setAttribute('cy', sLy)
    }
  }

  // 2. Sombra de contacto au sol (D2 §5)
  if (props.volumetric && props.size >= 56 && groundEl.value) {
    const lift = clamp(-body.cy / R, 0, 0.4)
    const squash = clamp(body.sx / body.sy, 0.8, 1.25)
    const k = clamp((1 - 1.7 * lift) * squash, 0.45, 1.25)
    const op = Number(clamp(1 - 2.2 * lift, 0.25, 1).toFixed(3))
    const shiftX = Number((curNx * props.size * 0.03 + (body.cx / 316) * props.size).toFixed(1))
    const scaleY = Number((0.6 + 0.4 * k).toFixed(3))
    const kRounded = Number(k.toFixed(3))
    const groundTransform = `translateX(${shiftX}px) scale(${kRounded}, ${scaleY})`

    if (groundTransform !== lastGroundTransform) {
      lastGroundTransform = groundTransform
      groundEl.value.style.transform = groundTransform
    }
    if (op !== lastGroundOpacity) {
      lastGroundOpacity = op
      groundEl.value.style.opacity = String(op)
    }
  }

  // 3. Rotateur 3D et micro-parallaxe (D2 §6)
  if (rotatorEl.value) {
    const rotY = (curNx * 15).toFixed(2)
    const rotX = (-curNy * 11).toFixed(2)
    const petLeanDeg = petLean * petIntensity * 5.5
    const rotZ = (-curNx * curNy * 2.5 + petLeanDeg).toFixed(2)
    const tx = (curNx * props.size * 0.015).toFixed(2)
    const ty = (curNy * props.size * 0.015).toFixed(2)
    const tz = (proximity * 10).toFixed(2)
    const purr = props.interactivePet ? Math.sin(clock * 48) * petIntensity * 0.026 : 0
    const j = (props.interactiveJelly ? jellyPos : 0) + purr
    const sx = ((1 - 0.85 * j) * (1 + 0.04 * proximity)).toFixed(4)
    const sy = ((1 + j) * (1 + 0.04 * proximity)).toFixed(4)
    const rotatorTransform = `perspective(800px) rotateY(${rotY}deg) rotateX(${rotX}deg) rotateZ(${rotZ}deg) translate3d(${tx}px, ${ty}px, ${tz}px) scale(${sx}, ${sy})`

    if (rotatorTransform !== lastRotatorTransform) {
      lastRotatorTransform = rotatorTransform
      rotatorEl.value.style.transform = rotatorTransform
    }
  }
}

function redrawFrozen() {
  if (props.frozenAt === undefined) return
  frame.value = engine.sample(props.frozenAt)
  triggerRef(frame)
}

watch(block, (i) => {
  apply(i, pendingOffset)
  pendingOffset = 0
})

watch(state, (id) => {
  if (engine.state === id) return
  engine.setState(id, clock)
  redrawFrozen()
})

watch(playing, (on) => {
  if (on) apply(block.value, elapsed.value)
  else nextAt = Infinity
})

watch(
  () => props.cycle,
  (blocks) => {
    if (!blocks.length) {
      nextAt = Infinity
      return
    }
    const i = Math.min(block.value, blocks.length - 1)
    if (i !== block.value) {
      goToBlock(i)
      return
    }
    nextAt = playing.value ? blockStart + blocks[i]!.duration : Infinity
  }
)

watch(shapeRadii, (radii) => {
  engine.setShape(radii, clock)
  redrawFrozen()
})

watch(expression, (expr) => {
  engine.setExpression(expr, clock)
  redrawFrozen()
})

watch(() => props.frozenAt, redrawFrozen)

/* ---------------------------------------------------- Veille automatique (D5 §5) */

let autoSleepInterval: ReturnType<typeof setInterval> | null = null

function checkAutoSleep() {
  if (!props.autoSleepEnabled || props.frozenAt !== undefined) return
  if (state.value === 'sleep' || state.value === 'deepsleep' || isWakingUp) return

  const inactiveSeconds = clock - lastActivityClock
  const maxMinutes = isNight.value
    ? props.nightInactivitySleepMinutes
    : props.dayInactivitySleepMinutes

  if (maxMinutes <= 0) return

  if (inactiveSeconds >= maxMinutes * 60) {
    if (state.value === 'idle') {
      setState('sleep')
      emit('sleep')
    }
  }
}

/* ---------------------------------------------------- Big Bang & Colapso */

const isSpawning = ref(false)
const isCollapsing = ref(false)
let spawnTimer: ReturnType<typeof setTimeout> | null = null
let collapseTimer: ReturnType<typeof setTimeout> | null = null

function triggerSpawn() {
  if (spawnTimer) clearTimeout(spawnTimer)
  if (collapseTimer) clearTimeout(collapseTimer)
  isCollapsing.value = false
  isSpawning.value = false
  requestAnimationFrame(() => {
    isSpawning.value = true
    spawnTimer = setTimeout(() => {
      isSpawning.value = false
    }, 1150)
  })
}

function triggerCollapse(): Promise<void> {
  return new Promise((resolve) => {
    if (spawnTimer) clearTimeout(spawnTimer)
    if (collapseTimer) clearTimeout(collapseTimer)
    isSpawning.value = false
    isCollapsing.value = true
    collapseTimer = setTimeout(() => {
      isCollapsing.value = false
      resolve()
    }, 280)
  })
}

defineExpose({ seek, rendAt, triggerSpawn, triggerCollapse, setState, triggerPetReaction })

const isSleeping = computed(() => {
  return (
    (state.value === 'sleep' || state.value === 'deepsleep') &&
    !isSpawning.value &&
    !isCollapsing.value
  )
})

interface SpawnRay {
  id: number
  x1: number
  y1: number
  x2: number
  y2: number
  color: string
  delay: number
}

interface SpawnSpark {
  id: number
  r: number
  dx: number
  dy: number
  color: string
  delay: number
}

const SPAWN_RAYS: SpawnRay[] = Array.from({ length: 12 }, (_, i) => {
  const angle = (i * 30 * Math.PI) / 180
  const len = 95 + (i % 3) * 25
  return {
    id: i,
    x1: Math.cos(angle) * 8,
    y1: Math.sin(angle) * 8,
    x2: Math.cos(angle) * len,
    y2: Math.sin(angle) * len,
    color: i % 2 === 0 ? '#38bdf8' : '#a855f7',
    delay: (i % 4) * 0.035
  }
})

const SPAWN_SPARKS: SpawnSpark[] = Array.from({ length: 22 }, (_, i) => {
  const angle = (i * (360 / 22) * Math.PI) / 180 + ((i % 2 === 0 ? 1 : -1) * 0.12)
  const dist = 70 + (i * 19) % 75
  const colors = ['#ffffff', '#38bdf8', '#818cf8', '#c084fc', '#f59e0b', '#34d399']
  return {
    id: i,
    r: 1.5 + (i % 3) * 1.2,
    dx: Math.cos(angle) * dist,
    dy: Math.sin(angle) * dist,
    color: colors[i % colors.length]!,
    delay: (i % 6) * 0.03
  }
})

function dotAttrs(dot: BotFrame['dots'][number]) {
  const fill =
    dot.color ?? (dot.depth === undefined ? ink.value : mixHex(props.paper, ink.value, dot.depth))
  const common = { fill, opacity: dot.opacity }
  return dot.d
    ? {
        ...common,
        d: dot.d,
        transform: `translate(${dot.x} ${dot.y}) rotate(${dot.rot ?? 0}) scale(${R})`
      }
    : { ...common, cx: dot.x, cy: dot.y, r: dot.r }
}

/* ---------------------------------------------------- Portier de visibilité */

function onVisibilityChange() {
  if (document.hidden) {
    cancelAnimationFrame(raf)
    raf = 0
  } else {
    if (!raf && props.frozenAt === undefined) {
      last = 0
      raf = requestAnimationFrame(tick)
    }
  }
}

onMounted(() => {
  if (props.frozenAt !== undefined) return

  apply(block.value, elapsed.value)
  raf = requestAnimationFrame(tick)

  isSpawning.value = true
  spawnTimer = setTimeout(() => {
    isSpawning.value = false
  }, 1150)

  // Timer de nuit toutes les 30 secondes
  nightTimer = setInterval(() => {
    isNight.value = checkIsNight()
  }, 30000)

  // Évaluation de l'auto-veille toutes les 20 secondes
  autoSleepInterval = setInterval(checkAutoSleep, 20000)

  document.addEventListener('visibilitychange', onVisibilityChange)
})

onBeforeUnmount(() => {
  if (raf) cancelAnimationFrame(raf)
  if (spawnTimer) clearTimeout(spawnTimer)
  if (collapseTimer) clearTimeout(collapseTimer)
  if (flinchTimer) clearTimeout(flinchTimer)
  if (wakeupTimer) clearTimeout(wakeupTimer)
  if (nightTimer) clearInterval(nightTimer)
  if (autoSleepInterval) clearInterval(autoSleepInterval)
  document.removeEventListener('visibilitychange', onVisibilityChange)
})
</script>

<template>
  <div
    class="bloub-root relative select-none flex items-center justify-center contain-layout"
    :style="{
      width: `${props.size}px`,
      height: `${props.size}px`,
      perspective: '800px'
    }"
    @pointermove="handlePointerMoveOnAvatar"
  >
    <!-- 1. Halo nocturne statique (D2 §9) : 22h-06h, sans filtre coûteux -->
    <div
      v-if="isNightHaloVisible"
      class="bloub-halo pointer-events-none absolute"
      :style="haloStyle"
    />

    <!-- 2. Sombra de contacto au sol (D2 §5) -->
    <div
      v-if="props.size >= 56 && props.volumetric"
      ref="groundEl"
      class="bloub-ground pointer-events-none absolute"
      :style="groundStyle"
    />

    <!-- 3. Rotateur 3D avec micro-parallaxe (D2 §6) -->
    <div
      ref="rotatorEl"
      class="bloub-rotator w-full h-full flex items-center justify-center"
      style="transform-style: preserve-3d; will-change: transform;"
    >
      <svg
        ref="svg"
        :width="props.size"
        :height="props.size"
        :viewBox="`${-VB} ${-VB} ${VB * 2} ${VB * 2}`"
        shape-rendering="geometricPrecision"
        text-rendering="geometricPrecision"
        role="img"
        :aria-label="t('app.botAria')"
        class="block select-none gpu-composite contain-render"
      >
        <defs>
          <!-- Masque d'encre unifié (D2 §3) : corps blanc, yeux noirs, encoche noire -->
          <mask
            :id="`mask-${uid}`"
            maskUnits="userSpaceOnUse"
            :x="-VB"
            :y="-VB"
            :width="VB * 2"
            :height="VB * 2"
          >
            <rect :x="-VB" :y="-VB" :width="VB * 2" :height="VB * 2" fill="#000" />
            <path :d="frame.bodyPath" fill="#fff" />
            <g :class="{ 'bb-eyes-awake': isSpawning, 'bb-eyes-collapse': isCollapsing }">
              <path
                v-for="(eye, i) in frame.eyes"
                :key="i"
                :d="eye.d"
                :transform="eye.matrix"
                :opacity="eye.alpha"
                fill="#000"
                stroke="#000"
                stroke-width="0.4"
              />
            </g>
            <circle
              v-if="frame.notch"
              :cx="frame.notch.x"
              :cy="frame.notch.y"
              :r="frame.notch.r"
              fill="#000"
            />
          </mask>

          <!-- Dégradé volumétrique spéculaire -light (D2 §4) -->
          <radialGradient
            :id="`light-${uid}`"
            ref="lightEl"
            gradientUnits="userSpaceOnUse"
            :cx="-0.34 * R"
            :cy="-0.42 * R"
            :r="0.95 * R"
          >
            <stop offset="0%" stop-color="#ffffff" stop-opacity="0.34" />
            <stop offset="28%" stop-color="#ffffff" stop-opacity="0.12" />
            <stop offset="100%" stop-color="#ffffff" stop-opacity="0" />
          </radialGradient>

          <!-- Dégradé volumétrique terminateur d'ombre -shade (D2 §4) -->
          <radialGradient
            :id="`shade-${uid}`"
            ref="shadeEl"
            gradientUnits="userSpaceOnUse"
            :cx="-0.34 * R"
            :cy="-0.42 * R"
            :r="2.05 * R"
          >
            <stop offset="45%" stop-color="#000000" stop-opacity="0" />
            <stop offset="80%" stop-color="#000000" stop-opacity="0.20" />
            <stop offset="100%" stop-color="#000000" stop-opacity="0.34" />
          </radialGradient>

          <!-- Dégradés linéaires des orbites dynamiques -->
          <linearGradient
            v-for="arc in frame.arcs"
            :id="`${uid}-${arc.id}`"
            :key="arc.id"
            gradientUnits="userSpaceOnUse"
            :x1="arc.grad.x1"
            :y1="arc.grad.y1"
            :x2="arc.grad.x2"
            :y2="arc.grad.y2"
          >
            <stop
              v-for="(c, i) in arc.grad.stops"
              :key="i"
              :offset="i / (arc.grad.stops.length - 1)"
              :stop-color="c"
            />
          </linearGradient>

          <!-- Gradiente radial de la singularidad / Big Bang -->
          <radialGradient :id="`bb-singularity-${uid}`" cx="50%" cy="50%" r="50%">
            <stop offset="0%" stop-color="#ffffff" stop-opacity="1" />
            <stop offset="25%" stop-color="#67e8f9" stop-opacity="0.95" />
            <stop offset="60%" stop-color="#818cf8" stop-opacity="0.6" />
            <stop offset="100%" stop-color="#a855f7" stop-opacity="0" />
          </radialGradient>

          <!-- Gradiente radial del colapso gravitacional hacia la singularidad -->
          <radialGradient :id="`bb-collapse-${uid}`" cx="50%" cy="50%" r="50%">
            <stop offset="0%" stop-color="#ffffff" stop-opacity="1" />
            <stop offset="30%" stop-color="#38bdf8" stop-opacity="0.9" />
            <stop offset="70%" stop-color="#818cf8" stop-opacity="0.5" />
            <stop offset="100%" stop-color="#0f172a" stop-opacity="0" />
          </radialGradient>

          <!-- Règles CSS dynamiques uniques par instance pour les lettres de sommeil Zzz (D2 §8) -->
          <component :is="'style'">
            @keyframes zAnim1_{{ uid }} {
              0% { transform: translate(22px, -18px) scale(0.62) rotate(-8deg); opacity: 0; }
              18% { opacity: 0.88; }
              78% { opacity: 0.72; }
              100% { transform: translate(42px, -64px) scale(0.88) rotate(4deg); opacity: 0; }
            }
            @keyframes zAnim2_{{ uid }} {
              0% { transform: translate(34px, -34px) scale(0.72) rotate(-5deg); opacity: 0; }
              18% { opacity: 0.92; }
              78% { opacity: 0.76; }
              100% { transform: translate(64px, -102px) scale(1.04) rotate(10deg); opacity: 0; }
            }
            @keyframes zAnim3_{{ uid }} {
              0% { transform: translate(48px, -52px) scale(0.82) rotate(-2deg); opacity: 0; }
              18% { opacity: 0.96; }
              78% { opacity: 0.80; }
              100% { transform: translate(88px, -142px) scale(1.22) rotate(16deg); opacity: 0; }
            }
          </component>
        </defs>

        <!-- Capa de efectos del Mini Big Bang al aparecer -->
        <g v-if="isSpawning" class="bb-fx-layer pointer-events-none">
          <circle cx="0" cy="0" class="bb-shockwave bb-sw-1" />
          <circle cx="0" cy="0" class="bb-shockwave bb-sw-2" />
          <circle cx="0" cy="0" class="bb-shockwave bb-sw-3" />

          <line
            v-for="ray in SPAWN_RAYS"
            :key="`ray-${ray.id}`"
            :x1="ray.x1"
            :y1="ray.y1"
            :x2="ray.x2"
            :y2="ray.y2"
            :stroke="ray.color"
            class="bb-ray"
            :style="{ animationDelay: `${ray.delay}s` }"
          />

          <circle
            v-for="spark in SPAWN_SPARKS"
            :key="`sp-${spark.id}`"
            cx="0"
            cy="0"
            :r="spark.r"
            :fill="spark.color"
            class="bb-spark"
            :style="{
              '--dx': `${spark.dx}px`,
              '--dy': `${spark.dy}px`,
              animationDelay: `${spark.delay}s`
            }"
          />

          <circle cx="0" cy="0" :fill="`url(#bb-singularity-${uid})`" class="bb-singularity" />
        </g>

        <!-- Capa de efectos del Colapso Gravitacional / Warp a modo ventana -->
        <g v-if="isCollapsing" class="bb-collapse-layer pointer-events-none">
          <circle cx="0" cy="0" class="bb-collapse-wave bb-cw-1" />
          <circle cx="0" cy="0" class="bb-collapse-wave bb-cw-2" />
          <circle cx="0" cy="0" :fill="`url(#bb-collapse-${uid})`" class="bb-collapse-flash" />
        </g>

        <!-- Entidad Kyo (cuerpo, ojos, órbitas y partículas) -->
        <g :class="{ 'bb-body-inflate': isSpawning, 'bb-body-collapse': isCollapsing }">
          <!-- Moitié arrière des orbites -->
          <g fill="none" stroke-linecap="round">
            <path
              v-for="arc in frame.arcs"
              :key="`b${arc.id}`"
              :d="arc.back"
              :stroke="`url(#${uid}-${arc.id})`"
              :stroke-width="arc.width"
              :opacity="arc.opacity"
            />
          </g>

          <!-- Particules arrière (ex: burst) -->
          <g v-if="frame.dotsBehind">
            <component
              :is="dot.d ? 'path' : 'circle'"
              v-for="(dot, i) in frame.dots"
              :key="`pb${i}`"
              v-bind="dotAttrs(dot)"
            />
          </g>

          <!-- Corps volumétrique unifié (D2 §3) -->
          <g :opacity="frame.bodyAlpha">
            <!-- 1. Fond des yeux : silhouette sous le corps -->
            <path :d="frame.bodyPath" :fill="eyeFill" />

            <!-- 2. Encre masquée avec volume d'ombre et de lumière -->
            <g :mask="`url(#mask-${uid})`">
              <rect :x="-VB" :y="-VB" :width="VB * 2" :height="VB * 2" :fill="ink" />
              <rect
                v-if="props.volumetric"
                :x="-VB"
                :y="-VB"
                :width="VB * 2"
                :height="VB * 2"
                :fill="`url(#shade-${uid})`"
              />
              <rect
                v-if="props.volumetric"
                :x="-VB"
                :y="-VB"
                :width="VB * 2"
                :height="VB * 2"
                :fill="`url(#light-${uid})`"
              />
            </g>

            <!-- 3. Trait sub-pixel évitant tout effet de crénelage -->
            <path :d="frame.bodyPath" fill="none" :stroke="ink" stroke-width="0.8" />
          </g>

          <!-- Particules avant (notes, cœurs, étincelles, goutte) -->
          <g v-if="!frame.dotsBehind">
            <component
              :is="dot.d ? 'path' : 'circle'"
              v-for="(dot, i) in frame.dots"
              :key="`pf${i}`"
              v-bind="dotAttrs(dot)"
            />
          </g>

          <!-- Pastille de notification bleue -->
          <circle
            v-if="frame.notif"
            :cx="frame.notif.x"
            :cy="frame.notif.y"
            :r="frame.notif.r"
            :fill="NOTIF_BLUE"
          />

          <!-- Moitié avant des orbites -->
          <g fill="none" stroke-linecap="round">
            <path
              v-for="arc in frame.arcs"
              :key="`f${arc.id}`"
              :d="arc.front"
              :stroke="`url(#${uid}-${arc.id})`"
              :stroke-width="arc.width"
              :opacity="arc.opacity"
            />
          </g>

          <!-- Zzz flottants en mode sommeil (D2 §8) -->
          <g v-if="isSleeping" class="bloub-zzz select-none pointer-events-none">
            <text
              x="0"
              y="0"
              font-size="16"
              fill="#818cf8"
              stroke="#ffffff"
              stroke-width="0.8"
              :style="{
                animation: `zAnim1_${uid} 3s 0s cubic-bezier(0.25, 0.46, 0.45, 0.94) infinite`
              }"
            >z</text>
            <text
              x="0"
              y="0"
              font-size="21"
              fill="#a5b4fc"
              stroke="#ffffff"
              stroke-width="1.0"
              :style="{
                animation: `zAnim2_${uid} 3s 1s cubic-bezier(0.25, 0.46, 0.45, 0.94) infinite`
              }"
            >z</text>
            <text
              x="0"
              y="0"
              font-size="26"
              fill="#c084fc"
              stroke="#ffffff"
              stroke-width="1.2"
              :style="{
                animation: `zAnim3_${uid} 3s 2s cubic-bezier(0.25, 0.46, 0.45, 0.94) infinite`
              }"
            >Z</text>
          </g>
        </g>
      </svg>
    </div>
  </div>
</template>

<style scoped>
.bloub-root {
  transform-style: preserve-3d;
}

.bloub-ground {
  will-change: transform, opacity;
  transform-origin: center center;
}

.bloub-zzz {
  filter: drop-shadow(0 2px 10px rgba(129, 140, 248, 0.45));
  font-family: ui-rounded, 'Fredoka', 'Quicksand', system-ui, sans-serif;
  font-weight: 700;
  paint-order: stroke fill;
}

.bloub-zzz text {
  transform-box: view-box;
  transform-origin: 0 0;
}

/* Contenedor del efecto Big Bang */
.bb-fx-layer {
  transform-origin: 0 0;
}

/* 1. Destello de la singularidad central */
.bb-singularity {
  animation: bbFlash 0.65s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

@keyframes bbFlash {
  0% {
    r: 2px;
    opacity: 1;
  }
  35% {
    r: 95px;
    opacity: 0.95;
  }
  100% {
    r: 155px;
    opacity: 0;
  }
}

/* 2. Ondas gravitatorias de choque */
.bb-shockwave {
  fill: none;
  stroke-linecap: round;
}

.bb-sw-1 {
  stroke: rgba(103, 232, 249, 0.9);
  animation: bbWave 0.75s cubic-bezier(0.1, 0.9, 0.2, 1) forwards;
}

.bb-sw-2 {
  stroke: rgba(168, 85, 247, 0.8);
  animation: bbWave 0.85s 0.08s cubic-bezier(0.1, 0.9, 0.2, 1) forwards;
}

.bb-sw-3 {
  stroke: rgba(255, 255, 255, 0.65);
  animation: bbWave 0.95s 0.16s cubic-bezier(0.1, 0.9, 0.2, 1) forwards;
}

@keyframes bbWave {
  0% {
    r: 4px;
    stroke-width: 6px;
    opacity: 1;
  }
  50% {
    stroke-width: 3px;
    opacity: 0.85;
  }
  100% {
    r: 154px;
    stroke-width: 0.5px;
    opacity: 0;
  }
}

/* 3. Rayos de energía / Starburst */
.bb-ray {
  stroke-width: 2.2px;
  stroke-dasharray: 100;
  stroke-linecap: round;
  animation: bbRayAnim 0.7s cubic-bezier(0.15, 0.85, 0.35, 1) forwards;
}

@keyframes bbRayAnim {
  0% {
    stroke-dashoffset: 100;
    opacity: 0;
  }
  25% {
    opacity: 1;
  }
  70% {
    stroke-dashoffset: 15;
    opacity: 0.8;
  }
  100% {
    stroke-dashoffset: -80;
    opacity: 0;
  }
}

/* 4. Chispas de Stardust */
.bb-spark {
  animation: bbSparkAnim 0.85s cubic-bezier(0.12, 0.88, 0.32, 1) forwards;
}

@keyframes bbSparkAnim {
  0% {
    transform: translate(0px, 0px) scale(0);
    opacity: 1;
  }
  25% {
    opacity: 1;
    transform: translate(calc(var(--dx) * 0.4), calc(var(--dy) * 0.4)) scale(1.4);
  }
  75% {
    opacity: 0.75;
    transform: translate(calc(var(--dx) * 0.88), calc(var(--dy) * 0.88)) scale(0.9);
  }
  100% {
    transform: translate(var(--dx), var(--dy)) scale(0);
    opacity: 0;
  }
}

/* 5. Inflación cósmica del cuerpo de Kyo */
.bb-body-inflate {
  transform-origin: 0 0;
  animation: bbInflate 0.95s cubic-bezier(0.175, 0.885, 0.32, 1.275) both;
}

@keyframes bbInflate {
  0% {
    transform: scaleX(0) scaleY(0) rotate(-15deg);
    opacity: 0;
  }
  18% {
    transform: scaleX(0.1) scaleY(0.1) rotate(-8deg);
    opacity: 0.85;
  }
  48% {
    transform: scaleX(1.15) scaleY(0.88) rotate(5deg);
    opacity: 1;
  }
  68% {
    transform: scaleX(0.96) scaleY(1.06) rotate(-2deg);
    opacity: 1;
  }
  84% {
    transform: scaleX(1.02) scaleY(0.98) rotate(1deg);
    opacity: 1;
  }
  100% {
    transform: scaleX(1) scaleY(1) rotate(0deg);
    opacity: 1;
  }
}

/* 6. Despertar de ojos sincronizado */
.bb-eyes-awake {
  transform-origin: 0 0;
  animation: bbEyesAnim 0.95s ease-out both;
}

@keyframes bbEyesAnim {
  0%, 42% {
    opacity: 0;
    transform: scaleY(0);
  }
  48% {
    opacity: 1;
    transform: scaleY(0.1);
  }
  65% {
    opacity: 1;
    transform: scaleY(1.3) scaleX(1.08);
  }
  80% {
    transform: scaleY(0.88);
  }
  100% {
    opacity: 1;
    transform: scaleY(1) scaleX(1);
  }
}

/* ────────────────────────────────────────────────── Efecto Colapso a Modo Ventana */
.bb-collapse-layer {
  transform-origin: 0 0;
}

/* 1. Implosión del cuerpo de Kyo hacia la singularidad */
.bb-body-collapse {
  transform-origin: 0 0;
  animation: bbCollapse 0.28s cubic-bezier(0.55, 0, 0.1, 1) both;
}

@keyframes bbCollapse {
  0% {
    transform: scaleX(1) scaleY(1) rotate(0deg);
    opacity: 1;
  }
  25% {
    transform: scaleX(1.10) scaleY(0.92) rotate(-5deg);
    opacity: 1;
  }
  60% {
    transform: scaleX(0.68) scaleY(0.62) rotate(8deg);
    opacity: 0.88;
  }
  90% {
    transform: scaleX(0.12) scaleY(0.12) rotate(16deg);
    opacity: 0.35;
  }
  100% {
    transform: scaleX(0) scaleY(0) rotate(20deg);
    opacity: 0;
  }
}

/* 2. Colapso sincronizado de los ojos */
.bb-eyes-collapse {
  transform-origin: 0 0;
  animation: bbEyesCollapse 0.22s ease-in both;
}

@keyframes bbEyesCollapse {
  0% {
    transform: scale(1);
    opacity: 1;
  }
  40% {
    transform: scaleY(0.2) scaleX(0.9);
    opacity: 0.9;
  }
  100% {
    transform: scale(0);
    opacity: 0;
  }
}

/* 3. Onda gravitatoria de contracción/succión */
.bb-collapse-wave {
  fill: none;
  stroke-linecap: round;
}

.bb-cw-1 {
  stroke: rgba(56, 189, 248, 0.9);
  animation: bbImplodeWave 0.26s cubic-bezier(0.4, 0, 0.2, 1) both;
}

.bb-cw-2 {
  stroke: rgba(168, 85, 247, 0.75);
  animation: bbImplodeWave 0.26s 0.04s cubic-bezier(0.4, 0, 0.2, 1) both;
}

@keyframes bbImplodeWave {
  0% {
    r: 130px;
    stroke-width: 1px;
    opacity: 0;
  }
  30% {
    stroke-width: 4px;
    opacity: 0.95;
  }
  85% {
    stroke-width: 2px;
    opacity: 0.8;
  }
  100% {
    r: 1px;
    stroke-width: 0.5px;
    opacity: 0;
  }
}

/* 4. Destello concentrado de singularidad en el punto cero */
.bb-collapse-flash {
  animation: bbCollapseFlash 0.28s cubic-bezier(0.15, 0.85, 0.35, 1) both;
}

@keyframes bbCollapseFlash {
  0%, 50% {
    r: 0px;
    opacity: 0;
  }
  75% {
    r: 32px;
    opacity: 1;
  }
  100% {
    r: 1px;
    opacity: 0;
  }
}
</style>
