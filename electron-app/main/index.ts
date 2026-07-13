import { app, BrowserWindow, screen } from 'electron'
import { join } from 'node:path'
import { registerIpcHandlers } from './ipc-handlers'

const DEFAULT_WINDOW_WIDTH = 1280
const DEFAULT_WINDOW_HEIGHT = 900
const MIN_WINDOW_WIDTH = 960
const MIN_WINDOW_HEIGHT = 720

function getInitialWindowSize(): { width: number; height: number } {
  const workArea = screen.getPrimaryDisplay().workAreaSize
  return {
    width: Math.min(
      DEFAULT_WINDOW_WIDTH,
      Math.max(MIN_WINDOW_WIDTH, Math.floor(workArea.width * 0.9))
    ),
    height: Math.min(
      DEFAULT_WINDOW_HEIGHT,
      Math.max(MIN_WINDOW_HEIGHT, Math.floor(workArea.height * 0.9))
    ),
  }
}

function createWindow(): void {
  const initialSize = getInitialWindowSize()
  const iconPath = getApplicationIconPath()
  const win = new BrowserWindow({
    width: initialSize.width,
    height: initialSize.height,
    minWidth: MIN_WINDOW_WIDTH,
    minHeight: MIN_WINDOW_HEIGHT,
    title: 'Frameverse',
    icon: iconPath,
    webPreferences: {
      preload: join(__dirname, '../preload/preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
    autoHideMenuBar: true,
  })

  registerIpcHandlers(win)

  if (process.env['ELECTRON_RENDERER_URL']) {
    win.loadURL(process.env['ELECTRON_RENDERER_URL'])
  } else {
    win.loadFile(join(__dirname, '../renderer/index.html'))
  }
}

function getApplicationIconPath(): string {
  return app.isPackaged
    ? join(process.resourcesPath, 'icon.png')
    : join(__dirname, '../../build/icon.png')
}

app.whenReady().then(() => {
  if (process.platform === 'darwin') {
    app.dock?.setIcon(getApplicationIconPath())
  }

  createWindow()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow()
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})
