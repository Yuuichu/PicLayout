<template>
  <div class="card file-selector">
    <p class="section-title">图片选择</p>

    <div class="form-row">
      <label>输出文件名前缀</label>
      <input
        v-model="store.settings.prefix"
        type="text"
        placeholder="output"
        style="max-width: 160px"
        @change="store.saveSettings()"
      />
    </div>

    <div class="form-row">
      <label>导出目录</label>
      <button class="btn-secondary path-btn" :disabled="processing" @click="selectOutputDir">
        选择目录
      </button>
      <span class="hint dir-name" :title="store.settings.outputDir">
        {{ store.settings.outputDir || '未选择' }}
      </span>
    </div>

    <div class="file-actions">
      <button class="btn-success" :disabled="processing" @click="selectImages">
        选择图片
      </button>
      <button
        v-if="store.selectedFiles.length > 0"
        class="btn-secondary"
        :disabled="processing"
        @click="appendImages"
      >
        追加图片
      </button>
      <button
        v-if="store.selectedFiles.length > 0"
        class="btn-secondary"
        :disabled="processing"
        @click="clearImages"
      >
        清空
      </button>
      <span class="file-count">
        {{ fileCountText }}
      </span>
    </div>

    <WatermarkSettings embedded />

    <div
      v-if="store.selectedFiles.length > 0"
      class="preview-frame"
      :style="previewFrameStyle"
    >
      <div class="collage-preview" :style="thumbGridStyle">
        <div
          v-for="(f, i) in store.selectedFiles"
          :key="i"
          class="thumb-tile"
          :class="{ dragging: draggedIndex === i, dropTarget: draggedIndex !== null && draggedIndex !== i }"
          :title="f"
          draggable="true"
          @dragstart="onDragStart(i)"
          @dragover.prevent
          @drop="onDrop(i)"
          @dragend="onDragEnd"
        >
          <div class="thumb-frame">
            <img
              v-if="store.thumbnails[f]"
              class="thumb-img"
              :src="store.thumbnails[f]!"
              :alt="basename(f)"
              :style="thumbnailImageStyle(f)"
            />
            <span v-else class="thumb-fallback">加载中</span>
          </div>
          <span class="thumb-index">{{ i + 1 }}</span>
          <span class="thumb-name">{{ basename(f) }}</span>
          <div class="thumb-tools">
            <button
              class="tool-btn rotate-btn"
              :disabled="processing"
              :aria-label="`旋转 ${basename(f)}`"
              :title="`顺时针旋转 90°，当前 ${store.getImageRotation(f)}°`"
              @click.stop="rotateImage(f)"
            >
              ↻
            </button>
            <button
              class="tool-btn"
              :disabled="processing || i === 0"
              :aria-label="`上移 ${basename(f)}`"
              title="上移"
              @click.stop="moveImage(i, i - 1)"
            >
              ↑
            </button>
            <button
              class="tool-btn"
              :disabled="processing || i === store.selectedFiles.length - 1"
              :aria-label="`下移 ${basename(f)}`"
              title="下移"
              @click.stop="moveImage(i, i + 1)"
            >
              ↓
            </button>
            <button
              class="remove-btn"
              :disabled="processing"
              :aria-label="`移除 ${basename(f)}`"
              title="移除"
              @click.stop="removeImage(i)"
            >
              ×
            </button>
          </div>
        </div>
      </div>

      <img
        v-if="watermarkPreviewSrc"
        class="watermark-preview"
        :src="watermarkPreviewSrc"
        :style="watermarkPreviewStyle"
        alt="水印预览"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useAppStore } from '../stores/appStore'
import { BACKGROUND_COLOR_OPTIONS } from '../types/protocol'
import WatermarkSettings from './WatermarkSettings.vue'

const store = useAppStore()
const s = store.settings
const draggedIndex = ref<number | null>(null)

const processing = computed(() => store.processing)
const gridCols = computed(() => Math.max(1, Math.ceil(Math.sqrt(store.selectedFiles.length))))
const gridRows = computed(() =>
  Math.max(1, Math.ceil(store.selectedFiles.length / gridCols.value))
)

const PREVIEW_MIN_HEIGHT = 220
const PREVIEW_MAX_HEIGHT = 520
const PREVIEW_RESERVED_HEIGHT = 580
const TILE_MAX_SIZE = 180
const TILE_GAP = 8

const backgroundColorHex = computed(() => {
  return BACKGROUND_COLOR_OPTIONS.find((opt) => opt.value === s.backgroundColor)?.hex ?? '#ffffff'
})

const previewGeometry = computed(() => {
  const cols = gridCols.value
  const rows = gridRows.value
  const borderSize = Math.max(1, s.borderSize || 1)
  const gridWidth = cols * borderSize
  const gridHeight = rows * borderSize
  const border = calculateDynamicBorder(cols)
  const finalSize = Math.max(1, s.finalSize || 1)
  const innerSize = Math.max(1, finalSize - border * 2)
  const scale = innerSize / Math.max(gridWidth, gridHeight)
  const scaledWidth = Math.max(1, gridWidth * scale)
  const scaledHeight = Math.max(1, gridHeight * scale)
  const canvasWidth = scaledWidth + border * 2
  const canvasHeight = scaledHeight + border * 2

  return { cols, rows, border, scaledWidth, scaledHeight, canvasWidth, canvasHeight }
})

const previewFrameStyle = computed(() => {
  const geom = previewGeometry.value
  const ratio = geom.canvasWidth / geom.canvasHeight
  const maxWidthByHeight = `clamp(${Math.round(
    PREVIEW_MIN_HEIGHT * ratio
  )}px, calc(${(ratio * 100).toFixed(3)}vh - ${Math.round(
    PREVIEW_RESERVED_HEIGHT * ratio
  )}px), ${Math.round(PREVIEW_MAX_HEIGHT * ratio)}px)`
  const maxWidthByTile = geom.cols * TILE_MAX_SIZE + Math.max(0, geom.cols - 1) * TILE_GAP

  return {
    aspectRatio: `${geom.canvasWidth} / ${geom.canvasHeight}`,
    maxWidth: `min(${maxWidthByTile}px, ${maxWidthByHeight})`,
    backgroundColor: backgroundColorHex.value,
    '--preview-bg': backgroundColorHex.value,
  }
})

const thumbGridStyle = computed(() => {
  const geom = previewGeometry.value

  return {
    gridTemplateColumns: `repeat(${geom.cols}, minmax(0, 1fr))`,
    left: `${(geom.border / geom.canvasWidth) * 100}%`,
    top: `${(geom.border / geom.canvasHeight) * 100}%`,
    width: `${(geom.scaledWidth / geom.canvasWidth) * 100}%`,
    height: `${(geom.scaledHeight / geom.canvasHeight) * 100}%`,
  }
})

const watermarkPreviewSrc = computed(() => {
  if (!s.watermarkEnabled || !s.watermark.path) return null
  return store.thumbnails[s.watermark.path] ?? null
})

const watermarkPreviewStyle = computed(() => {
  const geom = previewGeometry.value
  const size = s.watermark.path ? store.imageSizes[s.watermark.path] : null
  const watermarkWidth = Math.max(1, size?.width ?? 240)
  const watermarkHeight = Math.max(1, size?.height ?? 240)
  const scale = Math.max(0.01, s.watermark.scale_percent / 100)
  const x = clamp(s.watermark.position_x_percent, 0, 100)
  const y = clamp(s.watermark.position_y_percent, 0, 100)

  return {
    left: `${x}%`,
    top: `${y}%`,
    width: `${(watermarkWidth * scale / geom.canvasWidth) * 100}%`,
    height: `${(watermarkHeight * scale / geom.canvasHeight) * 100}%`,
  }
})

const fileCountText = computed(() => {
  const n = store.selectedFiles.length
  if (n === 0) return '未选择图片'
  return `已选择 ${n} 张图片`
})

function basename(path: string): string {
  return path.replace(/\\/g, '/').split('/').pop() ?? path
}

async function selectImages() {
  const files = await window.electronAPI.openImages()
  if (!files.length) return

  applySelectedFiles(files, false)
}

async function appendImages() {
  const files = await window.electronAPI.openImages()
  if (!files.length) return

  applySelectedFiles(files, true)
}

function applySelectedFiles(files: string[], append: boolean) {
  const current = append ? store.selectedFiles : []
  const seen = new Set(current)
  const uniqueNew = files.filter((file) => {
    if (seen.has(file)) return false
    seen.add(file)
    return true
  })
  const total = current.length + uniqueNew.length
  const max = store.settings.maxImages
  if (total > max) {
    alert(`选择的图片数量不能超过 ${max} 张，当前将达到 ${total} 张。`)
    return
  }

  if (append) {
    store.appendSelectedFiles(files)
  } else {
    store.setSelectedFiles(uniqueNew)
  }
}

function removeImage(index: number) {
  store.setSelectedFiles(store.selectedFiles.filter((_, i) => i !== index))
}

function moveImage(from: number, to: number) {
  store.moveSelectedFile(from, to)
}

function rotateImage(path: string) {
  store.rotateImage(path)
}

function thumbnailImageStyle(path: string) {
  return {
    transform: `rotate(${store.getImageRotation(path)}deg)`,
  }
}

function clearImages() {
  store.setSelectedFiles([])
}

function onDragStart(index: number) {
  draggedIndex.value = index
}

function onDrop(index: number) {
  if (draggedIndex.value !== null) {
    store.moveSelectedFile(draggedIndex.value, index)
  }
  draggedIndex.value = null
}

function onDragEnd() {
  draggedIndex.value = null
}

async function selectOutputDir() {
  const dir = await window.electronAPI.openDirectory()
  if (!dir) return
  store.settings.outputDir = dir
  store.saveSettings()
}

function calculateDynamicBorder(cols: number): number {
  if (cols >= 10) return 200
  if (cols <= 2) return 1000
  return 200 + (1000 - 200) * (10 - cols) / 8
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, Number.isFinite(value) ? value : min))
}

watch(
  () => ({
    files: [...store.selectedFiles],
    watermarkEnabled: s.watermarkEnabled,
    watermarkPath: s.watermark.path,
  }),
  ({ files, watermarkEnabled, watermarkPath }) => {
    for (const file of files) {
      void store.ensureThumbnail(file)
    }
    if (watermarkEnabled && watermarkPath) {
      void store.ensureThumbnail(watermarkPath)
      void store.ensureImageSize(watermarkPath)
    }
  },
  { immediate: true }
)
</script>

<style scoped>
.file-selector {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.file-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.file-count {
  font-size: 13px;
  color: var(--color-primary);
  font-weight: 500;
}

.preview-frame {
  position: relative;
  width: 100%;
  align-self: center;
  border: 1px solid var(--color-border);
  border-radius: 6px;
  overflow: hidden;
  box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.03);
}

.collage-preview {
  position: absolute;
  display: grid;
  gap: 1.5%;
}

.thumb-tile {
  position: relative;
  background: var(--preview-bg);
  border: 1px solid var(--color-border);
  border-radius: 5px;
  padding: 0;
  color: var(--color-text-secondary);
  min-width: 0;
  aspect-ratio: 1;
  overflow: hidden;
  cursor: grab;
  transition: border-color 0.15s, box-shadow 0.15s, opacity 0.15s, transform 0.15s;
}

.thumb-tile.dragging {
  opacity: 0.55;
}

.thumb-tile.dropTarget:hover {
  border-color: var(--color-primary);
  box-shadow: 0 0 0 2px rgba(33, 150, 243, 0.18);
  transform: translateY(-1px);
}

.thumb-frame {
  width: 100%;
  height: 100%;
  border-radius: 4px;
  background: var(--preview-bg);
  overflow: hidden;
}

.thumb-img {
  width: 100%;
  height: 100%;
  object-fit: contain;
  display: block;
  transform-origin: center;
  transition: transform 0.15s ease;
}

.thumb-fallback {
  display: flex;
  width: 100%;
  height: 100%;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  color: var(--color-text-secondary);
}

.thumb-index {
  position: absolute;
  top: 4px;
  left: 4px;
  min-width: 20px;
  height: 20px;
  border-radius: 999px;
  background: rgba(33, 150, 243, 0.92);
  color: white;
  font-size: 11px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
}

.thumb-name {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  padding: 18px 6px 5px;
  background: linear-gradient(to top, rgba(0, 0, 0, 0.62), rgba(0, 0, 0, 0));
  color: white;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.thumb-tools {
  position: absolute;
  top: 4px;
  right: 4px;
  display: flex;
  gap: 4px;
  max-width: calc(100% - 30px);
  flex-wrap: wrap;
  justify-content: flex-end;
}

.tool-btn,
.remove-btn {
  width: 22px;
  height: 22px;
  padding: 0;
  font-size: 12px;
  line-height: 1;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.92);
}

.tool-btn {
  color: var(--color-primary);
  border: 1px solid var(--color-primary);
}

.remove-btn {
  color: var(--color-danger);
  border: 1px solid var(--color-danger);
}

.tool-btn:disabled,
.remove-btn:disabled {
  opacity: 0.35;
}

.watermark-preview {
  position: absolute;
  transform: translate(-50%, -50%);
  object-fit: contain;
  pointer-events: none;
  opacity: 0.78;
  filter: drop-shadow(0 1px 3px rgba(0, 0, 0, 0.35));
  z-index: 5;
}

.path-btn {
  width: auto;
  padding: 5px 10px;
  font-size: 12px;
}

.dir-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
