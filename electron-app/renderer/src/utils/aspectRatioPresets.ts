import type { TargetAspectRatio } from '../types/protocol'

export type CanvasAspectPreset =
  | 'auto'
  | 'instagram_grid_3_4'
  | 'instagram_content_3_4'
  | 'instagram_portrait_4_5'
  | 'instagram_square_1_1'
  | 'instagram_landscape_1_91_1'
  | 'xiaohongshu_portrait_3_4'
  | 'xiaohongshu_square_1_1'
  | 'xiaohongshu_landscape_4_3'
  | 'custom'

export interface CanvasAspectSettings {
  canvasAspectPreset: CanvasAspectPreset
  customAspectWidth: number
  customAspectHeight: number
}

export interface CanvasAspectOption {
  value: CanvasAspectPreset
  label: string
  shortLabel: string
  ratio: TargetAspectRatio | null
}

export const CANVAS_ASPECT_OPTIONS: CanvasAspectOption[] = [
  { value: 'auto', label: 'Auto', shortLabel: 'Auto', ratio: null },
  {
    value: 'instagram_grid_3_4',
    label: 'Instagram cover/grid 3:4',
    shortLabel: 'IG 3:4',
    ratio: { width: 3, height: 4 },
  },
  {
    value: 'instagram_content_3_4',
    label: 'Instagram content 3:4',
    shortLabel: 'IG content',
    ratio: { width: 3, height: 4 },
  },
  {
    value: 'instagram_portrait_4_5',
    label: 'Instagram portrait 4:5',
    shortLabel: 'IG 4:5',
    ratio: { width: 4, height: 5 },
  },
  {
    value: 'instagram_square_1_1',
    label: 'Instagram square 1:1',
    shortLabel: 'IG 1:1',
    ratio: { width: 1, height: 1 },
  },
  {
    value: 'instagram_landscape_1_91_1',
    label: 'Instagram landscape 1.91:1',
    shortLabel: 'IG 1.91:1',
    ratio: { width: 1.91, height: 1 },
  },
  {
    value: 'xiaohongshu_portrait_3_4',
    label: 'Xiaohongshu portrait 3:4',
    shortLabel: 'XHS 3:4',
    ratio: { width: 3, height: 4 },
  },
  {
    value: 'xiaohongshu_square_1_1',
    label: 'Xiaohongshu square 1:1',
    shortLabel: 'XHS 1:1',
    ratio: { width: 1, height: 1 },
  },
  {
    value: 'xiaohongshu_landscape_4_3',
    label: 'Xiaohongshu landscape 4:3',
    shortLabel: 'XHS 4:3',
    ratio: { width: 4, height: 3 },
  },
  { value: 'custom', label: 'Custom ratio', shortLabel: 'Custom', ratio: null },
]

export function isCanvasAspectPreset(value: unknown): value is CanvasAspectPreset {
  return CANVAS_ASPECT_OPTIONS.some((option) => option.value === value)
}

export function findCanvasAspectOption(value: CanvasAspectPreset): CanvasAspectOption {
  return CANVAS_ASPECT_OPTIONS.find((option) => option.value === value) ?? CANVAS_ASPECT_OPTIONS[0]
}

export function resolveCanvasAspectRatio(settings: CanvasAspectSettings): TargetAspectRatio | null {
  if (settings.canvasAspectPreset === 'custom') {
    return {
      width: settings.customAspectWidth,
      height: settings.customAspectHeight,
    }
  }

  const ratio = findCanvasAspectOption(settings.canvasAspectPreset).ratio
  return ratio ? { ...ratio } : null
}

export function formatAspectRatio(ratio: TargetAspectRatio | null): string {
  if (!ratio) return 'Auto'
  return `${formatRatioNumber(ratio.width)}:${formatRatioNumber(ratio.height)}`
}

function formatRatioNumber(value: number): string {
  if (Number.isInteger(value)) return String(value)
  return String(Math.round(value * 100) / 100)
}
