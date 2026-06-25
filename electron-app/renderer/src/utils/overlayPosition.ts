import type { PositionReference } from '../types/protocol'
import type { PreviewGeometry } from './previewLayout'

export interface OverlayPosition {
  x: number
  y: number
}

export interface OverlayReferenceRect {
  x: number
  y: number
  width: number
  height: number
}

export interface OverlayPositionBounds {
  minX: number
  maxX: number
  minY: number
  maxY: number
}

export function overlayReferenceRect(
  geometry: PreviewGeometry,
  reference: PositionReference
): OverlayReferenceRect {
  if (reference === 'content') {
    return {
      x: geometry.contentX,
      y: geometry.contentY,
      width: Math.max(1, geometry.scaledWidth),
      height: Math.max(1, geometry.scaledHeight),
    }
  }

  return {
    x: 0,
    y: 0,
    width: Math.max(1, geometry.canvasWidth),
    height: Math.max(1, geometry.canvasHeight),
  }
}

export function overlayPositionToCanvasPoint(
  geometry: PreviewGeometry,
  reference: PositionReference,
  position: OverlayPosition
): OverlayPosition {
  const rect = overlayReferenceRect(geometry, reference)
  return {
    x: rect.x + rect.width * normalizePercent(position.x) / 100,
    y: rect.y + rect.height * normalizePercent(position.y) / 100,
  }
}

export function canvasPointToOverlayPosition(
  geometry: PreviewGeometry,
  reference: PositionReference,
  point: OverlayPosition
): OverlayPosition {
  const rect = overlayReferenceRect(geometry, reference)
  return {
    x: ((normalizeCoordinate(point.x) - rect.x) / rect.width) * 100,
    y: ((normalizeCoordinate(point.y) - rect.y) / rect.height) * 100,
  }
}

export function convertOverlayPositionReference(
  geometry: PreviewGeometry,
  from: PositionReference,
  to: PositionReference,
  position: OverlayPosition
): OverlayPosition {
  if (from === to) return { ...position }
  return canvasPointToOverlayPosition(
    geometry,
    to,
    overlayPositionToCanvasPoint(geometry, from, position)
  )
}

export function overlayPositionBounds(
  geometry: PreviewGeometry,
  reference: PositionReference
): OverlayPositionBounds {
  const topLeft = canvasPointToOverlayPosition(geometry, reference, { x: 0, y: 0 })
  const bottomRight = canvasPointToOverlayPosition(geometry, reference, {
    x: geometry.canvasWidth,
    y: geometry.canvasHeight,
  })
  return {
    minX: roundOverlayPercent(Math.min(topLeft.x, bottomRight.x)),
    maxX: roundOverlayPercent(Math.max(topLeft.x, bottomRight.x)),
    minY: roundOverlayPercent(Math.min(topLeft.y, bottomRight.y)),
    maxY: roundOverlayPercent(Math.max(topLeft.y, bottomRight.y)),
  }
}

export function overlaySizeScale(
  geometry: PreviewGeometry,
  reference: PositionReference,
  finalSize: number
): number {
  if (reference === 'canvas') return 1
  return geometry.scaledWidth / Math.max(1, finalSize)
}

export function overlayWidthReference(
  geometry: PreviewGeometry,
  reference: PositionReference
): number {
  return reference === 'content'
    ? Math.max(1, geometry.scaledWidth)
    : Math.max(1, geometry.canvasWidth)
}

export function roundOverlayPercent(value: number): number {
  return Math.round(value * 100) / 100
}

function normalizePercent(value: unknown): number {
  const numberValue = Number(value)
  return Number.isFinite(numberValue) ? numberValue : 0
}

function normalizeCoordinate(value: unknown): number {
  const numberValue = Number(value)
  return Number.isFinite(numberValue) ? numberValue : 0
}
