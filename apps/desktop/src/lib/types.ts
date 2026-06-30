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

export interface CellFormat {
  bold?: boolean
  italic?: boolean
  underline?: boolean
  strikethrough?: boolean
  font_size?: number
  font_name?: string
  font_color?: string
  bg_color?: string
  h_align?: 'left' | 'center' | 'right' | 'general'
  v_align?: 'top' | 'middle' | 'bottom'
  wrap_text?: boolean
  number_format?: string
}

export type CellFormatMap = Record<string, CellFormat>
