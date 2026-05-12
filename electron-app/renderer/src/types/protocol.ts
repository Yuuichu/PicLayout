export interface CollageConfig {
  image_paths: string[]
  output_dir: string
  prefix: string
  resample_size: number
  border_size: number
  final_size: number
  dpi: number
  background_color: BackgroundColor
  watermark: WatermarkConfig | null
  overwrite?: boolean
  output_settings: OutputSettings
  color_management: ColorManagementConfig
}

export type BackgroundColor =
  | 'white' | 'black' | 'grey' | 'lightgrey' | 'beige' | 'lightblue' | 'lightyellow'

export interface WatermarkConfig {
  path: string
  scale_percent: number
  position_x_percent: number
  position_y_percent: number
}

export interface OutputSettings {
  jpeg_quality: number
  auto_orient: boolean
}

export type TargetProfileMode = 'srgb' | 'custom'
export type RenderingIntent = 'perceptual' | 'relative_colorimetric'

export interface ColorManagementConfig {
  enabled: boolean
  target_profile: TargetProfileMode
  target_profile_path?: string | null
  rendering_intent: RenderingIntent
}

export interface FailedImage {
  path: string
  message: string
}

export interface CollageResult {
  outputs: string[]
  processed_count: number
  failed_images: FailedImage[]
  warnings: string[]
}

export type ProgressMessage =
  | { type: 'image_processed'; index: number; total: number }
  | { type: 'stage_changed'; stage: string; message: string }
  | {
      type: 'completed'
      outputs: string[]
      processed_count: number
      failed_images: FailedImage[]
      warnings: string[]
    }
  | { type: 'cancelled'; message: string; partial_outputs: string[] }
  | { type: 'error'; message: string }

export const BACKGROUND_COLOR_OPTIONS: { value: BackgroundColor; label: string; hex: string }[] = [
  { value: 'white',       label: '白色',   hex: '#ffffff' },
  { value: 'black',       label: '黑色',   hex: '#000000' },
  { value: 'grey',        label: '灰色',   hex: '#808080' },
  { value: 'lightgrey',   label: '浅灰',   hex: '#d3d3d3' },
  { value: 'beige',       label: '米色',   hex: '#f5f5dc' },
  { value: 'lightblue',   label: '浅蓝',   hex: '#add8e6' },
  { value: 'lightyellow', label: '浅黄',   hex: '#ffffe0' },
]
