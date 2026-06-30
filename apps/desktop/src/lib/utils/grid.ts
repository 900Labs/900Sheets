import type { CellRange } from '../types'

export function colLabel(col: number): string {
  let label = ''
  let c = col
  while (c >= 0) {
    label = String.fromCharCode(65 + (c % 26)) + label
    c = Math.floor(c / 26) - 1
  }
  return label
}

export function cellKey(row: number, col: number): string {
  return `${colLabel(col)}${row + 1}`
}

export function normalizeRange(range: CellRange): CellRange {
  return {
    startRow: Math.min(range.startRow, range.endRow),
    startCol: Math.min(range.startCol, range.endCol),
    endRow: Math.max(range.startRow, range.endRow),
    endCol: Math.max(range.startCol, range.endCol),
  }
}

export function rangeContains(range: CellRange, row: number, col: number): boolean {
  const r = normalizeRange(range)
  return row >= r.startRow && row <= r.endRow && col >= r.startCol && col <= r.endCol
}

export function rangeSize(range: CellRange): { rows: number; cols: number } {
  const r = normalizeRange(range)
  return {
    rows: r.endRow - r.startRow + 1,
    cols: r.endCol - r.startCol + 1,
  }
}

export function rangeLabel(range: CellRange): string {
  const r = normalizeRange(range)
  if (r.startRow === r.endRow && r.startCol === r.endCol) {
    return cellKey(r.startRow, r.startCol)
  }
  return `${cellKey(r.startRow, r.startCol)}:${cellKey(r.endRow, r.endCol)}`
}

export function parseCellKey(key: string): { row: number; col: number } {
  let col = 0
  let i = 0
  while (i < key.length && /[A-Za-z]/.test(key[i])) {
    col = col * 26 + (key.toUpperCase().charCodeAt(i) - 64)
    i++
  }
  const row = parseInt(key.slice(i), 10) - 1
  return { row, col: col - 1 }
}
