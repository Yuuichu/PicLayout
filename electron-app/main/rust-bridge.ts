import { spawn, ChildProcess } from 'child_process'
import { join } from 'path'
import { app } from 'electron'
import { existsSync } from 'fs'

export interface CollageConfig {
  image_paths: string[]
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

function getRustCorePath(): string {
  if (app.isPackaged) {
    return join(process.resourcesPath, 'rust-core.exe')
  }
  // 开发模式：优先 release，其次 debug
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
  const paths = [
    join(config.output_dir, `${config.prefix}_collage.jpg`),
    join(config.output_dir, `${config.prefix}_collage_final.jpg`),
  ]
  if (config.watermark) {
    paths.push(join(config.output_dir, `${config.prefix}_collage_final_watermarked.jpg`))
  }
  return paths
}

function existingPaths(paths: string[]): string[] {
  return paths.filter((path) => existsSync(path))
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
      this.cancelled = false
      this.expectedOutputs = getExpectedOutputPaths(config)
      let settled = false

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
            })
          } else if (msg.type === 'error') {
            rejectOnce(new Error(msg.message))
          }
        } catch (e) {
          console.error('解析进度消息失败:', line, e)
        }
      }

      this.process = spawn(exePath, [], {
        stdio: ['pipe', 'pipe', 'pipe'],
      })

      // 写入 JSON 配置到 stdin（一行）
      this.process.stdin!.write(JSON.stringify(config) + '\n')
      this.process.stdin!.end()

      let buffer = ''

      // 逐行读取 stdout NDJSON
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
        const partialOutputs = existingPaths(this.expectedOutputs)
        this.process = null
        this.expectedOutputs = []
        this.cancelled = false
        if (wasCancelled) {
          onProgress({
            type: 'cancelled',
            message: '已取消处理，可能存在半成品文件。',
            partial_outputs: partialOutputs,
          })
          resolveOnce({
            outputs: partialOutputs,
            processed_count: 0,
            failed_images: [],
            warnings: ['任务已取消'],
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
