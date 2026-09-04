<script setup lang="ts">
import { computed, ref } from 'vue';
import BloubBot from './BloubBot.vue';
import KyoIcon from './KyoIcon.vue';
import SettingSlider from './SettingSlider.vue';
import { settings } from '@/composables/useSettings';
import { COLORS, SHAPES, type ColorId, type ShapeId } from '@/bot/skins';
import { EXPRESSIONS, type ExpressionId } from '@/bot/expressions';

const emit = defineEmits<{ (e: 'change'): void }>();

/**
 * Editor del personalizador nativo de Bloub, adaptado a la pestana de ajustes.
 *
 * Las vinetas son instancias REALES del motor congeladas con `frozenAt`: sin
 * bucle de animacion propia, se dibujan una vez al montarse y no vuelven a
 * costar nada. Asi se ve la forma y la expresion tal como quedaran, sin pagar
 * veinticuatro `requestAnimationFrame` en paralelo.
 */
const POSE = 1.15;

const isPreviewPlaying = ref(true);

const shapeLabels: Record<ShapeId, string> = {
  cercle: 'Círculo',
  galet: 'Guijarro',
  squircle: 'Squircle',
  capsule: 'Cápsula',
  triangle: 'Triángulo',
  hexagone: 'Hexágono',
  nuage: 'Nube',
  goutte: 'Gota'
};

const colorLabels: Record<ColorId, string> = {
  encre: 'Tinta',
  brun: 'Marrón',
  rouge: 'Rojo',
  orange: 'Naranja',
  ambre: 'Ámbar',
  vert: 'Verde',
  turquoise: 'Turquesa',
  bleu: 'Azul',
  violet: 'Violeta',
  rose: 'Rosa',
  gris: 'Gris',
  creme: 'Crema'
};

const expressionLabels: Record<ExpressionId, string> = {
  neutre: 'Neutral',
  attentif: 'Atento',
  surpris: 'Sorprendido',
  excite: 'Entusiasmado',
  heureux: 'Feliz',
  hilare: 'Risueño',
  colere: 'Enfadado',
  triste: 'Triste',
  effraye: 'Asustado',
  mefiant: 'Desconfiado',
  confus: 'Confundido',
  curieux: 'Curioso',
  fier: 'Orgulloso',
  timide: 'Tímido',
  blase: 'Indiferente',
  somnolent: 'Somnoliento'
};

const eyePresets = [
  { hex: '#ffffff', label: 'Blanco puro' },
  { hex: '#f1efe9', label: 'Crema suave' },
  { hex: '#38bdf8', label: 'Cian glaciar' },
  { hex: '#f472b6', label: 'Rosa neón' },
  { hex: '#a3e635', label: 'Lima eléctrico' }
];

/** Fondo del lienzo: el motor lo usa para la bruma de las particulas. */
const paper = computed(() => (settings.theme === 'light' ? '#f8fafc' : '#12141a'));

const currentShapeLabel = computed(
  () => shapeLabels[settings.shape as ShapeId] ?? settings.shape
);
const currentExpressionLabel = computed(
  () => expressionLabels[settings.expression as ExpressionId] ?? settings.expression
);

function pick<K extends keyof typeof settings>(key: K, value: (typeof settings)[K]) {
  if (settings[key] === value) return;
  settings[key] = value;
  emit('change');
}
</script>

<template>
  <div class="space-y-4">
    <!-- Lienzo de vista previa -->
    <section class="kyo-card overflow-hidden">
      <header class="flex items-center justify-between px-4 pt-3.5">
        <div class="flex items-center gap-2">
          <KyoIcon name="sparkles" class="w-3.5 h-3.5 text-blue-500" />
          <h3 class="kyo-card-title">Editor Bloub</h3>
        </div>
        <button
          type="button"
          class="kyo-chip"
          :title="isPreviewPlaying ? 'Congelar vista previa' : 'Animar vista previa'"
          @click="isPreviewPlaying = !isPreviewPlaying"
        >
          <KyoIcon :name="isPreviewPlaying ? 'pause' : 'play'" class="w-3.5 h-3.5" />
          <span>{{ isPreviewPlaying ? 'Animado' : 'Congelado' }}</span>
        </button>
      </header>

      <div class="kyo-canvas mt-3 flex items-center justify-center py-5">
        <BloubBot
          :key="isPreviewPlaying ? 'live' : 'static'"
          :size="Math.min(settings.avatarSize, 150)"
          :shape="settings.shape"
          :color="settings.color"
          :expression="settings.expression"
          :eye-color="settings.eyeColor"
          :paper="paper"
          :frozen-at="isPreviewPlaying ? undefined : POSE"
        />
      </div>

      <footer class="flex items-center justify-between gap-3 px-4 py-2.5 border-t kyo-divider">
        <span class="kyo-hint">
          {{ currentShapeLabel }} · {{ currentExpressionLabel }}
        </span>
        <span class="kyo-value">{{ settings.avatarSize }} px</span>
      </footer>
    </section>

    <!-- Forma base -->
    <section class="kyo-card p-4 space-y-3">
      <div class="flex items-center gap-2">
        <KyoIcon name="shapes" class="w-3.5 h-3.5 text-blue-500" />
        <h3 class="kyo-card-title">Forma base</h3>
      </div>
      <p class="kyo-hint">Silueta generada por armónicos radiales del motor Bloub.</p>

      <div class="grid grid-cols-4 gap-2">
        <button
          v-for="s in SHAPES"
          :key="s.id"
          type="button"
          class="kyo-tile"
          :class="settings.shape === s.id ? 'kyo-tile-on' : ''"
          :title="shapeLabels[s.id]"
          @click="pick('shape', s.id)"
        >
          <BloubBot
            :size="40"
            :shape="s.id"
            :color="settings.color"
            :expression="settings.expression"
            :eye-color="settings.eyeColor"
            :paper="paper"
            :frozen-at="POSE"
          />
          <span class="kyo-tile-label">{{ shapeLabels[s.id] }}</span>
        </button>
      </div>
    </section>

    <!-- Expresión de reposo -->
    <section class="kyo-card p-4 space-y-3">
      <div class="flex items-center gap-2">
        <KyoIcon name="face" class="w-3.5 h-3.5 text-blue-500" />
        <h3 class="kyo-card-title">Expresión de reposo</h3>
      </div>
      <p class="kyo-hint">Pose facial por defecto: orientación de cabeza, apertura e inclinación de cada ojo.</p>

      <div class="grid grid-cols-4 gap-2">
        <button
          v-for="e in EXPRESSIONS"
          :key="e.id"
          type="button"
          class="kyo-tile"
          :class="settings.expression === e.id ? 'kyo-tile-on' : ''"
          :title="expressionLabels[e.id]"
          @click="pick('expression', e.id)"
        >
          <BloubBot
            :size="38"
            :shape="settings.shape"
            :color="settings.color"
            :expression="e.id"
            :eye-color="settings.eyeColor"
            :paper="paper"
            :frozen-at="POSE"
          />
          <span class="kyo-tile-label">{{ expressionLabels[e.id] }}</span>
        </button>
      </div>
    </section>

    <!-- Color del cuerpo -->
    <section class="kyo-card p-4 space-y-3">
      <div class="flex items-center gap-2">
        <KyoIcon name="palette" class="w-3.5 h-3.5 text-blue-500" />
        <h3 class="kyo-card-title">Tinta del cuerpo</h3>
      </div>
      <p class="kyo-hint">Paleta original del personalizador de Bloub.</p>

      <div class="flex flex-wrap items-center gap-2.5">
        <button
          v-for="c in COLORS"
          :key="c.id"
          type="button"
          class="kyo-swatch"
          :class="settings.color === c.id ? 'kyo-swatch-on' : ''"
          :style="{ backgroundColor: c.hex }"
          :title="colorLabels[c.id]"
          @click="pick('color', c.id)"
        >
          <KyoIcon
            v-if="settings.color === c.id"
            name="check"
            :width="3"
            class="w-3.5 h-3.5 text-white mix-blend-difference"
          />
        </button>
      </div>
    </section>

    <!-- Color de ojos -->
    <section class="kyo-card p-4 space-y-3">
      <div class="flex items-center gap-2">
        <KyoIcon name="eye" class="w-3.5 h-3.5 text-blue-500" />
        <h3 class="kyo-card-title">Color de ojos</h3>
      </div>
      <p class="kyo-hint">Los ojos son perforaciones del cuerpo: este color es el que se ve a través.</p>

      <div class="flex flex-wrap items-center gap-2.5">
        <button
          v-for="e in eyePresets"
          :key="e.hex"
          type="button"
          class="kyo-swatch"
          :class="settings.eyeColor === e.hex ? 'kyo-swatch-on' : ''"
          :style="{ backgroundColor: e.hex }"
          :title="e.label"
          @click="pick('eyeColor', e.hex)"
        >
          <KyoIcon
            v-if="settings.eyeColor === e.hex"
            name="check"
            :width="3"
            class="w-3.5 h-3.5 text-slate-900"
          />
        </button>

        <label class="kyo-chip cursor-pointer">
          <KyoIcon name="palette" class="w-3.5 h-3.5" />
          <span>Personalizado</span>
          <input
            type="color"
            class="w-4 h-4 rounded border-0 bg-transparent p-0 cursor-pointer"
            :value="settings.eyeColor"
            @change="pick('eyeColor', ($event.target as HTMLInputElement).value)"
          />
        </label>
      </div>
    </section>

    <!-- Tamaño -->
    <section class="kyo-card p-4 space-y-3">
      <div class="flex items-center justify-between gap-4">
        <div class="flex items-center gap-2">
          <KyoIcon name="resize" class="w-3.5 h-3.5 text-blue-500" />
          <div>
            <h3 class="kyo-card-title">Tamaño del avatar</h3>
            <p class="kyo-hint">Escala en píxeles del avatar flotante en el escritorio.</p>
          </div>
        </div>
        <span class="kyo-value">{{ settings.avatarSize }} px</span>
      </div>
      <SettingSlider
        v-model="settings.avatarSize"
        :min="120"
        :max="175"
        :step="5"
        label="Tamaño del avatar"
        @commit="emit('change')"
      />
    </section>
  </div>
</template>
