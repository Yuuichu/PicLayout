import assert from 'node:assert/strict'
import test from 'node:test'
import { computePreviewLayout, type PreviewGeometry } from './previewLayout.ts'

const AUTO_LAYOUT_INPUT = {
  imageCount: 37,
  finalSize: 10_000,
  contentLongEdgePercent: 40,
  tileBorderPercent: 1,
  gapXPercent: 0,
  gapYPercent: 0,
  outerBorderMode: 'auto' as const,
  outerBorderPercent: 0,
}

function canvasMargins(geometry: PreviewGeometry) {
  return {
    left: geometry.contentX,
    right: geometry.canvasWidth - geometry.contentX - geometry.scaledWidth,
    top: geometry.contentY,
    bottom: geometry.canvasHeight - geometry.contentY - geometry.scaledHeight,
  }
}

test('Auto 画布在 37 张图片的 7x6 网格中保持四边留白一致', () => {
  const geometry = computePreviewLayout({
    ...AUTO_LAYOUT_INPUT,
    targetAspectRatio: null,
  })

  assert.equal(geometry.cols, 7)
  assert.equal(geometry.rows, 6)
  assert.deepEqual(canvasMargins(geometry), {
    left: geometry.border,
    right: geometry.border,
    top: geometry.border,
    bottom: geometry.border,
  })
})

test('固定画幅仍将拼图居中，整数取整误差不超过一个像素', () => {
  const geometry = computePreviewLayout({
    ...AUTO_LAYOUT_INPUT,
    targetAspectRatio: { width: 3, height: 4 },
  })
  const margins = canvasMargins(geometry)

  assert.ok(Math.abs(margins.left - margins.right) <= 1)
  assert.ok(Math.abs(margins.top - margins.bottom) <= 1)
})
