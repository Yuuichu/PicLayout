import { defineStore } from 'pinia'
import { ref, reactive } from 'vue'
import type { BackgroundColor, WatermarkConfig } from '../types/protocol'

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
    maxImages: 100,
    resampleSize: 4000,
    borderSize: 4200,
    finalSize: 10000,
    dpi: 300,
    backgroundColor: 'white',
    prefix: 'output',
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

  // 处理状态
  const processing = ref(false)
  const progress = ref(0)        // 0–100
  const statusMessage = ref('')
  const outputFiles = ref<string[]>([])
  const errorMessage = ref('')

  function resetProgress() {
    progress.value = 0
    statusMessage.value = ''
    outputFiles.value = []
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
    processing,
    progress,
    statusMessage,
    outputFiles,
    errorMessage,
    resetProgress,
    setProgress,
  }
})
