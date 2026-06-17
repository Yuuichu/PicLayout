<template>
  <div v-if="props.variant === 'task'" class="task-rail">
    <section class="rail-section">
      <p class="section-title">当前任务</p>
      <div class="task-stat">
        <span>源图片</span>
        <strong>{{ selectedCountLabel }}</strong>
      </div>
      <div class="task-stat">
        <span>输出</span>
        <strong :title="store.settings.outputDir">{{ outputDirName || '未选择' }}</strong>
      </div>
      <div class="task-stat">
        <span>状态</span>
        <strong>{{ taskStatus }}</strong>
      </div>
    </section>

    <section class="rail-section">
      <label class="field-label" for="prefix-input">文件名前缀</label>
      <input
        id="prefix-input"
        v-model="store.settings.prefix"
        type="text"
        placeholder="output"
        @change="store.saveSettings()"
      />

      <label class="field-label">导出目录</label>
      <button class="quiet-button full-button" :disabled="processing" @click="selectOutputDir">
        <FolderOpen :size="15" />
        选择目录
      </button>
      <p class="path-readout" :title="store.settings.outputDir">
        {{ store.settings.outputDir || '等待选择输出目录' }}
      </p>
    </section>

    <section class="rail-section">
      <div class="rail-actions">
        <button class="quiet-button full-button" :disabled="processing" @click="selectImages">
          <ImagePlus :size="15" />
          导入图片
        </button>
        <button class="quiet-button full-button" :disabled="processing" @click="appendImages">
          <Plus :size="15" />
          追加图片
        </button>
        <button
          class="quiet-button full-button danger-muted"
          :disabled="processing || store.selectedFiles.length === 0"
          @click="clearImages"
        >
          <Trash2 :size="15" />
          清空队列
        </button>
      </div>
    </section>

    <section v-if="resultSummary" class="rail-section result-section">
      <p class="section-title">最近结果</p>
      <p class="result-line">{{ resultSummary }}</p>
      <button
        v-if="store.outputFiles.length > 0"
        class="quiet-button full-button"
        @click="openOutputDir"
      >
        <ExternalLink :size="15" />
        打开输出目录
      </button>
    </section>
  </div>

  <div v-else-if="props.variant === 'viewer'" class="viewer-workspace">
    <div class="viewer-chrome">
      <span>Viewer</span>
      <span>{{ canvasMeta }}</span>
    </div>

    <div class="viewer-stage">
      <div
        v-if="store.selectedFiles.length > 0"
        ref="previewFrameRef"
        class="preview-frame viewer-preview-frame"
        :style="previewFrameStyle"
      >
        <div class="collage-preview" :style="thumbGridStyle">
          <div
            v-for="(f, i) in store.selectedFiles"
            :key="f"
            class="thumb-tile"
            :class="{ dragging: draggedIndex === i, dropTarget: draggedIndex !== null && draggedIndex !== i }"
            :style="thumbTileStyle(i)"
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
                :style="[thumbImagePlacementStyle(f, i), thumbnailImageStyle(f)]"
              />
              <span v-else class="thumb-fallback">加载中</span>
            </div>
            <span class="thumb-index">{{ i + 1 }}</span>
            <span class="thumb-name">{{ basename(f) }}</span>
          </div>
        </div>

        <img
          v-if="watermarkPreviewSrc"
          class="watermark-preview"
          :src="watermarkPreviewSrc"
          :style="watermarkPreviewStyle"
          alt="watermark preview"
        />

        <div
          v-if="textBlockPreviewText"
          class="text-block-preview"
          :style="textBlockPreviewStyle"
        >
          {{ textBlockPreviewText }}
        </div>
      </div>

      <div v-else class="empty-viewer">
        <Images :size="42" />
        <h2>导入图片后开始排版</h2>
        <p>当前画布会实时预览边距、间距、水印和文本块。</p>
        <button class="quiet-button" :disabled="processing" @click="selectImages">
          <ImagePlus :size="16" />
          导入图片
        </button>
      </div>
    </div>
  </div>

  <div v-else class="filmstrip-shell">
    <div class="filmstrip-header">
      <button class="collapse-button" @click="store.setFilmstripCollapsed(!store.ui.filmstripCollapsed)">
        <ChevronDown v-if="store.ui.filmstripCollapsed" :size="16" />
        <ChevronUp v-else :size="16" />
      </button>
      <div class="filmstrip-title">
        <span>图片队列</span>
        <strong>{{ selectedCountLabel }}</strong>
      </div>
      <div v-if="!store.ui.filmstripCollapsed" class="filmstrip-actions">
        <button class="quiet-button small-button" :disabled="processing" @click="appendImages">
          <Plus :size="14" />
          追加
        </button>
        <button
          class="quiet-button small-button danger-muted"
          :disabled="processing || store.selectedFiles.length === 0"
          @click="clearImages"
        >
          <Trash2 :size="14" />
          清空
        </button>
      </div>
    </div>

    <div v-if="!store.ui.filmstripCollapsed" class="filmstrip-body">
      <div v-if="store.selectedFiles.length === 0" class="empty-filmstrip">
        <span>没有图片在队列中</span>
        <button class="quiet-button small-button" :disabled="processing" @click="selectImages">
          <ImagePlus :size="14" />
          导入图片
        </button>
      </div>

      <div v-else class="filmstrip-scroll">
        <article
          v-for="(f, i) in store.selectedFiles"
          :key="f"
          class="queue-thumb"
          :class="{ dragging: draggedIndex === i }"
          :title="f"
          draggable="true"
          @dragstart="onDragStart(i)"
          @dragover.prevent
          @drop="onDrop(i)"
          @dragend="onDragEnd"
        >
          <GripVertical class="drag-handle" :size="14" />
          <div class="queue-image">
            <img
              v-if="store.thumbnails[f]"
              :src="store.thumbnails[f]!"
              :alt="basename(f)"
              :style="thumbnailImageStyle(f)"
            />
            <span v-else>加载中</span>
          </div>
          <div class="queue-meta">
            <span class="queue-index">{{ String(i + 1).padStart(2, '0') }}</span>
            <span class="queue-name">{{ basename(f) }}</span>
          </div>
          <div class="queue-tools">
            <button class="icon-tool" :disabled="processing" title="旋转" @click.stop="rotateImage(f)">
              <RotateCw :size="13" />
            </button>
            <button class="icon-tool" :disabled="processing || i === 0" title="前移" @click.stop="moveImage(i, i - 1)">
              <ArrowLeft :size="13" />
            </button>
            <button
              class="icon-tool"
              :disabled="processing || i === store.selectedFiles.length - 1"
              title="后移"
              @click.stop="moveImage(i, i + 1)"
            >
              <ArrowRight :size="13" />
            </button>
            <button class="icon-tool danger-icon" :disabled="processing" title="移除" @click.stop="removeImage(i)">
              <X :size="13" />
            </button>
          </div>
        </article>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import {
  ArrowLeft,
  ArrowRight,
  ChevronDown,
  ChevronUp,
  ExternalLink,
  FolderOpen,
  GripVertical,
  ImagePlus,
  Images,
  Plus,
  RotateCw,
  Trash2,
  X,
} from 'lucide-vue-next'
import { useAppStore } from '../stores/appStore'
import { BACKGROUND_COLOR_OPTIONS } from '../types/protocol'
import {
  computePreviewLayout,
  computeTileFrame,
  computeTilePlacement,
  type ImageSizeLike,
} from '../utils/previewLayout'

const props = withDefaults(defineProps<{
  variant?: 'task' | 'viewer' | 'filmstrip'
}>(), {
  variant: 'viewer',
})

const store = useAppStore()
const s = store.settings
const draggedIndex = ref<number | null>(null)
const previewFrameRef = ref<HTMLElement | null>(null)
const previewFrameWidth = ref(0)
let previewResizeObserver: ResizeObserver | null = null

const processing = computed(() => store.processing)
const gridCols = computed(() => Math.max(1, Math.ceil(Math.sqrt(store.selectedFiles.length))))
const gridRows = computed(() =>
  Math.max(1, Math.ceil(store.selectedFiles.length / gridCols.value))
)

const PREVIEW_MIN_HEIGHT = 260
const PREVIEW_MAX_HEIGHT = 660
const TILE_MAX_SIZE = 190

const backgroundColorHex = computed(() => {
  return BACKGROUND_COLOR_OPTIONS.find((opt) => opt.value === s.backgroundColor)?.hex ?? '#ffffff'
})

const currentColorLabel = computed(
  () => BACKGROUND_COLOR_OPTIONS.find((opt) => opt.value === s.backgroundColor)?.label ?? ''
)

const selectedCountLabel = computed(() => {
  const n = store.selectedFiles.length
  return n === 0 ? '0 张' : `${n} 张`
})

const outputDirName = computed(() => {
  if (!store.settings.outputDir) return ''
  return basename(store.settings.outputDir)
})

const taskStatus = computed(() => {
  if (store.processing) return store.statusMessage || '处理中'
  if (store.errorMessage) return '失败'
  if (store.cancelledMessage) return '已取消'
  if (store.outputFiles.length) return '完成'
  if (!store.settings.outputDir) return '待选择输出'
  if (!store.selectedFiles.length) return '待导入'
  return '就绪'
})

const resultSummary = computed(() => {
  if (store.outputFiles.length) {
    return `完成 ${store.processedCount} 张，输出 ${store.outputFiles.length} 个文件`
  }
  if (store.cancelledMessage) return store.cancelledMessage
  if (store.errorMessage) return store.errorMessage
  return ''
})

const canvasMeta = computed(() => {
  if (!store.selectedFiles.length) return '等待图片'
  return `${gridCols.value}×${gridRows.value} · ${s.finalSize}px · ${currentColorLabel.value}`
})

const previewGeometry = computed(() => {
  return computePreviewLayout({
    imageCount: store.selectedFiles.length,
    finalSize: s.finalSize,
    contentLongEdgePercent: s.contentLongEdgePercent,
    tileBorderPercent: s.tileBorderPercent,
    gapXPercent: s.gapXPercent,
    gapYPercent: s.gapYPercent,
    outerBorderMode: s.outerBorderMode,
    outerBorderPercent: s.outerBorderPercent,
  })
})

const previewFrameStyle = computed(() => {
  const geom = previewGeometry.value
  const ratio = geom.canvasWidth / geom.canvasHeight
  const reserved = store.ui.filmstripCollapsed ? 142 : 272
  const maxWidthByViewer = `min(100%, calc((100vh - ${reserved}px) * ${ratio.toFixed(4)}))`
  const maxWidthByTask = `min(${geom.cols * TILE_MAX_SIZE}px, ${Math.round(PREVIEW_MAX_HEIGHT * ratio)}px)`
  const maxWidth = props.variant === 'viewer' ? maxWidthByViewer : maxWidthByTask

  return {
    aspectRatio: `${geom.canvasWidth} / ${geom.canvasHeight}`,
    maxWidth,
    minHeight: `${PREVIEW_MIN_HEIGHT}px`,
    backgroundColor: backgroundColorHex.value,
    '--preview-bg': backgroundColorHex.value,
  }
})

const previewDisplayScale = computed(() => {
  const measuredWidth = previewFrameWidth.value
  if (measuredWidth > 0) {
    return measuredWidth / previewGeometry.value.canvasWidth
  }

  const geom = previewGeometry.value
  const ratio = geom.canvasWidth / geom.canvasHeight
  return Math.min(geom.cols * TILE_MAX_SIZE, PREVIEW_MAX_HEIGHT * ratio) / geom.canvasWidth
})

const thumbGridStyle = computed(() => {
  return {
    inset: '0',
  }
})

function thumbTileStyle(index: number) {
  const geom = previewGeometry.value
  const tile = computeTileFrame(geom, index)

  return {
    left: `${(tile.x / geom.canvasWidth) * 100}%`,
    top: `${(tile.y / geom.canvasHeight) * 100}%`,
    width: `${(tile.width / geom.canvasWidth) * 100}%`,
    height: `${(tile.height / geom.canvasHeight) * 100}%`,
  }
}

function thumbImagePlacementStyle(path: string, index: number) {
  const geom = previewGeometry.value
  const tile = computeTileFrame(geom, index)
  const image = computeTilePlacement(geom, index, previewImageSize(path))

  return {
    left: `${((image.x - tile.x) / tile.width) * 100}%`,
    top: `${((image.y - tile.y) / tile.height) * 100}%`,
    width: `${(image.width / tile.width) * 100}%`,
    height: `${(image.height / tile.height) * 100}%`,
  }
}

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

const textBlockPreviewText = computed(() => {
  if (!s.textBlockEnabled) return ''
  return s.textBlock.text.trim()
})

const textBlockPreviewStyle = computed(() => {
  const text = s.textBlock
  const scale = previewDisplayScale.value
  const fontSize = Math.max(1, text.font_size_px * scale)
  const lineHeight = Math.max(fontSize, text.line_height_px * scale)
  const padding = Math.max(0, text.padding_px * scale)

  return {
    left: `${clamp(text.position_x_percent, 0, 100)}%`,
    top: `${clamp(text.position_y_percent, 0, 100)}%`,
    width: `${clamp(text.max_width_percent, 1, 100)}%`,
    color: rgbaCss(text.text_rgba),
    backgroundColor: rgbaCss(text.background_rgba),
    padding: `${padding}px`,
    fontFamily: text.font_family,
    fontWeight: `${text.font_weight}`,
    fontStyle: text.font_style,
    fontSize: `${fontSize}px`,
    lineHeight: `${lineHeight}px`,
    textAlign: text.align,
  }
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
    store.appendSelectedFiles(uniqueNew)
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
  const manualRotation = store.getImageRotation(path)
  if (manualRotation !== 0) {
    return {
      transform: `rotate(${manualRotation}deg)`,
    }
  }

  return {
    transform: s.autoOrient ? exifOrientationTransform(store.getImageOrientation(path)) : 'none',
  }
}

function previewImageSize(path: string): ImageSizeLike | null {
  const size = store.imageSizes[path]
  if (!size) return null

  const manualRotation = store.getImageRotation(path)
  if (manualRotation === 90 || manualRotation === 270) {
    return {
      width: size.height,
      height: size.width,
    }
  }

  return size
}

function exifOrientationTransform(orientation: number | null): string {
  switch (orientation) {
    case 2:
      return 'scaleX(-1)'
    case 3:
      return 'rotate(180deg)'
    case 4:
      return 'scaleY(-1)'
    case 5:
      return 'rotate(90deg) scaleX(-1)'
    case 6:
      return 'rotate(90deg)'
    case 7:
      return 'rotate(270deg) scaleX(-1)'
    case 8:
      return 'rotate(270deg)'
    default:
      return 'none'
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

async function openOutputDir() {
  if (!store.settings.outputDir) return
  const error = await window.electronAPI.openPath(store.settings.outputDir)
  if (error) {
    store.errorMessage = error
  }
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, Number.isFinite(value) ? value : min))
}

function rgbaCss(color: [number, number, number, number]): string {
  const [r, g, b, a] = color
  return `rgba(${clamp(r, 0, 255)}, ${clamp(g, 0, 255)}, ${clamp(b, 0, 255)}, ${clamp(a, 0, 255) / 255})`
}

function updatePreviewFrameWidth() {
  previewFrameWidth.value = previewFrameRef.value?.getBoundingClientRect().width ?? 0
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
      void store.ensureImageSize(file)
    }
    if (watermarkEnabled && watermarkPath) {
      void store.ensureThumbnail(watermarkPath)
      void store.ensureImageSize(watermarkPath)
    }
  },
  { immediate: true }
)

watch(
  previewFrameRef,
  (element, previousElement) => {
    if (!previewResizeObserver) {
      previewResizeObserver = new ResizeObserver(updatePreviewFrameWidth)
    }

    if (previousElement) {
      previewResizeObserver.unobserve(previousElement)
    }
    if (element) {
      previewResizeObserver.observe(element)
    }
    updatePreviewFrameWidth()
  },
  { flush: 'post' }
)

onBeforeUnmount(() => {
  previewResizeObserver?.disconnect()
})
</script>

<style scoped>
.task-rail,
.viewer-workspace,
.filmstrip-shell {
  height: 100%;
}

.task-rail {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 14px;
  overflow-y: auto;
}

.rail-section {
  display: flex;
  flex-direction: column;
  gap: 9px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--color-border);
}

.rail-section:last-child {
  border-bottom: 0;
}

.task-stat {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: center;
  min-height: 30px;
  color: var(--color-text-muted);
  font-size: 12px;
}

.task-stat strong {
  max-width: 112px;
  color: var(--color-text);
  font-size: 12px;
  font-weight: 650;
  overflow: hidden;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.field-label {
  color: var(--color-text-muted);
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
}

.path-readout {
  max-height: 48px;
  color: var(--color-text-subtle);
  font-size: 11px;
  line-height: 1.4;
  overflow: hidden;
  overflow-wrap: anywhere;
}

.rail-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.result-section {
  margin-top: auto;
}

.result-line {
  color: var(--color-text-muted);
  font-size: 12px;
  line-height: 1.45;
}

.viewer-workspace {
  display: flex;
  min-width: 0;
  flex-direction: column;
}

.viewer-chrome {
  height: 34px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  padding: 0 14px;
  border-bottom: 1px solid var(--color-border);
  color: var(--color-text-muted);
  font-size: 11px;
  letter-spacing: 0.02em;
}

.viewer-stage {
  flex: 1;
  min-height: 0;
  display: grid;
  place-items: center;
  padding: 20px;
  background:
    linear-gradient(var(--viewer-grid-line) 1px, transparent 1px),
    linear-gradient(90deg, var(--viewer-grid-line) 1px, transparent 1px),
    var(--color-viewer-bg);
  background-size: 32px 32px;
}

.empty-viewer {
  width: min(420px, 100%);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  color: var(--color-text-muted);
  text-align: center;
}

.empty-viewer h2 {
  color: var(--color-text);
  font-size: 18px;
  font-weight: 650;
}

.empty-viewer p {
  max-width: 320px;
  font-size: 12px;
  line-height: 1.6;
}

.preview-frame {
  position: relative;
  width: 100%;
  align-self: center;
  border: 1px solid var(--color-canvas-border);
  border-radius: 2px;
  overflow: hidden;
  box-shadow: var(--shadow-canvas);
}

.viewer-preview-frame {
  min-height: 0 !important;
}

.collage-preview {
  position: absolute;
}

.thumb-tile {
  position: absolute;
  min-width: 0;
  overflow: hidden;
  padding: 0;
  border: 1px solid color-mix(in srgb, var(--color-canvas-border), transparent 35%);
  border-radius: 1px;
  background: var(--preview-bg);
  color: var(--color-text-muted);
  cursor: grab;
  transition: border-color 0.15s, opacity 0.15s, transform 0.15s;
}

.thumb-tile.dragging,
.queue-thumb.dragging {
  opacity: 0.52;
}

.thumb-tile.dropTarget:hover {
  border-color: var(--color-accent);
  transform: translateY(-1px);
}

.thumb-frame {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: var(--preview-bg);
}

.thumb-img {
  position: absolute;
  display: block;
  object-fit: fill;
  transform-origin: center;
  transition: transform 0.15s ease;
}

.thumb-fallback {
  display: flex;
  width: 100%;
  height: 100%;
  align-items: center;
  justify-content: center;
  color: var(--color-text-subtle);
  font-size: 12px;
}

.thumb-index {
  position: absolute;
  top: 5px;
  left: 5px;
  min-width: 22px;
  height: 20px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid rgba(255, 255, 255, 0.18);
  background: rgba(0, 0, 0, 0.58);
  color: #fff;
  font-size: 10px;
  font-weight: 700;
}

.thumb-name {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  padding: 20px 6px 5px;
  background: linear-gradient(to top, rgba(0, 0, 0, 0.68), transparent);
  color: #fff;
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.watermark-preview,
.text-block-preview {
  position: absolute;
  transform: translate(-50%, -50%);
  pointer-events: none;
  z-index: 5;
}

.watermark-preview {
  object-fit: contain;
  opacity: 0.78;
  filter: drop-shadow(0 1px 3px rgba(0, 0, 0, 0.35));
}

.text-block-preview {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  box-sizing: border-box;
}

.filmstrip-shell {
  display: flex;
  flex-direction: column;
}

.filmstrip-header {
  height: 38px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 10px;
  border-bottom: 1px solid var(--color-border);
}

.collapse-button {
  width: 26px;
  height: 26px;
  padding: 0;
  border: 1px solid var(--color-border);
  background: var(--color-control);
  color: var(--color-text-muted);
}

.filmstrip-title {
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
  color: var(--color-text);
  font-size: 12px;
  font-weight: 650;
}

.filmstrip-title strong {
  color: var(--color-text-muted);
  font-size: 11px;
  font-weight: 500;
}

.filmstrip-actions {
  display: flex;
  gap: 7px;
  margin-left: auto;
}

.filmstrip-body {
  height: 120px;
  min-height: 0;
  padding: 10px;
}

.empty-filmstrip {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--color-text-muted);
  font-size: 12px;
}

.filmstrip-scroll {
  height: 100%;
  display: flex;
  gap: 10px;
  overflow-x: auto;
  overflow-y: hidden;
  padding-bottom: 4px;
}

.queue-thumb {
  flex: 0 0 184px;
  display: grid;
  grid-template-columns: 14px 72px minmax(0, 1fr);
  grid-template-rows: 1fr 24px;
  gap: 7px;
  padding: 7px;
  border: 1px solid var(--color-border);
  background: var(--color-panel-raised);
  color: var(--color-text);
  cursor: grab;
}

.drag-handle {
  grid-row: 1 / -1;
  align-self: center;
  color: var(--color-text-subtle);
}

.queue-image {
  grid-row: 1 / -1;
  width: 72px;
  height: 72px;
  display: grid;
  place-items: center;
  overflow: hidden;
  background: var(--color-viewer-bg);
}

.queue-image img {
  width: 100%;
  height: 100%;
  display: block;
  object-fit: contain;
  transform-origin: center;
}

.queue-image span {
  color: var(--color-text-subtle);
  font-size: 11px;
}

.queue-meta {
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 5px;
}

.queue-index {
  color: var(--color-text-subtle);
  font-size: 10px;
  letter-spacing: 0.08em;
}

.queue-name {
  color: var(--color-text);
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.queue-tools {
  display: flex;
  align-items: center;
  gap: 4px;
}

.icon-tool {
  width: 22px;
  height: 22px;
  padding: 0;
  border: 1px solid var(--color-border);
  background: var(--color-control);
  color: var(--color-text-muted);
}

.danger-icon {
  color: var(--color-danger);
}

.quiet-button,
.small-button,
.full-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
}

.quiet-button {
  border: 1px solid var(--color-border);
  background: var(--color-control);
  color: var(--color-text);
}

.full-button {
  width: 100%;
}

.small-button {
  height: 26px;
  padding: 0 9px;
  font-size: 11px;
}

.danger-muted {
  color: var(--color-danger);
}
</style>
