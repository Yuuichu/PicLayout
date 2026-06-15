import { spawn, ChildProcess } from 'child_process'
import { dirname, join } from 'path'
import { app } from 'electron'
import { existsSync, readdirSync, unlinkSync } from 'fs'

export interface CollageConfig {
  image_paths: string[]
  image_rotations?: Record<string, 0 | 90 | 180 | 270>
  processing_mode?: 'standard_high_quality' | 'maximum_quality' | 'fast_preview'
  output_dir: string
  prefix: string
  resample_size?: number
  border_size?: number
  final_size?: number
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
    position_x_percent?: number
    position_y_percent?: number
  } | null
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
  | { type: 'cancelled'; message: string; partial_outputs: string[] }
  | { type: 'error'; message: string }

function getRustCorePath(): string {
  if (app.isPackaged) {
    return join(process.resourcesPath, 'rust-core.exe')
  }

  const releaseExe = join(__dirname, '../../../rust-core/target/release/rust-core.exe')
  const debugExe = join(__dirname, '../../../rust-core/target/debug/rust-core.exe')
  try {
    require('fs').accessSync(releaseExe)
    return releaseExe
  } catch {
    return debugExe
  }
}

function getExpectedOutputPaths(config: CollageConfig): string[] {
  if (config.watermark) {
    return [join(config.output_dir, `${config.prefix}_collage_final_watermarked.jpg`)]
  }
  return [join(config.output_dir, `${config.prefix}_collage_final.jpg`)]
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
