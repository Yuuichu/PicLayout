export interface CollageConfig {
  image_paths: string[]
  image_rotations: Record<string, ImageRotationDegrees>
  processing_mode: ProcessingMode
  output_dir: string
  prefix: string
  content_long_edge_percent: number
  tile_border_percent: number
  gap_x_percent: number
  gap_y_percent: number
  outer_border_percent?: number | null
  final_size: number
  dpi: number
  background_color: BackgroundColor
  watermark: WatermarkConfig | null
  text_block: TextBlockConfig | null
  overwrite?: boolean
  output_settings: OutputSettings
  color_management: ColorManagementConfig
}

export type ImageRotationDegrees = 0 | 90 | 180 | 270
export type ProcessingMode = 'standard_high_quality' | 'maximum_quality' | 'fast_preview'

export type BackgroundColor =
  | 'white' | 'black' | 'grey' | 'lightgrey' | 'beige' | 'lightblue' | 'lightyellow'

export interface WatermarkConfig {
  path: string
  scale_percent: number
  position_x_percent: number
  position_y_percent: number
}

export type TextFontStyle = 'normal' | 'italic' | 'oblique'
export type TextAlign = 'left' | 'center' | 'right'

export interface TextBlockConfig {
  text: string
  font_family: string
  font_weight: number
  font_style: TextFontStyle
  font_size_px: number
  line_height_px: number
  max_width_percent: number
  align: TextAlign
  text_rgba: [number, number, number, number]
  background_rgba: [number, number, number, number]
  padding_px: number
  position_x_percent: number
  position_y_percent: number
}

export interface FontFaceInfo {
  family: string
  post_script_name: string
  weight: number
  style: TextFontStyle | string
  monospaced: boolean
}

export interface OutputSettings {
  jpeg_quality: number
  auto_orient: boolean
  linear_light_resize: boolean
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

export interface StageTimingDetail {
  name: string
  elapsed_ms: number
}

export interface StageTiming {
  stage: string
  elapsed_ms: number
  details?: StageTimingDetail[]
}

export interface CollageResult {
  outputs: string[]
  processed_count: number
  failed_images: FailedImage[]
  warnings: string[]
  elapsed_ms: number
  wall_elapsed_ms: number
  stage_timings: StageTiming[]
}

export interface PreviewImageResult {
  data_url: string
  width: number
  height: number
  final_width: number
  final_height: number
}

export interface PreviewResult extends PreviewImageResult {
  processed_count: number
  failed_images: FailedImage[]
  warnings: string[]
  elapsed_ms: number
  stage_timings: StageTiming[]
}

export type ProgressMessage =
  | { type: 'job_started'; total: number }
  | { type: 'image_processed'; index: number; total: number; elapsed_ms: number }
  | { type: 'stage_changed'; stage: string; message: string; elapsed_ms: number }
  | {
      type: 'stage_finished'
      stage: string
      elapsed_ms: number
      total_elapsed_ms: number
      details?: StageTimingDetail[]
    }
  | {
      type: 'completed'
      outputs: string[]
      processed_count: number
      failed_images: FailedImage[]
      warnings: string[]
      elapsed_ms: number
      stage_timings: StageTiming[]
    }
  | {
      type: 'preview_completed'
      output_path: string
      width: number
      height: number
      final_width: number
      final_height: number
      processed_count: number
      failed_images: FailedImage[]
      warnings: string[]
      elapsed_ms: number
      stage_timings: StageTiming[]
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
