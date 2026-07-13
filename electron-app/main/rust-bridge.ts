import { type ChildProcessWithoutNullStreams, spawn } from 'node:child_process'
import { app } from 'electron'
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, unlinkSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import type {
  CollageConfig,
  CollageResult,
  PreviewResult,
  ProgressMessage,
} from '../shared/protocol'

type CompletedMessage = Extract<ProgressMessage, { type: 'completed' }>
type PreviewCompletedMessage = Extract<ProgressMessage, { type: 'preview_completed' }>

interface RustProcessResult {
  exitCode: number | null
  wasCancelled: boolean
  stderr: string
  elapsedMs: number
}

export function getRustCoreExecutableName(platform = process.platform): string {
  return platform === 'win32' ? 'rust-core.exe' : 'rust-core'
}

export function getRustCorePath(): string {
  const executableName = getRustCoreExecutableName()
  if (app.isPackaged) {
    return join(process.resourcesPath, executableName)
  }

  const releasePath = join(__dirname, '../../../rust-core/target/release', executableName)
  const debugPath = join(__dirname, '../../../rust-core/target/debug', executableName)
  const preferDebug = process.env.PICLAYOUT_RUST_PROFILE === 'debug'
  const candidates = preferDebug ? [debugPath, releasePath] : [releasePath, debugPath]

  return candidates.find(existsSync) ?? candidates[0]
}

function describeRustCoreBuild(executablePath: string): string {
  const normalized = executablePath.replace(/\\/g, '/')
  if (normalized.includes('/target/release/')) return 'release'
  if (normalized.includes('/target/debug/')) return 'debug'
  if (app.isPackaged) return 'packaged'
  return 'unknown'
}

function getExpectedOutputPaths(config: CollageConfig): string[] {
  const suffix = hasOverlay(config) ? '_collage_final_watermarked.jpg' : '_collage_final.jpg'
  return [join(config.output_dir, `${config.prefix}${suffix}`)]
}

function hasOverlay(config: CollageConfig): boolean {
  return Boolean(config.watermark || config.text_block?.text.trim())
}

function existingPaths(paths: string[]): string[] {
  return paths.filter(existsSync)
}

function cleanupTempOutputs(paths: string[]): void {
  const directories = new Set(paths.map(dirname))
  for (const directory of directories) {
    try {
      for (const entry of readdirSync(directory)) {
        if (entry.startsWith('.piclayout-') && entry.endsWith('.tmp')) {
          unlinkSync(join(directory, entry))
        }
      }
    } catch (error) {
      console.error('清理临时输出失败:', directory, error)
    }
  }
}

function processExitError(operation: string, result: RustProcessResult): Error {
  const stderr = result.stderr.trim()
  const detail = stderr ? `\n${stderr}` : ''
  return new Error(`rust-core ${operation}失败，退出代码: ${result.exitCode}${detail}`)
}

export class RustBridge {
  private process: ChildProcessWithoutNullStreams | null = null
  private cancelled = false

  async start(
    config: CollageConfig,
    onProgress: (message: ProgressMessage) => void
  ): Promise<CollageResult> {
    const expectedOutputs = getExpectedOutputPaths(config)
    const messages: {
      completed: CompletedMessage | null
      reportedError: string | null
    } = { completed: null, reportedError: null }

    const result = await this.runRustCore(config, [], '处理', (message) => {
      onProgress(message)
      if (message.type === 'completed') messages.completed = message
      if (message.type === 'error') messages.reportedError = message.message
    })

    if (result.wasCancelled) {
      cleanupTempOutputs(expectedOutputs)
      const partialOutputs = existingPaths(expectedOutputs)
      onProgress({
        type: 'cancelled',
        message: '已取消处理，临时输出文件已清理。',
        partial_outputs: partialOutputs,
      })
      return {
        outputs: partialOutputs,
        processed_count: 0,
        failed_images: [],
        warnings: ['任务已取消'],
        elapsed_ms: result.elapsedMs,
        wall_elapsed_ms: result.elapsedMs,
        stage_timings: [],
      }
    }

    if (messages.reportedError) throw new Error(messages.reportedError)
    if (result.exitCode !== 0) throw processExitError('处理', result)
    if (!messages.completed) throw new Error('rust-core 已退出，但没有返回 completed 消息')

    return {
      outputs: messages.completed.outputs,
      processed_count: messages.completed.processed_count,
      failed_images: messages.completed.failed_images,
      warnings: messages.completed.warnings,
      elapsed_ms: messages.completed.elapsed_ms,
      wall_elapsed_ms: result.elapsedMs,
      stage_timings: messages.completed.stage_timings,
    }
  }

  async renderPreview(config: CollageConfig, previewLongEdge = 1800): Promise<PreviewResult> {
    const tempDirectory = mkdtempSync(join(tmpdir(), 'piclayout-preview-'))
    const previewPath = join(tempDirectory, 'preview.png')
    const normalizedLongEdge = Math.max(1, Math.round(previewLongEdge))
    const messages: {
      completed: PreviewCompletedMessage | null
      reportedError: string | null
    } = { completed: null, reportedError: null }

    try {
      const result = await this.runRustCore(
        config,
        ['--render-preview', previewPath, String(normalizedLongEdge)],
        '预览渲染',
        (message) => {
          if (message.type === 'preview_completed') messages.completed = message
          if (message.type === 'error') messages.reportedError = message.message
        }
      )

      if (result.wasCancelled) throw new Error('预览渲染已取消')
      if (messages.reportedError) throw new Error(messages.reportedError)
      if (result.exitCode !== 0) throw processExitError('预览渲染', result)
      if (!messages.completed)
        throw new Error('rust-core 已退出，但没有返回 preview_completed 消息')

      const image = readFileSync(messages.completed.output_path)
      return {
        data_url: `data:image/png;base64,${image.toString('base64')}`,
        width: messages.completed.width,
        height: messages.completed.height,
        final_width: messages.completed.final_width,
        final_height: messages.completed.final_height,
        processed_count: messages.completed.processed_count,
        failed_images: messages.completed.failed_images,
        warnings: messages.completed.warnings,
        elapsed_ms: messages.completed.elapsed_ms,
        stage_timings: messages.completed.stage_timings,
      }
    } finally {
      rmSync(tempDirectory, { recursive: true, force: true })
    }
  }

  cancel(): void {
    if (!this.process) return
    this.cancelled = true
    this.process.kill()
  }

  isRunning(): boolean {
    return this.process !== null
  }

  private runRustCore(
    config: CollageConfig,
    args: string[],
    operation: string,
    onMessage: (message: ProgressMessage) => void
  ): Promise<RustProcessResult> {
    if (this.process) {
      throw new Error('已有 Frameverse 任务正在运行')
    }

    const executablePath = getRustCorePath()
    const build = describeRustCoreBuild(executablePath)
    console.info(`[rust-core] ${operation}使用 ${build} sidecar: ${executablePath}`)
    if (build === 'debug') {
      console.warn('[rust-core] debug sidecar 性能较低；日常使用请构建 release 版本')
    }

    const startedAt = Date.now()
    this.cancelled = false

    return new Promise((resolve, reject) => {
      const child = spawn(executablePath, args, {
        stdio: ['pipe', 'pipe', 'pipe'],
      })
      this.process = child

      let stdoutBuffer = ''
      let stderr = ''
      let protocolError: Error | null = null
      let settled = false

      const resetState = () => {
        if (this.process === child) this.process = null
        this.cancelled = false
      }

      const rejectOnce = (error: Error) => {
        if (settled) return
        settled = true
        resetState()
        reject(error)
      }

      const handleLine = (line: string) => {
        if (!line.trim() || protocolError) return
        try {
          onMessage(JSON.parse(line) as ProgressMessage)
        } catch (error) {
          protocolError = new Error(
            `无法解析 rust-core NDJSON: ${error instanceof Error ? error.message : String(error)}`
          )
          child.kill()
        }
      }

      child.stdout.on('data', (data: Buffer) => {
        stdoutBuffer += data.toString()
        const lines = stdoutBuffer.split('\n')
        stdoutBuffer = lines.pop() ?? ''
        for (const line of lines) handleLine(line)
      })

      child.stderr.on('data', (data: Buffer) => {
        const chunk = data.toString()
        stderr += chunk
        console.error('[rust-core stderr]', chunk)
      })

      child.once('error', (error) => {
        rejectOnce(new Error(`启动 rust-core 失败: ${error.message}\n路径: ${executablePath}`))
      })

      child.once('close', (exitCode) => {
        if (settled) return
        handleLine(stdoutBuffer)
        const wasCancelled = this.cancelled
        settled = true
        resetState()

        if (protocolError) {
          reject(protocolError)
          return
        }

        resolve({
          exitCode,
          wasCancelled,
          stderr,
          elapsedMs: Date.now() - startedAt,
        })
      })

      child.stdin.end(`${JSON.stringify(config)}\n`)
    })
  }
}

export const rustBridge = new RustBridge()
