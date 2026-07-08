import { spawn, ChildProcess } from 'child_process'
import { dirname, join } from 'path'
import { app } from 'electron'
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, unlinkSync } from 'fs'
import { tmpdir } from 'os'

export interface CollageConfig {
  image_paths: string[]
  image_rotations?: Record<string, 0 | 90 | 180 | 270>
  processing_mode?: 'standard_high_quality' | 'maximum_quality' | 'fast_preview'
  output_dir: string
  prefix: string
  content_long_edge_percent?: number
  tile_border_percent?: number
  gap_x_percent?: number
  gap_y_percent?: number
  outer_border_percent?: number | null
  resample_size?: number
  border_size?: number
  tile_border_px?: number | null
  gap_x_px?: number
  gap_y_px?: number
  outer_border_px?: number | null
  final_size?: number
  target_aspect_ratio?: {
    width: number
    height: number
  } | null
  dpi?: number
  background_color?: string
  overwrite?: boolean
  output_settings?: {
    jpeg_quality?: number
    auto_orient?: boolean
    linear_light_resize?: boolean
  }
  color_management?: {
    enabled?: boolean
    target_profile?: 'srgb' | 'custom'
    target_profile_path?: string | null
    rendering_intent?: 'perceptual' | 'relative_colorimetric'
  }
  watermark?: {
    path: string
    scale_percent?: number
    position_reference?: PositionReference
    position_x_percent?: number
    position_y_percent?: number
  } | null
  text_block?: TextBlockConfig | null
}

export type TextFontStyle = 'normal' | 'italic' | 'oblique'
export type TextAlign = 'left' | 'center' | 'right'
export type PositionReference = 'canvas' | 'content'

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
  position_reference?: PositionReference
  position_x_percent: number
  position_y_percent: number
}

export interface FailedImage {
  path: string
  message: string
}

export interface StageTiming {
  stage: string
  elapsed_ms: number
  details?: StageTimingDetail[]
}

export interface StageTimingDetail {
  name: string
  elapsed_ms: number
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

function getRustCoreExecutableName(): string {
  return process.platform === 'win32' ? 'rust-core.exe' : 'rust-core'
}

export function getRustCorePath(): string {
  const executableName = getRustCoreExecutableName()

  if (app.isPackaged) {
    return join(process.resourcesPath, executableName)
  }

  const releaseExe = join(__dirname, '../../../rust-core/target/release', executableName)
  const debugExe = join(__dirname, '../../../rust-core/target/debug', executableName)

  if (process.env.PICLAYOUT_RUST_PROFILE === 'debug' && existsSync(debugExe)) {
    return debugExe
  }

  if (existsSync(releaseExe)) return releaseExe
  return debugExe
}

function describeRustCoreBuild(exePath: string): string {
  const normalized = exePath.replace(/\\/g, '/')
  if (normalized.includes('/target/release/')) return 'release'
  if (normalized.includes('/target/debug/')) return 'debug'
  if (app.isPackaged) return 'packaged'
  return 'unknown'
}

function getExpectedOutputPaths(config: CollageConfig): string[] {
  if (hasOverlay(config)) {
    return [join(config.output_dir, `${config.prefix}_collage_final_watermarked.jpg`)]
  }
  return [join(config.output_dir, `${config.prefix}_collage_final.jpg`)]
}

function hasOverlay(config: CollageConfig): boolean {
  return !!config.watermark || !!config.text_block?.text?.trim()
}

function existingPaths(paths: string[]): string[] {
  return paths.filter((path) => existsSync(path))
}

function cleanupTempOutputs(paths: string[]): void {
  const dirs = new Set(paths.map((path) => dirname(path)))
  for (const dir of dirs) {
    try {
      for (const entry of readdirSync(dir)) {
        if (entry.startsWith('.piclayout-') && entry.endsWith('.tmp')) {
          unlinkSync(join(dir, entry))
        }
      }
    } catch (err) {
      console.error('cleanup temp outputs failed:', dir, err)
    }
  }
}

export class RustBridge {
  private process: ChildProcess | null = null
  private cancelled = false
  private expectedOutputs: string[] = []

  async start(
    config: CollageConfig,
    onProgress: (msg: ProgressMessage) => void
  ): Promise<CollageResult> {
    return new Promise((resolve, reject) => {
      const exePath = getRustCorePath()
      const rustBuild = describeRustCoreBuild(exePath)
      console.info(`[rust-core] using ${rustBuild} sidecar: ${exePath}`)
      if (rustBuild === 'debug') {
        console.warn(
          '[rust-core] debug sidecar is much slower; build release or set PICLAYOUT_RUST_PROFILE=debug only for Rust debugging'
        )
      }
      const startedAt = Date.now()
      this.cancelled = false
      this.expectedOutputs = getExpectedOutputPaths(config)
      let settled = false

      const wallElapsed = () => Date.now() - startedAt

      const resolveOnce = (result: CollageResult) => {
        if (settled) return
        settled = true
        resolve(result)
      }

      const rejectOnce = (error: Error) => {
        if (settled) return
        settled = true
        reject(error)
      }

      const handleLine = (line: string) => {
        if (!line.trim()) return
        try {
          const msg: ProgressMessage = JSON.parse(line)
          onProgress(msg)
          if (msg.type === 'completed') {
            resolveOnce({
              outputs: msg.outputs,
              processed_count: msg.processed_count,
              failed_images: msg.failed_images,
              warnings: msg.warnings,
              elapsed_ms: msg.elapsed_ms,
              wall_elapsed_ms: wallElapsed(),
              stage_timings: msg.stage_timings,
            })
          } else if (msg.type === 'error') {
            rejectOnce(new Error(msg.message))
          }
        } catch (e) {
          console.error('failed to parse rust-core progress message:', line, e)
        }
      }

      this.process = spawn(exePath, [], {
        stdio: ['pipe', 'pipe', 'pipe'],
      })

      this.process.stdin!.write(JSON.stringify(config) + '\n')
      this.process.stdin!.end()

      let buffer = ''

      this.process.stdout!.on('data', (data: Buffer) => {
        buffer += data.toString()
        const lines = buffer.split('\n')
        buffer = lines.pop() ?? ''

        for (const line of lines) {
          handleLine(line)
        }
      })

      this.process.stderr!.on('data', (data: Buffer) => {
        console.error('[rust-core stderr]', data.toString())
      })

      this.process.on('error', (err) => {
        this.process = null
        this.expectedOutputs = []
        this.cancelled = false
        rejectOnce(new Error(`启动 rust-core 失败: ${err.message}\n路径: ${exePath}`))
      })

      this.process.on('close', (code) => {
        handleLine(buffer)
        buffer = ''

        const wasCancelled = this.cancelled
        if (wasCancelled) {
          cleanupTempOutputs(this.expectedOutputs)
        }
        const partialOutputs = existingPaths(this.expectedOutputs)
        this.process = null
        this.expectedOutputs = []
        this.cancelled = false

        if (wasCancelled) {
          onProgress({
            type: 'cancelled',
            message: '已取消处理，临时输出文件已清理。',
            partial_outputs: partialOutputs,
          })
          resolveOnce({
            outputs: partialOutputs,
            processed_count: 0,
            failed_images: [],
            warnings: ['任务已取消'],
            elapsed_ms: wallElapsed(),
            wall_elapsed_ms: wallElapsed(),
            stage_timings: [],
          })
          return
        }
        if (code !== 0 && code !== null) {
          rejectOnce(new Error(`rust-core 以非零代码退出: ${code}`))
          return
        }
        if (!settled) {
          rejectOnce(new Error('rust-core 已退出，但没有返回完成消息'))
        }
      })
    })
  }

  async renderPreview(config: CollageConfig, previewLongEdge = 1800): Promise<PreviewResult> {
    if (this.process) {
      throw new Error('Another Frameverse task is already running')
    }

    const tempDir = mkdtempSync(join(tmpdir(), 'piclayout-preview-'))
    const previewPath = join(tempDir, 'preview.png')
    const normalizedLongEdge = Math.max(1, Math.round(previewLongEdge))

    try {
      return await new Promise((resolve, reject) => {
        const exePath = getRustCorePath()
        const rustBuild = describeRustCoreBuild(exePath)
        console.info(`[rust-core] using ${rustBuild} sidecar for preview: ${exePath}`)
        const startedAt = Date.now()
        this.cancelled = false
        this.expectedOutputs = []
        let settled = false

        const wallElapsed = () => Date.now() - startedAt

        const resolveOnce = (result: PreviewResult) => {
          if (settled) return
          settled = true
          resolve(result)
        }

        const rejectOnce = (error: Error) => {
          if (settled) return
          settled = true
          reject(error)
        }

        const handleLine = (line: string) => {
          if (!line.trim()) return
          try {
            const msg: ProgressMessage = JSON.parse(line)
            if (msg.type === 'preview_completed') {
              const data = readFileSync(msg.output_path)
              resolveOnce({
                data_url: `data:image/png;base64,${data.toString('base64')}`,
                width: msg.width,
                height: msg.height,
                final_width: msg.final_width,
                final_height: msg.final_height,
                processed_count: msg.processed_count,
                failed_images: msg.failed_images,
                warnings: msg.warnings,
                elapsed_ms: msg.elapsed_ms,
                stage_timings: msg.stage_timings,
              })
            } else if (msg.type === 'error') {
              rejectOnce(new Error(msg.message))
            }
          } catch (e) {
            console.error('failed to parse rust-core preview message:', line, e)
            rejectOnce(e instanceof Error ? e : new Error(String(e)))
          }
        }

        this.process = spawn(exePath, [
          '--render-preview',
          previewPath,
          String(normalizedLongEdge),
        ], {
          stdio: ['pipe', 'pipe', 'pipe'],
        })

        this.process.stdin!.write(JSON.stringify(config) + '\n')
        this.process.stdin!.end()

        let buffer = ''

        this.process.stdout!.on('data', (data: Buffer) => {
          buffer += data.toString()
          const lines = buffer.split('\n')
          buffer = lines.pop() ?? ''

          for (const line of lines) {
            handleLine(line)
          }
        })

        this.process.stderr!.on('data', (data: Buffer) => {
          console.error('[rust-core stderr]', data.toString())
        })

        this.process.on('error', (err) => {
          this.process = null
          this.expectedOutputs = []
          this.cancelled = false
          rejectOnce(new Error(`Failed to start rust-core: ${err.message}\nPath: ${exePath}`))
        })

        this.process.on('close', (code) => {
          handleLine(buffer)
          buffer = ''

          const wasCancelled = this.cancelled
          this.process = null
          this.expectedOutputs = []
          this.cancelled = false

          if (wasCancelled) {
            rejectOnce(new Error('Preview render was cancelled'))
            return
          }
          if (code !== 0 && code !== null) {
            rejectOnce(new Error(`rust-core exited with non-zero code ${code}`))
            return
          }
          if (!settled) {
            rejectOnce(new Error(`rust-core exited without preview_completed (${wallElapsed()}ms)`))
          }
        })
      })
    } finally {
      rmSync(tempDir, { recursive: true, force: true })
    }
  }

  cancel(): void {
    if (this.process) {
      this.cancelled = true
      this.process.kill()
    }
  }

  isRunning(): boolean {
    return this.process !== null
  }
}

export const rustBridge = new RustBridge()
