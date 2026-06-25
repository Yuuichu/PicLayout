<template>
  <div class="settings-panel">
    <div class="tool-tabs" role="tablist" aria-label="工具面板">
      <button
        v-for="tool in tools"
        :key="tool.id"
        class="tool-tab"
        :class="{ active: activeTool === tool.id }"
        type="button"
        role="tab"
        :aria-selected="activeTool === tool.id"
        @click="activeTool = tool.id"
      >
        <component :is="tool.icon" :size="15" />
        {{ tool.label }}
      </button>
    </div>

    <div class="tool-body">
      <section v-if="activeTool === 'layout'" class="tool-section">
        <p class="section-title">Layout</p>

        <div class="field-grid">
          <label class="compact-field">
            <span>最大图片</span>
            <input v-model.number="s.maxImages" type="number" min="1" max="500" @change="save" />
          </label>

          <label class="compact-field">
            <span>内容长边 (%)</span>
            <input :value="layoutSliderValue('contentLongEdgePercent')" data-layout-field="content-long-edge" class="range-input" type="range" min="0" max="100" step="1" @input="setLayoutPercentFromSlider($event, 'contentLongEdgePercent')" @change="save" />
            <input v-model.number="s.contentLongEdgePercent" data-layout-field="content-long-edge" type="number" min="0.01" max="100" step="0.01" @change="handleLayoutNumberChange('contentLongEdgePercent')" />
          </label>

          <label class="compact-field">
            <span>单图边框 (%)</span>
            <input :value="layoutSliderValue('tileBorderPercent')" data-layout-field="tile-padding" class="range-input" type="range" min="0" max="100" step="1" @input="setLayoutPercentFromSlider($event, 'tileBorderPercent')" @change="save" />
            <input v-model.number="s.tileBorderPercent" data-layout-field="tile-padding" type="number" min="0" max="50" step="0.01" @change="handleLayoutNumberChange('tileBorderPercent')" />
          </label>

          <label class="compact-field">
            <span>横向间隔 (%)</span>
            <input :value="layoutSliderValue('imageGapPercent')" data-layout-field="image-gap" class="range-input" type="range" min="0" max="100" step="1" @input="setLayoutPercentFromSlider($event, 'imageGapPercent')" @change="save" />
            <input v-model.number="s.imageGapPercent" data-layout-field="image-gap" type="number" min="0" max="100" step="0.01" @change="handleLayoutNumberChange('imageGapPercent')" />
          </label>

          <label class="compact-field">
            <span>纵向间隔 (%)</span>
            <input :value="layoutSliderValue('gapYPercent')" data-layout-field="legacy-gap-y" class="range-input" type="range" min="0" max="100" step="1" @input="setLayoutPercentFromSlider($event, 'gapYPercent')" @change="save" />
            <input v-model.number="s.gapYPercent" data-layout-field="legacy-gap-y" type="number" min="0" max="100" step="0.01" @change="handleLayoutNumberChange('gapYPercent')" />
          </label>

          <label class="compact-field">
            <span>最终长边 (px)</span>
            <input v-model.number="s.finalSize" data-layout-field="final-size" type="number" min="1000" max="30000" @change="save" />
          </label>
        </div>

        <div class="stacked-field">
          <span>画布比例</span>
          <select v-model="s.canvasAspectPreset" @change="handleCanvasAspectChange">
            <option
              v-for="option in canvasAspectOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </option>
          </select>
          <div v-if="s.canvasAspectPreset === 'custom'" class="field-grid custom-aspect-grid">
            <label class="compact-field">
              <span>宽</span>
              <input
                v-model.number="s.customAspectWidth"
                type="number"
                min="0.1"
                max="100"
                step="0.01"
                @change="handleCustomAspectChange"
              />
            </label>
            <label class="compact-field">
              <span>高</span>
              <input
                v-model.number="s.customAspectHeight"
                type="number"
                min="0.1"
                max="100"
                step="0.01"
                @change="handleCustomAspectChange"
              />
            </label>
          </div>
          <em>{{ currentCanvasAspectHelp }}</em>
        </div>

        <div class="stacked-field canvas-margin-field">
          <span>最终外边距 (%)</span>
          <div class="segmented-control">
            <button type="button" :class="{ active: s.outerBorderMode === 'auto' }" @click="setOuterBorderMode('auto')">
              自动
            </button>
            <button type="button" :class="{ active: s.outerBorderMode === 'custom' }" @click="setOuterBorderMode('custom')">
              自定义
            </button>
          </div>
          <template v-if="s.outerBorderMode === 'custom'">
            <input
              :value="layoutSliderValue('outerBorderPercent')"
              class="range-input"
              type="range"
              min="0"
              max="100"
              step="1"
              @input="setLayoutPercentFromSlider($event, 'outerBorderPercent')"
              @change="save"
            />
            <input
              v-model.number="s.outerBorderPercent"
              type="number"
              min="0"
              max="49.99"
              step="0.01"
              @change="handleLayoutNumberChange('outerBorderPercent')"
            />
          </template>
        </div>

        <p class="setting-note">高分辨率和大量图片会增加内存占用；如果处理失败，优先降低图片数量或最终尺寸。</p>
      </section>

      <section v-else-if="activeTool === 'output'" class="tool-section">
        <p class="section-title">Output</p>

        <div class="stacked-field">
          <span>质量模式</span>
          <div class="segmented-control vertical">
            <button
              type="button"
              :class="{ active: s.processingMode === 'standard_high_quality' }"
              @click="setProcessingMode('standard_high_quality')"
            >
              标准高画质
            </button>
            <button
              type="button"
              :class="{ active: s.processingMode === 'maximum_quality' }"
              @click="setProcessingMode('maximum_quality')"
            >
              极致高画质
            </button>
            <button
              type="button"
              :class="{ active: s.processingMode === 'fast_preview' }"
              @click="setProcessingMode('fast_preview')"
            >
              快速预览
            </button>
          </div>
        </div>

        <div class="field-grid">
          <label class="compact-field">
            <span>JPEG 质量</span>
            <input v-model.number="s.jpegQuality" type="number" min="1" max="100" @change="save" />
          </label>

          <label class="compact-field">
            <span>DPI</span>
            <select v-model.number="s.dpi" @change="save">
              <option :value="72">72</option>
              <option :value="150">150</option>
              <option :value="300">300</option>
              <option :value="600">600</option>
            </select>
          </label>
        </div>

        <label class="toggle-row">
          <input type="checkbox" v-model="s.autoOrient" @change="save" />
          <span>EXIF 自动旋正</span>
        </label>

        <label class="toggle-row">
          <input type="checkbox" v-model="s.linearLightResize" @change="handleLinearLightResizeChange" />
          <span>线性光高画质缩放</span>
        </label>
      </section>

      <section v-else-if="activeTool === 'color'" class="tool-section">
        <p class="section-title">Color</p>

        <div class="stacked-field">
          <span>背景颜色</span>
          <div class="color-selector">
            <button
              v-for="opt in colorOptions"
              :key="opt.value"
              class="color-swatch"
              :class="{ active: s.backgroundColor === opt.value }"
              :style="{ background: opt.hex }"
              :title="opt.label"
              type="button"
              @click="selectColor(opt.value)"
            />
          </div>
          <em>{{ currentColorLabel }}</em>
        </div>

        <label class="toggle-row">
          <input type="checkbox" v-model="s.colorManagementEnabled" @change="save" />
          <span>启用 ICC 转换</span>
        </label>

        <template v-if="s.colorManagementEnabled">
          <label class="compact-field full-field">
            <span>目标 Profile</span>
            <select v-model="s.targetProfileMode" @change="save">
              <option value="srgb">sRGB</option>
              <option value="custom">自定义 ICC</option>
            </select>
          </label>

          <button
            v-if="s.targetProfileMode === 'custom'"
            class="quiet-button"
            type="button"
            @click="selectIccProfile"
          >
            选择 ICC
          </button>
          <p v-if="s.targetProfileMode === 'custom'" class="path-note" :title="s.targetProfilePath">
            {{ iccBasename || '未选择' }}
          </p>

          <label class="compact-field full-field">
            <span>渲染意图</span>
            <select v-model="s.renderingIntent" @change="save">
              <option value="perceptual">Perceptual</option>
              <option value="relative_colorimetric">Relative Colorimetric</option>
            </select>
          </label>
        </template>
      </section>

      <section v-else-if="activeTool === 'watermark'" class="tool-section">
        <WatermarkSettings embedded :geometry="overlayGeometry" />
      </section>

      <section v-else-if="activeTool === 'text'" class="tool-section">
        <TextBlockSettings embedded :geometry="overlayGeometry" />
      </section>

      <section v-else class="tool-section">
        <p class="section-title">Activity</p>
        <ProgressBar variant="panel" />
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { Activity, Droplets, FileOutput, LayoutGrid, Palette, Type } from 'lucide-vue-next'
import { useAppStore } from '../stores/appStore'
import { BACKGROUND_COLOR_OPTIONS, type BackgroundColor, type ProcessingMode } from '../types/protocol'
import {
  CANVAS_ASPECT_OPTIONS,
  findCanvasAspectOption,
  formatAspectRatio,
  resolveCanvasAspectRatio,
} from '../utils/aspectRatioPresets'
import { computePreviewLayout } from '../utils/previewLayout'
import ProgressBar from './ProgressBar.vue'
import TextBlockSettings from './TextBlockSettings.vue'
import WatermarkSettings from './WatermarkSettings.vue'

type ToolId = 'layout' | 'output' | 'color' | 'watermark' | 'text' | 'activity'
type LayoutPercentKey =
  | 'contentLongEdgePercent'
  | 'tileBorderPercent'
  | 'imageGapPercent'
  | 'gapYPercent'
  | 'outerBorderPercent'
type LayoutSliderCurve = 'linear' | 'quadratic'

const tools: { id: ToolId; label: string; icon: unknown }[] = [
  { id: 'layout', label: 'Layout', icon: LayoutGrid },
  { id: 'output', label: 'Output', icon: FileOutput },
  { id: 'color', label: 'Color', icon: Palette },
  { id: 'watermark', label: 'Watermark', icon: Droplets },
  { id: 'text', label: 'Text', icon: Type },
  { id: 'activity', label: 'Activity', icon: Activity },
]

const store = useAppStore()
const s = store.settings
const colorOptions = BACKGROUND_COLOR_OPTIONS
const canvasAspectOptions = CANVAS_ASPECT_OPTIONS
const activeTool = ref<ToolId>('layout')
const layoutPercentRanges: Record<LayoutPercentKey, { min: number; max: number; curve?: LayoutSliderCurve }> = {
  contentLongEdgePercent: { min: 0.01, max: 100 },
  tileBorderPercent: { min: 0, max: 50, curve: 'quadratic' },
  imageGapPercent: { min: 0, max: 100 },
  gapYPercent: { min: 0, max: 100 },
  outerBorderPercent: { min: 0, max: 49.99 },
}

const currentColorLabel = computed(
  () => colorOptions.find((o) => o.value === s.backgroundColor)?.label ?? ''
)

const currentCanvasAspectHelp = computed(() => {
  if (s.canvasAspectPreset === 'custom') {
    return `Custom ${formatAspectRatio({ width: s.customAspectWidth, height: s.customAspectHeight })}`
  }

  const option = findCanvasAspectOption(s.canvasAspectPreset)
  return option.ratio ? `${option.shortLabel} ${formatAspectRatio(option.ratio)}` : 'Auto keeps the collage shape'
})

const overlayGeometry = computed(() => computePreviewLayout({
  imageCount: store.selectedFiles.length,
  finalSize: s.finalSize,
  targetAspectRatio: resolveCanvasAspectRatio(s),
  contentLongEdgePercent: s.contentLongEdgePercent,
  tileBorderPercent: s.tileBorderPercent,
  gapXPercent: s.gapXPercent,
  gapYPercent: s.gapYPercent,
  outerBorderMode: s.outerBorderMode,
  outerBorderPercent: s.outerBorderPercent,
}))

const iccBasename = computed(() => {
  if (!s.targetProfilePath) return ''
  return s.targetProfilePath.replace(/\\/g, '/').split('/').pop() ?? s.targetProfilePath
})

function selectColor(val: BackgroundColor) {
  s.backgroundColor = val
  save()
}

function setProcessingMode(mode: ProcessingMode) {
  s.processingMode = mode
  s.linearLightResize = mode === 'maximum_quality'
  save()
}

function setOuterBorderMode(mode: 'auto' | 'custom') {
  s.outerBorderMode = mode
  save()
}

function handleCanvasAspectChange() {
  save()
}

function handleCustomAspectChange() {
  s.customAspectWidth = normalizeAspectNumber(s.customAspectWidth, 3)
  s.customAspectHeight = normalizeAspectNumber(s.customAspectHeight, 4)
  save()
}

function normalizeAspectNumber(value: number, fallback: number) {
  const numberValue = Number(value)
  const normalized = Number.isFinite(numberValue) ? numberValue : fallback
  return Math.round(Math.min(100, Math.max(0.1, normalized)) * 100) / 100
}

function roundLayoutPercent(value: number) {
  const numberValue = Number(value)
  return Number.isFinite(numberValue) ? Math.round(numberValue * 100) / 100 : 0
}

function normalizeLayoutPercent(key: LayoutPercentKey) {
  const { min, max } = layoutPercentRanges[key]
  s[key] = roundLayoutPercent(Math.min(max, Math.max(min, s[key])))
}

function layoutSliderValue(key: LayoutPercentKey) {
  const { min, max } = layoutPercentRanges[key]
  const span = Math.max(0.01, max - min)
  const ratio = Math.min(1, Math.max(0, (s[key] - min) / span))
  return Math.min(100, Math.max(0, Math.round(sliderRatioFromValueRatio(key, ratio) * 100)))
}

function setLayoutPercentFromSlider(event: Event, key: LayoutPercentKey) {
  const input = event.target as HTMLInputElement
  const sliderPosition = Number(input.value)
  const sliderRatio = Number.isFinite(sliderPosition) ? Math.min(100, Math.max(0, sliderPosition)) / 100 : 0
  const valueRatio = valueRatioFromSliderRatio(key, sliderRatio)
  const { min, max } = layoutPercentRanges[key]
  s[key] = roundLayoutPercent(min + (max - min) * valueRatio)
}

function handleLayoutNumberChange(key: LayoutPercentKey) {
  normalizeLayoutPercent(key)
  save()
}

function valueRatioFromSliderRatio(key: LayoutPercentKey, ratio: number) {
  if (layoutPercentRanges[key].curve === 'quadratic') {
    return ratio * ratio
  }
  return ratio
}

function sliderRatioFromValueRatio(key: LayoutPercentKey, ratio: number) {
  if (layoutPercentRanges[key].curve === 'quadratic') {
    return Math.sqrt(ratio)
  }
  return ratio
}

function handleLinearLightResizeChange() {
  if (s.linearLightResize) {
    s.processingMode = 'maximum_quality'
  } else if (s.processingMode === 'maximum_quality') {
    s.processingMode = 'standard_high_quality'
  }
  save()
}

function save() {
  store.saveSettings()
}

async function selectIccProfile() {
  const path = await window.electronAPI.openIccProfile()
  if (!path) return
  s.targetProfilePath = path
  save()
}
</script>

<style scoped>
.settings-panel {
  height: 100%;
  display: grid;
  grid-template-columns: 44px minmax(0, 1fr);
}

.tool-tabs {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  border-right: 1px solid var(--color-border);
  background: var(--color-panel-deep);
}

.tool-tab {
  width: 43px;
  height: 58px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  padding: 0;
  border: 0;
  border-bottom: 1px solid var(--color-border);
  border-radius: 0;
  background: transparent;
  color: var(--color-text-muted);
  font-size: 9px;
  font-weight: 650;
}

.tool-tab.active {
  background: var(--color-panel);
  color: var(--color-text);
  box-shadow: inset 2px 0 0 var(--color-accent);
}

.tool-body {
  min-height: 0;
  overflow-y: auto;
}

.tool-section {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 14px;
}

.field-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}

.field-grid > .compact-field:has([data-layout-field="content-long-edge"]),
.field-grid > .compact-field:has([data-layout-field="image-gap"]),
.field-grid > .compact-field:has([data-layout-field="legacy-gap-y"]) {
  display: none;
}

.field-grid > .compact-field:has([data-layout-field="final-size"]) {
  order: 2;
}

.field-grid > .compact-field:has([data-layout-field="tile-padding"]) {
  order: 3;
}

.field-grid > .compact-field:has([data-layout-field="image-gap"]) {
  order: 4;
}

.field-grid > .compact-field:has([data-layout-field="tile-padding"]) > span,
.field-grid > .compact-field:has([data-layout-field="image-gap"]) > span,
.canvas-margin-field > span {
  font-size: 0;
}

.field-grid > .compact-field:has([data-layout-field="tile-padding"]) > span::after,
.field-grid > .compact-field:has([data-layout-field="image-gap"]) > span::after,
.canvas-margin-field > span::after {
  color: var(--color-text-muted);
  font-size: 11px;
  font-weight: 700;
}

.field-grid > .compact-field:has([data-layout-field="tile-padding"]) > span::after {
  content: '图片留白/间距 (%)';
}

.field-grid > .compact-field:has([data-layout-field="image-gap"]) > span::after {
  content: '图片间隔 (%)';
}

.canvas-margin-field > span::after {
  content: '画布边距 (%)';
}

.compact-field,
.stacked-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.slider-field {
  gap: 7px;
}

.compact-field span,
.stacked-field > span {
  color: var(--color-text-muted);
  font-size: 11px;
  font-weight: 700;
}

.full-field {
  width: 100%;
}

.range-input {
  width: 100%;
  min-width: 0;
}

.segmented-control {
  display: inline-flex;
  overflow: hidden;
  border: 1px solid var(--color-border);
  border-radius: 5px;
}

.segmented-control.vertical {
  flex-direction: column;
}

.segmented-control button {
  min-height: 28px;
  flex: 1;
  border: 0;
  border-right: 1px solid var(--color-border);
  border-radius: 0;
  background: var(--color-control);
  color: var(--color-text-muted);
  font-size: 11px;
  font-weight: 650;
}

.segmented-control.vertical button {
  border-right: 0;
  border-bottom: 1px solid var(--color-border);
}

.segmented-control button:last-child {
  border-right: 0;
  border-bottom: 0;
}

.segmented-control button.active {
  background: var(--color-control-active);
  color: var(--color-text);
}

.toggle-row {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--color-text);
  font-size: 12px;
}

.color-selector {
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
}

.color-swatch {
  width: 24px;
  height: 24px;
  padding: 0;
  border: 1px solid var(--color-border-strong);
  border-radius: 50%;
  box-shadow: inset 0 0 0 2px var(--color-panel);
}

.color-swatch.active {
  outline: 2px solid var(--color-accent);
  outline-offset: 2px;
}

.stacked-field em,
.path-note,
.setting-note {
  color: var(--color-text-subtle);
  font-size: 11px;
  font-style: normal;
  line-height: 1.45;
}

.path-note {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.quiet-button {
  height: 30px;
  border: 1px solid var(--color-border);
  background: var(--color-control);
  color: var(--color-text);
  font-size: 12px;
  font-weight: 650;
}
</style>
