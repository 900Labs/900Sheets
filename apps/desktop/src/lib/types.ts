export interface SheetInfo {
  id: number
  name: string
}

export interface CellData {
  row: number
  col: number
  value: string
  display: string
  cell_type: string
}

export interface CellRange {
  startRow: number
  startCol: number
  endRow: number
  endCol: number
}

export interface ClipboardData {
  range: CellRange
  cells: string[][]
  isCut: boolean
}

export interface HistoryEntry {
  sheetId: number
  row: number
  col: number
  oldValue: string
  newValue: string
}
