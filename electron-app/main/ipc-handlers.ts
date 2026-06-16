import { BrowserWindow, dialog, ipcMain, nativeImage, shell } from 'electron'
import { listSystemFonts } from './font-metadata'
import { readExifOrientation, sizeWithExifOrientation } from './image-metadata'
import { rustBridge, CollageConfig, ProgressMessage } from './rust-bridge'

export function registerIpcHandlers(win: BrowserWindow): void {
  ipcMain.handle('dialog:openImages', async () => {
    const result = await dialog.showOpenDialog(win, {
      title: '选择图片',
      filters: [{ name: '图片文件', extensions: ['jpg', 'jpeg', 'png', 'bmp', 'tiff'] }],
      properties: ['openFile', 'multiSelections'],
    })
    return result.canceled ? [] : result.filePaths
  })

  ipcMain.handle('dialog:openWatermark', async () => {
    const result = await dialog.showOpenDialog(win, {
      title: '选择水印图片',
      filters: [{ name: '图片文件', extensions: ['png', 'jpg', 'jpeg', 'bmp'] }],
      properties: ['openFile'],
    })
    return result.canceled ? null : result.filePaths[0]
  })

  ipcMain.handle('dialog:openIccProfile', async () => {
    const result = await dialog.showOpenDialog(win, {
      title: '选择 ICC profile',
      filters: [{ name: 'ICC Profile', extensions: ['icc', 'icm'] }],
      properties: ['openFile'],
    })
    return result.canceled ? null : result.filePaths[0]
  })

  ipcMain.handle('dialog:openDirectory', async () => {
    const result = await dialog.showOpenDialog(win, {
      title: '选择导出目录',
      properties: ['openDirectory', 'createDirectory'],
    })
    return result.canceled ? null : result.filePaths[0]
  })

  ipcMain.handle('shell:openPath', async (_event, path: string) => {
    return shell.openPath(path)
  })

  ipcMain.handle('image:thumbnail', async (_event, path: string) => {
    const img = nativeImage.createFromPath(path)
    if (img.isEmpty()) return null

    const { width, height } = img.getSize()
    if (width <= 0 || height <= 0) return null

    const resizeOptions =
      width >= height
        ? { width: 240, quality: 'good' as const }
        : { height: 240, quality: 'good' as const }

    return img.resize(resizeOptions).toDataURL()
  })

  ipcMain.handle('image:orientation', async (_event, path: string) => {
    return readExifOrientation(path)
  })

  ipcMain.handle('image:size', async (_event, path: string) => {
    const img = nativeImage.createFromPath(path)
    if (img.isEmpty()) return null
    return sizeWithExifOrientation(img.getSize(), readExifOrientation(path))
  })

  ipcMain.handle('fonts:list', async () => {
    return listSystemFonts()
  })

  ipcMain.handle('collage:start', async (_event, config: CollageConfig) => {
    if (rustBridge.isRunning()) {
      throw new Error('已有任务正在处理中')
    }

    const onProgress = (msg: ProgressMessage) => {
      win.webContents.send('collage:progress', msg)
    }

    return rustBridge.start(config, onProgress)
  })

  ipcMain.handle('collage:cancel', () => {
    rustBridge.cancel()
  })
}
