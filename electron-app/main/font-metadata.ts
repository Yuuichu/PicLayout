import { spawn } from 'child_process'
import { getRustCorePath } from './rust-bridge'

export interface FontFaceInfo {
  family: string
  post_script_name: string
  weight: number
  style: 'normal' | 'italic' | 'oblique' | string
  monospaced: boolean
}

let fontCache: FontFaceInfo[] | null = null
let pendingFontLoad: Promise<FontFaceInfo[]> | null = null

export function listSystemFonts(): Promise<FontFaceInfo[]> {
  if (fontCache) return Promise.resolve(fontCache)
  if (pendingFontLoad) return pendingFontLoad

  pendingFontLoad = new Promise((resolve, reject) => {
    const child = spawn(getRustCorePath(), ['--list-fonts'], {
      stdio: ['ignore', 'pipe', 'pipe'],
    })

    let stdout = ''
    let stderr = ''
    child.stdout.on('data', (data: Buffer) => {
      stdout += data.toString()
    })
    child.stderr.on('data', (data: Buffer) => {
      stderr += data.toString()
    })
    child.on('error', (err) => {
      pendingFontLoad = null
      reject(new Error(`failed to start rust-core font scanner: ${err.message}`))
    })
    child.on('close', (code) => {
      pendingFontLoad = null
      if (code !== 0) {
        reject(new Error(`font scanner exited with code ${code}: ${stderr.trim()}`))
        return
      }
      try {
        fontCache = JSON.parse(stdout) as FontFaceInfo[]
        resolve(fontCache)
      } catch (err) {
        reject(new Error(`failed to parse font list: ${err instanceof Error ? err.message : String(err)}`))
      }
    })
  })

  return pendingFontLoad
}
