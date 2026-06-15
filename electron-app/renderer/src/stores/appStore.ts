import { defineStore } from 'pinia'
import { ref, reactive } from 'vue'
import type {
  BackgroundColor,
  FailedImage,
  ImageRotationDegrees,
  ProcessingMode,
  RenderingIntent,
  StageTiming,
  TargetProfileMode,
  WatermarkConfig,
} from '../types/protocol'

// 用户设置（持久化到 localStorage）
const SETTINGS_KEY = 'piclayout_settings'

interface ImageSize {
  width: number
  height: number
}

interface Settings {
  maxImages: number
  resampleSize: number
  borderSize: number
  finalSize: number
  dpi: number
  backgroundColor: BackgroundColor
  prefix: string
  outputDir: string
  processingMode: ProcessingMode
  jpegQuality: number
  autoOrient: boolean
  linearLightResize: boolean
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
    if (stored) {
      const parsed = JSON.parse(stored)
      const settings = { ...defaultSettings(), ...parsed }
      normalizeQualitySettings(settings, parsed)
      return settings
    }
  } catch {}
  return defaultSettings()
}

function normalizeQualitySettings(settings: Settings, parsed: Partial<Settings> & { processingMode?: string }) {
  const storedMode = parsed.processingMode
  if (!storedMode) {
    settings.processingMode = 'standard_high_quality'
    settings.linearLightResize = false
    return
  }

  if (storedMode === 'high_quality') {
    settings.processingMode = 'maximum_quality'
  } else if (storedMode === 'standard') {
    settings.processingMode = 'standard_high_quality'
  } else if (storedMode === 'fast') {
    settings.processingMode = 'fast_preview'
  }

  if (settings.processingMode === 'maximum_quality' && parsed.linearLightResize === undefined) {
    settings.linearLightResize = true
  }

  if (settings.linearLightResize) {
    settings.processingMode = 'maximum_quality'
  } else if (settings.processingMode === 'maximum_quality') {
    settings.processingMode = 'standard_high_quality'
  }
}

function defaultSettings(): Settings {
  return {
    maxImages: 40,
    resampleSize: 4000,
    borderSize: 4200,
    finalSize: 10000,
    dpi: 300,
    backgroundColor: 'white',
    prefix: 'output',
    outputDir: '',
    processingMode: 'standard_high_quality',
    jpegQuality: 95,
    autoOrient: true,
    linearLightResize: false,
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
  const thumbnails = reactive<Record<string, string | null>>({})
  const imageSizes = reactive<Record<string, ImageSize | null>>({})
  const imageOrientations = reactive<Record<string, number | null>>({})
  const imageRotations = reactive<Record<string, ImageRotationDegrees>>({})

  function setSelectedFiles(files: string[]) {
    selectedFiles.value = files
    pruneThumbnails()
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
    pruneThumbnails()
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

  function rotateImage(path: string) {
    const current = imageRotations[path] ?? 0
    const next = ((current + 90) % 360) as ImageRotationDegrees
    if (next === 0) {
      delete imageRotations[path]
    } else {
      imageRotations[path] = next
    }
  }

  function getImageRotation(path: string): ImageRotationDegrees {
    return imageRotations[path] ?? 0
  }

  function getImageOrientation(path: string): number | null {
    return imageOrientations[path] ?? null
  }

  function selectedImageRotations(): Record<string, ImageRotationDegrees> {
    const rotations: Record<string, ImageRotationDegrees> = {}
    for (const path of selectedFiles.value) {
      const degrees = getImageRotation(path)
      if (degrees !== 0) {
        rotations[path] = degrees
      }
    }
    return rotations
  }

  function swapSelectedFiles(first: number, second: number) {
    if (
      first === second ||
      first < 0 ||
      second < 0 ||
      first >= selectedFiles.value.length ||
      second >= selectedFiles.value.length
    ) {
      return
    }
    const next = [...selectedFiles.value]
    ;[next[first], next[second]] = [next[second], next[first]]
    selectedFiles.value = next
  }

  async function ensureThumbnail(path: string) {
    if (path in thumbnails) return
    thumbnails[path] = null
    const [thumbnail, orientation] = await Promise.all([
      window.electronAPI.getThumbnail(path),
      window.electronAPI.getImageOrientation(path),
    ])
    thumbnails[path] = thumbnail
    imageOrientations[path] = orientation
  }

  async function ensureImageOrientation(path: string) {
    if (path in imageOrientations) return
    imageOrientations[path] = await window.electronAPI.getImageOrientation(path)
  }

  async function ensureImageSize(path: string) {
    if (path in imageSizes) return
    imageSizes[path] = null
    imageSizes[path] = await window.electronAPI.getImageSize(path)
  }

  function pruneThumbnails() {
    const selected = new Set(selectedFiles.value)
    if (settings.watermark.path) {
      selected.add(settings.watermark.path)
    }
    for (const path of Object.keys(thumbnails)) {
      if (!selected.has(path)) {
        delete thumbnails[path]
      }
    }
    for (const path of Object.keys(imageSizes)) {
      if (!selected.has(path)) {
        delete imageSizes[path]
      }
    }
    for (const path of Object.keys(imageOrientations)) {
      if (!selected.has(path)) {
        delete imageOrientations[path]
      }
    }
    for (const path of Object.keys(imageRotations)) {
      if (!selected.has(path)) {
        delete imageRotations[path]
      }
    }
  }

  // 处理状态
  const processing = ref(false)
  const progress = ref(0)        // 0–100
  const statusMessage = ref('')
  const outputFiles = ref<string[]>([])
  const processedCount = ref(0)
  const failedImages = ref<FailedImage[]>([])
  const warnings = ref<string[]>([])
  const elapsedMs = ref(0)
  const wallElapsedMs = ref(0)
  const stageTimings = ref<StageTiming[]>([])
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
    elapsedMs.value = 0
    wallElapsedMs.value = 0
    stageTimings.value = []
    cancelledMessage.value = ''
    partialOutputs.value = []
    errorMessage.value = ''
  }

  function setProgress(pct: number, msg: string) {
    progress.value = pct
    statusMessage.value = msg
  }

  function setElapsed(ms: number) {
    elapsedMs.value = ms
  }

  function setStageTiming(timing: StageTiming) {
    const next = stageTimings.value.filter((item) => item.stage !== timing.stage)
    next.push(timing)
    stageTimings.value = next
  }

  return {
    settings,
    saveSettings,
    selectedFiles,
    thumbnails,
    imageSizes,
    imageOrientations,
    imageRotations,
    setSelectedFiles,
    appendSelectedFiles,
    moveSelectedFile,
    swapSelectedFiles,
    rotateImage,
    getImageRotation,
    getImageOrientation,
    selectedImageRotations,
    ensureThumbnail,
    ensureImageSize,
    ensureImageOrientation,
    processing,
    progress,
    statusMessage,
    outputFiles,
    processedCount,
    failedImages,
    warnings,
    elapsedMs,
    wallElapsedMs,
    stageTimings,
    cancelledMessage,
    partialOutputs,
    errorMessage,
    resetProgress,
    setProgress,
    setElapsed,
    setStageTiming,
  }
})
