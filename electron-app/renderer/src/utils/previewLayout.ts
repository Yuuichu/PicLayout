import type { TargetAspectRatio } from '@shared/protocol'

export interface PreviewLayoutInput {
  imageCount: number
  finalSize: number
  targetAspectRatio?: TargetAspectRatio | null
  contentLongEdgePercent: number
  tileBorderPercent: number
  gapXPercent: number
  gapYPercent: number
  outerBorderMode: 'auto' | 'custom'
  outerBorderPercent: number
}

export interface PreviewGeometry {
  cols: number
  rows: number
  border: number
  contentLongEdge: number
  tileBorder: number
  tileSize: number
  gapX: number
  gapY: number
  gridWidth: number
  gridHeight: number
  scaledWidth: number
  scaledHeight: number
  contentX: number
  contentY: number
  canvasWidth: number
  canvasHeight: number
  scale: number
}

export interface PreviewTilePlacement {
  x: number
  y: number
  width: number
  height: number
}

export interface ImageSizeLike {
  width: number
  height: number
}

export function computePreviewLayout(input: PreviewLayoutInput): PreviewGeometry {
  const count = Math.max(1, Math.round(input.imageCount))
  const { cols, rows } = gridShape(count)
  const finalSize = Math.max(1, Math.round(normalizeNumber(input.finalSize, 1)))
  const contentLongEdge = Math.max(1, percentToPx(input.contentLongEdgePercent, finalSize))
  const tileBorder = Math.max(0, percentToPx(input.tileBorderPercent, finalSize))
  const tileSize = Math.max(1, contentLongEdge + tileBorder * 2)
  const gapX = Math.max(0, percentToPx(input.gapXPercent, finalSize))
  const gapY = Math.max(0, percentToPx(input.gapYPercent, finalSize))
  const gridWidth = spacedExtent(cols, tileSize, gapX)
  const gridHeight = spacedExtent(rows, tileSize, gapY)
  const border =
    input.outerBorderMode === 'custom'
      ? Math.max(0, percentToPx(input.outerBorderPercent, finalSize))
      : percentToPx(calculateDynamicBorderPercent(cols), finalSize)
  const targetCanvas = resolveTargetCanvas(finalSize, input.targetAspectRatio)
  const doubleBorder = border * 2
  const availableWidth = Math.max(1, (targetCanvas?.width ?? finalSize) - doubleBorder)
  const availableHeight = Math.max(1, (targetCanvas?.height ?? finalSize) - doubleBorder)
  const scale = targetCanvas
    ? Math.min(availableWidth / Math.max(1, gridWidth), availableHeight / Math.max(1, gridHeight))
    : Math.min(availableWidth, availableHeight) / Math.max(gridWidth, gridHeight)
  const scaledWidth = Math.min(availableWidth, Math.max(1, Math.round(gridWidth * scale)))
  const scaledHeight = Math.min(availableHeight, Math.max(1, Math.round(gridHeight * scale)))
  const canvasWidth = targetCanvas?.width ?? scaledWidth + doubleBorder
  const canvasHeight = targetCanvas?.height ?? scaledHeight + doubleBorder
  const contentX = targetCanvas
    ? border + Math.max(0, Math.round((availableWidth - scaledWidth) / 2))
    : border
  const contentY = targetCanvas
    ? border + Math.max(0, Math.round((availableHeight - scaledHeight) / 2))
    : border

  return {
    cols,
    rows,
    border,
    contentLongEdge,
    tileBorder,
    tileSize,
    gapX,
    gapY,
    gridWidth,
    gridHeight,
    scaledWidth,
    scaledHeight,
    contentX,
    contentY,
    canvasWidth,
    canvasHeight,
    scale,
  }
}

export function computeTilePlacement(
  geometry: PreviewGeometry,
  index: number,
  imageSize: ImageSizeLike | null | undefined
): PreviewTilePlacement {
  const col = index % geometry.cols
  const row = Math.floor(index / geometry.cols)
  const tileX = col * (geometry.tileSize + geometry.gapX)
  const tileY = row * (geometry.tileSize + geometry.gapY)
  const { width: fittedWidth, height: fittedHeight } = fitLongEdge(
    imageSize?.width ?? 1,
    imageSize?.height ?? 1,
    geometry.contentLongEdge
  )
  const offsetX = Math.floor(Math.max(0, geometry.tileSize - fittedWidth) / 2)
  const offsetY = Math.floor(Math.max(0, geometry.tileSize - fittedHeight) / 2)
  const x0 = scaleCoord(tileX + offsetX, geometry.scale) + geometry.contentX
  const y0 = scaleCoord(tileY + offsetY, geometry.scale) + geometry.contentY
  const x1 = scaleCoord(tileX + offsetX + fittedWidth, geometry.scale) + geometry.contentX
  const y1 = scaleCoord(tileY + offsetY + fittedHeight, geometry.scale) + geometry.contentY

  return {
    x: x0,
    y: y0,
    width: Math.max(1, x1 - x0),
    height: Math.max(1, y1 - y0),
  }
}

export function computeTileFrame(geometry: PreviewGeometry, index: number): PreviewTilePlacement {
  const col = index % geometry.cols
  const row = Math.floor(index / geometry.cols)
  const tileX = col * (geometry.tileSize + geometry.gapX)
  const tileY = row * (geometry.tileSize + geometry.gapY)
  const x0 = scaleCoord(tileX, geometry.scale) + geometry.contentX
  const y0 = scaleCoord(tileY, geometry.scale) + geometry.contentY
  const x1 = scaleCoord(tileX + geometry.tileSize, geometry.scale) + geometry.contentX
  const y1 = scaleCoord(tileY + geometry.tileSize, geometry.scale) + geometry.contentY

  return {
    x: x0,
    y: y0,
    width: Math.max(1, x1 - x0),
    height: Math.max(1, y1 - y0),
  }
}

export function gridShape(tileCount: number): { cols: number; rows: number } {
  const count = Math.max(1, Math.round(tileCount))
  const cols = Math.max(1, Math.ceil(Math.sqrt(count)))
  const rows = Math.max(1, Math.ceil(count / cols))
  return { cols, rows }
}

export function calculateDynamicBorderPercent(cols: number): number {
  if (cols >= 10) return 2
  if (cols <= 2) return 10
  return 2 + ((10 - 2) * (10 - cols)) / 8
}

export function percentToPx(percent: number, basePx: number): number {
  const safePercent = normalizeNumber(percent, 0)
  if (safePercent < 0) return 0
  return Math.round((Math.max(1, basePx) * safePercent) / 100)
}

function fitLongEdge(width: number, height: number, maxSize: number): ImageSizeLike {
  const safeWidth = Math.max(1, Math.round(normalizeNumber(width, 1)))
  const safeHeight = Math.max(1, Math.round(normalizeNumber(height, 1)))
  const safeMax = Math.max(1, Math.round(normalizeNumber(maxSize, 1)))

  if (safeWidth >= safeHeight) {
    return {
      width: safeMax,
      height: Math.max(1, Math.round((safeMax / safeWidth) * safeHeight)),
    }
  }

  return {
    width: Math.max(1, Math.round((safeMax / safeHeight) * safeWidth)),
    height: safeMax,
  }
}

function spacedExtent(count: number, tileSize: number, gap: number): number {
  if (count <= 0) return 0
  return count * tileSize + Math.max(0, count - 1) * gap
}

function scaleCoord(value: number, scale: number): number {
  return Math.round(value * scale)
}

function normalizeNumber(value: unknown, fallback: number): number {
  const numberValue = Number(value)
  return Number.isFinite(numberValue) ? numberValue : fallback
}

function resolveTargetCanvas(
  finalSize: number,
  targetAspectRatio: TargetAspectRatio | null | undefined
): { width: number; height: number } | null {
  if (!targetAspectRatio) return null

  const width = normalizeNumber(targetAspectRatio.width, 0)
  const height = normalizeNumber(targetAspectRatio.height, 0)
  if (width <= 0 || height <= 0) return null

  if (width >= height) {
    return {
      width: finalSize,
      height: Math.max(1, Math.round((finalSize * height) / width)),
    }
  }

  return {
    width: Math.max(1, Math.round((finalSize * width) / height)),
    height: finalSize,
  }
}
