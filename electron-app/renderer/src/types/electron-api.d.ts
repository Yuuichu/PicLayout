import type { CollageConfig, CollageResult, ProgressMessage } from './protocol'

export type ElectronAPI = {
  openImages: () => Promise<string[]>
  openWatermark: () => Promise<string | null>
  openIccProfile: () => Promise<string | null>
  openDirectory: () => Promise<string | null>
  openPath: (path: string) => Promise<string>
  getThumbnail: (path: string) => Promise<string | null>
  getImageOrientation: (path: string) => Promise<number | null>
  getImageSize: (path: string) => Promise<{ width: number; height: number } | null>
  startCollage: (config: CollageConfig) => Promise<CollageResult>
  cancelCollage: () => Promise<void>
  onProgress: (callback: (msg: ProgressMessage) => void) => () => void
}

declare global {
  interface Window {
    electronAPI: ElectronAPI
  }
}

export {}
