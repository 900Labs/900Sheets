<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { open, save } from '@tauri-apps/plugin-dialog'
  import { onMount } from 'svelte'
  import type { SheetInfo, CellData, CellRange, ClipboardData, HistoryEntry, CellFormat, CellFormatMap } from './lib/types'
  import { colLabel, cellKey, normalizeRange, rangeContains, rangeSize, rangeLabel, parseCellKey } from './lib/utils/grid'
  import { UndoRedoStack } from './lib/utils/undoRedo'

  function focusInput(node: HTMLInputElement) {
    node.focus()
    node.select()
  }

  let sheets: SheetInfo[] = $state([])
  let activeSheetId: number = $state(0)
  let cellContents: Record<string, string> = $state({})
  let cellDisplays: Record<string, string> = $state({})
  let selectedRow: number = $state(0)
  let selectedCol: number = $state(0)
  let selectionStart: { row: number; col: number } = $state({ row: 0, col: 0 })
  let selectionEnd: { row: number; col: number } = $state({ row: 0, col: 0 })
  let editingCell: string | null = $state(null)
  let editValue: string = $state('')
  let formulaBarValue: string = $state('')
  let isSelecting: boolean = $state(false)
  let clipboard: ClipboardData | null = null
  let renamingSheetId: number | null = $state(null)
  let renameValue: string = $state('')
  let canUndo: boolean = $state(false)
  let canRedo: boolean = $state(false)
  let statusMessage: string = $state('')
  let errorMessage: string = $state('')
  let currentFilePath: string | null = $state(null)
  let cellFormats: CellFormatMap = $state({})
  type MenuKey = 'file' | 'edit' | 'view' | 'insert' | 'format' | 'data' | 'tools' | 'help'
  type PanelKey = 'functions' | 'find' | 'chart' | 'pivot' | 'validation' | 'conditional' | 'print' | 'protection' | 'comment' | 'goalSeek' | 'shortcuts' | 'about'
  type MenuAction =
    | 'newWorkbook' | 'openXlsx' | 'importCsv' | 'importJson' | 'saveXlsx' | 'exportCsv' | 'exportJson' | 'exportPdf'
    | 'undo' | 'redo' | 'cut' | 'copy' | 'paste' | 'delete' | 'findReplace'
    | 'toggleFormulaBar' | 'toggleGridlines' | 'toggleCompact' | 'zoomIn' | 'zoomOut' | 'zoomReset'
    | 'functions' | 'addSheet' | 'comment' | 'chart' | 'pivot'
    | 'bold' | 'italic' | 'underline' | 'strike' | 'alignLeft' | 'alignCenter' | 'alignRight' | 'wrapText' | 'fillYellow' | 'fillGreen' | 'fillRed' | 'textBlue' | 'textRed' | 'conditional'
    | 'sortAsc' | 'sortDesc' | 'validation'
    | 'protection' | 'lockRange' | 'unlockRange' | 'goalSeek' | 'shortcuts' | 'about' | 'notReady'

  interface MenuItem {
    label: string
    action: MenuAction
    shortcut?: string
    disabled?: boolean
  }

  interface MenuDefinition {
    key: MenuKey
    label: string
    sections: MenuItem[][]
  }

  interface SearchResultData {
    row: number
    col: number
    matched_text: string
  }

  interface ValidationErrorData {
    row: number
    col: number
    error: string
  }

  interface ChartResult {
    title: string
    chart_type: string
    svg: string
  }

  interface ColumnInfo {
    col: number
    name: string
  }

  interface GoalSeekResult {
    success: boolean
    input_value: number
    achieved_value: number
    iterations: number
    error?: string | null
  }

  interface CellComment {
    row: number
    col: number
    text: string
    author: string
    visible: boolean
  }

  let openMenu: MenuKey | null = $state(null)
  let formulaMenuOpen: boolean = $state(false)
  let formulaMenuX: number = $state(0)
  let formulaMenuY: number = $state(0)
  let activePanel: PanelKey | null = $state(null)
  let showFormulaBar: boolean = $state(true)
  let showGridlines: boolean = $state(true)
  let compactControls: boolean = $state(false)
  let zoomPercent: number = $state(100)
  let findQuery: string = $state('')
  let replaceValue: string = $state('')
  let findMatchCase: boolean = $state(false)
  let findResults: SearchResultData[] = $state([])
  let functionSearch: string = $state('')
  let chartTitle: string = $state('Chart')
  let chartType: string = $state('Column')
  let chartSeriesName: string = $state('Series 1')
  let chartSvg: string = $state('')
  let pivotAggregation: string = $state('Sum')
  let pivotColumns: ColumnInfo[] = $state([])
  let validationType: string = $state('WholeNumber')
  let validationOperator: string = $state('Between')
  let validationFormula1: string = $state('0')
  let validationFormula2: string = $state('100')
  let validationSource: string = $state('')
  let validationResults: ValidationErrorData[] = $state([])
  let conditionalType: string = $state('CellValue')
  let conditionalOperator: string = $state('GreaterThan')
  let conditionalValue1: string = $state('0')
  let conditionalValue2: string = $state('')
  let conditionalFill: string = $state('#fef3c7')
  let conditionalMatches: Array<[number, number]> = $state([])
  let printPageSize: string = $state('Letter')
  let printOrientation: string = $state('Portrait')
  let printPageCount: number | null = $state(null)
  let protectionPassword: string = $state('')
  let commentText: string = $state('')
  let commentAuthor: string = $state('900Sheets user')
  let currentComment: CellComment | null = $state(null)
  let allComments: CellComment[] = $state([])
  let goalSeekTargetCell: string = $state('A1')
  let goalSeekInputCell: string = $state('A2')
  let goalSeekTargetValue: string = $state('0')
  let goalSeekResult: GoalSeekResult | null = $state(null)
  let dragScrollTimer: ReturnType<typeof setInterval> | null = null
  let dragScrollDir: 'down' | 'up' | 'left' | 'right' | null = null
  let gridContainerEl: HTMLElement | null = $state(null)

  const FORMULA_FUNCTIONS: Record<string, string[]> = {
    Math: ['SUM', 'AVERAGE', 'MIN', 'MAX', 'COUNT', 'COUNTA', 'PRODUCT', 'ABS', 'ROUND', 'ROUNDUP', 'ROUNDDOWN', 'FLOOR', 'CEILING', 'MOD', 'POWER', 'SQRT', 'SQRTPI', 'INT', 'EXP', 'LN', 'LOG10', 'LOG', 'LOG2', 'PI', 'RAND', 'RANDBETWEEN', 'SIGN', 'TRUNC', 'QUOTIENT', 'GCD', 'LCM', 'COMBIN', 'COMBINA', 'PERMUT', 'PERMUTA', 'FACT', 'FACTDOUBLE', 'MROUND', 'MULTINOMIAL', 'SERIESSUM'],
    Trig: ['SIN', 'COS', 'TAN', 'ASIN', 'ACOS', 'ATAN', 'ATAN2', 'DEGREES', 'RADIANS', 'SINH', 'COSH', 'TANH', 'ASINH', 'ACOSH', 'ATANH'],
    Statistical: ['MEDIAN', 'MODE', 'STDEV', 'STDEVP', 'VAR', 'VARP', 'LARGE', 'SMALL', 'RANK', 'PERCENTILE', 'QUARTILE', 'PERCENTRANK', 'FORECAST', 'SLOPE', 'INTERCEPT', 'CORREL', 'COVAR', 'AVERAGEIF'],
    Logical: ['IF', 'AND', 'OR', 'NOT', 'TRUE', 'FALSE', 'IFERROR', 'IFNA', 'XOR'],
    Text: ['LEN', 'UPPER', 'LOWER', 'PROPER', 'TRIM', 'LEFT', 'RIGHT', 'MID', 'MIDB', 'CONCATENATE', 'SUBSTITUTE', 'REPT', 'FIND', 'SEARCH', 'REPLACE', 'TEXT', 'VALUE', 'CONCAT', 'TEXTJOIN', 'EXACT', 'CHAR', 'CODE', 'CLEAN', 'FIXED', 'T', 'N', 'UNICODE', 'TEXTBEFORE', 'TEXTAFTER'],
    Date: ['DATE', 'TIME', 'NOW', 'TODAY', 'YEAR', 'MONTH', 'DAY', 'HOUR', 'MINUTE', 'SECOND', 'WEEKDAY', 'WEEKNUM', 'ISOWEEKNUM', 'DATEVALUE', 'TIMEVALUE', 'EDATE', 'EOMONTH', 'DATEDIF', 'DAYS'],
    Lookup: ['VLOOKUP', 'HLOOKUP', 'INDEX', 'MATCH', 'CHOOSE', 'ROW', 'COLUMN', 'ROWS', 'COLUMNS', 'ADDRESS', 'LOOKUP'],
    Financial: ['PMT', 'PV', 'FV', 'NPV', 'RATE', 'NPER', 'SLN', 'SYD', 'DDB'],
    Engineering: ['BIN2DEC', 'DEC2BIN', 'HEX2DEC', 'DEC2HEX', 'OCT2DEC', 'DEC2OCT', 'BITAND', 'BITOR', 'BITXOR', 'BITLSHIFT', 'BITRSHIFT', 'GESTEP', 'DELTA'],
    Info: ['ISNUMBER', 'ISTEXT', 'ISLOGICAL', 'ISERROR', 'ISEMPTY', 'ISBLANK', 'ISNONTEXT', 'ISODD', 'ISEVEN', 'NA', 'TYPE'],
  }

  const MENU_DEFINITIONS: MenuDefinition[] = [
    {
      key: 'file',
      label: 'File',
      sections: [
        [
          { label: 'New Workbook', action: 'newWorkbook', shortcut: 'Ctrl+N' },
          { label: 'Open XLSX...', action: 'openXlsx', shortcut: 'Ctrl+O' },
        ],
        [
          { label: 'Import CSV...', action: 'importCsv' },
          { label: 'Import JSON...', action: 'importJson' },
        ],
        [
          { label: 'Save as XLSX...', action: 'saveXlsx', shortcut: 'Ctrl+S' },
          { label: 'Export as CSV...', action: 'exportCsv' },
          { label: 'Export as JSON...', action: 'exportJson' },
          { label: 'Export as PDF...', action: 'exportPdf', shortcut: 'Ctrl+P' },
        ],
      ],
    },
    {
      key: 'edit',
      label: 'Edit',
      sections: [
        [
          { label: 'Undo', action: 'undo', shortcut: 'Ctrl+Z' },
          { label: 'Redo', action: 'redo', shortcut: 'Ctrl+Y' },
        ],
        [
          { label: 'Cut', action: 'cut', shortcut: 'Ctrl+X' },
          { label: 'Copy', action: 'copy', shortcut: 'Ctrl+C' },
          { label: 'Paste', action: 'paste', shortcut: 'Ctrl+V' },
          { label: 'Delete Selection', action: 'delete', shortcut: 'Delete' },
        ],
        [
          { label: 'Find and Replace...', action: 'findReplace', shortcut: 'Ctrl+F' },
        ],
      ],
    },
    {
      key: 'view',
      label: 'View',
      sections: [
        [
          { label: 'Toggle Formula Bar', action: 'toggleFormulaBar' },
          { label: 'Toggle Gridlines', action: 'toggleGridlines' },
          { label: 'Compact Controls', action: 'toggleCompact' },
        ],
        [
          { label: 'Zoom In', action: 'zoomIn' },
          { label: 'Zoom Out', action: 'zoomOut' },
          { label: 'Reset Zoom', action: 'zoomReset' },
        ],
        [
          { label: 'Freeze Panes', action: 'notReady', disabled: true },
        ],
      ],
    },
    {
      key: 'insert',
      label: 'Insert',
      sections: [
        [
          { label: 'Function...', action: 'functions' },
          { label: 'Chart...', action: 'chart' },
          { label: 'Pivot Table...', action: 'pivot' },
          { label: 'Comment...', action: 'comment' },
        ],
        [
          { label: 'New Sheet', action: 'addSheet', shortcut: 'Shift+F11' },
          { label: 'Rows / Columns', action: 'notReady', disabled: true },
        ],
      ],
    },
    {
      key: 'format',
      label: 'Format',
      sections: [
        [
          { label: 'Bold', action: 'bold', shortcut: 'Ctrl+B' },
          { label: 'Italic', action: 'italic', shortcut: 'Ctrl+I' },
          { label: 'Underline', action: 'underline', shortcut: 'Ctrl+U' },
          { label: 'Wrap Text', action: 'wrapText' },
        ],
        [
          { label: 'Align Left', action: 'alignLeft' },
          { label: 'Align Center', action: 'alignCenter' },
          { label: 'Align Right', action: 'alignRight' },
        ],
        [
          { label: 'Yellow Fill', action: 'fillYellow' },
          { label: 'Green Fill', action: 'fillGreen' },
          { label: 'Red Fill', action: 'fillRed' },
          { label: 'Blue Text', action: 'textBlue' },
          { label: 'Red Text', action: 'textRed' },
        ],
        [
          { label: 'Conditional Formatting...', action: 'conditional' },
        ],
      ],
    },
    {
      key: 'data',
      label: 'Data',
      sections: [
        [
          { label: 'Sort Ascending', action: 'sortAsc' },
          { label: 'Sort Descending', action: 'sortDesc' },
          { label: 'Find and Replace...', action: 'findReplace' },
        ],
        [
          { label: 'Data Validation...', action: 'validation' },
          { label: 'Pivot Table...', action: 'pivot' },
          { label: 'Goal Seek...', action: 'goalSeek' },
        ],
        [
          { label: 'Filter', action: 'notReady', disabled: true },
          { label: 'Remove Duplicates', action: 'notReady', disabled: true },
          { label: 'Named Ranges', action: 'notReady', disabled: true },
        ],
      ],
    },
    {
      key: 'tools',
      label: 'Tools',
      sections: [
        [
          { label: 'Protect Sheet...', action: 'protection' },
          { label: 'Lock Selected Range', action: 'lockRange' },
          { label: 'Unlock Selected Range', action: 'unlockRange' },
        ],
        [
          { label: 'Goal Seek...', action: 'goalSeek' },
          { label: 'Scenarios', action: 'notReady', disabled: true },
          { label: 'Locale Settings', action: 'notReady', disabled: true },
        ],
      ],
    },
    {
      key: 'help',
      label: 'Help',
      sections: [
        [
          { label: 'Keyboard Shortcuts', action: 'shortcuts' },
          { label: 'About 900Sheets', action: 'about' },
        ],
      ],
    },
  ]

  const undoRedo = new UndoRedoStack()

  const COLS = 52
  const ROWS = 1000
  const COL_WIDTH = 80
  const ROW_HEIGHT = 24
  const HEADER_HEIGHT = 28

  const VISIBLE_ROWS = 40
  let scrollTop: number = $state(0)
  let scrollLeft: number = $state(0)

  let visibleRowStart: number = $derived(Math.floor(scrollTop / ROW_HEIGHT))
  let visibleRowEnd: number = $derived(Math.min(visibleRowStart + VISIBLE_ROWS + 5, ROWS))
  let visibleColStart: number = $derived(Math.max(0, Math.floor(scrollLeft / COL_WIDTH)))
  let visibleColEnd: number = $derived(Math.min(visibleColStart + COLS, COLS))

  let currentRange: CellRange = $derived(
    normalizeRange({
      startRow: selectionStart.row,
      startCol: selectionStart.col,
      endRow: selectionEnd.row,
      endCol: selectionEnd.col,
    })
  )

  let selectionLabel: string = $derived(
    rangeLabel(currentRange)
  )
  let isMultiSelection: boolean = $derived(
    currentRange.startRow !== currentRange.endRow || currentRange.startCol !== currentRange.endCol
  )
  let formulaFunctionEntries = $derived(
    Object.entries(FORMULA_FUNCTIONS).flatMap(([category, funcs]) =>
      funcs.map((name) => ({ category, name }))
    )
  )
  let filteredFormulaFunctions = $derived(
    formulaFunctionEntries.filter((fn) => {
      const query = functionSearch.trim().toLowerCase()
      if (!query) return true
      return fn.name.toLowerCase().includes(query) || fn.category.toLowerCase().includes(query)
    })
  )
  let selectionStats = $derived(calculateSelectionStats())

  function filename(path: string): string {
    return path.split(/[\\/]/).pop() || path
  }

  function calculateSelectionStats() {
    const r = normalizeRange(currentRange)
    let count = 0
    let numericCount = 0
    let sum = 0
    let min = Number.POSITIVE_INFINITY
    let max = Number.NEGATIVE_INFINITY
    for (let row = r.startRow; row <= r.endRow; row++) {
      for (let col = r.startCol; col <= r.endCol; col++) {
        const raw = cellContents[cellKey(row, col)]
        if (raw == null || raw === '') continue
        count += 1
        const value = Number(raw)
        if (Number.isFinite(value)) {
          numericCount += 1
          sum += value
          min = Math.min(min, value)
          max = Math.max(max, value)
        }
      }
    }
    return {
      count,
      numericCount,
      sum,
      average: numericCount > 0 ? sum / numericCount : 0,
      min: numericCount > 0 ? min : 0,
      max: numericCount > 0 ? max : 0,
    }
  }

  function formatStat(value: number): string {
    if (!Number.isFinite(value)) return ''
    return Number.isInteger(value) ? String(value) : value.toFixed(2)
  }

  function colIndexFromLabel(label: string): number | null {
    const cleaned = label.trim().toUpperCase()
    if (!/^[A-Z]+$/.test(cleaned)) return null
    let value = 0
    for (const ch of cleaned) {
      value = value * 26 + (ch.charCodeAt(0) - 64)
    }
    return value - 1
  }

  function parseCellAddress(address: string): { row: number; col: number } | null {
    const match = address.trim().toUpperCase().match(/^([A-Z]+)([1-9]\d*)$/)
    if (!match) return null
    const col = colIndexFromLabel(match[1])
    if (col == null) return null
    return { row: Number(match[2]) - 1, col }
  }

  function activeSheetName(): string {
    return sheets.find((sheet) => sheet.id === activeSheetId)?.name ?? 'Sheet'
  }

  function defaultPrintConfig() {
    return {
      page_size: printPageSize,
      orientation: printOrientation,
      margins: {
        top: 36,
        bottom: 36,
        left: 36,
        right: 36,
        header: 18,
        footer: 18,
      },
      scaling: 'FitToPageWidth',
      header: { left: activeSheetName(), center: null, right: null },
      footer: { left: null, center: 'Page {page} of {pages}', right: null },
      gridlines: showGridlines,
      headings: true,
      print_area: isMultiSelection
        ? [currentRange.startRow, currentRange.startCol, currentRange.endRow, currentRange.endCol]
        : null,
      repeat_rows: null,
      repeat_cols: null,
      horizontal_center: false,
      vertical_center: false,
    }
  }

  function selectedPath(path: string | string[] | null): string | null {
    if (Array.isArray(path)) return path[0] ?? null
    return path
  }

  function describeError(error: unknown): string {
    return error instanceof Error ? error.message : String(error)
  }

  function setStatus(message: string) {
    statusMessage = message
    errorMessage = ''
  }

  function setError(error: unknown, context: string) {
    const message = describeError(error)
    errorMessage = `${context}: ${message}`
    statusMessage = ''
    console.error(context, error)
  }

  async function loadSheetList(result: SheetInfo[], preferredSheetId: number = 0) {
    sheets = result
    const nextSheet = result.find((sheet) => sheet.id === preferredSheetId) ?? result[0]
    undoRedo.clear()
    updateUndoRedoState()
    if (nextSheet) {
      await selectSheet(nextSheet.id)
    } else {
      activeSheetId = 0
      cellContents = {}
      cellDisplays = {}
    }
  }

  function queueSetCell(row: number, col: number, value: string, context: string) {
    invoke('set_cell', {
      sheetId: activeSheetId,
      row,
      col,
      value,
    })
      .then(() => {
        if (value.startsWith('=')) {
          refreshSheetData()
        }
      })
      .catch((error) => {
        setError(error, context)
        refreshSheetData()
      })
  }

  function queueClearCell(row: number, col: number, context: string) {
    invoke('clear_cell', { sheetId: activeSheetId, row, col })
      .catch((error) => {
        setError(error, context)
        refreshSheetData()
      })
  }

  function getCellValue(row: number, col: number): string {
    return cellContents[cellKey(row, col)] ?? ''
  }

  function getCellDisplay(row: number, col: number): string {
    return cellDisplays[cellKey(row, col)] ?? ''
  }

  function getCellFormat(row: number, col: number): CellFormat {
    return cellFormats[cellKey(row, col)] ?? {}
  }

  function getCellStyle(row: number, col: number): string {
    const fmt = getCellFormat(row, col)
    const styles: string[] = []
    if (fmt.bold) styles.push('font-weight: bold')
    if (fmt.italic) styles.push('font-style: italic')
    if (fmt.underline) styles.push('text-decoration: underline')
    if (fmt.strikethrough) styles.push('text-decoration: line-through')
    if (fmt.underline && fmt.strikethrough) styles.push('text-decoration: underline line-through')
    if (fmt.font_size) styles.push(`font-size: ${fmt.font_size}px`)
    if (fmt.font_color) styles.push(`color: ${fmt.font_color}`)
    if (fmt.bg_color) styles.push(`background: ${fmt.bg_color}`)
    if (fmt.h_align) styles.push(`text-align: ${fmt.h_align === 'general' ? 'left' : fmt.h_align}`)
    return styles.join('; ')
  }

  async function applyFormatToSelection(format: Partial<CellFormat>) {
    const r = normalizeRange(currentRange)
    for (let row = r.startRow; row <= r.endRow; row++) {
      for (let col = r.startCol; col <= r.endCol; col++) {
        const key = cellKey(row, col)
        const existing = cellFormats[key] ?? {}
        cellFormats[key] = { ...existing, ...format }
        try {
          await invoke('set_cell_format', {
            sheetId: activeSheetId,
            row,
            col,
            format: { ...existing, ...format },
          })
        } catch (e) {
          setError(e, 'Failed to apply format')
        }
      }
    }
  }

  function toggleBold() {
    const fmt = getCellFormat(selectedRow, selectedCol)
    applyFormatToSelection({ bold: !fmt.bold })
  }

  function toggleItalic() {
    const fmt = getCellFormat(selectedRow, selectedCol)
    applyFormatToSelection({ italic: !fmt.italic })
  }

  function toggleUnderline() {
    const fmt = getCellFormat(selectedRow, selectedCol)
    applyFormatToSelection({ underline: !fmt.underline })
  }

  function toggleStrikethrough() {
    const fmt = getCellFormat(selectedRow, selectedCol)
    applyFormatToSelection({ strikethrough: !fmt.strikethrough })
  }

  function setAlignment(align: 'left' | 'center' | 'right') {
    applyFormatToSelection({ h_align: align })
  }

  function setNumberFormat(format: string) {
    applyFormatToSelection({ number_format: format })
  }

  function increaseFontSize() {
    const fmt = getCellFormat(selectedRow, selectedCol)
    const size = fmt.font_size ?? 13
    applyFormatToSelection({ font_size: Math.min(size + 1, 72) })
  }

  function decreaseFontSize() {
    const fmt = getCellFormat(selectedRow, selectedCol)
    const size = fmt.font_size ?? 13
    applyFormatToSelection({ font_size: Math.max(size - 1, 8) })
  }

  async function handleSort(ascending: boolean) {
    const r = normalizeRange(currentRange)
    try {
      await invoke('sort_data', {
        sheetId: activeSheetId,
        sortCol: r.startCol,
        startRow: r.startRow,
        endRow: r.endRow,
        ascending,
      })
      await refreshSheetData()
      setStatus(`Sorted ${ascending ? 'ascending' : 'descending'}`)
    } catch (e) {
      setError(e, 'Sort failed')
    }
  }

  function toggleMenu(menu: MenuKey) {
    openMenu = openMenu === menu ? null : menu
    formulaMenuOpen = false
  }

  function closeMenus() {
    openMenu = null
  }

  function closePopovers() {
    openMenu = null
    formulaMenuOpen = false
  }

  function toggleFormulaMenu(event: MouseEvent) {
    const trigger = event.currentTarget as HTMLElement
    const rect = trigger.getBoundingClientRect()
    const toolbarRect = trigger.closest('.format-toolbar')?.getBoundingClientRect()
    const menuWidth = 220
    formulaMenuX = Math.max(8, Math.min(rect.left, window.innerWidth - menuWidth - 8))
    formulaMenuY = Math.max(rect.bottom + 4, (toolbarRect?.bottom ?? rect.bottom) + 4)
    formulaMenuOpen = !formulaMenuOpen
    openMenu = null
  }

  function closeFormulaMenu() {
    formulaMenuOpen = false
  }

  function insertFunction(name: string) {
    closeFormulaMenu()
    activePanel = null
    const prefix = '='
    const snippet = `${name}()`
    if (editingCell) {
      const current = editValue
      if (current.startsWith('=')) {
        editValue = current + snippet
      } else {
        editValue = prefix + snippet
      }
    } else {
      startEdit(selectedRow, selectedCol)
      editValue = prefix + snippet
    }
  }

  function openPanel(panel: PanelKey) {
    activePanel = panel
    closePopovers()
    if (panel === 'comment') {
      loadCommentPanel()
    }
    if (panel === 'pivot') {
      loadPivotColumns()
    }
    if (panel === 'chart') {
      chartSvg = ''
      const r = normalizeRange(currentRange)
      chartTitle = `${activeSheetName()} ${rangeLabel(r)}`
    }
    if (panel === 'print') {
      printPageCount = null
    }
  }

  function closePanel() {
    activePanel = null
  }

  function isInSelection(row: number, col: number): boolean {
    return rangeContains(currentRange, row, col)
  }

  function selectCell(row: number, col: number, extend: boolean = false) {
    if (editingCell) commitEdit()
    selectedRow = row
    selectedCol = col
    if (extend) {
      selectionEnd = { row, col }
    } else {
      selectionStart = { row, col }
      selectionEnd = { row, col }
    }
    formulaBarValue = getCellValue(row, col)
    editingCell = null
  }

  function startEdit(row: number, col: number) {
    editingCell = cellKey(row, col)
    editValue = getCellValue(row, col)
  }

  function commitEdit() {
    if (editingCell) {
      const key = editingCell
      const { row, col } = parseCellKey(key)
      const oldValue = cellContents[key] ?? ''
      if (editValue !== oldValue) {
        cellContents[key] = editValue
        if (!editValue.startsWith('=')) {
          cellDisplays[key] = editValue
        }
        undoRedo.push([{
          sheetId: activeSheetId,
          row,
          col,
          oldValue,
          newValue: editValue,
        }])
        updateUndoRedoState()
        queueSetCell(row, col, editValue, 'Unable to edit cell')
      }
      formulaBarValue = editValue
      editingCell = null
    }
  }

  function cancelEdit() {
    editingCell = null
  }

  async function refreshSheetData() {
    try {
      const data = await invoke<CellData[]>('get_sheet_data', { sheetId: activeSheetId })
      const contents: Record<string, string> = {}
      const displays: Record<string, string> = {}
      const formats: CellFormatMap = {}
      for (const cell of data) {
        const key = cellKey(cell.row, cell.col)
        contents[key] = cell.value
        displays[key] = cell.display
        try {
          const fmt = await invoke<CellFormat | null>('get_cell_format', {
            sheetId: activeSheetId,
            row: cell.row,
            col: cell.col,
          })
          if (fmt) formats[key] = fmt
        } catch {
          // format fetch is optional
        }
      }
      cellContents = contents
      cellDisplays = displays
      cellFormats = formats
    } catch (e) {
      console.error('Failed to refresh sheet data:', e)
    }
  }

  function updateUndoRedoState() {
    canUndo = undoRedo.canUndo()
    canRedo = undoRedo.canRedo()
  }

  function doUndo() {
    if (editingCell) commitEdit()
    const entries = undoRedo.undo()
    if (entries) {
      for (const entry of entries) {
        const key = cellKey(entry.row, entry.col)
        if (entry.oldValue) {
          cellContents[key] = entry.oldValue
          cellDisplays[key] = entry.oldValue
        } else {
          delete cellContents[key]
          delete cellDisplays[key]
        }
        invoke('set_cell', {
          sheetId: entry.sheetId,
          row: entry.row,
          col: entry.col,
          value: entry.oldValue,
        }).catch((error) => setError(error, 'Unable to undo edit'))
      }
      const last = entries[0]
      selectCell(last.row, last.col)
      updateUndoRedoState()
    }
  }

  function doRedo() {
    if (editingCell) commitEdit()
    const entries = undoRedo.redo()
    if (entries) {
      for (const entry of entries) {
        const key = cellKey(entry.row, entry.col)
        cellContents[key] = entry.newValue
        cellDisplays[key] = entry.newValue
        invoke('set_cell', {
          sheetId: entry.sheetId,
          row: entry.row,
          col: entry.col,
          value: entry.newValue,
        }).catch((error) => setError(error, 'Unable to redo edit'))
      }
      const last = entries[0]
      selectCell(last.row, last.col)
      updateUndoRedoState()
    }
  }

  function copySelection(isCut: boolean = false) {
    const r = normalizeRange(currentRange)
    const size = rangeSize(r)
    const cells: string[][] = []
    for (let row = 0; row < size.rows; row++) {
      const rowData: string[] = []
      for (let col = 0; col < size.cols; col++) {
        rowData.push(getCellValue(r.startRow + row, r.startCol + col))
      }
      cells.push(rowData)
    }
    clipboard = { range: r, cells, isCut }

    const tsv = cells.map((row) => row.join('\t')).join('\n')
    navigator.clipboard.writeText(tsv).catch(() => {})

    if (isCut) {
      const history: HistoryEntry[] = []
      for (let row = 0; row < size.rows; row++) {
        for (let col = 0; col < size.cols; col++) {
          const r2 = r.startRow + row
          const c2 = r.startCol + col
          const key = cellKey(r2, c2)
          const oldVal = cellContents[key] ?? ''
          if (oldVal) {
            history.push({ sheetId: activeSheetId, row: r2, col: c2, oldValue: oldVal, newValue: '' })
            delete cellContents[key]
            queueClearCell(r2, c2, 'Unable to cut cells')
          }
        }
      }
      if (history.length > 0) {
        undoRedo.push(history)
        updateUndoRedoState()
      }
    }
  }

  function pasteFromClipboard() {
    if (!clipboard) return
    const r = normalizeRange(currentRange)
    const history: HistoryEntry[] = []
    for (let row = 0; row < clipboard.cells.length; row++) {
      for (let col = 0; col < clipboard.cells[row].length; col++) {
        const targetRow = r.startRow + row
        const targetCol = r.startCol + col
        if (targetRow >= ROWS || targetCol >= COLS) continue
        const key = cellKey(targetRow, targetCol)
        const oldValue = cellContents[key] ?? ''
        const newValue = clipboard.cells[row][col]
        if (oldValue !== newValue) {
          history.push({ sheetId: activeSheetId, row: targetRow, col: targetCol, oldValue, newValue })
          cellContents[key] = newValue
          queueSetCell(targetRow, targetCol, newValue, 'Unable to paste cells')
        }
      }
    }
    if (history.length > 0) {
      undoRedo.push(history)
      updateUndoRedoState()
    }
  }

  async function pasteFromSystemClipboard() {
    try {
      const text = await navigator.clipboard.readText()
      if (!text) return
      const rows = text.split('\n')
      if (rows.length > 0 && rows[rows.length - 1] === '') {
        rows.pop()
      }
      const cells = rows.map((r) => r.split('\t'))
      const r = normalizeRange(currentRange)
      const history: HistoryEntry[] = []
      for (let row = 0; row < cells.length; row++) {
        for (let col = 0; col < cells[row].length; col++) {
          const targetRow = r.startRow + row
          const targetCol = r.startCol + col
          if (targetRow >= ROWS || targetCol >= COLS) continue
          const key = cellKey(targetRow, targetCol)
          const oldValue = cellContents[key] ?? ''
          const newValue = cells[row][col]
          if (oldValue !== newValue) {
            history.push({ sheetId: activeSheetId, row: targetRow, col: targetCol, oldValue, newValue })
            cellContents[key] = newValue
            queueSetCell(targetRow, targetCol, newValue, 'Unable to paste cells')
          }
        }
      }
      if (history.length > 0) {
        undoRedo.push(history)
        updateUndoRedoState()
      }
    } catch {
      pasteFromClipboard()
    }
  }

  function deleteSelection() {
    if (editingCell) return
    const r = normalizeRange(currentRange)
    const history: HistoryEntry[] = []
    for (let row = r.startRow; row <= r.endRow; row++) {
      for (let col = r.startCol; col <= r.endCol; col++) {
        const key = cellKey(row, col)
        const oldValue = cellContents[key] ?? ''
        if (oldValue) {
          history.push({ sheetId: activeSheetId, row, col, oldValue, newValue: '' })
          delete cellContents[key]
          queueClearCell(row, col, 'Unable to delete cells')
        }
      }
    }
    if (history.length > 0) {
      undoRedo.push(history)
      updateUndoRedoState()
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (editingCell) {
      if (e.key === 'Enter') {
        e.preventDefault()
        commitEdit()
        if (selectedRow < ROWS - 1) selectCell(selectedRow + 1, selectedCol)
      } else if (e.key === 'Escape') {
        e.preventDefault()
        cancelEdit()
      } else if (e.key === 'Tab') {
        e.preventDefault()
        commitEdit()
        if (selectedCol < COLS - 1) selectCell(selectedRow, selectedCol + 1)
      }
      return
    }

    const ctrl = e.ctrlKey || e.metaKey
    const key = e.key.toLowerCase()

    if (ctrl && key === 'n') {
      e.preventDefault()
      handleNewWorkbook()
      return
    }
    if (ctrl && key === 'o') {
      e.preventDefault()
      handleOpenXlsx()
      return
    }
    if (ctrl && key === 's') {
      e.preventDefault()
      handleSaveXlsx()
      return
    }
    if (ctrl && key === 'p') {
      e.preventDefault()
      openPanel('print')
      return
    }
    if (ctrl && key === 'f') {
      e.preventDefault()
      openPanel('find')
      return
    }

    if (ctrl && key === 'z' && !e.shiftKey) {
      e.preventDefault()
      doUndo()
      return
    }
    if ((ctrl && key === 'y') || (ctrl && e.shiftKey && key === 'z')) {
      e.preventDefault()
      doRedo()
      return
    }
    if (ctrl && key === 'c') {
      e.preventDefault()
      copySelection(false)
      return
    }
    if (ctrl && key === 'b') {
      e.preventDefault()
      toggleBold()
      return
    }
    if (ctrl && key === 'i') {
      e.preventDefault()
      toggleItalic()
      return
    }
    if (ctrl && key === 'u') {
      e.preventDefault()
      toggleUnderline()
      return
    }
    if (ctrl && key === 'x') {
      e.preventDefault()
      copySelection(true)
      return
    }
    if (ctrl && key === 'v') {
      e.preventDefault()
      pasteFromSystemClipboard()
      return
    }
    if (ctrl && key === 'a') {
      e.preventDefault()
      selectionStart = { row: 0, col: 0 }
      selectionEnd = { row: ROWS - 1, col: COLS - 1 }
      return
    }

    if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault()
      deleteSelection()
      return
    }

    const shift = e.shiftKey

    if (e.key === 'ArrowDown') {
      e.preventDefault()
      const newRow = Math.min(selectedRow + 1, ROWS - 1)
      selectCell(newRow, selectedCol, shift)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      const newRow = Math.max(selectedRow - 1, 0)
      selectCell(newRow, selectedCol, shift)
    } else if (e.key === 'ArrowRight') {
      e.preventDefault()
      const newCol = Math.min(selectedCol + 1, COLS - 1)
      selectCell(selectedRow, newCol, shift)
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault()
      const newCol = Math.max(selectedCol - 1, 0)
      selectCell(selectedRow, newCol, shift)
    } else if (e.key === 'Enter' || e.key === 'F2') {
      e.preventDefault()
      startEdit(selectedRow, selectedCol)
    } else if (e.key === 'Tab') {
      e.preventDefault()
      if (selectedCol < COLS - 1) selectCell(selectedRow, selectedCol + 1)
    } else if (e.key.length === 1 && !ctrl && !e.metaKey) {
      e.preventDefault()
      startEdit(selectedRow, selectedCol)
      editValue = e.key
    }
  }

  function handleFormulaBarKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault()
      const key = cellKey(selectedRow, selectedCol)
      const oldValue = cellContents[key] ?? ''
      if (formulaBarValue !== oldValue) {
        cellContents[key] = formulaBarValue
        if (!formulaBarValue.startsWith('=')) {
          cellDisplays[key] = formulaBarValue
        }
        undoRedo.push([{
          sheetId: activeSheetId,
          row: selectedRow,
          col: selectedCol,
          oldValue,
          newValue: formulaBarValue,
        }])
        updateUndoRedoState()
        queueSetCell(selectedRow, selectedCol, formulaBarValue, 'Unable to update formula bar')
      }
    }
  }

  function handleMouseDown(row: number, col: number, e: MouseEvent) {
    if (e.shiftKey) {
      selectionEnd = { row, col }
      selectedRow = row
      selectedCol = col
    } else {
      selectCell(row, col)
      isSelecting = true
    }
  }

  function handleMouseEnter(row: number, col: number) {
    if (isSelecting) {
      selectionEnd = { row, col }
      selectedRow = row
      selectedCol = col
    }
  }

  function handleMouseUp() {
    isSelecting = false
    stopDragScroll()
  }

  function stopDragScroll() {
    if (dragScrollTimer) {
      clearInterval(dragScrollTimer)
      dragScrollTimer = null
      dragScrollDir = null
    }
  }

  function startDragScroll(direction: 'down' | 'up' | 'left' | 'right') {
    if (dragScrollDir === direction && dragScrollTimer) return
    stopDragScroll()
    dragScrollDir = direction
    dragScrollTimer = setInterval(() => {
      if (!gridContainerEl) return
      const STEP = ROW_HEIGHT
      if (direction === 'down') {
        gridContainerEl.scrollTop = Math.min(gridContainerEl.scrollTop + STEP, gridContainerEl.scrollHeight)
      } else if (direction === 'up') {
        gridContainerEl.scrollTop = Math.max(gridContainerEl.scrollTop - STEP, 0)
      } else if (direction === 'right') {
        gridContainerEl.scrollLeft = Math.min(gridContainerEl.scrollLeft + COL_WIDTH, gridContainerEl.scrollWidth)
      } else if (direction === 'left') {
        gridContainerEl.scrollLeft = Math.max(gridContainerEl.scrollLeft - COL_WIDTH, 0)
      }
    }, 50)
  }

  function handleGridMouseMove(e: MouseEvent) {
    if (!isSelecting || !gridContainerEl) return
    const rect = gridContainerEl.getBoundingClientRect()
    const margin = 30
    const relX = e.clientX - rect.left
    const relY = e.clientY - rect.top

    if (relY > rect.height - margin) {
      startDragScroll('down')
    } else if (relY < margin + HEADER_HEIGHT) {
      startDragScroll('up')
    } else if (relX > rect.width - margin) {
      startDragScroll('right')
    } else if (relX < margin + COL_WIDTH * 0.6) {
      startDragScroll('left')
    } else {
      stopDragScroll()
    }
  }

  function handleScroll(e: Event) {
    const target = e.target as HTMLElement
    scrollTop = target.scrollTop
    scrollLeft = target.scrollLeft
  }

  async function selectSheet(id: number) {
    if (editingCell) commitEdit()
    activeSheetId = id
    cellContents = {}
    cellDisplays = {}
    cellFormats = {}
    try {
      const data = await invoke<CellData[]>('get_sheet_data', { sheetId: id })
      const contents: Record<string, string> = {}
      const displays: Record<string, string> = {}
      for (const cell of data) {
        contents[cellKey(cell.row, cell.col)] = cell.value
        displays[cellKey(cell.row, cell.col)] = cell.display
      }
      cellContents = contents
      cellDisplays = displays
    } catch (e) {
      console.error('Failed to load sheet data:', e)
    }
    selectCell(0, 0)
  }

  async function handleAddSheet() {
    const name = `Sheet${sheets.length + 1}`
    try {
      const result = await invoke<SheetInfo[]>('add_sheet', { name })
      sheets = result
      const newId = result[result.length - 1].id
      await selectSheet(newId)
    } catch (e) {
      setError(e, 'Failed to add sheet')
    }
  }

  async function handleDeleteSheet(id: number) {
    if (sheets.length <= 1) return
    try {
      const result = await invoke<SheetInfo[]>('delete_sheet', { sheetId: id })
      sheets = result
      if (activeSheetId === id) {
        await selectSheet(0)
      }
    } catch (e) {
      setError(e, 'Failed to delete sheet')
    }
  }

  function startRenameSheet(id: number, currentName: string) {
    renamingSheetId = id
    renameValue = currentName
  }

  async function commitRename() {
    if (renamingSheetId !== null && renameValue.trim()) {
      try {
        const result = await invoke<SheetInfo[]>('rename_sheet', {
          sheetId: renamingSheetId,
          name: renameValue.trim(),
        })
        sheets = result
    } catch (e) {
        setError(e, 'Failed to rename sheet')
      }
    }
    renamingSheetId = null
  }

  async function handleNewWorkbook() {
    try {
      const result = await invoke<SheetInfo[]>('new_workbook')
      currentFilePath = null
      await loadSheetList(result)
      setStatus('New workbook')
    } catch (e) {
      setError(e, 'Failed to create workbook')
    }
  }

  async function handleOpenXlsx() {
    try {
      const path = selectedPath(await open({
        multiple: false,
        filters: [{ name: 'Excel Workbook', extensions: ['xlsx'] }],
      }))
      if (!path) return
      const result = await invoke<SheetInfo[]>('import_xlsx_file', { filePath: path })
      currentFilePath = path
      await loadSheetList(result)
      setStatus(`Opened ${filename(path)}`)
    } catch (e) {
      setError(e, 'Failed to open XLSX')
    }
  }

  async function handleImportCsv() {
    try {
      const path = selectedPath(await open({
        multiple: false,
        filters: [{ name: 'CSV', extensions: ['csv', 'tsv', 'txt'] }],
      }))
      if (!path) return
      const delimiter = path.toLowerCase().endsWith('.tsv') ? '\t' : ','
      const result = await invoke<SheetInfo[]>('import_csv_file', {
        filePath: path,
        delimiter,
        sheetId: activeSheetId,
      })
      await loadSheetList(result, activeSheetId)
      setStatus(`Imported ${filename(path)}`)
    } catch (e) {
      setError(e, 'Failed to import CSV')
    }
  }

  async function handleImportJson() {
    try {
      const path = selectedPath(await open({
        multiple: false,
        filters: [{ name: 'JSON', extensions: ['json'] }],
      }))
      if (!path) return
      const result = await invoke<SheetInfo[]>('import_json_file', { filePath: path })
      currentFilePath = null
      await loadSheetList(result)
      setStatus(`Imported ${filename(path)}`)
    } catch (e) {
      setError(e, 'Failed to import JSON')
    }
  }

  async function handleSaveXlsx() {
    try {
      let path = currentFilePath
      if (!path) {
        path = await save({
          defaultPath: '900Sheets.xlsx',
          filters: [{ name: 'Excel Workbook', extensions: ['xlsx'] }],
        })
      }
      if (!path) return
      await invoke('export_xlsx_file', { filePath: path })
      currentFilePath = path
      setStatus(`Saved ${filename(path)}`)
    } catch (e) {
      setError(e, 'Failed to save XLSX')
    }
  }

  async function handleExportCsv() {
    try {
      const path = await save({
        defaultPath: `${sheets.find((sheet) => sheet.id === activeSheetId)?.name ?? 'Sheet'}.csv`,
        filters: [{ name: 'CSV', extensions: ['csv'] }],
      })
      if (!path) return
      await invoke('export_csv_file', {
        sheetId: activeSheetId,
        delimiter: ',',
        filePath: path,
      })
      setStatus(`Exported ${filename(path)}`)
    } catch (e) {
      setError(e, 'Failed to export CSV')
    }
  }

  async function handleExportJson() {
    try {
      const path = await save({
        defaultPath: '900Sheets.json',
        filters: [{ name: 'JSON', extensions: ['json'] }],
      })
      if (!path) return
      await invoke('export_json_file', { filePath: path })
      setStatus(`Exported ${filename(path)}`)
    } catch (e) {
      setError(e, 'Failed to export JSON')
    }
  }

  async function handleExportPdf() {
    try {
      const path = await save({
        defaultPath: `${activeSheetName()}.pdf`,
        filters: [{ name: 'PDF', extensions: ['pdf'] }],
      })
      if (!path) return
      await invoke('save_pdf_to_file', {
        sheetId: activeSheetId,
        config: defaultPrintConfig(),
        filePath: path,
      })
      setStatus(`Exported ${filename(path)}`)
    } catch (e) {
      setError(e, 'Failed to export PDF')
    }
  }

  async function runFind() {
    if (!findQuery.trim()) {
      findResults = []
      return
    }
    try {
      findResults = await invoke<SearchResultData[]>('find_in_sheet_cmd', {
        sheetId: activeSheetId,
        query: findQuery,
        matchCase: findMatchCase,
      })
      setStatus(`${findResults.length} match${findResults.length === 1 ? '' : 'es'} found`)
      if (findResults[0]) jumpToResult(findResults[0])
    } catch (e) {
      setError(e, 'Find failed')
    }
  }

  async function runReplace() {
    if (!findQuery.trim()) return
    try {
      const count = await invoke<number>('replace_in_sheet_cmd', {
        sheetId: activeSheetId,
        find: findQuery,
        replace: replaceValue,
        matchCase: findMatchCase,
      })
      await refreshSheetData()
      findResults = []
      setStatus(`Replaced ${count} match${count === 1 ? '' : 'es'}`)
    } catch (e) {
      setError(e, 'Replace failed')
    }
  }

  function jumpToResult(result: SearchResultData | ValidationErrorData | { row: number; col: number }) {
    selectCell(result.row, result.col)
  }

  async function runChart() {
    const r = normalizeRange(currentRange)
    const xCol = r.startCol
    const yCol = Math.min(r.startCol + 1, r.endCol)
    if (xCol === yCol || r.startRow === r.endRow) {
      setError('Select at least two columns and two rows before creating a chart', 'Chart unavailable')
      return
    }
    try {
      const result = await invoke<ChartResult>('create_chart', {
        sheetId: activeSheetId,
        config: {
          title: chartTitle || 'Chart',
          chart_type: chartType,
          series: [{
            name: chartSeriesName || colLabel(yCol),
            x_column: xCol,
            y_column: yCol,
            color: null,
          }],
          header_row: r.startRow,
          data_start_row: Math.min(r.startRow + 1, r.endRow),
          data_end_row: r.endRow,
          x_axis_label: colLabel(xCol),
          y_axis_label: colLabel(yCol),
          legend_position: 'Bottom',
        },
      })
      chartSvg = result.svg
      setStatus(`Created ${chartType.toLowerCase()} chart from ${rangeLabel(r)}`)
    } catch (e) {
      setError(e, 'Chart failed')
    }
  }

  async function loadPivotColumns() {
    const r = normalizeRange(currentRange)
    try {
      pivotColumns = await invoke<ColumnInfo[]>('get_pivot_columns', {
        sheetId: activeSheetId,
        headerRow: r.startRow,
        startCol: r.startCol,
        endCol: r.endCol,
      })
    } catch {
      pivotColumns = []
    }
  }

  async function runPivotSheet() {
    const r = normalizeRange(currentRange)
    if (r.startRow === r.endRow || r.startCol === r.endCol) {
      setError('Select a table with headers, row labels, and at least one value column', 'Pivot unavailable')
      return
    }
    const rowCol = pivotColumns[0]?.col ?? r.startCol
    const valueCol = pivotColumns[pivotColumns.length - 1]?.col ?? r.endCol
    try {
      const result = await invoke<SheetInfo[]>('create_pivot_sheet', {
        sheetId: activeSheetId,
        config: {
          source_sheet: activeSheetId,
          data_range: [r.startRow, r.startCol, r.endRow, r.endCol],
          header_row: r.startRow,
          row_fields: [{ column: rowCol, label: pivotColumns.find((c) => c.col === rowCol)?.name ?? colLabel(rowCol) }],
          column_fields: [],
          value_fields: [{
            column: valueCol,
            label: pivotColumns.find((c) => c.col === valueCol)?.name ?? colLabel(valueCol),
            aggregation: pivotAggregation,
          }],
          filter_field: null,
          filter_values: [],
        },
      })
      await loadSheetList(result, result[result.length - 1]?.id ?? activeSheetId)
      setStatus(`Created pivot sheet from ${rangeLabel(r)}`)
      closePanel()
    } catch (e) {
      setError(e, 'Pivot failed')
    }
  }

  function buildValidation() {
    return {
      validation_type: validationType,
      operator: validationOperator,
      formula1: validationType === 'List' ? null : validationFormula1,
      formula2: validationType === 'List' || validationOperator !== 'Between' && validationOperator !== 'NotBetween' ? null : validationFormula2,
      source: validationType === 'List' ? validationSource : null,
      allow_blank: true,
      show_dropdown: validationType === 'List',
      error_style: 'Stop',
      error_title: 'Invalid value',
      error_message: 'The selected value does not match the validation rule.',
      prompt_title: null,
      prompt_message: null,
    }
  }

  async function runValidation() {
    const r = normalizeRange(currentRange)
    try {
      validationResults = await invoke<ValidationErrorData[]>('validate_range_cmd', {
        sheetId: activeSheetId,
        rule: {
          range: [r.startRow, r.startCol, r.endRow, r.endCol],
          validation: buildValidation(),
        },
      })
      setStatus(validationResults.length === 0 ? `Validation passed for ${rangeLabel(r)}` : `${validationResults.length} validation issue${validationResults.length === 1 ? '' : 's'} found`)
    } catch (e) {
      setError(e, 'Validation failed')
    }
  }

  function buildConditionalRule() {
    const r = normalizeRange(currentRange)
    return {
      id: `rule-${Date.now()}`,
      condition_type: conditionalType,
      range: [r.startRow, r.startCol, r.endRow, r.endCol],
      operator: conditionalType === 'CellValue' ? conditionalOperator : null,
      value1: conditionalType === 'Blanks' || conditionalType === 'NoBlanks' || conditionalType === 'Duplicate' ? null : conditionalValue1,
      value2: conditionalOperator === 'Between' || conditionalOperator === 'NotBetween' ? conditionalValue2 : null,
      format: { bg_color: conditionalFill, bold: true },
      color_scale_stops: [],
      bar_color: null,
      show_bar_value: true,
      icon_set_type: null,
      is_top: true,
      rank: 10,
      is_above_average: true,
      std_dev: 0,
      priority: 0,
      stop_if_true: false,
    }
  }

  async function applyConditionalFormat() {
    try {
      const matches = await invoke<Array<[number, number]>>('find_conditional_format_matches', {
        sheetId: activeSheetId,
        rule: buildConditionalRule(),
      })
      conditionalMatches = matches
      for (const [row, col] of matches) {
        const key = cellKey(row, col)
        const existing = cellFormats[key] ?? {}
        const next = { ...existing, bg_color: conditionalFill, bold: true }
        cellFormats[key] = next
        await invoke('set_cell_format', {
          sheetId: activeSheetId,
          row,
          col,
          format: next,
        })
      }
      setStatus(`Applied conditional format to ${matches.length} cell${matches.length === 1 ? '' : 's'}`)
    } catch (e) {
      setError(e, 'Conditional formatting failed')
    }
  }

  async function updatePrintPreview() {
    try {
      printPageCount = await invoke<number>('get_page_count', {
        sheetId: activeSheetId,
        config: defaultPrintConfig(),
      })
      setStatus(`${printPageCount} print page${printPageCount === 1 ? '' : 's'} estimated`)
    } catch (e) {
      setError(e, 'Print preview failed')
    }
  }

  async function setProtection(protectedSheet: boolean) {
    if (!protectionPassword) {
      setError('Enter a password first', 'Protection failed')
      return
    }
    try {
      if (protectedSheet) {
        await invoke('protect_sheet', { sheetId: activeSheetId, password: protectionPassword })
        setStatus('Sheet protected')
      } else {
        const ok = await invoke<boolean>('unprotect_sheet', { sheetId: activeSheetId, password: protectionPassword })
        setStatus(ok ? 'Sheet unprotected' : 'Password did not match')
      }
      protectionPassword = ''
    } catch (e) {
      setError(e, protectedSheet ? 'Protect sheet failed' : 'Unprotect sheet failed')
    }
  }

  async function setRangeLock(locked: boolean) {
    const r = normalizeRange(currentRange)
    try {
      await invoke('lock_cell_range', {
        sheetId: activeSheetId,
        startRow: r.startRow,
        startCol: r.startCol,
        endRow: r.endRow,
        endCol: r.endCol,
        locked,
      })
      setStatus(`${locked ? 'Locked' : 'Unlocked'} ${rangeLabel(r)}`)
    } catch (e) {
      setError(e, locked ? 'Lock range failed' : 'Unlock range failed')
    }
  }

  async function loadCommentPanel() {
    try {
      currentComment = await invoke<CellComment | null>('get_cell_comment', {
        row: selectedRow,
        col: selectedCol,
      })
      commentText = currentComment?.text ?? ''
      allComments = await invoke<CellComment[]>('list_comments')
    } catch (e) {
      setError(e, 'Comment load failed')
    }
  }

  async function saveComment() {
    try {
      await invoke('add_cell_comment', {
        row: selectedRow,
        col: selectedCol,
        text: commentText,
        author: commentAuthor || '900Sheets user',
      })
      await loadCommentPanel()
      setStatus(`Saved comment on ${cellKey(selectedRow, selectedCol)}`)
    } catch (e) {
      setError(e, 'Comment save failed')
    }
  }

  async function removeCurrentComment() {
    try {
      await invoke('remove_cell_comment', { row: selectedRow, col: selectedCol })
      commentText = ''
      await loadCommentPanel()
      setStatus(`Removed comment from ${cellKey(selectedRow, selectedCol)}`)
    } catch (e) {
      setError(e, 'Comment removal failed')
    }
  }

  async function runGoalSeek() {
    const target = parseCellAddress(goalSeekTargetCell)
    const input = parseCellAddress(goalSeekInputCell)
    const targetValue = Number(goalSeekTargetValue)
    if (!target || !input || !Number.isFinite(targetValue)) {
      setError('Use addresses like A1 and a numeric target value', 'Goal seek failed')
      return
    }
    try {
      goalSeekResult = await invoke<GoalSeekResult>('goal_seek_cmd', {
        sheetId: activeSheetId,
        config: {
          target_cell_row: target.row,
          target_cell_col: target.col,
          target_value: targetValue,
          input_cell_row: input.row,
          input_cell_col: input.col,
          max_iterations: 100,
          tolerance: 0.001,
        },
      })
      if (goalSeekResult.success) {
        const oldValue = cellContents[cellKey(input.row, input.col)] ?? ''
        const newValue = String(goalSeekResult.input_value)
        await invoke('set_cell', {
          sheetId: activeSheetId,
          row: input.row,
          col: input.col,
          value: newValue,
        })
        undoRedo.push([{ sheetId: activeSheetId, row: input.row, col: input.col, oldValue, newValue }])
        updateUndoRedoState()
        await refreshSheetData()
        selectCell(input.row, input.col)
      }
      setStatus(goalSeekResult.success ? `Goal seek applied ${formatStat(goalSeekResult.input_value)}` : goalSeekResult.error ?? 'Goal seek did not converge')
    } catch (e) {
      setError(e, 'Goal seek failed')
    }
  }

  async function executeMenuAction(action: MenuAction) {
    closePopovers()
    switch (action) {
      case 'newWorkbook': return handleNewWorkbook()
      case 'openXlsx': return handleOpenXlsx()
      case 'importCsv': return handleImportCsv()
      case 'importJson': return handleImportJson()
      case 'saveXlsx': return handleSaveXlsx()
      case 'exportCsv': return handleExportCsv()
      case 'exportJson': return handleExportJson()
      case 'exportPdf': return handleExportPdf()
      case 'undo': return doUndo()
      case 'redo': return doRedo()
      case 'cut': return copySelection(true)
      case 'copy': return copySelection(false)
      case 'paste': return pasteFromSystemClipboard()
      case 'delete': return deleteSelection()
      case 'findReplace': return openPanel('find')
      case 'toggleFormulaBar': showFormulaBar = !showFormulaBar; return
      case 'toggleGridlines': showGridlines = !showGridlines; return
      case 'toggleCompact': compactControls = !compactControls; return
      case 'zoomIn': zoomPercent = Math.min(150, zoomPercent + 10); return
      case 'zoomOut': zoomPercent = Math.max(70, zoomPercent - 10); return
      case 'zoomReset': zoomPercent = 100; return
      case 'functions': return openPanel('functions')
      case 'addSheet': return handleAddSheet()
      case 'comment': return openPanel('comment')
      case 'chart': return openPanel('chart')
      case 'pivot': return openPanel('pivot')
      case 'bold': return toggleBold()
      case 'italic': return toggleItalic()
      case 'underline': return toggleUnderline()
      case 'strike': return toggleStrikethrough()
      case 'alignLeft': return setAlignment('left')
      case 'alignCenter': return setAlignment('center')
      case 'alignRight': return setAlignment('right')
      case 'wrapText': return applyFormatToSelection({ wrap_text: !getCellFormat(selectedRow, selectedCol).wrap_text })
      case 'fillYellow': return applyFormatToSelection({ bg_color: '#fef3c7' })
      case 'fillGreen': return applyFormatToSelection({ bg_color: '#dcfce7' })
      case 'fillRed': return applyFormatToSelection({ bg_color: '#fee2e2' })
      case 'textBlue': return applyFormatToSelection({ font_color: '#1d4ed8' })
      case 'textRed': return applyFormatToSelection({ font_color: '#b91c1c' })
      case 'conditional': return openPanel('conditional')
      case 'sortAsc': return handleSort(true)
      case 'sortDesc': return handleSort(false)
      case 'validation': return openPanel('validation')
      case 'protection': return openPanel('protection')
      case 'lockRange': return setRangeLock(true)
      case 'unlockRange': return setRangeLock(false)
      case 'goalSeek': return openPanel('goalSeek')
      case 'shortcuts': return openPanel('shortcuts')
      case 'about': return openPanel('about')
      case 'notReady': setStatus('This workflow is planned but not wired yet'); return
    }
  }

  onMount(async () => {
    await handleNewWorkbook()
  })
</script>

<svelte:window onkeydown={handleKeydown} onmouseup={handleMouseUp} onclick={closePopovers} />

<div class="app" class:compact={compactControls} class:no-gridlines={!showGridlines}>
  <div class="toolbar">
    <span class="app-title">900Sheets</span>
    <nav class="menu-bar" aria-label="Application menus">
      {#each MENU_DEFINITIONS as menu}
        <div class="menu-wrapper">
          <button
            type="button"
            class="toolbar-btn text"
            class:active={openMenu === menu.key}
            onclick={(e) => { e.stopPropagation(); toggleMenu(menu.key) }}
          >
            {menu.label}
          </button>
          {#if openMenu === menu.key}
            <div class="dropdown-menu app-menu" role="menu" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
              {#each menu.sections as section, sectionIndex}
                {#if sectionIndex > 0}
                  <div class="dropdown-divider"></div>
                {/if}
                {#each section as item}
                  <button
                    type="button"
                    class="dropdown-item menu-item"
                    disabled={item.disabled}
                    onclick={(e) => { e.preventDefault(); e.stopPropagation(); executeMenuAction(item.action) }}
                  >
                    <span>{item.label}</span>
                    {#if item.shortcut}
                      <span class="menu-shortcut">{item.shortcut}</span>
                    {/if}
                  </button>
                {/each}
              {/each}
            </div>
          {/if}
        </div>
      {/each}
    </nav>
    <div class="toolbar-divider"></div>
    <button type="button" class="toolbar-btn" onclick={doUndo} disabled={!canUndo} title="Undo (Ctrl+Z)">↶</button>
    <button type="button" class="toolbar-btn" onclick={doRedo} disabled={!canRedo} title="Redo (Ctrl+Y)">↷</button>
    <div class="toolbar-status" class:error={!!errorMessage}>
      {errorMessage || statusMessage}
    </div>
  </div>

  <div class="format-toolbar">
    <div class="ribbon-group">
      <button type="button" class="fmt-btn fx-btn" onclick={(e) => { e.stopPropagation(); toggleFormulaMenu(e) }} title="Insert function">fx ▾</button>
      <button type="button" class="fmt-btn" onclick={() => openPanel('functions')} title="Function browser">Functions</button>
    </div>
    <div class="fmt-divider"></div>
    <div class="ribbon-group">
      <button type="button" class="fmt-btn" onclick={() => copySelection(false)} title="Copy (Ctrl+C)">Copy</button>
      <button type="button" class="fmt-btn" onclick={() => pasteFromSystemClipboard()} title="Paste (Ctrl+V)">Paste</button>
    </div>
    <div class="fmt-divider"></div>
    <div class="ribbon-group">
      <button type="button" class="fmt-btn" onclick={toggleBold} title="Bold (Ctrl+B)"><b>B</b></button>
      <button type="button" class="fmt-btn" onclick={toggleItalic} title="Italic (Ctrl+I)"><i>I</i></button>
      <button type="button" class="fmt-btn" onclick={toggleUnderline} title="Underline (Ctrl+U)"><u>U</u></button>
      <button type="button" class="fmt-btn" onclick={toggleStrikethrough} title="Strikethrough"><s>S</s></button>
      <button type="button" class="fmt-btn" onclick={() => applyFormatToSelection({ wrap_text: !getCellFormat(selectedRow, selectedCol).wrap_text })} title="Wrap text">Wrap</button>
    </div>
    <div class="fmt-divider"></div>
    <div class="ribbon-group">
      <button type="button" class="fmt-btn" onclick={() => setAlignment('left')} title="Align left">⬅</button>
      <button type="button" class="fmt-btn" onclick={() => setAlignment('center')} title="Align center">↔</button>
      <button type="button" class="fmt-btn" onclick={() => setAlignment('right')} title="Align right">➡</button>
    </div>
    <div class="fmt-divider"></div>
    <div class="ribbon-group">
      <button type="button" class="fmt-btn" onclick={increaseFontSize} title="Increase font size">A+</button>
      <button type="button" class="fmt-btn" onclick={decreaseFontSize} title="Decrease font size">A−</button>
      <button type="button" class="swatch-btn yellow" onclick={() => applyFormatToSelection({ bg_color: '#fef3c7' })} title="Yellow fill"></button>
      <button type="button" class="swatch-btn green" onclick={() => applyFormatToSelection({ bg_color: '#dcfce7' })} title="Green fill"></button>
      <button type="button" class="swatch-btn red" onclick={() => applyFormatToSelection({ bg_color: '#fee2e2' })} title="Red fill"></button>
    </div>
    <div class="fmt-divider"></div>
    <div class="ribbon-group">
      <select class="fmt-select" onchange={(e) => setNumberFormat((e.target as HTMLSelectElement).value)} title="Number format">
        <option value="">General</option>
        <option value="#,##0">Number</option>
        <option value="$#,##0.00">Currency</option>
        <option value="0%">Percentage</option>
        <option value="0.00%">Percentage (2 dp)</option>
        <option value="yyyy-mm-dd">Date (ISO)</option>
        <option value="mm/dd/yyyy">Date (US)</option>
        <option value="hh:mm">Time</option>
        <option value="#,##0.00">Number (2 dp)</option>
      </select>
    </div>
    <div class="fmt-divider"></div>
    <div class="ribbon-group">
      <button type="button" class="fmt-btn" onclick={() => handleSort(true)} title="Sort ascending">↑ Sort</button>
      <button type="button" class="fmt-btn" onclick={() => handleSort(false)} title="Sort descending">↓ Sort</button>
      <button type="button" class="fmt-btn" onclick={() => openPanel('find')} title="Find and replace">Find</button>
      <button type="button" class="fmt-btn" onclick={() => openPanel('validation')} title="Validate selected range">Validate</button>
    </div>
    <div class="fmt-divider"></div>
    <div class="ribbon-group">
      <button type="button" class="fmt-btn" onclick={() => openPanel('chart')} title="Create chart">Chart</button>
      <button type="button" class="fmt-btn" onclick={() => openPanel('pivot')} title="Create pivot table">Pivot</button>
      <button type="button" class="fmt-btn" onclick={() => openPanel('print')} title="Print and export PDF">Print</button>
    </div>
  </div>

  {#if formulaMenuOpen}
    <div
      class="dropdown-menu formula-menu"
      style="left: {formulaMenuX}px; top: {formulaMenuY}px;"
      role="menu"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      {#each Object.entries(FORMULA_FUNCTIONS) as [category, funcs]}
        <div class="dropdown-category">{category}</div>
        {#each funcs as fn}
          <button type="button" class="dropdown-item" onclick={() => insertFunction(fn)}>{fn}</button>
        {/each}
      {/each}
    </div>
  {/if}

  {#if showFormulaBar}
    <div class="formula-bar">
      <span class="cell-ref">{rangeLabel(currentRange)}</span>
      <span class="fx">fx</span>
      <input
        type="text"
        bind:value={formulaBarValue}
        onkeydown={handleFormulaBarKeydown}
        placeholder="Enter value or formula"
      />
    </div>
  {/if}

  <div class="grid-container" style="--grid-zoom: {zoomPercent / 100};" role="grid" tabindex="-1" bind:this={gridContainerEl} onscroll={handleScroll} onmousemove={handleGridMouseMove}>
    <div
      class="grid"
      style="grid-template-columns: {COL_WIDTH * 0.6}px repeat({COLS}, {COL_WIDTH}px); height: {HEADER_HEIGHT + ROWS * ROW_HEIGHT}px;"
    >
      <div class="corner-cell"></div>
      {#each Array(COLS) as _, c}
        <div class="col-header">{colLabel(c)}</div>
      {/each}

      <div class="grid-spacer" style="height: {visibleRowStart * ROW_HEIGHT}px;"></div>

      {#each Array(visibleRowEnd - visibleRowStart) as _, i}
        {@const r = visibleRowStart + i}
        <div class="row-header" style="height: {ROW_HEIGHT}px;">{r + 1}</div>
        {#each Array(COLS) as _, c}
          <button
            type="button"
            class="cell"
            class:selected={isInSelection(r, c)}
            class:active={selectedRow === r && selectedCol === c}
            onmousedown={(e) => handleMouseDown(r, c, e)}
            onmouseenter={() => handleMouseEnter(r, c)}
            ondblclick={() => startEdit(r, c)}
            role="gridcell"
            aria-label={cellKey(r, c)}
            style="height: {ROW_HEIGHT}px; {getCellStyle(r, c)}"
          >
            {#if editingCell === cellKey(r, c)}
              <input
                type="text"
                bind:value={editValue}
                onblur={commitEdit}
                use:focusInput
                class="cell-input"
              />
            {:else}
              <span class="cell-value">{getCellDisplay(r, c)}</span>
            {/if}
          </button>
        {/each}
      {/each}

      <div class="grid-spacer" style="height: {Math.max(0, (ROWS - visibleRowEnd) * ROW_HEIGHT)}px;"></div>
    </div>
    {#if isMultiSelection}
      <div class="selection-label">{selectionLabel}</div>
    {/if}
  </div>

  <div class="sheet-tabs">
    {#each sheets as sheet}
      <div class="sheet-tab-wrapper">
        {#if renamingSheetId === sheet.id}
          <input
            type="text"
            class="sheet-rename-input"
            bind:value={renameValue}
            onblur={commitRename}
            onkeydown={(e) => { if (e.key === 'Enter') commitRename(); if (e.key === 'Escape') renamingSheetId = null; }}
          />
        {:else}
          <button
            type="button"
            class="sheet-tab"
            class:active={activeSheetId === sheet.id}
            onclick={() => selectSheet(sheet.id)}
            ondblclick={() => startRenameSheet(sheet.id, sheet.name)}
          >
            {sheet.name}
          </button>
          {#if activeSheetId === sheet.id && sheets.length > 1}
            <button
              type="button"
              class="sheet-tab-delete"
              onclick={() => handleDeleteSheet(sheet.id)}
              title="Delete sheet"
            >×</button>
          {/if}
        {/if}
      </div>
    {/each}
    <button type="button" class="sheet-tab-add" onclick={handleAddSheet} title="Add sheet">+</button>
  </div>

  <div class="status-bar">
    <span>{activeSheetName()}</span>
    <span>{rangeLabel(currentRange)}</span>
    <span>Count {selectionStats.count}</span>
    {#if selectionStats.numericCount > 0}
      <span>Sum {formatStat(selectionStats.sum)}</span>
      <span>Avg {formatStat(selectionStats.average)}</span>
      <span>Min {formatStat(selectionStats.min)}</span>
      <span>Max {formatStat(selectionStats.max)}</span>
    {/if}
    <span class="status-spacer"></span>
    <span>{zoomPercent}%</span>
  </div>

  {#if activePanel}
    <div class="modal-backdrop" role="presentation" onclick={closePanel}>
      <section class="panel" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
        <header class="panel-header">
          <div>
            <p class="panel-kicker">
              {#if activePanel === 'functions'}Insert
              {:else if activePanel === 'find'}Edit
              {:else if activePanel === 'chart'}Analyze
              {:else if activePanel === 'pivot'}Analyze
              {:else if activePanel === 'validation'}Data
              {:else if activePanel === 'conditional'}Format
              {:else if activePanel === 'print'}File
              {:else if activePanel === 'protection'}Tools
              {:else if activePanel === 'comment'}Review
              {:else if activePanel === 'goalSeek'}Tools
              {:else}Help{/if}
            </p>
            <h2>
              {#if activePanel === 'functions'}Function Browser
              {:else if activePanel === 'find'}Find and Replace
              {:else if activePanel === 'chart'}Chart Builder
              {:else if activePanel === 'pivot'}Pivot Table
              {:else if activePanel === 'validation'}Validate Range
              {:else if activePanel === 'conditional'}Conditional Formatting
              {:else if activePanel === 'print'}Print and PDF
              {:else if activePanel === 'protection'}Protection
              {:else if activePanel === 'comment'}Cell Comment
              {:else if activePanel === 'goalSeek'}Goal Seek
              {:else if activePanel === 'shortcuts'}Keyboard Shortcuts
              {:else}About 900Sheets{/if}
            </h2>
          </div>
          <button type="button" class="icon-btn" onclick={closePanel} aria-label="Close">×</button>
        </header>

        {#if activePanel === 'functions'}
          <div class="panel-body">
            <input class="panel-input" type="search" bind:value={functionSearch} placeholder="Search 174 functions by name or category" />
            <div class="function-list">
              {#each filteredFormulaFunctions as fn}
                <button type="button" class="function-row" onclick={() => insertFunction(fn.name)}>
                  <span class="function-name">{fn.name}</span>
                  <span class="function-category">{fn.category}</span>
                </button>
              {/each}
            </div>
          </div>
        {:else if activePanel === 'find'}
          <div class="panel-body form-grid">
            <label>Find<input class="panel-input" bind:value={findQuery} placeholder="Text or value" /></label>
            <label>Replace with<input class="panel-input" bind:value={replaceValue} placeholder="Replacement" /></label>
            <label class="check-row"><input type="checkbox" bind:checked={findMatchCase} /> Match case</label>
            <div class="panel-actions">
              <button type="button" class="primary-btn" onclick={runFind}>Find</button>
              <button type="button" class="secondary-btn" onclick={runReplace}>Replace All</button>
            </div>
            <div class="result-list">
              {#each findResults as result}
                <button type="button" class="result-row" onclick={() => jumpToResult(result)}>
                  <span>{cellKey(result.row, result.col)}</span>
                  <span>{result.matched_text}</span>
                </button>
              {/each}
            </div>
          </div>
        {:else if activePanel === 'chart'}
          <div class="panel-body form-grid">
            <label>Title<input class="panel-input" bind:value={chartTitle} /></label>
            <label>Type
              <select class="panel-input" bind:value={chartType}>
                <option>Column</option>
                <option>Bar</option>
                <option>Line</option>
                <option>Area</option>
                <option>Pie</option>
                <option>Doughnut</option>
                <option>Scatter</option>
              </select>
            </label>
            <label>Series name<input class="panel-input" bind:value={chartSeriesName} /></label>
            <p class="panel-note">Uses the first selected column as labels and the second selected column as values. Current range: {rangeLabel(currentRange)}.</p>
            <div class="panel-actions">
              <button type="button" class="primary-btn" onclick={runChart}>Create Chart</button>
            </div>
            {#if chartSvg}
              <div class="chart-preview">{@html chartSvg}</div>
            {/if}
          </div>
        {:else if activePanel === 'pivot'}
          <div class="panel-body form-grid">
            <p class="panel-note">Uses the first detected column as the row field and the last detected column as the value field. Current range: {rangeLabel(currentRange)}.</p>
            <label>Aggregation
              <select class="panel-input" bind:value={pivotAggregation}>
                <option>Sum</option>
                <option>Count</option>
                <option>Average</option>
                <option>Min</option>
                <option>Max</option>
                <option>Product</option>
              </select>
            </label>
            <div class="chip-row">
              {#each pivotColumns as col}
                <span class="chip">{col.name || colLabel(col.col)}</span>
              {/each}
            </div>
            <div class="panel-actions">
              <button type="button" class="primary-btn" onclick={runPivotSheet}>Create Pivot Sheet</button>
            </div>
          </div>
        {:else if activePanel === 'validation'}
          <div class="panel-body form-grid">
            <label>Rule type
              <select class="panel-input" bind:value={validationType}>
                <option>WholeNumber</option>
                <option>Decimal</option>
                <option>TextLength</option>
                <option>List</option>
                <option>Date</option>
                <option>Time</option>
                <option>Custom</option>
              </select>
            </label>
            <label>Operator
              <select class="panel-input" bind:value={validationOperator}>
                <option>Between</option>
                <option>NotBetween</option>
                <option>Equal</option>
                <option>NotEqual</option>
                <option>GreaterThan</option>
                <option>LessThan</option>
                <option>GreaterThanOrEqual</option>
                <option>LessThanOrEqual</option>
              </select>
            </label>
            {#if validationType === 'List'}
              <label>Allowed values<input class="panel-input" bind:value={validationSource} placeholder="Open,Closed,Pending" /></label>
            {:else}
              <label>Value 1<input class="panel-input" bind:value={validationFormula1} /></label>
              <label>Value 2<input class="panel-input" bind:value={validationFormula2} /></label>
            {/if}
            <div class="panel-actions">
              <button type="button" class="primary-btn" onclick={runValidation}>Validate {rangeLabel(currentRange)}</button>
            </div>
            <div class="result-list">
              {#each validationResults as result}
                <button type="button" class="result-row" onclick={() => jumpToResult(result)}>
                  <span>{cellKey(result.row, result.col)}</span>
                  <span>{result.error}</span>
                </button>
              {/each}
            </div>
          </div>
        {:else if activePanel === 'conditional'}
          <div class="panel-body form-grid">
            <label>Condition
              <select class="panel-input" bind:value={conditionalType}>
                <option>CellValue</option>
                <option>TextContains</option>
                <option>Blanks</option>
                <option>NoBlanks</option>
                <option>Duplicate</option>
              </select>
            </label>
            <label>Operator
              <select class="panel-input" bind:value={conditionalOperator}>
                <option>GreaterThan</option>
                <option>LessThan</option>
                <option>Equal</option>
                <option>NotEqual</option>
                <option>Between</option>
                <option>NotBetween</option>
              </select>
            </label>
            <label>Value<input class="panel-input" bind:value={conditionalValue1} /></label>
            <label>Second value<input class="panel-input" bind:value={conditionalValue2} /></label>
            <label>Fill color<input class="panel-input" type="color" bind:value={conditionalFill} /></label>
            <div class="panel-actions">
              <button type="button" class="primary-btn" onclick={applyConditionalFormat}>Apply to Matches</button>
            </div>
            <p class="panel-note">Applied to {conditionalMatches.length} cells. These are static formats based on the current values.</p>
          </div>
        {:else if activePanel === 'print'}
          <div class="panel-body form-grid">
            <label>Page size
              <select class="panel-input" bind:value={printPageSize}>
                <option>Letter</option>
                <option>A4</option>
                <option>A3</option>
                <option>Legal</option>
                <option>Tabloid</option>
              </select>
            </label>
            <label>Orientation
              <select class="panel-input" bind:value={printOrientation}>
                <option>Portrait</option>
                <option>Landscape</option>
              </select>
            </label>
            <p class="panel-note">Print area uses the current selection when more than one cell is selected.</p>
            <div class="panel-actions">
              <button type="button" class="secondary-btn" onclick={updatePrintPreview}>Estimate Pages</button>
              <button type="button" class="primary-btn" onclick={handleExportPdf}>Export PDF</button>
            </div>
            {#if printPageCount !== null}
              <p class="panel-note">{printPageCount} page{printPageCount === 1 ? '' : 's'} estimated.</p>
            {/if}
          </div>
        {:else if activePanel === 'protection'}
          <div class="panel-body form-grid">
            <label>Password<input class="panel-input" type="password" bind:value={protectionPassword} /></label>
            <div class="panel-actions">
              <button type="button" class="primary-btn" onclick={() => setProtection(true)}>Protect Sheet</button>
              <button type="button" class="secondary-btn" onclick={() => setProtection(false)}>Unprotect Sheet</button>
            </div>
            <div class="panel-actions">
              <button type="button" class="secondary-btn" onclick={() => setRangeLock(true)}>Lock {rangeLabel(currentRange)}</button>
              <button type="button" class="secondary-btn" onclick={() => setRangeLock(false)}>Unlock {rangeLabel(currentRange)}</button>
            </div>
          </div>
        {:else if activePanel === 'comment'}
          <div class="panel-body form-grid">
            <p class="panel-note">Selected cell: {cellKey(selectedRow, selectedCol)}</p>
            <label>Author<input class="panel-input" bind:value={commentAuthor} /></label>
            <label>Comment<textarea class="panel-input" bind:value={commentText}></textarea></label>
            <div class="panel-actions">
              <button type="button" class="primary-btn" onclick={saveComment}>Save Comment</button>
              <button type="button" class="secondary-btn" onclick={removeCurrentComment}>Remove</button>
            </div>
            <div class="result-list">
              {#each allComments as comment}
                <button type="button" class="result-row" onclick={() => jumpToResult(comment)}>
                  <span>{cellKey(comment.row, comment.col)}</span>
                  <span>{comment.text}</span>
                </button>
              {/each}
            </div>
          </div>
        {:else if activePanel === 'goalSeek'}
          <div class="panel-body form-grid">
            <label>Formula cell<input class="panel-input" bind:value={goalSeekTargetCell} placeholder="B1" /></label>
            <label>Target value<input class="panel-input" bind:value={goalSeekTargetValue} placeholder="100" /></label>
            <label>Changing cell<input class="panel-input" bind:value={goalSeekInputCell} placeholder="A1" /></label>
            <div class="panel-actions">
              <button type="button" class="primary-btn" onclick={runGoalSeek}>Run Goal Seek</button>
            </div>
            {#if goalSeekResult}
              <p class="panel-note">
                {goalSeekResult.success ? 'Solved' : 'Not solved'} after {goalSeekResult.iterations} iterations.
                Input {formatStat(goalSeekResult.input_value)}, achieved {formatStat(goalSeekResult.achieved_value)}.
              </p>
            {/if}
          </div>
        {:else if activePanel === 'shortcuts'}
          <div class="panel-body shortcut-grid">
            <span>Ctrl+Z</span><span>Undo</span>
            <span>Ctrl+Y</span><span>Redo</span>
            <span>Ctrl+C / X / V</span><span>Copy, cut, paste</span>
            <span>Ctrl+B / I / U</span><span>Bold, italic, underline</span>
            <span>Ctrl+F</span><span>Find and replace</span>
            <span>Delete</span><span>Clear selection</span>
            <span>F2</span><span>Edit active cell</span>
          </div>
        {:else if activePanel === 'about'}
          <div class="panel-body">
            <p class="panel-note">900Sheets is a local-first spreadsheet app from 900 Labs. This build runs offline, stores workbook state locally, and exposes spreadsheet workflows without accounts or subscriptions.</p>
            <div class="chip-row">
              <span class="chip">Tauri</span>
              <span class="chip">Svelte</span>
              <span class="chip">Rust engine</span>
              <span class="chip">Offline-first</span>
            </div>
          </div>
        {/if}
      </section>
    </div>
  {/if}
</div>
