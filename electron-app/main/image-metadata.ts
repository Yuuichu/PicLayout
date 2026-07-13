import { readFileSync } from 'node:fs'

export interface ImageSize {
  width: number
  height: number
}

export function readExifOrientation(path: string): number | null {
  try {
    return readExifOrientationFromBuffer(readFileSync(path))
  } catch {
    return null
  }
}

export function readExifOrientationFromBuffer(data: Buffer): number | null {
  if (data.length < 4 || data[0] !== 0xff || data[1] !== 0xd8) return null

  let offset = 2
  while (offset + 4 <= data.length) {
    if (data[offset] !== 0xff) return null
    while (offset < data.length && data[offset] === 0xff) offset += 1
    if (offset >= data.length) return null

    const marker = data[offset]
    offset += 1
    if (marker === 0xda || marker === 0xd9) return null
    if (offset + 2 > data.length) return null

    const segmentLength = data.readUInt16BE(offset)
    offset += 2
    if (segmentLength < 2 || offset + segmentLength - 2 > data.length) return null

    if (marker === 0xe1) {
      const orientation = readExifOrientationFromApp1(data, offset, segmentLength - 2)
      if (orientation) return orientation
    }

    offset += segmentLength - 2
  }

  return null
}

export function sizeWithExifOrientation(size: ImageSize, orientation: number | null): ImageSize {
  if (orientation && orientation >= 5 && orientation <= 8) {
    return { width: size.height, height: size.width }
  }
  return size
}

function readExifOrientationFromApp1(data: Buffer, start: number, length: number): number | null {
  const exifHeader = Buffer.from('Exif\0\0', 'ascii')
  if (length < exifHeader.length + 8) return null
  if (!data.subarray(start, start + exifHeader.length).equals(exifHeader)) return null

  const tiffStart = start + exifHeader.length
  const byteOrder = data.toString('ascii', tiffStart, tiffStart + 2)
  const littleEndian = byteOrder === 'II'
  if (!littleEndian && byteOrder !== 'MM') return null

  const readU16 = (pos: number) => (littleEndian ? data.readUInt16LE(pos) : data.readUInt16BE(pos))
  const readU32 = (pos: number) => (littleEndian ? data.readUInt32LE(pos) : data.readUInt32BE(pos))

  if (readU16(tiffStart + 2) !== 42) return null
  const ifd0Offset = readU32(tiffStart + 4)
  const ifd0Start = tiffStart + ifd0Offset
  const segmentEnd = start + length
  if (ifd0Start + 2 > segmentEnd) return null

  const entryCount = readU16(ifd0Start)
  let entryOffset = ifd0Start + 2
  for (let i = 0; i < entryCount; i += 1) {
    if (entryOffset + 12 > segmentEnd) return null
    const tag = readU16(entryOffset)
    const type = readU16(entryOffset + 2)
    const count = readU32(entryOffset + 4)
    if (tag === 0x0112 && type === 3 && count >= 1) {
      const value = readU16(entryOffset + 8)
      return value >= 1 && value <= 8 ? value : null
    }
    entryOffset += 12
  }

  return null
}
