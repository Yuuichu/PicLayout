import { spawn, ChildProcess } from 'child_process'
import { join } from 'path'
import { app } from 'electron'

export interface CollageConfig {
  image_paths: string[]
  output_dir: string
  prefix: string
  resample_size?: number
  border_size?: number
  final_size?: number
  dpi?: number
  background_color?: string
  watermark?: {
    path: string
    scale_percent?: number
    position_x_percent?: number
    position_y_percent?: number
  } | null
}

export type ProgressMessage =
  | { type: 'image_processed'; index: number; total: number }
  | { type: 'stage_changed'; stage: string; message: string }
  | { type: 'completed'; outputs: string[] }
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

export class RustBridge {
  private process: ChildProcess | null = null

  async start(
    config: CollageConfig,
    onProgress: (msg: ProgressMessage) => void
  ): Promise<string[]> {
    return new Promise((resolve, reject) => {
      const exePath = getRustCorePath()

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
          if (!line.trim()) continue
          try {
            const msg: ProgressMessage = JSON.parse(line)
            onProgress(msg)
            if (msg.type === 'completed') {
              resolve(msg.outputs)
            } else if (msg.type === 'error') {
              reject(new Error(msg.message))
            }
          } catch (e) {
            console.error('解析进度消息失败:', line, e)
          }
        }
      })

      this.process.stderr!.on('data', (data: Buffer) => {
        console.error('[rust-core stderr]', data.toString())
      })

      this.process.on('error', (err) => {
        reject(new Error(`启动 rust-core 失败: ${err.message}\n路径: ${exePath}`))
      })

      this.process.on('close', (code) => {
        this.process = null
        if (code !== 0 && code !== null) {
          reject(new Error(`rust-core 以非零代码退出: ${code}`))
        }
      })
    })
  }

  cancel(): void {
    if (this.process) {
      this.process.kill()
      this.process = null
    }
  }

  isRunning(): boolean {
    return this.process !== null
  }
}

export const rustBridge = new RustBridge()
