import { BrowserWindow, dialog, ipcMain } from 'electron'
import { rustBridge, CollageConfig, ProgressMessage } from './rust-bridge'

export function registerIpcHandlers(win: BrowserWindow): void {
  // 选择图片文件
  ipcMain.handle('dialog:openImages', async () => {
    const result = await dialog.showOpenDialog(win, {
      title: '选择图片',
      filters: [{ name: '图片文件', extensions: ['jpg', 'jpeg', 'png', 'bmp', 'tiff'] }],
      properties: ['openFile', 'multiSelections'],
    })
    return result.canceled ? [] : result.filePaths
  })

  // 选择水印图片
  ipcMain.handle('dialog:openWatermark', async () => {
    const result = await dialog.showOpenDialog(win, {
      title: '选择水印图片',
      filters: [{ name: '图片文件', extensions: ['png', 'jpg', 'jpeg', 'bmp'] }],
      properties: ['openFile'],
    })
    return result.canceled ? null : result.filePaths[0]
  })

  // 选择导出目录
  ipcMain.handle('dialog:openDirectory', async () => {
    const result = await dialog.showOpenDialog(win, {
      title: '选择导出目录',
      properties: ['openDirectory', 'createDirectory'],
    })
    return result.canceled ? null : result.filePaths[0]
  })

  // 启动拼贴处理
  ipcMain.handle('collage:start', async (_event, config: CollageConfig) => {
    if (rustBridge.isRunning()) {
      throw new Error('已有任务正在处理中')
    }

    const onProgress = (msg: ProgressMessage) => {
      win.webContents.send('collage:progress', msg)
    }

    return rustBridge.start(config, onProgress)
  })

  // 取消处理
  ipcMain.handle('collage:cancel', () => {
    rustBridge.cancel()
  })
}
