import { contextBridge, ipcRenderer } from 'electron'
import type {
  CollageConfig,
  CollageResult,
  PreviewImageResult,
  PreviewResult,
  ProgressMessage,
} from '../main/rust-bridge'
import type { FontFaceInfo } from '../main/font-metadata'

contextBridge.exposeInMainWorld('electronAPI', {
  openImages: (): Promise<string[]> =>
    ipcRenderer.invoke('dialog:openImages'),

  openWatermark: (): Promise<string | null> =>
    ipcRenderer.invoke('dialog:openWatermark'),

  openIccProfile: (): Promise<string | null> =>
    ipcRenderer.invoke('dialog:openIccProfile'),

  openDirectory: (): Promise<string | null> =>
    ipcRenderer.invoke('dialog:openDirectory'),

  openPath: (path: string): Promise<string> =>
    ipcRenderer.invoke('shell:openPath', path),

  getThumbnail: (path: string): Promise<string | null> =>
    ipcRenderer.invoke('image:thumbnail', path),

  getImageOrientation: (path: string): Promise<number | null> =>
    ipcRenderer.invoke('image:orientation', path),

  getImageSize: (path: string): Promise<{ width: number; height: number } | null> =>
    ipcRenderer.invoke('image:size', path),

  getImagePreviewDataUrl: (path: string, longEdge?: number): Promise<PreviewImageResult | null> =>
    ipcRenderer.invoke('image:previewDataUrl', path, longEdge),

  listFonts: (): Promise<FontFaceInfo[]> =>
    ipcRenderer.invoke('fonts:list'),

  startCollage: (config: CollageConfig): Promise<CollageResult> =>
    ipcRenderer.invoke('collage:start', config),

  renderPreview: (config: CollageConfig, longEdge?: number): Promise<PreviewResult> =>
    ipcRenderer.invoke('preview:render', config, longEdge),

  cancelCollage: (): Promise<void> =>
    ipcRenderer.invoke('collage:cancel'),

  onProgress: (callback: (msg: ProgressMessage) => void): (() => void) => {
    const listener = (_event: Electron.IpcRendererEvent, msg: ProgressMessage) => callback(msg)
    ipcRenderer.on('collage:progress', listener)
    return () => ipcRenderer.removeListener('collage:progress', listener)
  },
})

export type ElectronAPI = {
  openImages: () => Promise<string[]>
  openWatermark: () => Promise<string | null>
  openIccProfile: () => Promise<string | null>
  openDirectory: () => Promise<string | null>
  openPath: (path: string) => Promise<string>
  getThumbnail: (path: string) => Promise<string | null>
  getImageOrientation: (path: string) => Promise<number | null>
  getImageSize: (path: string) => Promise<{ width: number; height: number } | null>
  getImagePreviewDataUrl: (path: string, longEdge?: number) => Promise<PreviewImageResult | null>
  listFonts: () => Promise<FontFaceInfo[]>
  startCollage: (config: CollageConfig) => Promise<CollageResult>
  renderPreview: (config: CollageConfig, longEdge?: number) => Promise<PreviewResult>
  cancelCollage: () => Promise<void>
  onProgress: (callback: (msg: ProgressMessage) => void) => () => void
}

declare global {
  interface Window {
    electronAPI: ElectronAPI
  }
}
