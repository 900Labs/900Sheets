import type { CellRange } from '../types'

export const MAX_GRID_ROWS = 1_000_000
export const MAX_GRID_COLS = 16_384

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

function parseBoundedCellKey(key: string): { row: number; col: number } | null {
  const match = key.trim().toUpperCase().match(/^([A-Z]+)([1-9]\d*)$/)
  if (!match) return null

  let col = 0
  for (const character of match[1]) {
    col = col * 26 + character.charCodeAt(0) - 64
    if (col > MAX_GRID_COLS) return null
  }
  const row = Number(match[2])
  if (!Number.isSafeInteger(row) || row < 1 || row > MAX_GRID_ROWS) return null
  return { row: row - 1, col: col - 1 }
}

/** Parse a strict, bounded A1 cell or range reference. */
export function parseA1Range(value: string): CellRange | null {
  const parts = value.trim().split(':')
  if (parts.length < 1 || parts.length > 2) return null
  const start = parseBoundedCellKey(parts[0])
  const end = parseBoundedCellKey(parts[1] ?? parts[0])
  if (!start || !end) return null
  return normalizeRange({
    startRow: start.row,
    startCol: start.col,
    endRow: end.row,
    endCol: end.col,
  })
}

/** Return sorted, valid hidden row indexes without allocating for every grid row. */
export function sortedHiddenRows(hiddenRows: Record<number, boolean>, rowCount = MAX_GRID_ROWS): number[] {
  return Object.entries(hiddenRows)
    .filter(([row, hidden]) => hidden && Number.isInteger(Number(row)) && Number(row) >= 0 && Number(row) < rowCount)
    .map(([row]) => Number(row))
    .sort((a, b) => a - b)
}

/** Count sorted indexes lower than an exclusive upper bound. */
export function countIndexesBefore(indexes: number[], upperBound: number): number {
  let low = 0
  let high = indexes.length
  while (low < high) {
    const middle = Math.floor((low + high) / 2)
    if (indexes[middle] < upperBound) low = middle + 1
    else high = middle
  }
  return low
}

/** Map a visible zero-based row position to its logical row in bounded logarithmic time. */
export function visibleRowAt(
  visibleIndex: number,
  hiddenIndexes: number[],
  rowCount = MAX_GRID_ROWS,
): number | null {
  const visibleCount = rowCount - hiddenIndexes.length
  if (!Number.isInteger(visibleIndex) || visibleIndex < 0 || visibleIndex >= visibleCount) return null

  let low = visibleIndex
  let high = Math.min(rowCount - 1, visibleIndex + hiddenIndexes.length)
  while (low < high) {
    const middle = Math.floor((low + high) / 2)
    const visibleThroughMiddle = middle + 1 - countIndexesBefore(hiddenIndexes, middle + 1)
    if (visibleThroughMiddle <= visibleIndex) low = middle + 1
    else high = middle
  }
  return low
}
