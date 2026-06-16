<template>
  <div class="text-block-settings" :class="{ card: !embedded, embedded }">
    <div class="text-block-header">
      <p class="section-title">文本块设置</p>
      <label class="toggle-label">
        <input v-model="s.textBlockEnabled" type="checkbox" @change="save" />
        启用文本块
      </label>
    </div>

    <template v-if="s.textBlockEnabled">
      <div class="text-grid">
        <div class="form-row compact-row wide-row">
          <label>文本内容</label>
          <textarea
            v-model="s.textBlock.text"
            class="text-input"
            rows="3"
            placeholder="输入要叠加到拼贴图上的文字"
            @change="save"
          />
        </div>

        <div class="form-row compact-row">
          <label>字体</label>
          <select v-model="s.textBlock.font_family" class="font-select" @change="save">
            <option value="sans-serif">sans-serif</option>
            <option value="serif">serif</option>
            <option value="monospace">monospace</option>
            <option v-for="family in fontFamilies" :key="family" :value="family">
              {{ family }}
            </option>
          </select>
        </div>

        <div class="form-row compact-row">
          <label>字重</label>
          <input v-model.number="s.textBlock.font_weight" type="number" min="1" max="999" class="number-input" @change="save" />
          <span class="hint">400 常规，700 粗体</span>
        </div>

        <div class="form-row compact-row">
          <label>样式</label>
          <select v-model="s.textBlock.font_style" class="small-select" @change="save">
            <option value="normal">Normal</option>
            <option value="italic">Italic</option>
            <option value="oblique">Oblique</option>
          </select>
        </div>

        <div class="form-row compact-row">
          <label>字号</label>
          <input v-model.number="s.textBlock.font_size_px" type="number" min="1" max="2000" class="number-input" @change="save" />
          <span class="hint">px</span>
        </div>

        <div class="form-row compact-row">
          <label>行高</label>
          <input v-model.number="s.textBlock.line_height_px" type="number" min="1" max="3000" class="number-input" @change="save" />
          <span class="hint">px</span>
        </div>

        <div class="form-row compact-row">
          <label>对齐</label>
          <select v-model="s.textBlock.align" class="small-select" @change="save">
            <option value="left">Left</option>
            <option value="center">Center</option>
            <option value="right">Right</option>
          </select>
        </div>

        <div class="form-row compact-row">
          <label>最大宽度</label>
          <input v-model.number="s.textBlock.max_width_percent" type="number" min="1" max="100" class="number-input" @change="save" />
          <span class="hint">%</span>
        </div>

        <div class="form-row compact-row">
          <label>内边距</label>
          <input v-model.number="s.textBlock.padding_px" type="number" min="0" max="5000" class="number-input" @change="save" />
          <span class="hint">px</span>
        </div>

        <div class="form-row compact-row">
          <label>文字颜色</label>
          <input type="color" :value="rgbaToHex(s.textBlock.text_rgba)" @input="setTextColor" />
          <input v-model.number="s.textBlock.text_rgba[3]" type="number" min="0" max="255" class="alpha-input" @change="save" />
        </div>

        <div class="form-row compact-row">
          <label>背景颜色</label>
          <input type="color" :value="rgbaToHex(s.textBlock.background_rgba)" @input="setBackgroundColor" />
          <input v-model.number="s.textBlock.background_rgba[3]" type="number" min="0" max="255" class="alpha-input" @change="save" />
        </div>

        <div class="form-row compact-row">
          <label>水平位置</label>
          <input v-model.number="s.textBlock.position_x_percent" type="range" min="0" max="100" class="range-input" @input="save" />
          <input v-model.number="s.textBlock.position_x_percent" type="number" min="0" max="100" class="number-input" @change="save" />
          <span class="hint">%</span>
        </div>

        <div class="form-row compact-row">
          <label>垂直位置</label>
          <input v-model.number="s.textBlock.position_y_percent" type="range" min="0" max="100" class="range-input" @input="save" />
          <input v-model.number="s.textBlock.position_y_percent" type="number" min="0" max="100" class="number-input" @change="save" />
          <span class="hint">%</span>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useAppStore } from '../stores/appStore'
import type { FontFaceInfo } from '../types/protocol'

defineProps<{
  embedded?: boolean
}>()

const store = useAppStore()
const s = store.settings
const fonts = ref<FontFaceInfo[]>([])

const fontFamilies = computed(() => {
  const seen = new Set<string>()
  for (const face of fonts.value) {
    if (face.family && !seen.has(face.family)) {
      seen.add(face.family)
    }
  }
  return [...seen].sort((a, b) => a.localeCompare(b))
})

onMounted(async () => {
  try {
    fonts.value = await window.electronAPI.listFonts()
  } catch (err) {
    console.error('load fonts failed', err)
  }
})

function rgbaToHex(color: [number, number, number, number]): string {
  const [r, g, b] = color
  return `#${hex(r)}${hex(g)}${hex(b)}`
}

function hex(value: number): string {
  return clamp255(value).toString(16).padStart(2, '0')
}

function setTextColor(event: Event) {
  setRgb(s.textBlock.text_rgba, (event.target as HTMLInputElement).value)
  save()
}

function setBackgroundColor(event: Event) {
  setRgb(s.textBlock.background_rgba, (event.target as HTMLInputElement).value)
  save()
}

function setRgb(target: [number, number, number, number], value: string) {
  const match = /^#?([0-9a-f]{6})$/i.exec(value)
  if (!match) return
  const raw = match[1]
  target[0] = parseInt(raw.slice(0, 2), 16)
  target[1] = parseInt(raw.slice(2, 4), 16)
  target[2] = parseInt(raw.slice(4, 6), 16)
}

function clamp255(value: number): number {
  return Math.min(255, Math.max(0, Math.round(Number.isFinite(value) ? value : 0)))
}

function save() {
  store.saveSettings()
}
</script>

<style scoped>
.text-block-settings {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.text-block-settings.embedded {
  padding: 0;
  border: 0;
  background: transparent;
}

.text-block-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.text-block-header .section-title {
  margin: 0;
  border: 0;
  padding: 0;
}

.toggle-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--color-text);
}

.text-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 10px;
}

.wide-row {
  grid-column: auto;
}

.compact-row {
  align-items: stretch;
  flex-direction: column;
  gap: 6px;
  margin: 0;
}

.compact-row label {
  min-width: 0;
  color: var(--color-text-muted);
  font-size: 11px;
  font-weight: 700;
}

.text-input {
  min-height: 68px;
  resize: vertical;
}

.font-select {
  max-width: none;
}

.small-select {
  max-width: none;
}

.number-input {
  max-width: 92px;
}

.alpha-input {
  max-width: 72px;
}

.range-input {
  width: 100%;
}

@media (max-width: 760px) {
  .text-block-header {
    align-items: flex-start;
    flex-direction: column;
  }
}
</style>
