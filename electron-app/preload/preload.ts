import { contextBridge, ipcRenderer } from 'electron'
import type { CollageConfig, ProgressMessage } from '../main/rust-bridge'

// 暴露给渲染进程的安全 API
contextBridge.exposeInMainWorld('electronAPI', {
  // 文件对话框
  openImages: (): Promise<string[]> =>
    ipcRenderer.invoke('dialog:openImages'),

  openWatermark: (): Promise<string | null> =>
    ipcRenderer.invoke('dialog:openWatermark'),

  openDirectory: (): Promise<string | null> =>
    ipcRenderer.invoke('dialog:openDirectory'),

  // 拼贴处理
  startCollage: (config: CollageConfig): Promise<string[]> =>
    ipcRenderer.invoke('collage:start', config),

  cancelCollage: (): Promise<void> =>
    ipcRenderer.invoke('collage:cancel'),

  // 监听进度（返回解除监听函数）
  onProgress: (callback: (msg: ProgressMessage) => void): (() => void) => {
    const listener = (_event: Electron.IpcRendererEvent, msg: ProgressMessage) => callback(msg)
    ipcRenderer.on('collage:progress', listener)
    return () => ipcRenderer.removeListener('collage:progress', listener)
  },
})

// TypeScript 类型声明（供渲染进程使用）
export type ElectronAPI = {
  openImages: () => Promise<string[]>
  openWatermark: () => Promise<string | null>
  openDirectory: () => Promise<string | null>
  startCollage: (config: CollageConfig) => Promise<string[]>
  cancelCollage: () => Promise<void>
  onProgress: (callback: (msg: ProgressMessage) => void) => () => void
}

declare global {
  interface Window {
    electronAPI: ElectronAPI
  }
}
