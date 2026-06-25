import { defineStore } from 'pinia'
import { ref, reactive } from 'vue'
import type {
  BackgroundColor,
  FailedImage,
  ImageRotationDegrees,
  PreviewResult,
  ProcessingMode,
  RenderingIntent,
  PositionReference,
  StageTiming,
  TargetProfileMode,
  TextBlockConfig,
  WatermarkConfig,
} from '../types/protocol'
import {
  isCanvasAspectPreset,
  type CanvasAspectPreset,
} from '../utils/aspectRatioPresets'

// 用户设置（持久化到 localStorage）
const SETTINGS_KEY = 'piclayout_settings'
const UI_KEY = 'piclayout_ui'

interface ImageSize {
  width: number
  height: number
}

export interface Settings {
  maxImages: number
  contentLongEdgePercent: number
  tileBorderPercent: number
  imageGapPercent: number
  gapXPercent: number
  gapYPercent: number
  outerBorderMode: 'auto' | 'custom'
  outerBorderPercent: number
  finalSize: number
  canvasAspectPreset: CanvasAspectPreset
  customAspectWidth: number
  customAspectHeight: number
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
  textBlockEnabled: boolean
  textBlock: TextBlockConfig
}

type AppTheme = 'dark' | 'light'
type RenderedPreviewSource = 'precise' | 'output'

interface UiState {
  theme: AppTheme
  filmstripCollapsed: boolean
}

interface RenderedPreviewState extends PreviewResult {
  signature: string
  source: RenderedPreviewSource | null
  rendering: boolean
  errorMessage: string
}

function emptyRenderedPreview(): RenderedPreviewState {
  return {
    data_url: '',
    width: 0,
    height: 0,
    final_width: 0,
    final_height: 0,
    processed_count: 0,
    failed_images: [],
    warnings: [],
    elapsed_ms: 0,
    stage_timings: [],
    signature: '',
    source: null,
    rendering: false,
    errorMessage: '',
  }
}

interface LegacyLayoutSettings {
  resampleSize?: number
  borderSize?: number
  tileBorderPx?: number
  gapXPx?: number
  gapYPx?: number
  outerBorderPx?: number
}

function loadSettings(): Settings {
  try {
    const stored = localStorage.getItem(SETTINGS_KEY)
    if (stored) {
      const parsed = JSON.parse(stored)
      const defaults = defaultSettings()
      const settings = {
        ...defaults,
        ...parsed,
        watermark: { ...defaults.watermark, ...parsed.watermark },
        textBlock: { ...defaults.textBlock, ...parsed.textBlock },
      }
      normalizeOverlayPositionSettings(settings, parsed)
      normalizeQualitySettings(settings, parsed)
      normalizeLayoutSettings(settings, parsed)
      normalizeAspectSettings(settings, parsed)
      return settings
    }
  } catch {}
  return defaultSettings()
}

function loadUiState(): UiState {
  try {
    const stored = localStorage.getItem(UI_KEY)
    if (stored) {
      const parsed = JSON.parse(stored) as Partial<UiState>
      return {
        theme: parsed.theme === 'light' ? 'light' : 'dark',
        filmstripCollapsed: !!parsed.filmstripCollapsed,
      }
    }
  } catch {}
  return {
    theme: 'dark',
    filmstripCollapsed: false,
  }
}

function normalizeQualitySettings(
  settings: Settings,
  parsed: Omit<Partial<Settings>, 'processingMode'> & { processingMode?: string }
) {
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
    contentLongEdgePercent: 40,
    tileBorderPercent: 1,
    imageGapPercent: 0,
    gapXPercent: 0,
    gapYPercent: 0,
    outerBorderMode: 'auto',
    outerBorderPercent: 10,
    finalSize: 10000,
    canvasAspectPreset: 'auto',
    customAspectWidth: 3,
    customAspectHeight: 4,
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
      position_reference: 'content',
      position_x_percent: 50,
      position_y_percent: 95,
    },
    textBlockEnabled: false,
    textBlock: {
      text: '',
      font_family: 'sans-serif',
      font_weight: 400,
      font_style: 'normal',
      font_size_px: 120,
      line_height_px: 144,
      max_width_percent: 60,
      align: 'center',
      text_rgba: [255, 255, 255, 255],
      background_rgba: [0, 0, 0, 0],
      padding_px: 0,
      position_reference: 'content',
      position_x_percent: 50,
      position_y_percent: 92,
    },
  }
}

function normalizeOverlayPositionSettings(settings: Settings, parsed: Partial<Settings>) {
  settings.watermark.position_reference = normalizeStoredPositionReference(
    parsed.watermark?.position_reference
  )
  settings.textBlock.position_reference = normalizeStoredPositionReference(
    parsed.textBlock?.position_reference
  )
  sanitizeOverlayPositions(settings)
}

function normalizeStoredPositionReference(value: unknown): PositionReference {
  return value === 'content' ? 'content' : 'canvas'
}

function sanitizeOverlayPositions(settings: Settings) {
  settings.watermark.position_reference = normalizePositionReference(
    settings.watermark.position_reference
  )
  settings.textBlock.position_reference = normalizePositionReference(
    settings.textBlock.position_reference
  )
  settings.watermark.position_x_percent = normalizePositionPercent(
    settings.watermark.position_x_percent,
    50
  )
  settings.watermark.position_y_percent = normalizePositionPercent(
    settings.watermark.position_y_percent,
    95
  )
  settings.textBlock.position_x_percent = normalizePositionPercent(
    settings.textBlock.position_x_percent,
    50
  )
  settings.textBlock.position_y_percent = normalizePositionPercent(
    settings.textBlock.position_y_percent,
    92
  )
}

function normalizePositionReference(value: unknown): PositionReference {
  return value === 'canvas' ? 'canvas' : 'content'
}

function normalizePositionPercent(value: unknown, fallback: number): number {
  const normalized = normalizeNumber(value, fallback)
  return Math.round(normalized * 100) / 100
}

function normalizeLayoutSettings(settings: Settings, parsed: Partial<Settings> & LegacyLayoutSettings) {
  const finalSize = normalizeNumber(settings.finalSize, 10000)
  settings.finalSize = Math.min(30000, Math.max(1000, Math.round(finalSize)))

  if (parsed.contentLongEdgePercent === undefined) {
    settings.contentLongEdgePercent = percentFromPx(
      normalizeNumber(parsed.resampleSize, 4000),
      settings.finalSize
    )
  }

  if (parsed.tileBorderPercent === undefined) {
    const resampleSize = normalizeNumber(parsed.resampleSize, 4000)
    const borderSize = normalizeNumber(parsed.borderSize, resampleSize + 200)
    const legacyTileBorderPx =
      parsed.tileBorderPx === undefined
        ? Math.max(0, (borderSize - resampleSize) / 2)
        : normalizeNumber(parsed.tileBorderPx, 100)
    settings.tileBorderPercent = percentFromPx(legacyTileBorderPx, settings.finalSize)
  }

  if (parsed.gapXPercent === undefined) {
    settings.gapXPercent = percentFromPx(normalizeNumber(parsed.gapXPx, 0), settings.finalSize)
  }
  if (parsed.gapYPercent === undefined) {
    settings.gapYPercent = percentFromPx(normalizeNumber(parsed.gapYPx, 0), settings.finalSize)
  }
  if (parsed.imageGapPercent === undefined) {
    settings.imageGapPercent = Math.max(settings.gapXPercent, settings.gapYPercent)
  }
  if (
    parsed.tileBorderPercent === undefined &&
    parsed.tileBorderPx === undefined &&
    settings.tileBorderPercent <= 0
  ) {
    settings.tileBorderPercent = roundPercent(settings.imageGapPercent / 2)
  }
  if (parsed.outerBorderPercent === undefined) {
    settings.outerBorderPercent = percentFromPx(
      normalizeNumber(parsed.outerBorderPx, 1000),
      settings.finalSize
    )
  }

  settings.outerBorderMode = settings.outerBorderMode === 'custom' ? 'custom' : 'auto'
  sanitizeLayoutSettings(settings)
  dropLegacyLayoutSettings(settings as Settings & LegacyLayoutSettings)
}

function normalizeAspectSettings(settings: Settings, parsed: Partial<Settings>) {
  settings.canvasAspectPreset = isCanvasAspectPreset(parsed.canvasAspectPreset)
    ? parsed.canvasAspectPreset
    : 'auto'
  settings.customAspectWidth = clampNumber(settings.customAspectWidth, 0.1, 100, 3)
  settings.customAspectHeight = clampNumber(settings.customAspectHeight, 0.1, 100, 4)
}

function sanitizeLayoutSettings(settings: Settings) {
  settings.contentLongEdgePercent = clampPercent(settings.contentLongEdgePercent, 0.01, 100, 40)
  settings.tileBorderPercent = clampPercent(settings.tileBorderPercent, 0, 50, 1)
  settings.imageGapPercent = 0
  settings.gapXPercent = 0
  settings.gapYPercent = 0
  settings.outerBorderPercent = clampPercent(settings.outerBorderPercent, 0, 49.99, 10)
  settings.finalSize = Math.min(30000, Math.max(1000, Math.round(normalizeNumber(settings.finalSize, 10000))))
  normalizeAspectSettings(settings, settings)
}

function percentFromPx(px: number, finalSize: number): number {
  return roundPercent((Math.max(0, px) / Math.max(1, finalSize)) * 100)
}

function roundPercent(value: number): number {
  return Math.round(value * 100) / 100
}

function clampPercent(value: number, min: number, max: number, fallback: number): number {
  const normalized = normalizeNumber(value, fallback)
  return roundPercent(Math.min(max, Math.max(min, normalized)))
}

function clampNumber(value: number, min: number, max: number, fallback: number): number {
  const normalized = normalizeNumber(value, fallback)
  return Math.round(Math.min(max, Math.max(min, normalized)) * 100) / 100
}

function normalizeNumber(value: unknown, fallback: number): number {
  const numberValue = Number(value)
  return Number.isFinite(numberValue) ? numberValue : fallback
}

function dropLegacyLayoutSettings(settings: Settings & LegacyLayoutSettings) {
  delete settings.resampleSize
  delete settings.borderSize
  delete settings.tileBorderPx
  delete settings.gapXPx
  delete settings.gapYPx
  delete settings.outerBorderPx
}

export const useAppStore = defineStore('app', () => {
  // 设置
  const settings = reactive<Settings>(loadSettings())
  const ui = reactive<UiState>(loadUiState())

  function saveSettings() {
    sanitizeLayoutSettings(settings)
    sanitizeOverlayPositions(settings)
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings))
  }

  function saveUiState() {
    localStorage.setItem(UI_KEY, JSON.stringify(ui))
  }

  function toggleTheme() {
    ui.theme = ui.theme === 'dark' ? 'light' : 'dark'
    saveUiState()
  }

  function setFilmstripCollapsed(collapsed: boolean) {
    ui.filmstripCollapsed = collapsed
    saveUiState()
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
  const renderedPreview = reactive<RenderedPreviewState>(emptyRenderedPreview())

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

  function setRenderedPreview(result: PreviewResult, signature: string, source: RenderedPreviewSource) {
    Object.assign(renderedPreview, {
      ...result,
      signature,
      source,
      rendering: false,
      errorMessage: '',
    })
  }

  function clearRenderedPreview() {
    Object.assign(renderedPreview, emptyRenderedPreview())
  }

  function setRenderedPreviewRendering(rendering: boolean) {
    renderedPreview.rendering = rendering
    if (rendering) {
      renderedPreview.errorMessage = ''
    }
  }

  function setRenderedPreviewError(message: string) {
    renderedPreview.rendering = false
    renderedPreview.errorMessage = message
  }

  return {
    settings,
    ui,
    saveSettings,
    toggleTheme,
    setFilmstripCollapsed,
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
    renderedPreview,
    resetProgress,
    setProgress,
    setElapsed,
    setStageTiming,
    setRenderedPreview,
    clearRenderedPreview,
    setRenderedPreviewRendering,
    setRenderedPreviewError,
  }
})
