<template>
  <div class="app-shell" :data-theme="store.ui.theme">
    <header class="app-toolbar">
      <div class="brand-block">
        <div class="brand-mark">PL</div>
        <div class="brand-copy">
          <h1>PicLayout</h1>
          <span>{{ fileCountText }} · {{ statusLabel }}</span>
        </div>
      </div>

      <div class="toolbar-actions">
        <button class="toolbar-button" :disabled="busy" @click="selectImages">
          <ImagePlus :size="16" />
          导入图片
        </button>
        <button class="toolbar-button" :disabled="busy" @click="appendImages">
          <Plus :size="16" />
          追加
        </button>
        <button class="toolbar-button output-button" :disabled="busy" @click="selectOutputDir">
          <FolderOpen :size="16" />
          {{ outputDirLabel }}
        </button>
        <button class="primary-action" :disabled="!canStart" @click="startCollage">
          <Loader2 v-if="store.processing" class="spin-icon" :size="16" />
          <Play v-else :size="16" />
          {{ store.processing ? '处理中' : '开始导出' }}
        </button>
        <button class="icon-button" :title="themeTitle" @click="store.toggleTheme()">
          <Sun v-if="store.ui.theme === 'dark'" :size="17" />
          <Moon v-else :size="17" />
        </button>
      </div>
    </header>

    <main class="workbench">
      <aside class="task-sidebar">
        <FileSelector variant="task" />
      </aside>

      <section class="viewer-panel" aria-label="拼贴预览">
        <FileSelector variant="viewer" />
      </section>

      <aside class="tool-sidebar">
        <SettingsPanel />
      </aside>
    </main>

    <footer class="bottom-dock" :class="{ collapsed: store.ui.filmstripCollapsed }">
      <FileSelector variant="filmstrip" />
    </footer>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import {
  FolderOpen,
  ImagePlus,
  Loader2,
  Moon,
  Play,
  Plus,
  Sun,
} from 'lucide-vue-next'
import { useAppStore } from './stores/appStore'
import type { CollageResult, PreviewResult, ProgressMessage } from './types/protocol'
import {
  buildCollageConfig,
  cloneCollageConfig,
  createCollageConfigSignature,
} from './utils/collageConfig'
import FileSelector from './components/FileSelector.vue'
import SettingsPanel from './components/SettingsPanel.vue'

const store = useAppStore()
const PREVIEW_LONG_EDGE = 1800

const busy = computed(() => store.processing || store.renderedPreview.rendering)
const canStart = computed(
  () => store.selectedFiles.length > 0 && !!store.settings.outputDir && !busy.value
)

const fileCountText = computed(() => {
  const n = store.selectedFiles.length
  return n === 0 ? '未选择图片' : `${n} 张图片`
})

const statusLabel = computed(() => {
  if (store.processing) return store.statusMessage || '正在处理'
  if (store.renderedPreview.rendering) return '生成精准预览'
  if (store.errorMessage) return '处理失败'
  if (store.cancelledMessage) return '已取消'
  if (store.outputFiles.length) return '已完成'
  return store.settings.outputDir ? '准备就绪' : '等待输出目录'
})

const outputDirLabel = computed(() => {
  if (!store.settings.outputDir) return '选择输出'
  return basename(store.settings.outputDir)
})

const themeTitle = computed(() =>
  store.ui.theme === 'dark' ? '切换到浅色主题' : '切换到暗色主题'
)

const STAGE_PROGRESS: Record<string, number> = {
  processing_images: 0,
  creating_collage: 62,
  adding_border: 82,
  adding_watermark: 92,
  saving_output: 96,
}

let removeProgressListener: (() => void) | null = null
let uiTaskStartedAt = 0
let wallTimer: ReturnType<typeof setInterval> | null = null
let lastStartedConfigSignature = ''

onMounted(() => {
  removeProgressListener = window.electronAPI.onProgress(handleProgress)
})

onUnmounted(() => {
  removeProgressListener?.()
  stopWallTimer()
})

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

async function selectOutputDir() {
  const dir = await window.electronAPI.openDirectory()
  if (!dir) return
  store.settings.outputDir = dir
  store.saveSettings()
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

function startWallTimer(startedAt: number) {
  uiTaskStartedAt = startedAt
  store.wallElapsedMs = 0
  stopWallTimer()
  wallTimer = setInterval(updateWallElapsed, 200)
  updateWallElapsed()
}

function stopWallTimer() {
  if (!wallTimer) return
  clearInterval(wallTimer)
  wallTimer = null
}

function updateWallElapsed() {
  if (!uiTaskStartedAt) return
  store.wallElapsedMs = Math.round(performance.now() - uiTaskStartedAt)
}

function finishWallTimer() {
  updateWallElapsed()
  stopWallTimer()
}

function handleProgress(msg: ProgressMessage) {
  if (msg.type === 'job_started') {
    store.setElapsed(0)
  } else if (msg.type === 'image_processed') {
    const pct = Math.round((msg.index / msg.total) * 60)
    store.setElapsed(msg.elapsed_ms)
    store.setProgress(pct, `处理图片 ${msg.index}/${msg.total}`)
  } else if (msg.type === 'stage_changed') {
    const basePct = STAGE_PROGRESS[msg.stage] ?? store.progress
    store.setElapsed(msg.elapsed_ms)
    store.setProgress(basePct, msg.message)
  } else if (msg.type === 'stage_finished') {
    store.setElapsed(msg.total_elapsed_ms)
    store.setStageTiming({
      stage: msg.stage,
      elapsed_ms: msg.elapsed_ms,
      details: msg.details ?? [],
    })
  } else if (msg.type === 'completed') {
    finishWallTimer()
    store.processing = false
    store.outputFiles = msg.outputs
    store.processedCount = msg.processed_count
    store.failedImages = msg.failed_images
    store.warnings = msg.warnings
    store.elapsedMs = msg.elapsed_ms
    store.stageTimings = msg.stage_timings
    store.setProgress(100, '处理完成')
    void loadOutputPreview(msg.outputs, lastStartedConfigSignature)
  } else if (msg.type === 'cancelled') {
    finishWallTimer()
    store.processing = false
    store.cancelledMessage = msg.message
    store.partialOutputs = msg.partial_outputs
    store.setProgress(store.progress, '已取消')
  } else if (msg.type === 'error') {
    finishWallTimer()
    store.processing = false
    store.errorMessage = msg.message
  }
}

function applyCompletedResult(result: CollageResult) {
  finishWallTimer()
  store.processing = false
  store.outputFiles = result.outputs
  store.processedCount = result.processed_count
  store.failedImages = result.failed_images
  store.warnings = result.warnings
  store.elapsedMs = result.elapsed_ms
  if (!store.wallElapsedMs) {
    store.wallElapsedMs = result.wall_elapsed_ms
  }
  store.stageTimings = result.stage_timings
  store.setProgress(100, '处理完成')
  void loadOutputPreview(result.outputs, lastStartedConfigSignature)
}

async function startCollage() {
  if (!canStart.value) return

  const clickedAt = performance.now()
  const config = buildCollageConfig(
    store.settings,
    store.selectedFiles,
    store.selectedImageRotations()
  )
  lastStartedConfigSignature = createCollageConfigSignature(config)

  store.processing = true
  store.resetProgress()
  store.outputFiles = []
  startWallTimer(clickedAt)

  try {
    const plainConfig = cloneCollageConfig(config)
    const result = await window.electronAPI.startCollage(plainConfig)
    if (store.processing && !store.cancelledMessage && !store.errorMessage) {
      applyCompletedResult(result)
    }
  } catch (err: unknown) {
    if (!store.outputFiles.length && !store.cancelledMessage) {
      finishWallTimer()
      store.errorMessage = formatCollageError(err)
      store.processing = false
    }
  }
}

async function loadOutputPreview(outputs: string[], signature: string) {
  const output = outputs[0]
  if (!output || !signature) return

  try {
    const imagePreview = await window.electronAPI.getImagePreviewDataUrl(output, PREVIEW_LONG_EDGE)
    if (!imagePreview) return
    const result: PreviewResult = {
      ...imagePreview,
      processed_count: store.processedCount,
      failed_images: store.failedImages,
      warnings: store.warnings,
      elapsed_ms: store.elapsedMs,
      stage_timings: store.stageTimings,
    }
    store.setRenderedPreview(result, signature, 'output')
  } catch (err) {
    console.error('failed to load output preview:', err)
  }
}

function basename(path: string): string {
  return path.replace(/\\/g, '/').split('/').pop() ?? path
}

function formatCollageError(err: unknown): string {
  const raw = err instanceof Error ? err.message : String(err)
  return raw
    .replace(/^Error invoking remote method 'collage:start': Error: /, '')
    .replace(/^Error invoking remote method 'collage:start': /, '')
}
</script>

<style scoped>
.app-shell {
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
  background: var(--color-bg);
  color: var(--color-text);
}

.app-toolbar {
  height: 56px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  padding: 0 14px 0 16px;
  border-bottom: 1px solid var(--color-border);
  background: var(--color-toolbar);
}

.brand-block {
  display: flex;
  align-items: center;
  gap: 11px;
  min-width: 210px;
}

.brand-mark {
  width: 30px;
  height: 30px;
  display: grid;
  place-items: center;
  border: 1px solid var(--color-border-strong);
  border-radius: 6px;
  color: var(--color-text);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.08em;
}

.brand-copy {
  min-width: 0;
}

.brand-copy h1 {
  font-size: 15px;
  line-height: 1.15;
  font-weight: 700;
}

.brand-copy span {
  display: block;
  margin-top: 2px;
  color: var(--color-text-muted);
  font-size: 11px;
  line-height: 1.2;
}

.toolbar-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  min-width: 0;
}

.toolbar-button,
.primary-action,
.icon-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  height: 32px;
  border: 1px solid var(--color-border);
  background: var(--color-control);
  color: var(--color-text);
  font-size: 12px;
  font-weight: 600;
}

.toolbar-button {
  padding: 0 11px;
}

.output-button {
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.primary-action {
  border-color: var(--color-accent);
  background: var(--color-accent);
  color: var(--color-accent-text);
  padding: 0 14px;
}

.icon-button {
  width: 32px;
  padding: 0;
}

.spin-icon {
  animation: spin 0.9s linear infinite;
}

.workbench {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 232px minmax(360px, 1fr) 326px;
  background: var(--color-bg);
}

.task-sidebar,
.tool-sidebar,
.viewer-panel {
  min-height: 0;
  overflow: hidden;
}

.task-sidebar {
  border-right: 1px solid var(--color-border);
  background: var(--color-panel);
}

.viewer-panel {
  background: var(--color-viewer-bg);
}

.tool-sidebar {
  border-left: 1px solid var(--color-border);
  background: var(--color-panel);
}

.bottom-dock {
  border-top: 1px solid var(--color-border);
  background: var(--color-panel);
}

.bottom-dock.collapsed {
  min-height: 42px;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

@media (max-width: 1120px) {
  .workbench {
    grid-template-columns: 214px minmax(320px, 1fr) 300px;
  }

  .toolbar-button {
    padding: 0 9px;
  }
}
</style>
