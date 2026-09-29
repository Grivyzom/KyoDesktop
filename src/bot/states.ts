import {
  COMET_DOT,
  COMET_RIBBONS,
  DOT_PEAK,
  DOT_R,
  DOT_X,
  HEART_GLYPH,
  NOTE_GLYPH,
  NOTIF_ANGLE,
  NOTIF_DIST,
  NOTIF_MARGIN,
  NOTIF_POP,
  NOTIF_R,
  RINGS,
  SPARKLE_GLYPH,
  SWOOSH,
  particles,
  type ArcSpec,
  type DotRender
} from './decor'
import { EYE_H, EYE_SPLIT, EYE_W, REST_GAZE, type HeadGaze } from './face'
import { TAU, clamp, easings, lerp } from './math'
import {
  circle,
  hullOfCircles,
  polyPath,
  profileFromPolygon,
  silhouette,
  type Silhouette
} from './shape'

export interface EyeCfg {
  /** largeur locale (axe court de la gelule), en unites de rayon de boule */
  w: number
  /** hauteur locale (axe long) */
  h: number
  /** 1 = ouvert, 0 = ferme */
  open: number
  /**
   * Inclinaison propre de la gelule, en degres, positif = le haut part a
   * droite. Appliquee APRES le repere tangent de la sphere. Sans elle, les deux
   * yeux penchent forcement du meme cote (le roulis de tete) et la colere comme
   * la tristesse, qui demandent des inclinaisons en miroir, sont hors de portee.
   */
  tilt?: number
}

export interface Pose {
  /** silhouette du corps, en unites de rayon de boule */
  sil: Silhouette
  /** decalage global du corps ET des yeux */
  offX: number
  offY: number
  gaze: HeadGaze
  /** demi-ecart des yeux sur la sphere, en degres */
  split: number
  /** [oeil interieur, oeil exterieur] */
  eyes: [EyeCfg, EyeCfg]
  /** opacite des yeux : sert aux etats sans visage */
  eyeAlpha: number
  bodyAlpha: number
  dots: DotRender[]
  arcs: ArcSpec[]
  notif: { x: number; y: number; r: number; notch: number } | null
  /** true = le decor passe derriere le corps (particules de l'eclatement) */
  dotsBehind: boolean
}

const eye = (w: number, h: number, tilt = 0, open = 1): EyeCfg => ({ w, h, tilt, open })
const pair = (w: number, h: number, tilt = 0, open = 1): [EyeCfg, EyeCfg] => [
  eye(w, h, tilt, open),
  eye(w, h, -tilt, open)
]

function base(over: Partial<Pose> = {}): Pose {
  return {
    sil: circle(1),
    offX: 0,
    offY: 0,
    gaze: { ...REST_GAZE },
    split: EYE_SPLIT,
    eyes: pair(EYE_W, EYE_H),
    eyeAlpha: 1,
    bodyAlpha: 1,
    dots: [],
    arcs: [],
    notif: null,
    dotsBehind: false,
    ...over
  }
}

/* --------------------------------------------------- formes non radiales */

/**
 * Barre du "!" vertical : enveloppe convexe de deux cercles.
 * Mesure : cercle haut (0, -0.505) r 0.132, cercle bas (0, +0.130) r 0.075,
 * flancs rectilignes. Elle est donc tronconique (rapport haut/bas 1.76).
 */
const BAR_UPRIGHT_CY = -0.1875
const BAR_UPRIGHT = profileFromPolygon(
  hullOfCircles(0, -0.505, 0.132, 0, 0.13, 0.075),
  0,
  BAR_UPRIGHT_CY
)

/** Barre du "!" penche : capsule pure (largeur constante 0.269, longueur 0.776). */
const BAR_ITALIC = profileFromPolygon(hullOfCircles(0, -0.2535, 0.1345, 0, 0.2535, 0.1345), 0, 0)

const barUpright = (pose: Partial<Silhouette> = {}): Silhouette => ({
  radii: [...BAR_UPRIGHT],
  rot: 0,
  cx: 0,
  cy: BAR_UPRIGHT_CY,
  sx: 1,
  sy: 1,
  ...pose
})

const barItalic = (pose: Partial<Silhouette> = {}): Silhouette => ({
  radii: [...BAR_ITALIC],
  rot: 0,
  cx: 0,
  cy: 0,
  sx: 1,
  sy: 1,
  ...pose
})

/**
 * Le point du "!" penche n'est pas un disque : c'est une goutte, bout rond
 * (r 0.118) du cote de la barre et pointe effilee a l'oppose, longueur 0.300
 * dans l'axe du glyphe. Centree sur le barycentre du bout rond.
 */
const TEAR = polyPath(hullOfCircles(0, 0, 0.118, 0, 0.172, 0.012))

/**
 * Le triangle ne tourne pas sur lui-meme : son centre decrit un cercle de
 * rayon 0.213 autour de l'origine (mesure). C'est ce decalage qui donne
 * l'impression qu'il bascule au lieu de pivoter sur place.
 */
const TRI_ORBIT = 0.213

function spinningTriangle(rot: number): Silhouette {
  return silhouette('triangle', {
    rot,
    cx: -TRI_ORBIT * Math.sin(rot),
    cy: TRI_ORBIT * Math.cos(rot)
  })
}

/* ------------------------------------------------------------------ etats */

export type StateId =
  | 'idle'
  | 'thinking'
  | 'wink'
  | 'wide'
  | 'alert'
  | 'notify'
  | 'exclaim'
  | 'sleep'
  | 'egg'
  | 'hexagon'
  | 'play'
  | 'orbit'
  | 'burst'
  | 'comet'
  /** transition d'interface, pas une animation du catalogue : hors `SEQUENCE` */
  | 'swirl'
  | 'dance'
  | 'cuddle'
  | 'deepsleep'
  | 'deafen'
  | 'wakeup'
  | 'listen'
  | 'talk'
  | 'ponder'
  | 'joy'
  | 'oops'
  | 'greet'

export interface StateDef {
  id: StateId
  /** duree de maintien quand la sequence complete est jouee */
  duration: number
  /**
   * duree en dessous de laquelle l'animation est coupee avant d'aboutir : le
   * "!" ne revient pas, le corps reste eclate. Elle se lit dans les constantes
   * de `pose` ci-dessous, elle ne se choisit pas. Absente = l'etat ignore le
   * temps ou boucle, n'importe quelle duree lui va (voir `MIN_BLOCK`).
   */
  minDuration?: number
  /** duree du morph d'entree */
  morph: number
  /** true = l'entree est masquee par un clignement, comme dans la video */
  blinkIn: boolean
  /**
   * true = le corps est la silhouette "au repos", donc remplacable par la forme
   * choisie dans le personnalisateur. Les etats qui dessinent leur propre forme
   * (le "!", les points, l'oeuf, le triangle...) valent false : c'est cette forme
   * la qui EST l'animation.
   */
  baseBody: boolean
  /**
   * true = l'etat porte le visage "au repos", donc remplacable par l'expression
   * choisie. Seul `idle` : les autres etats a visage ont une expression relevee
   * sur la video, c'est precisement ce qu'on reproduit.
   */
  baseFace: boolean
  pose(local: number): Pose
}

/** Onde de pulsation qui parcourt les trois points de gauche a droite. */
function dotPulse(t: number, index: number): number {
  const p = ((((t - index * 0.5) / 1.5) % 1) + 1) % 1
  const k = p < 0.5 ? 0.5 - 0.5 * Math.cos(p * TAU) : 0
  return clamp(k * 2)
}

export const STATES: StateDef[] = [
  {
    id: 'idle',
    duration: 2.4,
    morph: 0.45,
    blinkIn: false,
    baseFace: true,
    baseBody: true,
    pose: () => base()
  },

  {
    id: 'thinking',
    duration: 2.6,
    morph: 0.4,
    baseFace: false,
    baseBody: false,
    blinkIn: true,
    pose: (t) => {
      const mid = dotPulse(t, 1)
      // Les points lateraux sortent des flancs de la boule : dans la video ils
      // restent fusionnes avec elle 1-2 frames avant de se detacher.
      const emerge = 0.3 + 0.7 * easings.easeOutCubic(clamp(t / 0.3))
      return base({
        // la boule DEVIENT le point du milieu : le morph reste continu
        sil: circle(DOT_R * (1 + (DOT_PEAK - 1) * mid), { cx: DOT_X[1]! }),
        eyeAlpha: 0,
        dots: [0, 2].map((i) => {
          const k = dotPulse(t, i)
          return {
            x: DOT_X[i]! * emerge,
            y: 0,
            r: DOT_R * (1 + (DOT_PEAK - 1) * k),
            opacity: 0.55 + 0.45 * k
          }
        })
      })
    }
  },

  {
    id: 'wink',
    duration: 1.6,
    morph: 0.3,
    blinkIn: true,
    baseFace: false,
    baseBody: true,
    pose: () =>
      base({
        gaze: { yaw: -5.37, pitch: 4.55, roll: 6.7 },
        split: 16.25,
        // L'oeil ferme n'est pas l'oeil ouvert ecrase : c'est un tiret
        // horizontal PLUS LARGE que l'oeil ouvert (0.447 contre 0.236).
        eyes: [
          { w: 0.236, h: 0.464, open: 1 },
          { w: 0.447, h: 0.089, open: 1 }
        ]
      })
  },

  {
    id: 'wide',
    duration: 1.8,
    morph: 0.55,
    blinkIn: true,
    baseFace: false,
    baseBody: true,
    pose: () =>
      base({
        gaze: { yaw: 6.92, pitch: -21.96, roll: 11.6 },
        split: 18.43,
        eyes: pair(0.356, 0.875)
      })
  },

  {
    id: 'alert',
    duration: 2.4,
    // le "!" revient en place a 1.6 + 0.4
    minDuration: 2,
    morph: 0.45,
    baseFace: false,
    baseBody: false,
    blinkIn: false,
    pose: (t) => {
      // Course mesuree : -0.087 -> +0.732 en 1.5 s, ease-in-out, micro-overshoot.
      const p = clamp(t / 1.5)
      const travel = easings.easeInOutCubic(p) * 0.82 - 0.087
      const back = t > 1.6 ? clamp((t - 1.6) / 0.4) : 0
      const x = travel * (1 - back) + 0.1 * back
      // Vibration secondaire a 2.5 Hz, barre et point en opposition de phase.
      const buzz = Math.sin(t * 2.5 * TAU) * 0.005
      const tilt = (17.7 * Math.PI) / 180
      return base({
        sil: barItalic({ rot: tilt, cx: x, cy: -0.325 - buzz }),
        eyeAlpha: 0,
        dots: [
          {
            // le point suit l'axe du glyphe, a 0.580 du centre de la barre
            x: x - Math.sin(tilt) * 0.58,
            y: -0.325 + Math.cos(tilt) * 0.58 + buzz * 2.8,
            r: 0.118,
            d: TEAR,
            rot: (tilt * 180) / Math.PI,
            opacity: 1
          }
        ]
      })
    }
  },

  {
    id: 'notify',
    duration: 2.2,
    morph: 0.5,
    blinkIn: true,
    baseFace: false,
    baseBody: true,
    pose: (t) => {
      // Pop du point bleu : pic a +14 % vers 0.3 s puis stabilisation.
      const p = clamp(t / 0.45)
      const pop = 1 + (NOTIF_POP - 1) * Math.sin(p * Math.PI) * (1 - p * 0.35)
      const r = NOTIF_R * (p < 1 ? pop : 1)
      const a = (NOTIF_ANGLE * Math.PI) / 180
      return base({
        // le regard part a l'oppose de la pastille
        gaze: { yaw: -21.94, pitch: -5.82, roll: -12.2 },
        split: 18.89,
        eyes: pair(0.505, 0.498),
        notif: {
          x: Math.cos(a) * NOTIF_DIST,
          y: Math.sin(a) * NOTIF_DIST,
          r,
          notch: r + NOTIF_MARGIN
        }
      })
    }
  },

  {
    id: 'exclaim',
    duration: 2,
    morph: 0.45,
    baseFace: false,
    baseBody: false,
    blinkIn: false,
    pose: () =>
      base({
        sil: barUpright(),
        eyeAlpha: 0,
        dots: [{ x: -0.012, y: 0.526, r: 0.113, opacity: 1 }]
      })
  },

  {
    id: 'sleep',
    duration: 2.4,
    morph: 0.5,
    baseFace: false,
    baseBody: true,
    blinkIn: false,
    pose: (t) => {
      const breath = Math.sin(t * (TAU / 2.4)) * 0.032
      return base({
        sil: circle(1, {
          sy: 1 + breath,
          sx: 1 - breath * 0.5,
          cy: 0.02
        }),
        gaze: { yaw: 0, pitch: -15, roll: 0 },
        split: 16,
        eyes: pair(0.24, 0.42, 5, 0.32)
      })
    }
  },

  {
    id: 'egg',
    duration: 1.8,
    morph: 0.4,
    baseFace: false,
    baseBody: false,
    blinkIn: true,
    pose: () =>
      base({
        sil: silhouette('egg'),
        gaze: { yaw: 19.97, pitch: 26.01, roll: -17.1 },
        // les yeux se resserrent comme le corps
        split: 11.07,
        eyes: pair(0.164, 0.385)
      })
  },

  {
    id: 'hexagon',
    duration: 1.6,
    morph: 0.4,
    baseFace: false,
    baseBody: false,
    blinkIn: true,
    pose: () =>
      base({
        sil: silhouette('hexagon'),
        gaze: { yaw: 23.11, pitch: 24.42, roll: -13.3 },
        split: 13.37,
        eyes: pair(0.177, 0.411)
      })
  },

  {
    id: 'play',
    duration: 2,
    morph: 0.5,
    baseFace: false,
    baseBody: false,
    blinkIn: true,
    pose: (t) => {
      // Le triangle reste quasi immobile pendant que le bouquet le traverse.
      const fade = clamp(t / 0.35) * clamp((2.2 - t) / 0.5)
      return base({
        sil: spinningTriangle(0),
        gaze: { yaw: 12, pitch: -8, roll: -6 },
        split: 15,
        eyes: pair(0.18, 0.34),
        // le bouquet balaie de la droite vers la gauche par-dessus le triangle
        arcs: SWOOSH.map((s, i) => ({
          id: `sw${i}`,
          seed: { ...s, cx: 0.45 - t * 0.42 },
          t,
          opacity: fade
        }))
      })
    }
  },

  {
    id: 'orbit',
    duration: 3.4,
    // le corps a fini de se relacher du triangle vers la boule a 1.6 + 0.9
    minDuration: 2.5,
    morph: 0.6,
    baseFace: false,
    baseBody: false,
    blinkIn: false,
    pose: (t) => {
      // Rotation mesuree : rampe sur 0.35 s puis 1.25 tour/s (sens antihoraire).
      const ramp = easings.easeInOutCubic(clamp(t / 0.35))
      const rot = -TAU * 1.25 * t * ramp
      // Le corps se relache du triangle vers la boule pendant l'orbite.
      const back = easings.easeInOutCubic(clamp((t - 1.6) / 0.9))
      const tri = spinningTriangle(rot)
      const ball = circle(1, { rot })
      const sil: Silhouette = {
        radii: tri.radii.map((r, i) => r + (ball.radii[i]! - r) * back),
        rot,
        cx: tri.cx * (1 - back),
        cy: tri.cy * (1 - back),
        sx: 1,
        sy: 1
      }
      const fade = clamp(t / 0.8) * clamp((3.6 - t) / 0.9)
      return base({
        sil,
        // les yeux filent autour de la sphere ~3x plus vite que la silhouette
        gaze: {
          yaw: REST_GAZE.yaw + Math.sin(t * 6.5) * 65 * (1 - back),
          pitch: -4 + back * 32,
          roll: -13
        },
        eyes: pair(0.18, 0.34 + back * 0.07),
        // les anneaux entrent un par un sur 0.8 s
        arcs: RINGS.map((s, i) => ({
          id: `rg${i}`,
          seed: s,
          t,
          opacity: fade * clamp((t - i * 0.13) / 0.3)
        }))
      })
    }
  },

  {
    /**
     * Entree dans la vue des reglages.
     *
     * SEUL etat qui n'est pas releve sur la video : il est CHOISI, comme la
     * couleur `--ink`. Il emprunte le vocabulaire d'`orbit` — les memes anneaux,
     * avec leurs parametres mesures — mais coupe court : 1 s au lieu de 3,4, la
     * moitie des anneaux, et aucun triangle.
     *
     * Les deux drapeaux a `true` sont tout l'interet de cet etat :
     *
     * - `baseBody` laisse la forme choisie remplacer le corps, donc la vue peut
     *   imposer le cercle et le galet ou la goutte y MORPHENT au lieu de sauter ;
     * - `baseFace` fait porter le visage de repos, donc le suivi du curseur
     *   s'applique des cette entree. Un etat qui aurait sa propre pose de regard
     *   (comme `orbit`) rendrait la main a l'etat suivant en pleine course, et
     *   les yeux sauteraient d'un coup a la reprise.
     *
     * Il n'est volontairement PAS dans `SEQUENCE` : ce n'est pas une animation du
     * catalogue, c'est une transition d'interface.
     */
    id: 'swirl',
    // un peu plus que le tour du regard (`TURN_TIME`, 1,1 s) : les yeux doivent
    // etre poses a gauche avant que les anneaux ne s'effacent
    duration: 1.3,
    minDuration: 1.3,
    morph: 0.3,
    baseFace: true,
    baseBody: true,
    // le morph de forme est masque par un clignement, comme partout ailleurs
    blinkIn: true,
    pose: (t) =>
      base({
        // trois anneaux sur les six d'`orbit` : la moitie du bouquet suffit a le
        // reconnaitre, et c'est autant d'arcs en moins a rasteriser par image
        arcs: RINGS.slice(0, 3).map((s, i) => ({
          id: `sw${i}`,
          seed: s,
          t,
          // ils entrent l'un apres l'autre puis s'effacent avant la fin du bloc,
          // pour que la reprise au repos se fasse sur une image deja propre
          opacity: clamp((t - i * 0.06) / 0.14) * clamp((1.22 - t) / 0.34)
        }))
      })
  },

  {
    id: 'burst',
    duration: 2.6,
    // le corps est recompose a 1.7 + 0.7
    minDuration: 2.4,
    morph: 0.4,
    baseFace: false,
    baseBody: false,
    blinkIn: false,
    pose: (t) => {
      // Effondrement mesure : 1.0 -> 0.166 en 0.7 s, ease-out, sans rebond.
      const collapse = 1 - 0.834 * easings.easeOutQuint(clamp(t / 0.7))
      const regrow = easings.easeOutQuint(clamp((t - 1.7) / 0.7))
      return base({
        sil: circle(collapse + (1 - collapse) * regrow),
        eyeAlpha: clamp((t - 1.85) / 0.4),
        dots: particles(t, 1),
        dotsBehind: true
      })
    }
  },

  {
    id: 'comet',
    duration: 2.4,
    // le point se recompose a 1.85 + 0.6 = 2.45, soit 0.05 s apres la coupe de
    // la video : ce reliquat se termine pendant le fondu suivant, comme dans la
    // reference. On ne descend donc pas sous la duree mesuree.
    minDuration: 2.4,
    morph: 0.45,
    baseFace: false,
    baseBody: false,
    blinkIn: false,
    pose: (t) => {
      const collapse = 1 - (1 - COMET_DOT) * easings.easeOutQuint(clamp(t / 0.55))
      const regrow = easings.easeOutQuint(clamp((t - 1.85) / 0.6))
      const fade = clamp((t - 0.15) / 0.25) * clamp((1.95 - t) / 0.3)
      return base({
        // Le point derive de 0.035 vers le bas puis remonte (wobble mesure).
        sil: circle(collapse + (1 - collapse) * regrow, {
          cy: Math.sin(clamp(t / 1.7) * Math.PI) * 0.035
        }),
        eyeAlpha: clamp((t - 2) / 0.35),
        arcs: COMET_RIBBONS.map((s, i) => ({ id: `cm${i}`, seed: s, t, opacity: fade }))
      })
    }
  },
  {
    id: 'dance',
    duration: 12,
    morph: 0.35,
    baseFace: false,
    baseBody: true,
    blinkIn: true,
    pose: (t) => {
      const BEAT = 0.5
      const PHRASE = 1.0
      const cycleNorm = (val: number, p: number) => (((val % p) + p) % p) / p
      const hopOf = (time: number) => Math.abs(Math.sin(Math.PI * cycleNorm(time, BEAT)))
      const landingOf = (time: number) => {
        const p = cycleNorm(time, BEAT)
        return Math.exp(-Math.pow(Math.min(p, 1 - p) / 0.13, 2))
      }

      interface DanceMove {
        offX: number
        offY: number
        rot: number
        sx: number
        sy: number
        yaw: number
        pitch: number
        roll: number
        eyeW: number
        eyeH: number
        eyeTilt: number
      }

      const move0 = (time: number): DanceMove => {
        const sway = Math.sin(TAU * cycleNorm(time, PHRASE))
        const swayLag = Math.sin(TAU * cycleNorm(time - 0.09, PHRASE))
        const hop = hopOf(time)
        const landing = landingOf(time)
        const stretch = 2 * hop - 1
        return {
          offX: 0.05 * sway,
          offY: -0.075 * hop + 0.015 * landing,
          rot: 0.1 * sway,
          sx: 1 - 0.08 * stretch + 0.12 * landing,
          sy: 1 + 0.10 * stretch - 0.14 * landing,
          yaw: REST_GAZE.yaw + 5 * swayLag,
          pitch: REST_GAZE.pitch - 4 * hop,
          roll: REST_GAZE.roll + 9 * swayLag,
          eyeW: 0.24,
          eyeH: 0.22,
          eyeTilt: 18
        }
      }

      const move1 = (time: number): DanceMove => {
        const twist = Math.sin(TAU * cycleNorm(time, PHRASE))
        const pulse = landingOf(time)
        return {
          offX: 0,
          offY: 0.02 * pulse,
          rot: 0.2 * twist,
          sx: 1 + 0.08 * Math.abs(twist),
          sy: 1 - 0.04 * Math.abs(twist),
          yaw: REST_GAZE.yaw - 14 * twist,
          pitch: REST_GAZE.pitch,
          roll: REST_GAZE.roll + 4 * twist,
          eyeW: 0.23,
          eyeH: 0.24,
          eyeTilt: 18 + 4 * pulse
        }
      }

      const move2 = (time: number, loc: number): DanceMove => {
        const phase2 = cycleNorm(time, 2.0)
        const sideSway = Math.sin(TAU * phase2)
        const vel = Math.cos(TAU * phase2)
        const hop = hopOf(time)
        const spinProgress = clamp((loc - 2.0) / 1.0)
        const spin = easings.easeInOutCubic(spinProgress)
        const spinAngle = spin >= 1 ? 0 : 360 * spin
        return {
          offX: 0.13 * sideSway,
          offY: -0.06 * hop,
          rot: 0.12 * vel,
          sx: 1 - 0.05 * hop,
          sy: 1 + 0.06 * hop,
          yaw: REST_GAZE.yaw + 10 * vel + spinAngle,
          pitch: REST_GAZE.pitch,
          roll: REST_GAZE.roll + 6 * vel,
          eyeW: 0.24,
          eyeH: 0.22,
          eyeTilt: 16
        }
      }

      const move3 = (time: number): DanceMove => {
        const p = cycleNorm(time, BEAT)
        const dip = Math.exp(-Math.pow(Math.min(p, 1 - p) / 0.18, 2))
        const grooveRoll = Math.sin(TAU * cycleNorm(time - 0.12, PHRASE))
        return {
          offX: 0.03 * grooveRoll,
          offY: 0.06 * dip,
          rot: 0.06 * grooveRoll,
          sx: 1 + 0.08 * dip,
          sy: 1 - 0.09 * dip,
          yaw: REST_GAZE.yaw + 4 * grooveRoll,
          pitch: REST_GAZE.pitch - 8 * dip,
          roll: REST_GAZE.roll + 15 * grooveRoll,
          eyeW: 0.26,
          eyeH: 0.16 + 0.04 * dip,
          eyeTilt: 20 + 4 * dip
        }
      }

      const MOVES = [
        (time: number, _l: number) => move0(time),
        (time: number, _l: number) => move1(time),
        (time: number, l: number) => move2(time, l),
        (time: number, _l: number) => move3(time)
      ]

      const tc = ((t % 12) + 12) % 12
      const step = Math.floor(tc / 3) % 4
      const local = tc - step * 3
      const curMove = MOVES[step]!(t, local)
      const blendK = easings.easeInOutCubic(clamp(local / 0.25))
      const prevStep = (step + 3) % 4
      const prevMove = MOVES[prevStep]!(t, local + 3)

      const m: DanceMove =
        blendK >= 1
          ? curMove
          : {
              offX: lerp(prevMove.offX, curMove.offX, blendK),
              offY: lerp(prevMove.offY, curMove.offY, blendK),
              rot: lerp(prevMove.rot, curMove.rot, blendK),
              sx: lerp(prevMove.sx, curMove.sx, blendK),
              sy: lerp(prevMove.sy, curMove.sy, blendK),
              yaw: lerp(prevMove.yaw, curMove.yaw, blendK),
              pitch: lerp(prevMove.pitch, curMove.pitch, blendK),
              roll: lerp(prevMove.roll, curMove.roll, blendK),
              eyeW: lerp(prevMove.eyeW, curMove.eyeW, blendK),
              eyeH: lerp(prevMove.eyeH, curMove.eyeH, blendK),
              eyeTilt: lerp(prevMove.eyeTilt, curMove.eyeTilt, blendK)
            }

      const noteColors = ['#818cf8', '#38bdf8', '#c084fc']
      const dots: DotRender[] = [0, 1, 2].map((i) => {
        const n = cycleNorm(t - i * 0.5, 1.5)
        const side = i % 2 === 0 ? 1 : -1
        return {
          x: side * (0.66 + 0.08 * Math.sin(TAU * n)),
          y: 0.16 - 1.12 * n,
          r: 0.1,
          d: NOTE_GLYPH,
          rot: side * 12 + Math.sin(TAU * n) * 8,
          color: noteColors[i % noteColors.length]!,
          depth: 0.7,
          opacity: clamp(n / 0.15) * clamp((1 - n) / 0.2)
        }
      })

      return base({
        sil: circle(1, { rot: m.rot, sx: m.sx, sy: m.sy }),
        offX: m.offX,
        offY: m.offY,
        gaze: { yaw: m.yaw, pitch: m.pitch, roll: m.roll },
        split: 17,
        eyes: [
          { w: m.eyeW, h: m.eyeH, open: 1, tilt: m.eyeTilt },
          { w: m.eyeW, h: m.eyeH, open: 1, tilt: -m.eyeTilt }
        ],
        dots
      })
    }
  },

  {
    id: 'cuddle',
    duration: 0.9,
    morph: 0.22,
    baseFace: false,
    baseBody: true,
    blinkIn: false,
    pose: (t) => {
      const c = ((t % 0.9) + 0.9) % 0.9
      const pat = Math.exp(-Math.pow(c / 0.12, 2))
      const bounce = Math.sin(3 * Math.PI * c) * Math.exp(-3.2 * c)
      const offY = 0.045 + 0.02 * pat + 0.015 * bounce
      const sx = 1.05 + 0.08 * pat
      const sy = 0.95 - 0.07 * pat
      const heartColors = ['#f43f5e', '#fb7185']
      const dots: DotRender[] = [0, 1].map((i) => {
        const u = (((t - i * 0.5) % 1.0) + 1.0) % 1.0
        const side = i === 0 ? 0.45 : -0.42
        return {
          x: side + Math.sin(u * TAU) * 0.06,
          y: -0.2 - u * 0.9,
          r: 0.11,
          d: HEART_GLYPH,
          rot: Math.sin(u * TAU) * 14,
          color: heartColors[i]!,
          depth: 0.85,
          opacity: clamp(u / 0.2) * clamp((1 - u) / 0.2)
        }
      })
      return base({
        sil: circle(1, { sx, sy }),
        offY,
        gaze: { yaw: 0, pitch: -8, roll: 0 },
        split: 16,
        eyes: [
          { w: 0.36, h: 0.09, open: 1, tilt: 22 },
          { w: 0.36, h: 0.09, open: 1, tilt: -22 }
        ],
        dots
      })
    }
  },

  {
    id: 'deepsleep',
    duration: 3.2,
    morph: 0.45,
    baseFace: false,
    baseBody: true,
    blinkIn: false,
    pose: (t) => {
      const breath = Math.sin(t * (TAU / 3.2)) * 0.048
      const u = (((t % 2.4) + 2.4) % 2.4) / 2.4
      const dots: DotRender[] = [
        {
          x: -0.75 + Math.sin(u * TAU) * 0.05,
          y: -0.2 - u * 0.85,
          r: 0.09,
          d: HEART_GLYPH,
          rot: -12 + Math.sin(u * TAU) * 10,
          color: '#fb7185',
          depth: 0.8,
          opacity: clamp(u / 0.2) * clamp((1 - u) / 0.25)
        }
      ]
      return base({
        sil: circle(1, {
          sx: 1 - breath * 0.4,
          sy: 1 + breath,
          cy: 0.04
        }),
        gaze: { yaw: -6, pitch: -18, roll: -3 },
        split: 16,
        eyes: [
          { w: 0.32, h: 0.10, open: 1, tilt: 24 },
          { w: 0.32, h: 0.10, open: 1, tilt: -24 }
        ],
        dots
      })
    }
  },

  {
    id: 'deafen',
    duration: 2.4,
    morph: 0.4,
    baseFace: false,
    baseBody: true,
    blinkIn: false,
    pose: (t) => {
      const breath = Math.sin(t * (TAU / 2.4)) * 0.032
      const earmuffs: DotRender[] = [
        { x: -0.82, y: 0.04, r: 0.22, opacity: 1, color: '#818cf8' },
        { x: 0.82, y: 0.04, r: 0.22, opacity: 1, color: '#818cf8' },
        { x: -0.74, y: 0.04, r: 0.12, opacity: 0.9, color: '#a5b4fc' },
        { x: 0.74, y: 0.04, r: 0.12, opacity: 0.9, color: '#a5b4fc' }
      ]
      return base({
        sil: circle(1, { sy: 1 + breath, sx: 1 - breath * 0.5, cy: 0.03 }),
        gaze: { yaw: 0, pitch: -15, roll: 0 },
        split: 16,
        eyes: pair(0.24, 0.42, 5, 0.32),
        dots: earmuffs
      })
    }
  },

  {
    id: 'wakeup',
    duration: 1.35,
    minDuration: 1.35,
    morph: 0.25,
    baseFace: false,
    baseBody: true,
    blinkIn: false,
    pose: (t) => {
      let sx = 1
      let sy = 1
      let rot = 0
      let eyeW = EYE_W
      let eyeH = EYE_H
      let open = 1
      if (t < 0.5) {
        const p = t / 0.5
        const stretch = Math.sin(p * Math.PI)
        sy = 1 + 0.15 * stretch
        sx = 1 - 0.12 * stretch
        open = 0.32 + 0.28 * p
        eyeW = 0.20
        eyeH = 0.30
      } else if (t < 0.9) {
        const p = (t - 0.45) / 0.45
        rot = Math.sin((t - 0.45) * 8 * Math.PI) * 0.08 * (1 - p)
        sx = 1 + 0.04 * Math.sin(t * 8 * Math.PI)
        sy = 1 - 0.04 * Math.sin(t * 8 * Math.PI)
        open = 0.6 + 0.4 * p
        eyeW = 0.22
        eyeH = 0.45
      } else {
        const p = clamp((t - 0.85) / 0.5)
        const ease = easings.easeOutCubic(p)
        open = 1
        eyeW = lerp(0.22, EYE_W, ease)
        eyeH = lerp(0.45, EYE_H, ease)
      }
      return base({
        sil: circle(1, { sx, sy, rot }),
        gaze: {
          yaw: REST_GAZE.yaw * clamp(t / 1.0),
          pitch: REST_GAZE.pitch * clamp(t / 1.0),
          roll: REST_GAZE.roll * clamp(t / 1.0)
        },
        eyes: pair(eyeW, eyeH, 0, open)
      })
    }
  },

  {
    id: 'listen',
    duration: 1.6,
    morph: 0.35,
    baseFace: true,
    baseBody: true,
    blinkIn: true,
    pose: (t) => {
      const rollTilt = -12 * clamp(t / 0.45)
      const nod = Math.sin(t * (TAU / 1.6)) * 6
      const dots: DotRender[] = [0, 1, 2].map((i) => {
        const u = (((t - i * 0.35) % 1.2) + 1.2) % 1.2 / 1.2
        const dist = 1.34 - u * 0.48
        const angle = (-18 * Math.PI) / 180
        return {
          x: Math.cos(angle) * dist,
          y: Math.sin(angle) * dist,
          r: 0.04 + u * 0.04,
          color: '#38bdf8',
          depth: 0.8,
          opacity: clamp(u / 0.2) * clamp((1 - u) / 0.25)
        }
      })
      return base({
        gaze: { yaw: 8, pitch: REST_GAZE.pitch + nod, roll: REST_GAZE.roll + rollTilt },
        split: 16,
        eyes: pair(0.20, 0.42),
        dots
      })
    }
  },

  {
    id: 'talk',
    duration: 2,
    morph: 0.3,
    baseFace: false,
    baseBody: true,
    blinkIn: false,
    pose: (t) => {
      const syllable = Math.max(0, Math.sin(TAU * 3.6 * t))
      const mod =
        (0.7 + 0.3 * Math.sin(TAU * 0.37 * t)) *
        (0.8 + 0.2 * Math.sin(TAU * 0.23 * t))
      const bounce = syllable * mod
      const sy = 1 + 0.09 * bounce
      const sx = 1 - 0.06 * bounce
      const offY = -0.04 * bounce
      return base({
        sil: circle(1, { sx, sy }),
        offY,
        gaze: {
          yaw: REST_GAZE.yaw + Math.sin(TAU * 0.8 * t) * 6,
          pitch: REST_GAZE.pitch + bounce * 4,
          roll: REST_GAZE.roll + Math.sin(TAU * 1.2 * t) * 3
        },
        split: 16,
        eyes: pair(0.20 + 0.04 * bounce, 0.42 - 0.06 * bounce)
      })
    }
  },

  {
    id: 'ponder',
    duration: 2.4,
    morph: 0.4,
    baseFace: false,
    baseBody: true,
    blinkIn: true,
    pose: (t) => {
      const bubbleCoords = [
        { x: 0.66, y: -0.80, r: 0.07 },
        { x: 0.86, y: -1.02, r: 0.10 },
        { x: 1.10, y: -1.20, r: 0.14 }
      ]
      const cycleTime = ((t % 2.4) + 2.4) % 2.4
      const dots: DotRender[] = bubbleCoords.map((b, i) => {
        const start = 0.2 + i * 0.3
        const alive = clamp((cycleTime - start) / 0.25) * clamp((2.2 - cycleTime) / 0.3)
        return {
          x: b.x,
          y: b.y + Math.sin(t * 2 + i) * 0.02,
          r: b.r * alive,
          color: '#818cf8',
          depth: 0.8,
          opacity: 0.85 * alive
        }
      })
      return base({
        gaze: { yaw: -14, pitch: 36, roll: -8 },
        split: 16,
        eyes: [
          { w: 0.18, h: 0.42, open: 1 },
          { w: 0.22, h: 0.26, open: 0.7 }
        ],
        dots
      })
    }
  },

  {
    id: 'joy',
    duration: 1.6,
    morph: 0.25,
    baseFace: false,
    baseBody: true,
    blinkIn: false,
    pose: (t) => {
      const c = ((t % 1.6) + 1.6) % 1.6
      let offY = 0
      let sx = 1
      let sy = 1
      let sparkOpacity = 0
      if (c < 0.22) {
        const p = c / 0.22
        offY = 0.06 * p
        sy = 1 - 0.12 * p
        sx = 1 + 0.12 * p
      } else if (c < 0.62) {
        const p = (c - 0.22) / 0.40
        const jump = Math.sin(p * Math.PI)
        offY = -0.22 * jump
        sy = 1 + 0.14 * jump
        sx = 1 - 0.10 * jump
      } else if (c < 0.78) {
        const p = (c - 0.62) / 0.16
        const squash = Math.sin(p * Math.PI)
        offY = 0.04 * squash
        sy = 1 - 0.15 * squash
        sx = 1 + 0.15 * squash
        sparkOpacity = Math.sin(p * Math.PI)
      } else {
        const p = (c - 0.78) / 0.82
        const decay = Math.exp(-4 * p)
        offY = -0.03 * Math.sin(p * TAU * 2) * decay
        sy = 1 + 0.05 * Math.sin(p * TAU * 2) * decay
        sx = 1 - 0.05 * Math.sin(p * TAU * 2) * decay
        sparkOpacity = Math.max(0, 1 - p * 3)
      }

      const sparkColors = ['#f59e0b', '#fbbf24', '#f59e0b']
      const angles = [(-90 - 48) * (Math.PI / 180), -90 * (Math.PI / 180), (-90 + 48) * (Math.PI / 180)]
      const dots: DotRender[] =
        sparkOpacity > 0.01
          ? angles.map((ang, i) => ({
              x: Math.cos(ang) * 1.15,
              y: Math.sin(ang) * 1.15,
              r: 0.12,
              d: SPARKLE_GLYPH,
              rot: (ang * 180) / Math.PI + c * 40,
              color: sparkColors[i]!,
              depth: 0.9,
              opacity: sparkOpacity
            }))
          : []

      return base({
        sil: circle(1, { sx, sy }),
        offY,
        gaze: { yaw: 0, pitch: 18, roll: 0 },
        split: 17,
        eyes: [
          { w: 0.26, h: 0.18, open: 1, tilt: 18 },
          { w: 0.26, h: 0.18, open: 1, tilt: -18 }
        ],
        dots
      })
    }
  },

  {
    id: 'oops',
    duration: 2.2,
    morph: 0.3,
    baseFace: false,
    baseBody: true,
    blinkIn: true,
    pose: (t) => {
      const c = ((t % 2.2) + 2.2) % 2.2
      const shake = Math.sin(TAU * 3.2 * c) * Math.exp(-4.5 * (c % 1.1))
      const tearProgress = (c % 1.1) / 1.1
      const dots: DotRender[] = [
        {
          x: 0.48,
          y: -0.2 + tearProgress * 0.42,
          r: 0.10,
          d: TEAR,
          rot: 180,
          color: '#60a5fa',
          depth: 0.85,
          opacity: clamp(tearProgress / 0.2) * clamp((1 - tearProgress) / 0.2)
        }
      ]
      return base({
        sil: circle(1, { rot: shake * 0.12 }),
        gaze: { yaw: shake * 18, pitch: -8, roll: -shake * 6 },
        split: 16,
        eyes: [
          { w: 0.20, h: 0.38, open: 1, tilt: 12 },
          { w: 0.24, h: 0.26, open: 0.85, tilt: -8 }
        ],
        dots
      })
    }
  },

  {
    id: 'greet',
    duration: 1.2,
    minDuration: 1.2,
    morph: 0.2,
    baseFace: false,
    baseBody: true,
    blinkIn: false,
    pose: (t) => {
      const p = clamp(t / 1.2)
      const hop = Math.sin(p * Math.PI) * Math.exp(-2.2 * p)
      const offY = -0.16 * hop
      const sx = 1 - 0.08 * hop
      const sy = 1 + 0.12 * hop
      return base({
        sil: circle(1, { sx, sy }),
        offY,
        gaze: {
          yaw: lerp(0, REST_GAZE.yaw, p),
          pitch: lerp(12, REST_GAZE.pitch, p),
          roll: lerp(0, REST_GAZE.roll, p)
        },
        split: EYE_SPLIT,
        eyes: [
          { w: lerp(0.24, EYE_W, p), h: lerp(0.36, EYE_H, p), open: 1, tilt: lerp(10, 0, p) },
          { w: lerp(0.24, EYE_W, p), h: lerp(0.36, EYE_H, p), open: 1, tilt: lerp(-10, 0, p) }
        ]
      })
    }
  }
]

export const STATE_BY_ID = new Map(STATES.map((s) => [s.id, s]))

/** Ordre de lecture de la sequence complete, calque sur la video de reference. */
/**
 * Date, en temps local, ou chaque etat est le plus lisible : c'est la pose que
 * montrent les vignettes et la planche. Rendu deterministe, donc comparable
 * d'une execution a l'autre. Le type force a couvrir tout nouvel etat.
 */
export const POSES: Record<StateId, number> = {
  idle: 1,
  thinking: 1.1,
  wink: 0.8,
  wide: 0.8,
  alert: 0.75,
  notify: 0.9,
  exclaim: 0.8,
  sleep: 0.45,
  egg: 0.8,
  hexagon: 0.8,
  play: 0.9,
  orbit: 1.2,
  swirl: 0.5,
  burst: 0.45,
  comet: 1.15,
  dance: 1.5,
  cuddle: 0.45,
  deepsleep: 1.6,
  deafen: 1.2,
  wakeup: 0.9,
  listen: 0.8,
  talk: 1.0,
  ponder: 1.2,
  joy: 0.65,
  oops: 1.1,
  greet: 0.6
}

export const SEQUENCE: StateId[] = [
  'idle',
  'thinking',
  'wink',
  'wide',
  'alert',
  'notify',
  'exclaim',
  'sleep',
  'egg',
  'hexagon',
  'play',
  'orbit',
  'burst',
  'comet'
]

const NO_FOLLOW = new Set<StateId>(['sleep', 'deepsleep', 'deafen', 'wakeup', 'cuddle'])

/**
 * Dit si le regard suit le curseur dans cet état.
 * Les états avec cara de repos (baseFace), alert, swirl et dance suivent le curseur.
 */
export function followsCursor(id: StateId): boolean {
  if (NO_FOLLOW.has(id)) return false
  const def = STATE_BY_ID.get(id)
  return !!(def?.baseFace || id === 'alert' || id === 'dance' || id === 'swirl')
}
