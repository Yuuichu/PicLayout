import { execFile } from 'node:child_process'
import { getRustCorePath } from './rust-bridge'
import type { FontFaceInfo } from '../shared/protocol'

let fontCache: FontFaceInfo[] | null = null
let pendingFontLoad: Promise<FontFaceInfo[]> | null = null

export function listSystemFonts(): Promise<FontFaceInfo[]> {
  if (fontCache) return Promise.resolve(fontCache)
  if (pendingFontLoad) return pendingFontLoad

  pendingFontLoad = new Promise((resolve, reject) => {
    execFile(
      getRustCorePath(),
      ['--list-fonts'],
      { maxBuffer: 16 * 1024 * 1024 },
      (error, stdout) => {
        pendingFontLoad = null
        if (error) {
          reject(new Error(`系统字体扫描失败: ${error.message}`))
          return
        }
        try {
          fontCache = JSON.parse(stdout) as FontFaceInfo[]
          resolve(fontCache)
        } catch (parseError) {
          reject(
            new Error(
              `无法解析系统字体列表: ${parseError instanceof Error ? parseError.message : String(parseError)}`
            )
          )
        }
      }
    )
  })

  return pendingFontLoad
}
