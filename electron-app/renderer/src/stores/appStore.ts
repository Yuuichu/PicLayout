import { defineStore } from 'pinia'
import { ref, reactive } from 'vue'
import type {
  BackgroundColor,
  FailedImage,
  RenderingIntent,
  TargetProfileMode,
  WatermarkConfig,
} from '../types/protocol'

// 用户设置（持久化到 localStorage）
const SETTINGS_KEY = 'piclayout_settings'

interface Settings {
  maxImages: number
  resampleSize: number
  borderSize: number
  finalSize: number
  dpi: number
  backgroundColor: BackgroundColor
  prefix: string
  outputDir: string
  jpegQuality: number
  autoOrient: boolean
  colorManagementEnabled: boolean
  targetProfileMode: TargetProfileMode
  targetProfilePath: string
  renderingIntent: RenderingIntent
  watermarkEnabled: boolean
  watermark: WatermarkConfig
}

function loadSettings(): Settings {
  try {
    const stored = localStorage.getItem(SETTINGS_KEY)
    if (stored) return { ...defaultSettings(), ...JSON.parse(stored) }
  } catch {}
  return defaultSettings()
}

function defaultSettings(): Settings {
  return {
    maxImages: 30,
    resampleSize: 4000,
    borderSize: 4200,
    finalSize: 10000,
    dpi: 300,
    backgroundColor: 'white',
    prefix: 'output',
    outputDir: '',
    jpegQuality: 95,
    autoOrient: true,
    colorManagementEnabled: true,
    targetProfileMode: 'srgb',
    targetProfilePath: '',
    renderingIntent: 'perceptual',
    watermarkEnabled: false,
    watermark: {
      path: '',
      scale_percent: 100,
      position_x_percent: 50,
      position_y_percent: 95,
    },
  }
}

export const useAppStore = defineStore('app', () => {
  // 设置
  const settings = reactive<Settings>(loadSettings())

  function saveSettings() {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings))
  }

  // 已选图片
  const selectedFiles = ref<string[]>([])

  function setSelectedFiles(files: string[]) {
    selectedFiles.value = files
  }

  function appendSelectedFiles(files: string[]) {
    const seen = new Set(selectedFiles.value)
    const merged = [...selectedFiles.value]
    for (const file of files) {
      if (!seen.has(file)) {
        seen.add(file)
        merged.push(file)
      }
    }
    selectedFiles.value = merged
  }

  function moveSelectedFile(from: number, to: number) {
    if (from === to || from < 0 || to < 0 || from >= selectedFiles.value.length || to >= selectedFiles.value.length) {
      return
    }
    const next = [...selectedFiles.value]
    const [item] = next.splice(from, 1)
    next.splice(to, 0, item)
    selectedFiles.value = next
  }

  // 处理状态
  const processing = ref(false)
  const progress = ref(0)        // 0–100
  const statusMessage = ref('')
  const outputFiles = ref<string[]>([])
  const processedCount = ref(0)
  const failedImages = ref<FailedImage[]>([])
  const warnings = ref<string[]>([])
  const cancelledMessage = ref('')
  const partialOutputs = ref<string[]>([])
  const errorMessage = ref('')

  function resetProgress() {
    progress.value = 0
    statusMessage.value = ''
    outputFiles.value = []
    processedCount.value = 0
    failedImages.value = []
    warnings.value = []
    cancelledMessage.value = ''
    partialOutputs.value = []
    errorMessage.value = ''
  }

  function setProgress(pct: number, msg: string) {
    progress.value = pct
    statusMessage.value = msg
  }

  return {
    settings,
    saveSettings,
    selectedFiles,
    setSelectedFiles,
    appendSelectedFiles,
    moveSelectedFile,
    processing,
    progress,
    statusMessage,
    outputFiles,
    processedCount,
    failedImages,
    warnings,
    cancelledMessage,
    partialOutputs,
    errorMessage,
    resetProgress,
    setProgress,
  }
})
