import type { Settings } from '../stores/appStore'
import type { CollageConfig, ImageRotationDegrees } from '../types/protocol'

export function buildCollageConfig(
  settings: Settings,
  selectedFiles: string[],
  imageRotations: Record<string, ImageRotationDegrees>
): CollageConfig {
  return {
    image_paths: selectedFiles,
    image_rotations: imageRotations,
    processing_mode: settings.processingMode,
    output_dir: settings.outputDir,
    prefix: settings.prefix || 'output',
    content_long_edge_percent: settings.contentLongEdgePercent,
    tile_border_percent: settings.tileBorderPercent,
    gap_x_percent: settings.gapXPercent,
    gap_y_percent: settings.gapYPercent,
    outer_border_percent:
      settings.outerBorderMode === 'custom' ? settings.outerBorderPercent : null,
    final_size: settings.finalSize,
    dpi: settings.dpi,
    background_color: settings.backgroundColor,
    overwrite: false,
    output_settings: {
      jpeg_quality: settings.jpegQuality,
      auto_orient: settings.autoOrient,
      linear_light_resize: settings.linearLightResize,
    },
    color_management: {
      enabled: settings.colorManagementEnabled,
      target_profile: settings.targetProfileMode,
      target_profile_path:
        settings.targetProfileMode === 'custom' && settings.targetProfilePath
          ? settings.targetProfilePath
          : null,
      rendering_intent: settings.renderingIntent,
    },
    watermark:
      settings.watermarkEnabled && settings.watermark.path
        ? { ...settings.watermark }
        : null,
    text_block:
      settings.textBlockEnabled && settings.textBlock.text.trim()
        ? { ...settings.textBlock }
        : null,
  }
}

export function cloneCollageConfig(config: CollageConfig): CollageConfig {
  return JSON.parse(JSON.stringify(config)) as CollageConfig
}

export function createCollageConfigSignature(config: CollageConfig): string {
  return JSON.stringify(cloneCollageConfig(config))
}
