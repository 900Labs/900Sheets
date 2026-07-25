export interface SheetInfo {
  id: number
  stable_id: number
  name: string
}

export interface RecoveryEntry {
  id: string
  modified_millis: number
  size_bytes: number
}

export interface RecoveryInspection {
  entry: RecoveryEntry
  sheets: SheetInfo[]
  metadata: Record<string, unknown>
}

export interface NativeBackupEntry {
  id: string
  document_id: string
  document_name: string
  created_millis: number
  size_bytes: number
}

export interface NativeBackupInspection {
  entry: NativeBackupEntry
  sheets: SheetInfo[]
  metadata: Record<string, unknown>
}

export interface NativeBackupWriteResult {
  entry: NativeBackupEntry
  rotation_warning?: string
}

export interface NativeSaveResult {
  backup?: NativeBackupEntry
  backup_warning?: string
}

export interface CellData {
  row: number
  col: number
  value: string
  display: string
  cell_type: string
  format?: CellFormat | null
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
