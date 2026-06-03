<template>
  <div class="app-layout">
    <!-- 标题栏 -->
    <header class="app-header">
      <h1 class="app-title">PicLayout</h1>
      <span class="app-subtitle">图片拼贴排版工具</span>
    </header>

    <!-- 标签页 -->
    <div class="tabs">
      <button
        class="tab-btn"
        :class="{ active: activeTab === 'home' }"
        @click="activeTab = 'home'"
      >主页</button>
      <button
        class="tab-btn"
        :class="{ active: activeTab === 'settings' }"
        @click="activeTab = 'settings'"
      >设置</button>
    </div>

    <!-- 主页 -->
    <div v-show="activeTab === 'home'" class="tab-content">
      <FileSelector />

      <div class="action-row">
        <button
          class="btn-primary start-btn"
          :disabled="!canStart"
          @click="startCollage"
        >
          {{ store.processing ? '处理中...' : '开始拼接' }}
        </button>
      </div>

      <ProgressBar />
    </div>

    <!-- 设置页 -->
    <div v-show="activeTab === 'settings'" class="tab-content">
      <SettingsPanel />
    </div>

    <!-- 底部版权 -->
    <footer class="app-footer">© Yuui's PicLayoutTool</footer>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useAppStore } from './stores/appStore'
import type { CollageConfig, CollageResult, ProgressMessage } from './types/protocol'
import FileSelector from './components/FileSelector.vue'
import SettingsPanel from './components/SettingsPanel.vue'
import ProgressBar from './components/ProgressBar.vue'

const store = useAppStore()
const activeTab = ref<'home' | 'settings'>('home')

const canStart = computed(
  () => store.selectedFiles.length > 0 && !!store.settings.outputDir && !store.processing
)

// 进度处理：阶段权重映射
const STAGE_PROGRESS: Record<string, number> = {
  processing_images: 0,
  creating_collage: 62,
  adding_border: 82,
  adding_watermark: 92,
}

let removeProgressListener: (() => void) | null = null

onMounted(() => {
  removeProgressListener = window.electronAPI.onProgress(handleProgress)
})

onUnmounted(() => {
  removeProgressListener?.()
})

function handleProgress(msg: ProgressMessage) {
  if (msg.type === 'image_processed') {
    const pct = Math.round((msg.index / msg.total) * 60)
    store.setProgress(pct, `处理图片 ${msg.index}/${msg.total}`)
  } else if (msg.type === 'stage_changed') {
    const basePct = STAGE_PROGRESS[msg.stage] ?? store.progress
    store.setProgress(basePct, msg.message)
  } else if (msg.type === 'completed') {
    store.processing = false
    store.outputFiles = msg.outputs
    store.processedCount = msg.processed_count
    store.failedImages = msg.failed_images
    store.warnings = msg.warnings
    store.setProgress(100, '处理完成')
  } else if (msg.type === 'cancelled') {
    store.processing = false
    store.cancelledMessage = msg.message
    store.partialOutputs = msg.partial_outputs
    store.setProgress(store.progress, '已取消')
  } else if (msg.type === 'error') {
    store.processing = false
    store.errorMessage = msg.message
  }
}

function applyCompletedResult(result: CollageResult) {
  store.processing = false
  store.outputFiles = result.outputs
  store.processedCount = result.processed_count
  store.failedImages = result.failed_images
  store.warnings = result.warnings
  store.setProgress(100, '处理完成')
}

async function startCollage() {
  if (!canStart.value) return

  const s = store.settings
  const config: CollageConfig = {
    image_paths: store.selectedFiles,
    output_dir: s.outputDir,
    prefix: s.prefix || 'output',
    resample_size: s.resampleSize,
    border_size: s.borderSize,
    final_size: s.finalSize,
    dpi: s.dpi,
    background_color: s.backgroundColor,
    overwrite: false,
    output_settings: {
      jpeg_quality: s.jpegQuality,
      auto_orient: s.autoOrient,
    },
    color_management: {
      enabled: s.colorManagementEnabled,
      target_profile: s.targetProfileMode,
      target_profile_path:
        s.targetProfileMode === 'custom' && s.targetProfilePath
          ? s.targetProfilePath
          : null,
      rendering_intent: s.renderingIntent,
    },
    watermark:
      s.watermarkEnabled && s.watermark.path
        ? { ...s.watermark }
        : null,
  }

  store.processing = true
  store.resetProgress()
  store.outputFiles = []

  try {
    // 将响应式 Proxy 转为纯对象，避免 IPC 结构化克隆失败
    const plainConfig = JSON.parse(JSON.stringify(config))
    const result = await window.electronAPI.startCollage(plainConfig)
    if (store.processing && !store.cancelledMessage && !store.errorMessage) {
      applyCompletedResult(result)
    }
  } catch (err: unknown) {
    if (!store.outputFiles.length && !store.cancelledMessage) {
      store.errorMessage = formatCollageError(err)
      store.processing = false
    }
  }
}

function formatCollageError(err: unknown): string {
  const raw = err instanceof Error ? err.message : String(err)
  return raw
    .replace(/^Error invoking remote method 'collage:start': Error: /, '')
    .replace(/^Error invoking remote method 'collage:start': /, '')
}
</script>

<style scoped>
.app-layout {
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
}

.app-header {
  background: var(--color-primary);
  color: white;
  padding: 10px 16px;
  display: flex;
  align-items: baseline;
  gap: 10px;
}

.app-title {
  font-size: 18px;
  font-weight: 700;
}

.app-subtitle {
  font-size: 12px;
  opacity: 0.85;
}

.tab-content {
  flex: 1;
  overflow-y: auto;
  padding: 12px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.action-row {
  display: flex;
  justify-content: center;
}

.start-btn {
  padding: 10px 48px;
  font-size: 15px;
  border-radius: 24px;
}

.app-footer {
  text-align: center;
  font-size: 11px;
  color: var(--color-text-secondary);
  padding: 6px;
  border-top: 1px solid var(--color-border);
}
</style>
