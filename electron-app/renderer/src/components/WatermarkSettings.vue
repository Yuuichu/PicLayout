<template>
  <div class="watermark-settings" :class="{ card: !embedded, embedded }">
    <div class="watermark-header">
      <p class="section-title">水印设置</p>
      <label class="toggle-label">
        <input v-model="s.watermarkEnabled" type="checkbox" @change="save" />
        启用水印
      </label>
    </div>

    <template v-if="s.watermarkEnabled">
      <div class="watermark-grid">
        <div class="form-row compact-row">
          <label>定位参照</label>
          <div class="segmented-control">
            <button
              type="button"
              :class="{ active: s.watermark.position_reference === 'content' }"
              @click="setPositionReference('content')"
            >
              拼图区域
            </button>
            <button
              type="button"
              :class="{ active: s.watermark.position_reference === 'canvas' }"
              @click="setPositionReference('canvas')"
            >
              整张画布
            </button>
          </div>
        </div>

        <div class="form-row compact-row">
          <label>水印图片</label>
          <button
            class="btn-secondary watermark-btn"
            :disabled="store.processing"
            @click="selectWatermark"
          >
            选择图片
          </button>
          <span class="hint file-name" :title="s.watermark.path">
            {{ watermarkBasename || '未选择' }}
          </span>
        </div>

        <div class="form-row compact-row">
          <label>缩放</label>
          <input
            v-model.number="s.watermark.scale_percent"
            class="range-input"
            type="range"
            min="10"
            max="300"
            step="0.01"
            @input="save"
          />
          <input
            v-model.number="s.watermark.scale_percent"
            type="number"
            min="10"
            max="300"
            step="0.01"
            class="number-input"
            @change="save"
          />
          <span class="hint">%</span>
        </div>

        <div class="form-row compact-row">
          <label>水平位置</label>
          <input
            :key="`watermark-x-${s.watermark.position_reference}-${positionBounds.minX}-${positionBounds.maxX}`"
            v-model.number="s.watermark.position_x_percent"
            class="range-input"
            type="range"
            :min="positionBounds.minX"
            :max="positionBounds.maxX"
            step="0.01"
            @input="save"
          />
          <input
            v-model.number="s.watermark.position_x_percent"
            type="number"
            step="0.01"
            class="number-input"
            @change="save"
          />
          <span class="hint">%</span>
        </div>

        <div class="form-row compact-row">
          <label>垂直位置</label>
          <input
            :key="`watermark-y-${s.watermark.position_reference}-${positionBounds.minY}-${positionBounds.maxY}`"
            v-model.number="s.watermark.position_y_percent"
            class="range-input"
            type="range"
            :min="positionBounds.minY"
            :max="positionBounds.maxY"
            step="0.01"
            @input="save"
          />
          <input
            v-model.number="s.watermark.position_y_percent"
            type="number"
            step="0.01"
            class="number-input"
            @change="save"
          />
          <span class="hint">%</span>
        </div>
      </div>

      <p class="tip">
        水印会叠加在下方拼贴预览上；导出时会使用原始水印图片，建议使用带透明通道的 PNG。
      </p>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useAppStore } from '../stores/appStore'
import type { PositionReference } from '@shared/protocol'
import {
  convertOverlayPositionReference,
  overlaySizeScale,
  overlayPositionBounds,
  roundOverlayPercent,
} from '../utils/overlayPosition'
import type { PreviewGeometry } from '../utils/previewLayout'

const props = defineProps<{
  embedded?: boolean
  geometry: PreviewGeometry
}>()

const store = useAppStore()
const s = store.settings

const watermarkBasename = computed(() => {
  if (!s.watermark.path) return ''
  return s.watermark.path.replace(/\\/g, '/').split('/').pop() ?? s.watermark.path
})

const positionBounds = computed(() =>
  overlayPositionBounds(props.geometry, s.watermark.position_reference)
)

function setPositionReference(reference: PositionReference) {
  const currentReference = s.watermark.position_reference
  if (reference === currentReference) return
  const converted = convertOverlayPositionReference(props.geometry, currentReference, reference, {
    x: s.watermark.position_x_percent,
    y: s.watermark.position_y_percent,
  })
  const fromScale = overlaySizeScale(props.geometry, currentReference, s.finalSize)
  const toScale = overlaySizeScale(props.geometry, reference, s.finalSize)
  s.watermark.position_reference = reference
  s.watermark.scale_percent = roundOverlayPercent(
    Math.min(300, Math.max(10, (s.watermark.scale_percent * fromScale) / toScale))
  )
  s.watermark.position_x_percent = roundOverlayPercent(converted.x)
  s.watermark.position_y_percent = roundOverlayPercent(converted.y)
  save()
}

async function selectWatermark() {
  const path = await window.electronAPI.openWatermark()
  if (path) {
    s.watermark.path = path
    await store.ensureThumbnail(path)
    await store.ensureImageSize(path)
    save()
  }
}

function save() {
  store.saveSettings()
}
</script>

<style scoped>
.watermark-settings {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.watermark-settings.embedded {
  padding: 0;
  border: 0;
  background: transparent;
}

.watermark-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.watermark-header .section-title {
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

.watermark-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 10px;
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

.watermark-btn {
  width: 100%;
  font-size: 12px;
}

.file-name {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.range-input {
  width: 100%;
}

.segmented-control {
  display: grid;
  grid-template-columns: 1fr 1fr;
}

.segmented-control button {
  min-width: 0;
  border: 1px solid var(--color-border);
  border-radius: 0;
  background: var(--color-control);
  color: var(--color-text-muted);
  padding: 7px 8px;
  font-size: 11px;
}

.segmented-control button:first-child {
  border-radius: 4px 0 0 4px;
}

.segmented-control button:last-child {
  margin-left: -1px;
  border-radius: 0 4px 4px 0;
}

.segmented-control button.active {
  position: relative;
  border-color: var(--color-accent);
  background: color-mix(in srgb, var(--color-accent), transparent 82%);
  color: var(--color-text);
}

.number-input {
  max-width: 86px;
}

.tip {
  font-size: 11px;
  color: var(--color-text-subtle);
  margin-top: 0;
}

@media (max-width: 760px) {
  .watermark-header {
    align-items: flex-start;
    flex-direction: column;
  }
}
</style>
