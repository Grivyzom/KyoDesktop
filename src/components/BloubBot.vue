<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, triggerRef, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { NOTIF_BLUE } from '@/bot/decor'
import { BotEngine, type BotFrame } from '@/bot/engine'
import { clamp, easings } from '@/bot/math'
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
import { STATE_BY_ID, type StateId } from '@/bot/states'

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
     * Le moteur etant une fonction pure du temps, on obtient une image
     * reproductible au pixel pres, sans boucle d'animation : utile pour une
     * planche d'etats, une vignette de personnalisateur ou un test.
     */
    frozenAt?: number
    /**
     * Montage joue par le lecteur : une suite d'etats, chacun tenu la duree de
     * son bloc. Par defaut, le cycle releve sur la video.
     */
    cycle?: Block[]
    /**
     * Le regard suit le pointeur. Hors de portee des vignettes figees, qui n'ont
     * pas de boucle d'animation : les brancher reveillerait autant de boucles
     * qu'il y a de cases.
     */
    follow?: boolean
    /**
     * Regard scripte de l'arrivee : evalue a chaque image avec le temps ecoule
     * depuis qu'il a ete pose. Independant de `follow`, qui vise le pointeur —
     * ici c'est le script qui decide de tout, y compris de sa duree.
     */
    gaze?: GazeScript | null
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
    gaze: null
  }
)

/**
 * Le curseur de lecture est un **index de bloc**, pas un etat : un montage peut
 * jouer deux fois le meme etat, et il faut alors savoir dans lequel des deux on
 * se trouve. `state` suit le bloc courant — c'est une sortie, l'exterieur pilote
 * la lecture par `block`.
 */
const block = defineModel<number>('block', { default: 0 })
const state = defineModel<StateId>('state', { default: 'idle' })
const playing = defineModel<boolean>('playing', { default: false })
/** Temps ecoule dans le bloc courant, pour la tete de lecture de la timeline. */
const elapsed = defineModel<number>('elapsed', { default: 0 })

// Le repere vient de `src/bot/` : c'est lui qui definit ce que le moteur rend, le
// composant n'en est qu'un client. Les noms courts restent, ils sont partout dans le
// gabarit.
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
/** Date d'horloge a laquelle le bloc courant a commence. */
let blockStart = 0

/**
 * Pose le bloc `i` : etat, moteur, et date de fin. Appele aussi bien par la
 * boucle que par le watcher, d'ou l'absence d'effet de bord sur `block` — c'est
 * l'appelant qui decide s'il deplace le curseur.
 */
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

/** Deplace le curseur et recale le moteur dans la foulee, sans passer par le watcher. */
function goToBlock(i: number) {
  block.value = i
  apply(i)
}

/**
 * Deplacement de la tete de lecture depuis la timeline : on tombe au milieu d'un
 * bloc, pas a son debut. L'offset transite par une variable plutot que par un
 * appel direct a `apply` : changer `block` declenchera le watcher, qui doit
 * poser la meme date que nous — sinon il remettrait le bloc a zero juste apres.
 */
let pendingOffset = 0

function seek(index: number, offset = 0) {
  if (block.value === index) {
    apply(index, offset)
    return
  }
  pendingOffset = offset
  block.value = index
}

/**
 * Rend le MONTAGE a la date absolue `t`, sans horloge. C'est ce qui permet de
 * capturer un cycle entier hors ecran, image par image et plus vite que le temps
 * reel.
 *
 * Pourquoi une methode a part, et pas `frozenAt` : `frozenAt` fige le temps DANS
 * l'etat courant, il ne parcourt pas les blocs. Et passer par `seek` ne suffirait
 * pas — `apply` cale le moteur sur `clock`, qui n'avance que dans la boucle et
 * reste donc a zero en mode fige. Tous les changements d'etat s'enregistreraient
 * a l'instant 0, et les fondus aux jointures de blocs seraient faux.
 *
 * D'ou le `setState` a l'offset ABSOLU du bloc : le moteur date ainsi la
 * transition la ou elle a vraiment lieu dans le cycle, et `sample(t)` retombe sur
 * la meme image que la lecture temps reel aurait produite.
 */
let dernierBloc = -1

function rendAt(t: number) {
  const blocs = props.cycle
  if (!blocs.length) return
  const { index } = blockAt(blocs, t)
  if (index !== dernierBloc) {
    const b = blocs[index]!
    state.value = b.state
    /*
     * Un RETOUR EN ARRIERE repart sans historique, la ou une avancee normale garde l'etat
     * quitte pour le fondre. Sans cette distinction, rejouer l'image 0 apres une passe
     * complete datait le premier etat a l'instant 0 avec le DERNIER en etat precedent, et
     * rendait donc la pose de celui-la : l'export GIF, qui fait deux passes, s'ouvrait sur
     * une boule sans yeux. Le lecteur est ainsi idempotent, et une passe peut etre rejouee
     * autant de fois qu'on veut.
     */
    if (index < dernierBloc) engine.reset(b.state, offsetOf(blocs, index))
    else engine.setState(b.state, offsetOf(blocs, index))
    dernierBloc = index
  }
  frame.value = engine.sample(t)
  triggerRef(frame)
}

/* ------------------------------------------------------- regard qui suit */

const svg = ref<SVGSVGElement | null>(null)

/** true = une cible est posee sur le moteur, donc il y a de quoi relacher. */
let aiming = false
/** Date d'horloge a laquelle le demi-tour a commence. */
let turnSince = 0

/** Coordonnees relatives de la souris a travers l'ensemble des ecrans. */
let globalDx = 0
let globalDy = 0
let lastCursorMove = 0
let inFlightCursor = false
/**
 * Cadence de l'interrogation du pointeur natif.
 *
 * Une image sur soixante, c'etait un aller-retour IPC vers Rust a CHAQUE image :
 * le pont serialise, reveille le thread principal de la fenetre et repasse par
 * l'evenement — soixante fois par seconde, pour un curseur qui ne bouge jamais
 * assez vite pour que 40 ms se voient. Le regard reste fluide, la machine
 * respire.
 */
const CURSOR_POLL = 0.04
let lastCursorPoll = -Infinity

async function trackGlobalCursor() {
  if (inFlightCursor) return
  inFlightCursor = true
  try {
    const [dx, dy] = await invoke<[number, number]>('get_cursor_relative')
    if (Math.abs(dx - globalDx) > 0.5 || Math.abs(dy - globalDy) > 0.5) {
      lastCursorMove = clock
    }
    globalDx = dx
    globalDy = dy
  } catch {
    // Si execute hors de Tauri ou en attente d'initialisation
  } finally {
    inFlightCursor = false
  }
}

function onPointerMove(event: PointerEvent) {
  if (event.pointerType === 'touch') return
  lastCursorMove = clock
}

function onPointerLeave() {
  // Pas de reinitialisation : le tracker global prend le relais hors de la fenetre
}

function release() {
  if (!aiming) return
  // meme duree qu'a l'aller : la tete revient pendant que la boule redescend
  engine.setLook(null, clock, TURN_TIME)
  aiming = false
}

/**
 * Vise le pointeur global a travers tous les ecrans.
 */
function aim() {
  // Le regard ne se pilote que sur les etats a VISAGE DE REPOS.
  if (!STATE_BY_ID.get(state.value)?.baseFace || !svg.value) {
    release()
    return
  }
  if (!aiming) {
    turnSince = clock
    lastCursorMove = clock
  }

  // Interroge le pointeur natif du systeme (moniteurs multiples), a cadence bornee
  if (clock - lastCursorPoll >= CURSOR_POLL) {
    lastCursorPoll = clock
    trackGlobalCursor()
  }

  // Portee de normalisation a l'echelle du moniteur (ou des moniteurs)
  const demiLargeur = Math.max(700, (window.screen.availWidth || 1920) / 2)
  const demiHauteur = Math.max(450, (window.screen.availHeight || 1080) / 2)

  const nx = clamp(globalDx / demiLargeur, -1, 1)
  const ny = clamp(globalDy / demiHauteur, -1, 1)

  // Si la souris n'a pas bouge depuis plus de 3.5 secondes, on permet a l'avatar
  // de respirer et cligner des yeux avec une legere derive naturelle (wander)
  const isRecentlyActive = (clock - lastCursorMove) < 3.5

  engine.setLook(
    lookTarget({
      nx,
      ny,
      tour: easings.easeOutQuint(clamp((clock - turnSince) / TURN_TIME)),
      pointer: isRecentlyActive
    }),
    clock
  )
  aiming = true
}

/* ------------------------------------------------- regard d'arrivee */

/** Date d'horloge a laquelle le script de regard a ete pose. */
let gazeSince = 0
/** true = un script tourne, donc il y a de quoi relacher. */
let scripted = false

/**
 * Rattrapage court, la ou le suivi du pointeur prend celui du moteur : le script
 * EST l'animation, et laisser le moteur en lisser une seconde par-dessus
 * retarderait son depart d'un quart de seconde — un script qui commence par
 * regarder au loin verrait ses yeux partir de la pose, y revenir, puis repartir.
 *
 * Non nul quand meme : a zero, `lookAtTime` divise zero par zero a l'image ou la
 * cible est posee, et un `NaN` s'installe dans le moteur pour de bon.
 */
const SCRIPT_MORPH = 1 / 60

/**
 * Le script decide de tout, y compris de sa duree : on ne fait que lui donner le
 * temps ecoule. La regle elle-meme est dans `@/ui/gaze`, comme celle du suivi.
 */
function scriptedGaze(run: GazeScript) {
  engine.setLook(run(clock - gazeSince), clock, SCRIPT_MORPH)
}

/**
 * Depart et relachement du script. `immediate` parce que la page s'ouvre DEJA en
 * arrivee — c'est meme son seul usage — et le relachement parce qu'un script
 * coupe avant sa fin (bloc raccourci, changement de vue) laisserait sinon les
 * yeux figes la ou il s'est arrete : le moteur GARDE la derniere cible.
 */
watch(
  () => props.gaze,
  (run) => {
    if (run) {
      gazeSince = clock
      scripted = true
      /*
       * Valeur de depart posee DATEE D'UN RATTRAPAGE PLUS TOT, pour qu'elle soit
       * deja pleinement appliquee a la premiere image.
       *
       * Sans ca, le moteur rend la premiere image avec le regard neutre et la
       * seconde avec celui du script : les yeux sautent d'un coup entre les deux.
       * Discret pour un script qui commence au repos, spectaculaire pour un qui
       * commence en regardant au loin — 127 px d'un coup sur une boule de 100 de
       * rayon, ce qui est exactement le defaut que ces scripts corrigent.
       */
      engine.setLook(run(0), clock - SCRIPT_MORPH, SCRIPT_MORPH)
      return
    }
    if (!scripted) return
    engine.setLook(null, clock)
    scripted = false
  },
  { immediate: true }
)

function tick(ms: number) {
  raf = requestAnimationFrame(tick)
  // Horloge de scene a delta borne : un onglet masque puis reaffiche reprend
  // sans sauter en avant (rAF est suspendu pendant ce temps-la).
  const dt = last ? Math.min((ms - last) / 1000, 0.064) : 0
  last = ms
  clock += dt

  // Le stop ne gele pas l'horloge : arrete, le bot continue de respirer et de
  // cligner. Seuls l'enchainement du montage et la tete de lecture sont
  // suspendus.
  if (playing.value) {
    if (clock >= nextAt && props.cycle.length) {
      goToBlock((block.value + 1) % props.cycle.length)
    } else {
      elapsed.value = clock - blockStart
    }
  }

  // Le suivi prime : les deux ecrivent la meme cible, et l'arrivee est terminee
  // bien avant qu'une vue a suivi ne s'ouvre.
  if (props.follow) aim()
  else if (props.gaze) scriptedGaze(props.gaze)

  frame.value = engine.sample(clock)
  triggerRef(frame)
}

/** Redessine sans la boucle : sert aux vignettes figees quand la forme change. */
function redrawFrozen() {
  if (props.frozenAt === undefined) return
  frame.value = engine.sample(props.frozenAt)
  triggerRef(frame)
}

// Curseur deplace de l'exterieur : clic sur un bloc de la timeline. Quand c'est
// `goToBlock` qui l'a bouge, `apply` est deja passe et repasse ici sans effet
// (setState sort si l'etat n'a pas change, la date de fin est la meme).
watch(block, (i) => {
  apply(i, pendingOffset)
  pendingOffset = 0
})

// Changement d'etat venu d'une prop : c'est le cas des vignettes figees, qui
// n'ont pas de curseur. En lecture, `apply` a deja fait le travail.
//
// Le garde n'est pas une optimisation, il corrompait une image a CHAQUE jointure
// de bloc a l'export : `rendAt` pose l'etat sur le moteur a son offset ABSOLU
// puis echantillonne a la bonne date, mais le watcher — dont la file est videe
// par le `nextTick` de l'export — repassait ensuite un `redrawFrozen()` non
// inerte (le lecteur hors ecran est monte avec `frozenAt: 0`). Or `sample(0)`
// juste apres un changement date a l'offset donne un ratio de melange nul, donc
// la pose de l'etat PRECEDENT : le point d'exclamation revenait au depart de sa
// course pendant une image, treize fois dans le montage par defaut.
watch(state, (id) => {
  // `rendAt` ou `apply` l'a deja applique, et a la bonne date
  if (engine.state === id) return
  engine.setState(id, clock)
  redrawFrozen()
})

// Reprise la ou la tete de lecture s'est arretee, pas au debut du bloc.
watch(playing, (on) => {
  if (on) apply(block.value, elapsed.value)
  else nextAt = Infinity
})

// Le montage a change sous nos pieds : bloc supprime, duree tiree, autre cycle
// choisi. On garde le curseur dans les bornes et on recale la date de fin sur
// la nouvelle duree — si le bloc a ete raccourci sous la position courante, la
// boucle passe au suivant des la frame suivante, ce qui est le comportement
// voulu.
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
  // on passe l'horloge : le moteur morphe vers la nouvelle forme au lieu de
  // l'appliquer d'un coup
  engine.setShape(radii, clock)
  redrawFrozen()
})

watch(expression, (expr) => {
  engine.setExpression(expr, clock)
  redrawFrozen()
})

/**
 * Deplacer `frozenAt` redessine. La prop ne servait qu'a poser une vignette une
 * fois pour toutes, donc personne ne la bougeait ; l'export anime, lui, avance
 * image par image sur une instance hors ecran. Sans ce watcher elle reste sur sa
 * premiere image et l'animation exportee ne bouge pas.
 */
watch(() => props.frozenAt, redrawFrozen)

/**
 * L'ecoute du pointeur ne vit que le temps du suivi. `immediate` parce que la
 * vue peut s'ouvrir deja en mode suivi ; le garde sur `frozenAt` parce qu'une
 * vignette figee n'a pas de boucle pour consommer la cible, donc rien a ecouter.
 */
watch(
  () => props.follow && props.frozenAt === undefined,
  (on) => {
    if (on) {
      window.addEventListener('pointermove', onPointerMove)
      document.addEventListener('pointerleave', onPointerLeave)
      return
    }
    detach()
    release()
  },
  { immediate: true }
)

function detach() {
  window.removeEventListener('pointermove', onPointerMove)
  document.removeEventListener('pointerleave', onPointerLeave)
}

onMounted(() => {
  if (props.frozenAt !== undefined) return
  // le curseur peut arriver deja pose (URL, cycle relu du stockage)
  apply(block.value, elapsed.value)
  raf = requestAnimationFrame(tick)
  // Activar la animación Big Bang de aparición inicial en el primer frame compuesto
  // (isSpawning arranca en false para no romper vignettes frozen; aquí lo encendemos)
  isSpawning.value = true
  spawnTimer = setTimeout(() => {
    isSpawning.value = false
  }, 1150)
})
onBeforeUnmount(() => {
  cancelAnimationFrame(raf)
  if (spawnTimer) clearTimeout(spawnTimer)
  if (collapseTimer) clearTimeout(collapseTimer)
  detach()
})

/**
 * Un point est un simple disque, sauf quand l'etat fournit une forme (la
 * goutte du "!" penche) : le path est alors en unites de rayon de boule et
 * centre sur l'origine, donc on le pose avec translate/rotate/scale.
 *
 * La couleur suit celle du corps par defaut ; `depth` sert aux particules, qui
 * se fondent dans le fond a mesure qu'elles s'eloignent.
 */
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

/* ---------------------------------------------------- animación Big Bang & Colapso */

// Comienza en false: el onMounted activa la animación sólo si el componente no es
// una vignette fija (frozenAt === undefined). Las vignettes congeladas nunca deben
// entrar en el flujo de animación de spawn y quedar atascadas en scale(0).
const isSpawning = ref(false)
const isCollapsing = ref(false)
let spawnTimer: ReturnType<typeof setTimeout> | null = null
let collapseTimer: ReturnType<typeof setTimeout> | null = null

function triggerSpawn() {
  if (spawnTimer) clearTimeout(spawnTimer)
  if (collapseTimer) clearTimeout(collapseTimer)
  isCollapsing.value = false
  // Forzar ciclo false→true aunque ya sea false: Vue sólo puede reiniciar la
  // animación CSS si el elemento se desmonta (isSpawning=false) y se remonta
  // (isSpawning=true). Un simple true→true no desmonta el nodo y la animación
  // no se reinicia. El requestAnimationFrame garantiza que Vue procese el
  // unmount antes de re-montar en el siguiente tick del compositor.
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
      // Limpiar el estado de colapso al finalizar para que la capa FX
      // salga del DOM y no deje nodos muertos invisibles flotando.
      isCollapsing.value = false
      resolve()
    }, 280)
  })
}

defineExpose({ seek, rendAt, triggerSpawn, triggerCollapse })

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
</script>

<template>
  <svg
    ref="svg"
    :width="props.size"
    :height="props.size"
    :viewBox="`${-VB} ${-VB} ${VB * 2} ${VB * 2}`"
    shape-rendering="geometricPrecision"
    text-rendering="geometricPrecision"
    role="img"
    :aria-label="t('app.botAria')"
    class="block select-none cursor-grab active:cursor-grabbing gpu-composite contain-render"
    style="will-change: transform;"
  >
    <defs>
      <!--
        Les yeux sont de vrais trous perces dans le corps (comme sur x.ai), pas
        des formes blanches posees dessus : ils restent donc automatiquement
        rognes par la silhouette quand ils glissent vers le bord.
      -->
      <clipPath :id="`clip-${uid}`">
        <path :d="frame.bodyPath" />
      </clipPath>

      <mask
        v-if="frame.notch"
        :id="`notch-${uid}`"
        maskUnits="userSpaceOnUse"
        :x="-VB"
        :y="-VB"
        :width="VB * 2"
        :height="VB * 2"
      >
        <rect :x="-VB" :y="-VB" :width="VB * 2" :height="VB * 2" fill="#fff" />
        <circle
          :cx="frame.notch.x"
          :cy="frame.notch.y"
          :r="frame.notch.r"
          fill="#000"
        />
      </mask>

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
    </defs>

    <!-- Capa de efectos del Mini Big Bang al aparecer -->
    <g v-if="isSpawning" class="bb-fx-layer pointer-events-none">
      <!-- Ondas de choque concéntricas -->
      <circle cx="0" cy="0" class="bb-shockwave bb-sw-1" />
      <circle cx="0" cy="0" class="bb-shockwave bb-sw-2" />
      <circle cx="0" cy="0" class="bb-shockwave bb-sw-3" />

      <!-- Rayos de energía estelares / Starburst -->
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

      <!-- Chispas cósmicas (stardust) en 360 grados -->
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

      <!-- Destello radiante central de la singularidad -->
      <circle cx="0" cy="0" :fill="`url(#bb-singularity-${uid})`" class="bb-singularity" />
    </g>

    <!-- Capa de efectos del Colapso Gravitacional / Warp a modo ventana -->
    <g v-if="isCollapsing" class="bb-collapse-layer pointer-events-none">
      <!-- Ondas gravitatorias de implosión / contracción hacia el centro -->
      <circle cx="0" cy="0" class="bb-collapse-wave bb-cw-1" />
      <circle cx="0" cy="0" class="bb-collapse-wave bb-cw-2" />

      <!-- Destello final de la singularidad al comprimirse -->
      <circle cx="0" cy="0" :fill="`url(#bb-collapse-${uid})`" class="bb-collapse-flash" />
    </g>

    <!-- Entidad de Kyo (cuerpo, ojos, órbitas y partículas) -->
    <g :class="{ 'bb-body-inflate': isSpawning, 'bb-body-collapse': isCollapsing }">
      <!-- moitie arriere des orbites : dessinee avant le corps, donc occultee -->
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

      <!-- particules de l'eclatement : elles passent derriere le noyau -->
      <g v-if="frame.dotsBehind">
        <component
          :is="dot.d ? 'path' : 'circle'"
          v-for="(dot, i) in frame.dots"
          :key="`pb${i}`"
          v-bind="dotAttrs(dot)"
        />
      </g>

      <g :opacity="frame.bodyAlpha">
        <!-- Corps noir pur : antialiasing direct et lisse sans frange blanche ni serrucho -->
        <path
          :d="frame.bodyPath"
          :fill="ink"
          :mask="frame.notch ? `url(#notch-${uid})` : undefined"
        />

        <!-- Yeux en blanc pur dessines au-dessus et rognes au contour du corps -->
        <g :clip-path="`url(#clip-${uid})`" :class="{ 'bb-eyes-awake': isSpawning, 'bb-eyes-collapse': isCollapsing }">
          <path
            v-for="(eye, i) in frame.eyes"
            :key="i"
            :d="eye.d"
            :transform="eye.matrix"
            :opacity="eye.alpha"
            :fill="eyeFill"
          />
        </g>
      </g>

      <g v-if="!frame.dotsBehind">
        <component
          :is="dot.d ? 'path' : 'circle'"
          v-for="(dot, i) in frame.dots"
          :key="`pf${i}`"
          v-bind="dotAttrs(dot)"
        />
      </g>

      <circle
        v-if="frame.notif"
        :cx="frame.notif.x"
        :cy="frame.notif.y"
        :r="frame.notif.r"
        :fill="NOTIF_BLUE"
      />

      <!-- moitie avant des orbites -->
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
    </g>
  </svg>
</template>

<style scoped>
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
    /* Sobre-expansión asimétrica: aplastado horizontal con rebote elástico */
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
    /* Anticipación: leve sobre-escala asimétrica (squash horizontal, stretch vertical) */
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
