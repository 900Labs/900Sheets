<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { open, save } from '@tauri-apps/plugin-dialog'
  import { onMount } from 'svelte'
  import type { SheetInfo, CellData, CellRange, ClipboardData, HistoryEntry, CellFormat, CellFormatMap } from './lib/types'
  import { colLabel, cellKey, normalizeRange, rangeContains, rangeSize, rangeLabel, parseCellKey } from './lib/utils/grid'
  import { UndoRedoStack } from './lib/utils/undoRedo'

  function focusInput(node: HTMLInputElement, selectText: boolean = true) {
    node.focus()
    if (selectText) {
      node.select()
    } else {
      const cursorPosition = node.value.length
      node.setSelectionRange(cursorPosition, cursorPosition)
    }
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
  let selectEditTextOnFocus: boolean = $state(true)
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
  type ToolbarMenuKey = 'data' | 'analyze' | 'output'
  type PanelKey =
    | 'functions' | 'find' | 'chart' | 'pivot' | 'validation' | 'conditional' | 'print' | 'protection'
    | 'comment' | 'goalSeek' | 'filter' | 'namedRanges' | 'structure' | 'templates' | 'shortcuts' | 'about'
  type MenuAction =
    | 'newWorkbook' | 'templates' | 'openXlsx' | 'importCsv' | 'importJson' | 'saveXlsx' | 'exportCsv' | 'exportJson' | 'exportPdf'
    | 'undo' | 'redo' | 'cut' | 'copy' | 'paste' | 'delete' | 'findReplace'
    | 'toggleFormulaBar' | 'toggleGridlines' | 'toggleCompact' | 'zoomIn' | 'zoomOut' | 'zoomReset' | 'freezePanes' | 'unfreezePanes'
    | 'functions' | 'addSheet' | 'comment' | 'chart' | 'pivot' | 'structure' | 'insertRow' | 'deleteRow' | 'insertColumn' | 'deleteColumn'
    | 'bold' | 'italic' | 'underline' | 'strike' | 'alignLeft' | 'alignCenter' | 'alignRight' | 'wrapText' | 'fillYellow' | 'fillGreen' | 'fillRed' | 'textBlue' | 'textRed' | 'conditional'
    | 'sortAsc' | 'sortDesc' | 'filter' | 'clearFilter' | 'removeDuplicates' | 'namedRanges' | 'validation'
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

  interface ValidationDefinition {
    validation_type: string
    operator: string
    formula1: string | null
    formula2: string | null
    source: string | null
    allow_blank: boolean
    show_dropdown: boolean
    error_style: string
    error_title: string | null
    error_message: string | null
    prompt_title: string | null
    prompt_message: string | null
  }

  interface StoredValidationRule {
    id: string
    label: string
    range: CellRange
    validation: ValidationDefinition
  }

  interface ConditionalRuleDefinition {
    id: string
    condition_type: string
    range: [number, number, number, number]
    operator: string | null
    value1: string | null
    value2: string | null
    format: CellFormat
    color_scale_stops: unknown[]
    bar_color: string | null
    show_bar_value: boolean
    icon_set_type: string | null
    is_top: boolean
    rank: number
    is_above_average: boolean
    std_dev: number
    priority: number
    stop_if_true: boolean
  }

  interface StoredConditionalRule {
    id: string
    label: string
    rule: ConditionalRuleDefinition
  }

  interface NamedRange {
    id: string
    name: string
    range: CellRange
  }

  interface FunctionHelp {
    syntax: string
    description: string
    example: string
  }

  let openMenu: MenuKey | null = $state(null)
  let toolbarMenuOpen: ToolbarMenuKey | null = $state(null)
  let toolbarMenuX: number = $state(0)
  let toolbarMenuY: number = $state(0)
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
  let chartColumns: ColumnInfo[] = $state([])
  let chartXColumn: string = $state('')
  let chartYColumn: string = $state('')
  let chartLegendPosition: string = $state('Bottom')
  let chartSvg: string = $state('')
  let pivotAggregation: string = $state('Sum')
  let pivotColumns: ColumnInfo[] = $state([])
  let pivotRowColumn: string = $state('')
  let pivotColumnColumn: string = $state('')
  let pivotValueColumn: string = $state('')
  let pivotFilterColumn: string = $state('')
  let pivotFilterValue: string = $state('')
  let filterColumn: string = $state('')
  let filterCondition: string = $state('Contains')
  let filterValue: string = $state('')
  let filterHasHeader: boolean = $state(true)
  let hiddenRows: Record<number, boolean> = $state({})
  let activeFilterLabel: string = $state('')
  let namedRangeName: string = $state('')
  let namedRanges: NamedRange[] = $state([])
  let validationRuleName: string = $state('Rule 1')
  let validationType: string = $state('WholeNumber')
  let validationOperator: string = $state('Between')
  let validationFormula1: string = $state('0')
  let validationFormula2: string = $state('100')
  let validationSource: string = $state('')
  let validationResults: ValidationErrorData[] = $state([])
  let validationRules: StoredValidationRule[] = $state([])
  let conditionalRuleName: string = $state('Highlight rule')
  let conditionalType: string = $state('CellValue')
  let conditionalOperator: string = $state('GreaterThan')
  let conditionalValue1: string = $state('0')
  let conditionalValue2: string = $state('')
  let conditionalFill: string = $state('#fef3c7')
  let conditionalMatches: Array<[number, number]> = $state([])
  let conditionalRules: StoredConditionalRule[] = $state([])
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
  let frozenRowCount: number = $state(0)
  let frozenColCount: number = $state(0)
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

  const FUNCTION_HELP: Record<string, FunctionHelp> = {
    SUM: { syntax: 'SUM(number1, [number2], ...)', description: 'Adds numbers or ranges.', example: '=SUM(B2:B12)' },
    AVERAGE: { syntax: 'AVERAGE(number1, [number2], ...)', description: 'Returns the arithmetic mean.', example: '=AVERAGE(C2:C12)' },
    COUNT: { syntax: 'COUNT(value1, [value2], ...)', description: 'Counts numeric values.', example: '=COUNT(D2:D100)' },
    COUNTA: { syntax: 'COUNTA(value1, [value2], ...)', description: 'Counts non-empty values.', example: '=COUNTA(A2:A100)' },
    IF: { syntax: 'IF(condition, value_if_true, value_if_false)', description: 'Returns one value when a condition is true and another when false.', example: '=IF(C2>0,"Open","Closed")' },
    VLOOKUP: { syntax: 'VLOOKUP(search_key, range, index, [is_sorted])', description: 'Looks up a value in the first column of a range.', example: '=VLOOKUP(A2,Products!A:D,4,FALSE)' },
    INDEX: { syntax: 'INDEX(range, row, [column])', description: 'Returns the value at a row and column inside a range.', example: '=INDEX(B2:D20,3,2)' },
    MATCH: { syntax: 'MATCH(search_key, range, [search_type])', description: 'Returns a value position within a range.', example: '=MATCH("Rice",A2:A50,0)' },
    TEXTJOIN: { syntax: 'TEXTJOIN(delimiter, ignore_empty, text1, ...)', description: 'Joins text values with a delimiter.', example: '=TEXTJOIN(", ",TRUE,A2:A5)' },
    PMT: { syntax: 'PMT(rate, periods, present_value)', description: 'Calculates a loan payment.', example: '=PMT(8%/12,24,5000)' },
    TODAY: { syntax: 'TODAY()', description: 'Returns the current date.', example: '=TODAY()' },
    ROUND: { syntax: 'ROUND(value, places)', description: 'Rounds a number to a fixed number of decimal places.', example: '=ROUND(B2,2)' },
  }

  const TEMPLATES: Record<string, { title: string; rows: string[][] }> = {
    budget: {
      title: 'Household Budget',
      rows: [
        ['Category', 'Planned', 'Actual', 'Difference'],
        ['Food', '250', '230', '=B2-C2'],
        ['Transport', '80', '96', '=B3-C3'],
        ['Utilities', '120', '118', '=B4-C4'],
        ['Savings', '100', '75', '=B5-C5'],
        ['Total', '=SUM(B2:B5)', '=SUM(C2:C5)', '=SUM(D2:D5)'],
      ],
    },
    inventory: {
      title: 'Inventory Tracker',
      rows: [
        ['Item', 'Category', 'In stock', 'Reorder level', 'Status'],
        ['Rice 10kg', 'Food', '42', '20', '=IF(C2<D2,"Reorder","OK")'],
        ['Soap', 'Household', '18', '24', '=IF(C3<D3,"Reorder","OK")'],
        ['Exercise books', 'School', '67', '30', '=IF(C4<D4,"Reorder","OK")'],
      ],
    },
    attendance: {
      title: 'Attendance Register',
      rows: [
        ['Name', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Present Days'],
        ['Student 1', '1', '1', '0', '1', '1', '=SUM(B2:F2)'],
        ['Student 2', '1', '0', '1', '1', '0', '=SUM(B3:F3)'],
        ['Student 3', '1', '1', '1', '1', '1', '=SUM(B4:F4)'],
      ],
    },
    invoice: {
      title: 'Simple Invoice',
      rows: [
        ['Description', 'Quantity', 'Unit Price', 'Line Total'],
        ['Service', '1', '50', '=B2*C2'],
        ['Materials', '3', '12', '=B3*C3'],
        ['Delivery', '1', '8', '=B4*C4'],
        ['', '', 'Total', '=SUM(D2:D4)'],
      ],
    },
  }

  const MENU_DEFINITIONS: MenuDefinition[] = [
    {
      key: 'file',
      label: 'File',
      sections: [
        [
          { label: 'New Workbook', action: 'newWorkbook', shortcut: 'Ctrl+N' },
          { label: 'New from Template...', action: 'templates' },
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
          { label: 'Freeze Panes at Selection', action: 'freezePanes' },
          { label: 'Unfreeze Panes', action: 'unfreezePanes' },
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
          { label: 'Rows / Columns...', action: 'structure' },
          { label: 'Insert Row Above', action: 'insertRow' },
          { label: 'Insert Column Left', action: 'insertColumn' },
          { label: 'Delete Row', action: 'deleteRow' },
          { label: 'Delete Column', action: 'deleteColumn' },
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
          { label: 'Filter...', action: 'filter' },
          { label: 'Clear Filter', action: 'clearFilter' },
          { label: 'Remove Duplicates', action: 'removeDuplicates' },
          { label: 'Named Ranges...', action: 'namedRanges' },
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

  let displayedRows: number[] = $derived(buildDisplayedRows())
  let frozenDisplayRows: number[] = $derived(displayedRows.filter((row) => row < frozenRowCount))
  let scrollableDisplayRows: number[] = $derived(displayedRows.filter((row) => row >= frozenRowCount))
  let visibleRowStart: number = $derived(Math.floor(scrollTop / ROW_HEIGHT))
  let visibleRowEnd: number = $derived(Math.min(visibleRowStart + VISIBLE_ROWS + 5, scrollableDisplayRows.length))
  let visibleRows: number[] = $derived([
    ...frozenDisplayRows,
    ...scrollableDisplayRows.slice(visibleRowStart, visibleRowEnd),
  ])
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

  function buildDisplayedRows(): number[] {
    const rows: number[] = []
    for (let row = 0; row < ROWS; row++) {
      if (!hiddenRows[row]) rows.push(row)
    }
    return rows
  }

  function rangeToTuple(range: CellRange): [number, number, number, number] {
    const r = normalizeRange(range)
    return [r.startRow, r.startCol, r.endRow, r.endCol]
  }

  function tupleToRange(range: [number, number, number, number]): CellRange {
    return {
      startRow: range[0],
      startCol: range[1],
      endRow: range[2],
      endCol: range[3],
    }
  }

  function cellInRange(range: CellRange, row: number, col: number): boolean {
    const r = normalizeRange(range)
    return row >= r.startRow && row <= r.endRow && col >= r.startCol && col <= r.endCol
  }

  function functionHelp(name: string): FunctionHelp {
    return FUNCTION_HELP[name] ?? {
      syntax: `${name}(...)`,
      description: 'Available in the formula engine. Use the function name followed by arguments in parentheses.',
      example: `=${name}()`,
    }
  }

  function cssDecls(styles: Array<string | null | undefined>): string {
    return styles.filter(Boolean).join('; ')
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

  function clearLocalCell(row: number, col: number) {
    const key = cellKey(row, col)
    delete cellContents[key]
    delete cellDisplays[key]
    delete cellFormats[key]
  }

  async function applyCellValueChanges(
    changes: Array<{ row: number; col: number; value: string }>,
    context: string,
    successMessage?: string,
  ) {
    const byKey = new Map<string, { row: number; col: number; value: string }>()
    for (const change of changes) {
      if (change.row < 0 || change.row >= ROWS || change.col < 0 || change.col >= COLS) continue
      byKey.set(cellKey(change.row, change.col), change)
    }

    const nextContents = { ...cellContents }
    const nextDisplays = { ...cellDisplays }
    const history: HistoryEntry[] = []
    const commands: Promise<unknown>[] = []

    for (const [key, change] of byKey.entries()) {
      const oldValue = cellContents[key] ?? ''
      if (oldValue === change.value) continue
      history.push({
        sheetId: activeSheetId,
        row: change.row,
        col: change.col,
        oldValue,
        newValue: change.value,
      })
      if (change.value) {
        nextContents[key] = change.value
        nextDisplays[key] = change.value.startsWith('=') ? (cellDisplays[key] ?? change.value) : change.value
        commands.push(invoke('set_cell', {
          sheetId: activeSheetId,
          row: change.row,
          col: change.col,
          value: change.value,
        }))
      } else {
        delete nextContents[key]
        delete nextDisplays[key]
        commands.push(invoke('clear_cell', {
          sheetId: activeSheetId,
          row: change.row,
          col: change.col,
        }))
      }
    }

    if (history.length === 0) {
      if (successMessage) setStatus(successMessage)
      return
    }

    cellContents = nextContents
    cellDisplays = nextDisplays
    undoRedo.push(history)
    updateUndoRedoState()

    try {
      await Promise.all(commands)
      if (history.some((entry) => entry.newValue.startsWith('='))) {
        await refreshSheetData()
      }
      if (successMessage) setStatus(successMessage)
    } catch (e) {
      setError(e, context)
      await refreshSheetData()
    }
  }

  async function replaceSheetSnapshot(
    nextContents: Record<string, string>,
    nextDisplays: Record<string, string>,
    nextFormats: CellFormatMap,
    context: string,
    successMessage: string,
  ) {
    const keys = new Set([...Object.keys(cellContents), ...Object.keys(nextContents)])
    const history: HistoryEntry[] = []
    const valueCommands: Promise<unknown>[] = []

    for (const key of keys) {
      const { row, col } = parseCellKey(key)
      const oldValue = cellContents[key] ?? ''
      const newValue = nextContents[key] ?? ''
      if (oldValue !== newValue) {
        history.push({ sheetId: activeSheetId, row, col, oldValue, newValue })
      }
      if (newValue) {
        valueCommands.push(invoke('set_cell', { sheetId: activeSheetId, row, col, value: newValue }))
      } else if (oldValue) {
        valueCommands.push(invoke('clear_cell', { sheetId: activeSheetId, row, col }))
      }
    }

    const formatKeys = new Set([...Object.keys(cellFormats), ...Object.keys(nextFormats)])
    const formatCommands: Promise<unknown>[] = []
    for (const key of formatKeys) {
      const { row, col } = parseCellKey(key)
      const nextFormat = nextFormats[key] ?? {}
      formatCommands.push(invoke('set_cell_format', {
        sheetId: activeSheetId,
        row,
        col,
        format: nextFormat,
      }))
    }

    cellContents = nextContents
    cellDisplays = nextDisplays
    cellFormats = nextFormats
    if (history.length > 0) {
      undoRedo.push(history)
      updateUndoRedoState()
    }

    try {
      await Promise.all([...valueCommands, ...formatCommands])
      if (Object.values(nextContents).some((value) => value.startsWith('='))) {
        await refreshSheetData()
      }
      setStatus(successMessage)
    } catch (e) {
      setError(e, context)
      await refreshSheetData()
    }
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

  function getEffectiveCellFormat(row: number, col: number): CellFormat {
    let fmt: CellFormat = { ...getCellFormat(row, col) }
    for (const stored of conditionalRules) {
      if (!conditionalApplies(stored.rule, row, col)) continue
      fmt = { ...fmt, ...stored.rule.format }
    }
    return fmt
  }

  function getCellStyle(row: number, col: number): string {
    const fmt = getEffectiveCellFormat(row, col)
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
    return cssDecls([...styles, getFreezeStyle(row, col)])
  }

  function getFreezeStyle(row: number, col: number): string {
    const styles: string[] = []
    const isFrozenRow = row < frozenRowCount
    const isFrozenCol = col < frozenColCount
    if (isFrozenRow) {
      const frozenIndex = frozenDisplayRows.indexOf(row)
      styles.push('position: sticky')
      styles.push(`top: ${HEADER_HEIGHT + Math.max(0, frozenIndex) * ROW_HEIGHT}px`)
    }
    if (isFrozenCol) {
      styles.push('position: sticky')
      styles.push(`left: ${COL_WIDTH * 0.6 + col * COL_WIDTH}px`)
    }
    if (isFrozenRow || isFrozenCol) {
      styles.push(`z-index: ${isFrozenRow && isFrozenCol ? 6 : 4}`)
      styles.push('background: var(--bg)')
    }
    return styles.join('; ')
  }

  function getRowHeaderStyle(row: number): string {
    if (row >= frozenRowCount) return `height: ${ROW_HEIGHT}px`
    const frozenIndex = frozenDisplayRows.indexOf(row)
    return cssDecls([
      `height: ${ROW_HEIGHT}px`,
      'top: ' + (HEADER_HEIGHT + Math.max(0, frozenIndex) * ROW_HEIGHT) + 'px',
      'z-index: 7',
    ])
  }

  function getColHeaderStyle(col: number): string {
    if (col >= frozenColCount) return ''
    return cssDecls([
      `left: ${COL_WIDTH * 0.6 + col * COL_WIDTH}px`,
      'z-index: 7',
    ])
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
    toolbarMenuOpen = null
  }

  function toggleToolbarMenu(menu: ToolbarMenuKey, event: MouseEvent) {
    const trigger = event.currentTarget as HTMLElement
    const rect = trigger.getBoundingClientRect()
    const toolbarRect = trigger.closest('.format-toolbar')?.getBoundingClientRect()
    const menuWidth = 190
    toolbarMenuX = Math.max(8, Math.min(rect.left, window.innerWidth - menuWidth - 8))
    toolbarMenuY = Math.max(rect.bottom + 4, (toolbarRect?.bottom ?? rect.bottom) + 4)
    toolbarMenuOpen = toolbarMenuOpen === menu ? null : menu
    openMenu = null
    formulaMenuOpen = false
  }

  function closeMenus() {
    openMenu = null
    toolbarMenuOpen = null
  }

  function closePopovers() {
    openMenu = null
    toolbarMenuOpen = null
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
    toolbarMenuOpen = null
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
      loadChartColumns()
    }
    if (panel === 'filter') {
      const r = normalizeRange(currentRange)
      filterColumn = String(selectedCol >= r.startCol && selectedCol <= r.endCol ? selectedCol : r.startCol)
      filterValue = ''
    }
    if (panel === 'namedRanges') {
      namedRangeName = nextNamedRangeName()
    }
    if (panel === 'validation') {
      validationRuleName = `Validation ${validationRules.length + 1}`
    }
    if (panel === 'conditional') {
      conditionalRuleName = `Highlight ${conditionalRules.length + 1}`
    }
    if (panel === 'print') {
      printPageCount = null
    }
  }

  function closePanel() {
    activePanel = null
  }

  function resetWorkbookSessionState() {
    hiddenRows = {}
    activeFilterLabel = ''
    namedRanges = []
    validationRules = []
    conditionalRules = []
    conditionalMatches = []
    validationResults = []
    frozenRowCount = 0
    frozenColCount = 0
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

  function startEdit(row: number, col: number, initialValue?: string) {
    editingCell = cellKey(row, col)
    editValue = initialValue ?? getCellValue(row, col)
    selectEditTextOnFocus = initialValue === undefined
  }

  function commitEdit() {
    if (editingCell) {
      const key = editingCell
      const { row, col } = parseCellKey(key)
      const oldValue = cellContents[key] ?? ''
      if (editValue !== oldValue) {
        const validationError = validationMessageForValue(row, col, editValue)
        if (validationError) {
          setError(validationError, 'Validation failed')
          return
        }
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
            clearLocalCell(r2, c2)
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
        const validationError = validationMessageForValue(targetRow, targetCol, newValue)
        if (validationError) {
          setError(validationError, 'Paste skipped invalid cell')
          continue
        }
        if (oldValue !== newValue) {
          history.push({ sheetId: activeSheetId, row: targetRow, col: targetCol, oldValue, newValue })
          cellContents[key] = newValue
          cellDisplays[key] = newValue.startsWith('=') ? (cellDisplays[key] ?? newValue) : newValue
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
          const validationError = validationMessageForValue(targetRow, targetCol, newValue)
          if (validationError) {
            setError(validationError, 'Paste skipped invalid cell')
            continue
          }
          if (oldValue !== newValue) {
            history.push({ sheetId: activeSheetId, row: targetRow, col: targetCol, oldValue, newValue })
            cellContents[key] = newValue
            cellDisplays[key] = newValue.startsWith('=') ? (cellDisplays[key] ?? newValue) : newValue
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
        const oldValue = cellContents[key] ?? cellDisplays[key] ?? ''
        if (oldValue) {
          history.push({ sheetId: activeSheetId, row, col, oldValue, newValue: '' })
          clearLocalCell(row, col)
          queueClearCell(row, col, 'Unable to delete cells')
        }
      }
    }
    if (history.length > 0) {
      undoRedo.push(history)
      updateUndoRedoState()
      setStatus(`Cleared ${history.length} cell${history.length === 1 ? '' : 's'}`)
    }
  }

  function isClearSelectionKey(e: KeyboardEvent): boolean {
    return e.key === 'Delete'
      || e.key === 'Backspace'
      || e.key === 'Del'
      || e.code === 'Delete'
      || e.code === 'Backspace'
  }

  function handleGridKeydown(e: KeyboardEvent) {
    if (editingCell) return
    if (isClearSelectionKey(e)) {
      e.preventDefault()
      e.stopPropagation()
      deleteSelection()
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

    if (isClearSelectionKey(e)) {
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
      startEdit(selectedRow, selectedCol, e.key)
    }
  }

  function handleFormulaBarKeydown(e: KeyboardEvent) {
    e.stopPropagation()
    if (e.key === 'Enter') {
      e.preventDefault()
      const key = cellKey(selectedRow, selectedCol)
      const oldValue = cellContents[key] ?? ''
      if (formulaBarValue !== oldValue) {
        const validationError = validationMessageForValue(selectedRow, selectedCol, formulaBarValue)
        if (validationError) {
          setError(validationError, 'Validation failed')
          return
        }
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
    hiddenRows = {}
    activeFilterLabel = ''
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
      resetWorkbookSessionState()
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
      resetWorkbookSessionState()
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
      resetWorkbookSessionState()
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
      resetWorkbookSessionState()
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
    const xCol = Number(chartXColumn)
    const yCol = Number(chartYColumn)
    if (xCol === yCol || r.startRow === r.endRow) {
      setError('Choose different label and value columns from a range with headers and data rows', 'Chart unavailable')
      return
    }
    if (!Number.isInteger(xCol) || !Number.isInteger(yCol) || xCol < r.startCol || xCol > r.endCol || yCol < r.startCol || yCol > r.endCol) {
      setError('Choose chart columns inside the selected range', 'Chart unavailable')
      return
    }
    const yLabel = chartColumns.find((col) => col.col === yCol)?.name ?? colLabel(yCol)
    const xLabel = chartColumns.find((col) => col.col === xCol)?.name ?? colLabel(xCol)
    try {
      const result = await invoke<ChartResult>('create_chart', {
        sheetId: activeSheetId,
        config: {
          title: chartTitle || 'Chart',
          chart_type: chartType,
          series: [{
            name: chartSeriesName || yLabel,
            x_column: xCol,
            y_column: yCol,
            color: null,
          }],
          header_row: r.startRow,
          data_start_row: Math.min(r.startRow + 1, r.endRow),
          data_end_row: r.endRow,
          x_axis_label: xLabel,
          y_axis_label: yLabel,
          legend_position: chartLegendPosition,
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
      pivotRowColumn = String(pivotColumns[0]?.col ?? r.startCol)
      pivotValueColumn = String(pivotColumns[pivotColumns.length - 1]?.col ?? r.endCol)
      pivotColumnColumn = ''
      pivotFilterColumn = ''
      pivotFilterValue = ''
    } catch {
      pivotColumns = []
      pivotRowColumn = String(r.startCol)
      pivotValueColumn = String(r.endCol)
    }
  }

  async function loadChartColumns() {
    const r = normalizeRange(currentRange)
    try {
      chartColumns = await invoke<ColumnInfo[]>('get_pivot_columns', {
        sheetId: activeSheetId,
        headerRow: r.startRow,
        startCol: r.startCol,
        endCol: r.endCol,
      })
      chartXColumn = String(chartColumns[0]?.col ?? r.startCol)
      chartYColumn = String(chartColumns[1]?.col ?? Math.min(r.startCol + 1, r.endCol))
      chartSeriesName = chartColumns.find((col) => String(col.col) === chartYColumn)?.name ?? 'Series 1'
    } catch {
      chartColumns = []
      chartXColumn = String(r.startCol)
      chartYColumn = String(Math.min(r.startCol + 1, r.endCol))
    }
  }

  async function runPivotSheet() {
    const r = normalizeRange(currentRange)
    if (r.startRow === r.endRow || r.startCol === r.endCol) {
      setError('Select a table with headers, row labels, and at least one value column', 'Pivot unavailable')
      return
    }
    const rowCol = Number(pivotRowColumn)
    const valueCol = Number(pivotValueColumn)
    const columnCol = pivotColumnColumn ? Number(pivotColumnColumn) : null
    const filterCol = pivotFilterColumn ? Number(pivotFilterColumn) : null
    if (!Number.isInteger(rowCol) || !Number.isInteger(valueCol)) {
      setError('Choose row and value fields before creating a pivot', 'Pivot unavailable')
      return
    }
    try {
      const result = await invoke<SheetInfo[]>('create_pivot_sheet', {
        sheetId: activeSheetId,
        config: {
          source_sheet: activeSheetId,
          data_range: [r.startRow, r.startCol, r.endRow, r.endCol],
          header_row: r.startRow,
          row_fields: [{ column: rowCol, label: pivotColumns.find((c) => c.col === rowCol)?.name ?? colLabel(rowCol) }],
          column_fields: columnCol == null ? [] : [{ column: columnCol, label: pivotColumns.find((c) => c.col === columnCol)?.name ?? colLabel(columnCol) }],
          value_fields: [{
            column: valueCol,
            label: pivotColumns.find((c) => c.col === valueCol)?.name ?? colLabel(valueCol),
            aggregation: pivotAggregation,
          }],
          filter_field: filterCol == null ? null : { column: filterCol, label: pivotColumns.find((c) => c.col === filterCol)?.name ?? colLabel(filterCol) },
          filter_values: pivotFilterValue.trim() ? pivotFilterValue.split(',').map((value) => value.trim()).filter(Boolean) : [],
        },
      })
      await loadSheetList(result, result[result.length - 1]?.id ?? activeSheetId)
      setStatus(`Created pivot sheet from ${rangeLabel(r)}`)
      closePanel()
    } catch (e) {
      setError(e, 'Pivot failed')
    }
  }

  function filterMatches(row: number, col: number): boolean {
    const raw = getCellValue(row, col)
    const display = getCellDisplay(row, col)
    const text = (display || raw).trim()
    const needle = filterValue.trim()
    const numeric = Number(text)
    const numericNeedle = Number(needle)

    switch (filterCondition) {
      case 'Equals': return text.localeCompare(needle, undefined, { sensitivity: 'accent' }) === 0
      case 'NotEquals': return text.localeCompare(needle, undefined, { sensitivity: 'accent' }) !== 0
      case 'Contains': return text.toLowerCase().includes(needle.toLowerCase())
      case 'StartsWith': return text.toLowerCase().startsWith(needle.toLowerCase())
      case 'EndsWith': return text.toLowerCase().endsWith(needle.toLowerCase())
      case 'GreaterThan': return Number.isFinite(numeric) && Number.isFinite(numericNeedle) && numeric > numericNeedle
      case 'LessThan': return Number.isFinite(numeric) && Number.isFinite(numericNeedle) && numeric < numericNeedle
      case 'Empty': return text === ''
      case 'NotEmpty': return text !== ''
      default: return true
    }
  }

  function applyFilter() {
    const r = normalizeRange(currentRange)
    const col = Number(filterColumn || r.startCol)
    if (!Number.isInteger(col) || col < r.startCol || col > r.endCol) {
      setError('Choose a filter column inside the selected range', 'Filter failed')
      return
    }
    const nextHidden: Record<number, boolean> = {}
    const firstDataRow = filterHasHeader ? r.startRow + 1 : r.startRow
    let hidden = 0
    for (let row = firstDataRow; row <= r.endRow; row++) {
      if (!filterMatches(row, col)) {
        nextHidden[row] = true
        hidden += 1
      }
    }
    hiddenRows = nextHidden
    activeFilterLabel = `${colLabel(col)} ${filterCondition}${filterCondition === 'Empty' || filterCondition === 'NotEmpty' ? '' : ` "${filterValue}"`}`
    setStatus(`Filter applied to ${rangeLabel(r)}; ${hidden} row${hidden === 1 ? '' : 's'} hidden`)
  }

  function clearFilter() {
    hiddenRows = {}
    activeFilterLabel = ''
    setStatus('Filter cleared')
  }

  async function removeDuplicateRows() {
    const r = normalizeRange(currentRange)
    if (r.endRow <= r.startRow) {
      setError('Select at least two rows before removing duplicates', 'Remove duplicates failed')
      return
    }

    const seen = new Set<string>()
    const keptRows: string[][] = []
    let duplicateCount = 0

    for (let row = r.startRow; row <= r.endRow; row++) {
      const values: string[] = []
      for (let col = r.startCol; col <= r.endCol; col++) {
        values.push(getCellValue(row, col))
      }
      const signature = values.map((value) => value.trim().toLowerCase()).join('\u0000')
      if (seen.has(signature)) {
        duplicateCount += 1
      } else {
        seen.add(signature)
        keptRows.push(values)
      }
    }

    if (duplicateCount === 0) {
      setStatus('No duplicate rows found')
      return
    }

    const changes: Array<{ row: number; col: number; value: string }> = []
    const width = r.endCol - r.startCol + 1
    for (let rowOffset = 0; rowOffset <= r.endRow - r.startRow; rowOffset++) {
      for (let colOffset = 0; colOffset < width; colOffset++) {
        changes.push({
          row: r.startRow + rowOffset,
          col: r.startCol + colOffset,
          value: keptRows[rowOffset]?.[colOffset] ?? '',
        })
      }
    }

    await applyCellValueChanges(
      changes,
      'Remove duplicates failed',
      `Removed ${duplicateCount} duplicate row${duplicateCount === 1 ? '' : 's'} from ${rangeLabel(r)}`,
    )
  }

  function nextNamedRangeName(): string {
    let index = namedRanges.length + 1
    let name = `Range_${index}`
    while (namedRanges.some((range) => range.name.toLowerCase() === name.toLowerCase())) {
      index += 1
      name = `Range_${index}`
    }
    return name
  }

  function addNamedRange() {
    const name = namedRangeName.trim()
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) {
      setError('Use letters, numbers, and underscores; start with a letter or underscore', 'Named range failed')
      return
    }
    if (parseCellAddress(name)) {
      setError('Named ranges cannot look like cell addresses', 'Named range failed')
      return
    }
    if (namedRanges.some((range) => range.name.toLowerCase() === name.toLowerCase())) {
      setError('A named range with that name already exists', 'Named range failed')
      return
    }
    namedRanges = [...namedRanges, { id: `named-${Date.now()}`, name, range: normalizeRange(currentRange) }]
    namedRangeName = nextNamedRangeName()
    setStatus(`Named ${rangeLabel(currentRange)} as ${name}`)
  }

  function removeNamedRange(id: string) {
    const removed = namedRanges.find((range) => range.id === id)
    namedRanges = namedRanges.filter((range) => range.id !== id)
    if (removed) setStatus(`Removed named range ${removed.name}`)
  }

  function selectNamedRange(range: NamedRange) {
    selectRange(range.range)
  }

  function selectRange(range: CellRange) {
    const r = normalizeRange(range)
    selectedRow = r.startRow
    selectedCol = r.startCol
    selectionStart = { row: r.startRow, col: r.startCol }
    selectionEnd = { row: r.endRow, col: r.endCol }
    formulaBarValue = getCellValue(r.startRow, r.startCol)
    if (gridContainerEl) {
      gridContainerEl.scrollTop = Math.max(0, r.startRow * ROW_HEIGHT - ROW_HEIGHT * 2)
      gridContainerEl.scrollLeft = Math.max(0, r.startCol * COL_WIDTH - COL_WIDTH)
    }
  }

  function insertNamedRangeReference(range: NamedRange) {
    const ref = rangeLabel(range.range)
    if (!editingCell) {
      startEdit(selectedRow, selectedCol)
      editValue = editValue || '='
    }
    editValue = editValue.endsWith('=') || editValue.endsWith('(') ? `${editValue}${ref}` : `${editValue}${ref}`
  }

  async function transformSheetCells(
    transform: (row: number, col: number) => { row: number; col: number } | null,
    context: string,
    successMessage: string,
  ) {
    const nextContents: Record<string, string> = {}
    const nextDisplays: Record<string, string> = {}
    const nextFormats: CellFormatMap = {}

    for (const [key, value] of Object.entries(cellContents)) {
      const { row, col } = parseCellKey(key)
      const target = transform(row, col)
      if (!target) continue
      nextContents[cellKey(target.row, target.col)] = value
    }

    for (const [key, value] of Object.entries(cellDisplays)) {
      const { row, col } = parseCellKey(key)
      const target = transform(row, col)
      if (!target) continue
      nextDisplays[cellKey(target.row, target.col)] = value
    }

    for (const [key, value] of Object.entries(cellFormats)) {
      const { row, col } = parseCellKey(key)
      const target = transform(row, col)
      if (!target) continue
      nextFormats[cellKey(target.row, target.col)] = value
    }

    await replaceSheetSnapshot(nextContents, nextDisplays, nextFormats, context, successMessage)
  }

  async function insertRowAbove() {
    const rowIndex = selectedRow
    await transformSheetCells((row, col) => {
      if (row < rowIndex) return { row, col }
      if (row >= ROWS - 1) return null
      return { row: row + 1, col }
    }, 'Insert row failed', `Inserted row above ${rowIndex + 1}`)
    selectCell(rowIndex, selectedCol)
  }

  async function deleteSelectedRow() {
    const rowIndex = selectedRow
    await transformSheetCells((row, col) => {
      if (row < rowIndex) return { row, col }
      if (row === rowIndex) return null
      return { row: row - 1, col }
    }, 'Delete row failed', `Deleted row ${rowIndex + 1}`)
    selectCell(Math.min(rowIndex, ROWS - 1), selectedCol)
  }

  async function insertColumnLeft() {
    const colIndex = selectedCol
    await transformSheetCells((row, col) => {
      if (col < colIndex) return { row, col }
      if (col >= COLS - 1) return null
      return { row, col: col + 1 }
    }, 'Insert column failed', `Inserted column before ${colLabel(colIndex)}`)
    selectCell(selectedRow, colIndex)
  }

  async function deleteSelectedColumn() {
    const colIndex = selectedCol
    await transformSheetCells((row, col) => {
      if (col < colIndex) return { row, col }
      if (col === colIndex) return null
      return { row, col: col - 1 }
    }, 'Delete column failed', `Deleted column ${colLabel(colIndex)}`)
    selectCell(selectedRow, Math.min(colIndex, COLS - 1))
  }

  function freezePanesAtSelection() {
    frozenRowCount = selectedRow
    frozenColCount = selectedCol
    if (selectedRow === 0 && selectedCol === 0) {
      frozenRowCount = 1
    }
    setStatus(`Frozen ${frozenRowCount} row${frozenRowCount === 1 ? '' : 's'} and ${frozenColCount} column${frozenColCount === 1 ? '' : 's'}`)
  }

  function unfreezePanes() {
    frozenRowCount = 0
    frozenColCount = 0
    setStatus('Panes unfrozen')
  }

  async function applyTemplate(key: string) {
    const template = TEMPLATES[key]
    if (!template) return
    const start = normalizeRange(currentRange)
    const changes: Array<{ row: number; col: number; value: string }> = []
    for (let row = 0; row < template.rows.length; row++) {
      for (let col = 0; col < template.rows[row].length; col++) {
        changes.push({
          row: start.startRow + row,
          col: start.startCol + col,
          value: template.rows[row][col],
        })
      }
    }
    await applyCellValueChanges(changes, 'Template insert failed', `Inserted ${template.title} template`)
    selectionStart = { row: start.startRow, col: start.startCol }
    selectionEnd = {
      row: start.startRow + template.rows.length - 1,
      col: start.startCol + template.rows[0].length - 1,
    }
  }

  function buildValidation(): ValidationDefinition {
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

  function validationLabel(rule: StoredValidationRule): string {
    if (rule.validation.validation_type === 'List') {
      return `${rule.label}: list values in ${rangeLabel(rule.range)}`
    }
    return `${rule.label}: ${rule.validation.validation_type} ${rule.validation.operator} ${rule.validation.formula1 ?? ''}${rule.validation.formula2 ? ` and ${rule.validation.formula2}` : ''} in ${rangeLabel(rule.range)}`
  }

  function checkValidationValue(validation: ValidationDefinition, value: string): string | null {
    if (value.startsWith('=')) return null
    if (!value.trim()) return validation.allow_blank ? null : 'Value is required'
    if (validation.validation_type === 'List') {
      const allowed = (validation.source ?? '').split(',').map((entry) => entry.trim()).filter(Boolean)
      return allowed.some((entry) => entry.toLowerCase() === value.trim().toLowerCase())
        ? null
        : `Value must be one of: ${allowed.join(', ')}`
    }
    const checkNumber = validation.validation_type === 'WholeNumber'
      || validation.validation_type === 'Decimal'
      || validation.validation_type === 'Date'
      || validation.validation_type === 'Time'
    const measured = validation.validation_type === 'TextLength' ? value.length : Number(value)
    if (checkNumber && !Number.isFinite(measured)) return 'Value must be numeric'
    if (validation.validation_type === 'WholeNumber' && !Number.isInteger(measured)) return 'Value must be a whole number'
    if (validation.validation_type === 'TextLength' && !Number.isFinite(measured)) return 'Text length could not be measured'
    if (validation.validation_type === 'Custom') return null

    const first = validation.formula1 == null || validation.formula1 === '' ? null : Number(validation.formula1)
    const second = validation.formula2 == null || validation.formula2 === '' ? null : Number(validation.formula2)
    const op = validation.operator
    const passes =
      op === 'Between' ? (first == null || measured >= first) && (second == null || measured <= second)
      : op === 'NotBetween' ? !((first == null || measured >= first) && (second == null || measured <= second))
      : op === 'Equal' ? first == null || measured === first
      : op === 'NotEqual' ? first == null || measured !== first
      : op === 'GreaterThan' ? first == null || measured > first
      : op === 'LessThan' ? first == null || measured < first
      : op === 'GreaterThanOrEqual' ? first == null || measured >= first
      : op === 'LessThanOrEqual' ? first == null || measured <= first
      : true

    return passes ? null : `Value does not satisfy ${validation.validation_type} ${validation.operator}`
  }

  function validationMessageForValue(row: number, col: number, value: string): string | null {
    for (const rule of validationRules) {
      if (!cellInRange(rule.range, row, col)) continue
      const error = checkValidationValue(rule.validation, value)
      if (error) return `${cellKey(row, col)} violates ${rule.label}: ${error}`
    }
    return null
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

  async function saveValidationRule() {
    const label = validationRuleName.trim() || `Validation ${validationRules.length + 1}`
    const r = normalizeRange(currentRange)
    const rule: StoredValidationRule = {
      id: `validation-${Date.now()}`,
      label,
      range: r,
      validation: buildValidation(),
    }
    validationRules = [...validationRules, rule]
    await runValidation()
    setStatus(`Saved validation rule for ${rangeLabel(r)}`)
  }

  function removeValidationRule(id: string) {
    const removed = validationRules.find((rule) => rule.id === id)
    validationRules = validationRules.filter((rule) => rule.id !== id)
    if (removed) setStatus(`Removed ${removed.label}`)
  }

  function buildConditionalRule(): ConditionalRuleDefinition {
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

  function compareConditionalNumber(value: number, rule: ConditionalRuleDefinition): boolean {
    const first = rule.value1 == null || rule.value1 === '' ? null : Number(rule.value1)
    const second = rule.value2 == null || rule.value2 === '' ? null : Number(rule.value2)
    if (!Number.isFinite(value)) return false
    switch (rule.operator) {
      case 'Between': return (first == null || value >= first) && (second == null || value <= second)
      case 'NotBetween': return !((first == null || value >= first) && (second == null || value <= second))
      case 'Equal': return first == null || value === first
      case 'NotEqual': return first == null || value !== first
      case 'GreaterThan': return first == null || value > first
      case 'LessThan': return first == null || value < first
      case 'GreaterThanOrEqual': return first == null || value >= first
      case 'LessThanOrEqual': return first == null || value <= first
      default: return true
    }
  }

  function conditionalApplies(rule: ConditionalRuleDefinition, row: number, col: number): boolean {
    const range = tupleToRange(rule.range)
    if (!cellInRange(range, row, col)) return false
    const raw = getCellValue(row, col)
    const text = raw.trim()
    switch (rule.condition_type) {
      case 'CellValue':
        return compareConditionalNumber(Number(raw), rule)
      case 'TextContains':
        return text.toLowerCase().includes((rule.value1 ?? '').toLowerCase())
      case 'TextNotContains':
        return !text.toLowerCase().includes((rule.value1 ?? '').toLowerCase())
      case 'TextBeginsWith':
        return text.toLowerCase().startsWith((rule.value1 ?? '').toLowerCase())
      case 'TextEndsWith':
        return text.toLowerCase().endsWith((rule.value1 ?? '').toLowerCase())
      case 'Blanks':
        return text === ''
      case 'NoBlanks':
        return text !== ''
      case 'Duplicate': {
        if (!text) return false
        let matches = 0
        for (let r = range.startRow; r <= range.endRow; r++) {
          for (let c = range.startCol; c <= range.endCol; c++) {
            if ((getCellValue(r, c) || '').trim().toLowerCase() === text.toLowerCase()) {
              matches += 1
              if (matches > 1) return true
            }
          }
        }
        return false
      }
      default:
        return false
    }
  }

  async function applyConditionalFormat() {
    try {
      const rule = buildConditionalRule()
      const matches = await invoke<Array<[number, number]>>('find_conditional_format_matches', {
        sheetId: activeSheetId,
        rule,
      })
      conditionalMatches = matches
      const label = conditionalRuleName.trim() || `Highlight ${conditionalRules.length + 1}`
      conditionalRules = [...conditionalRules, { id: rule.id, label, rule }]
      setStatus(`Saved conditional rule; ${matches.length} current match${matches.length === 1 ? '' : 'es'}`)
    } catch (e) {
      setError(e, 'Conditional formatting failed')
    }
  }

  function removeConditionalRule(id: string) {
    const removed = conditionalRules.find((rule) => rule.id === id)
    conditionalRules = conditionalRules.filter((rule) => rule.id !== id)
    conditionalMatches = []
    if (removed) setStatus(`Removed ${removed.label}`)
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
      case 'templates': return openPanel('templates')
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
      case 'freezePanes': return freezePanesAtSelection()
      case 'unfreezePanes': return unfreezePanes()
      case 'functions': return openPanel('functions')
      case 'addSheet': return handleAddSheet()
      case 'comment': return openPanel('comment')
      case 'chart': return openPanel('chart')
      case 'pivot': return openPanel('pivot')
      case 'structure': return openPanel('structure')
      case 'insertRow': return insertRowAbove()
      case 'deleteRow': return deleteSelectedRow()
      case 'insertColumn': return insertColumnLeft()
      case 'deleteColumn': return deleteSelectedColumn()
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
      case 'filter': return openPanel('filter')
      case 'clearFilter': return clearFilter()
      case 'removeDuplicates': return removeDuplicateRows()
      case 'namedRanges': return openPanel('namedRanges')
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
    <div class="ribbon-group compact-workflow-group">
      <div class="toolbar-menu-wrapper">
        <button
          type="button"
          class="fmt-btn toolbar-menu-trigger"
          class:active={toolbarMenuOpen === 'data'}
          onclick={(e) => { e.stopPropagation(); toggleToolbarMenu('data', e) }}
          title="Data tools"
        >Data ▾</button>
        {#if toolbarMenuOpen === 'data'}
          <div class="toolbar-popover" style="left: {toolbarMenuX}px; top: {toolbarMenuY}px;" role="menu" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
            <button type="button" class="toolbar-menu-item" onclick={() => executeMenuAction('sortAsc')}>Sort Ascending</button>
            <button type="button" class="toolbar-menu-item" onclick={() => executeMenuAction('sortDesc')}>Sort Descending</button>
            <div class="toolbar-menu-divider"></div>
            <button type="button" class="toolbar-menu-item" onclick={() => executeMenuAction('filter')}>Filter</button>
            <button type="button" class="toolbar-menu-item" onclick={() => executeMenuAction('removeDuplicates')}>Remove Duplicates</button>
            <button type="button" class="toolbar-menu-item" onclick={() => executeMenuAction('findReplace')}>Find</button>
            <button type="button" class="toolbar-menu-item" onclick={() => executeMenuAction('validation')}>Validate</button>
            <button type="button" class="toolbar-menu-item" onclick={() => executeMenuAction('namedRanges')}>Named Ranges</button>
          </div>
        {/if}
      </div>
      <div class="toolbar-menu-wrapper">
        <button
          type="button"
          class="fmt-btn toolbar-menu-trigger"
          class:active={toolbarMenuOpen === 'analyze'}
          onclick={(e) => { e.stopPropagation(); toggleToolbarMenu('analyze', e) }}
          title="Analysis tools"
        >Analyze ▾</button>
        {#if toolbarMenuOpen === 'analyze'}
          <div class="toolbar-popover" style="left: {toolbarMenuX}px; top: {toolbarMenuY}px;" role="menu" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
            <button type="button" class="toolbar-menu-item" onclick={() => executeMenuAction('chart')}>Chart</button>
            <button type="button" class="toolbar-menu-item" onclick={() => executeMenuAction('pivot')}>Pivot Table</button>
          </div>
        {/if}
      </div>
      <div class="toolbar-menu-wrapper">
        <button
          type="button"
          class="fmt-btn toolbar-menu-trigger"
          class:active={toolbarMenuOpen === 'output'}
          onclick={(e) => { e.stopPropagation(); toggleToolbarMenu('output', e) }}
          title="Templates and output"
        >Output ▾</button>
        {#if toolbarMenuOpen === 'output'}
          <div class="toolbar-popover" style="left: {toolbarMenuX}px; top: {toolbarMenuY}px;" role="menu" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
            <button type="button" class="toolbar-menu-item" onclick={() => executeMenuAction('templates')}>Templates</button>
            <button type="button" class="toolbar-menu-item" onclick={() => openPanel('print')}>Print / PDF</button>
          </div>
        {/if}
      </div>
    </div>
    <div class="ribbon-group workflow-group">
      <button type="button" class="fmt-btn" onclick={() => handleSort(true)} title="Sort ascending">↑ Sort</button>
      <button type="button" class="fmt-btn" onclick={() => handleSort(false)} title="Sort descending">↓ Sort</button>
      <button type="button" class="fmt-btn" onclick={() => openPanel('filter')} title="Filter selected range">Filter</button>
      <button type="button" class="fmt-btn" onclick={removeDuplicateRows} title="Remove duplicate rows in selected range">Deduplicate</button>
      <button type="button" class="fmt-btn" onclick={() => openPanel('find')} title="Find and replace">Find</button>
      <button type="button" class="fmt-btn" onclick={() => openPanel('validation')} title="Validate selected range">Validate</button>
      <button type="button" class="fmt-btn" onclick={() => openPanel('namedRanges')} title="Manage named ranges">Names</button>
    </div>
    <div class="fmt-divider workflow-divider"></div>
    <div class="ribbon-group workflow-group">
      <button type="button" class="fmt-btn" onclick={() => openPanel('chart')} title="Create chart">Chart</button>
      <button type="button" class="fmt-btn" onclick={() => openPanel('pivot')} title="Create pivot table">Pivot</button>
      <button type="button" class="fmt-btn" onclick={() => openPanel('templates')} title="Insert template">Templates</button>
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

  <div class="grid-container" style="--grid-zoom: {zoomPercent / 100};" role="grid" tabindex="-1" bind:this={gridContainerEl} onscroll={handleScroll} onmousemove={handleGridMouseMove} onkeydown={handleGridKeydown}>
    <div
      class="grid"
      style="grid-template-columns: {COL_WIDTH * 0.6}px repeat({COLS}, {COL_WIDTH}px); height: {HEADER_HEIGHT + displayedRows.length * ROW_HEIGHT}px;"
    >
      <div class="corner-cell"></div>
      {#each Array(COLS) as _, c}
        <div class="col-header" style={getColHeaderStyle(c)}>{colLabel(c)}</div>
      {/each}

      <div class="grid-spacer" style="height: {visibleRowStart * ROW_HEIGHT}px;"></div>

      {#each visibleRows as r}
        <div class="row-header" style={getRowHeaderStyle(r)}>{r + 1}</div>
        {#each Array(COLS) as _, c}
          <button
            type="button"
            class="cell"
            class:selected={isInSelection(r, c)}
            class:active={selectedRow === r && selectedCol === c}
            class:frozen={r < frozenRowCount || c < frozenColCount}
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
                use:focusInput={selectEditTextOnFocus}
                class="cell-input"
              />
            {:else}
              <span class="cell-value">{getCellDisplay(r, c)}</span>
            {/if}
          </button>
        {/each}
      {/each}

      <div class="grid-spacer" style="height: {Math.max(0, (scrollableDisplayRows.length - visibleRowEnd) * ROW_HEIGHT)}px;"></div>
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
    {#if activeFilterLabel}
      <span>Filter {activeFilterLabel}</span>
    {/if}
    {#if frozenRowCount || frozenColCount}
      <span>Frozen {frozenRowCount}R/{frozenColCount}C</span>
    {/if}
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
              {:else if activePanel === 'filter'}Data
              {:else if activePanel === 'namedRanges'}Data
              {:else if activePanel === 'structure'}Insert
              {:else if activePanel === 'templates'}File
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
              {:else if activePanel === 'filter'}Filter Rows
              {:else if activePanel === 'namedRanges'}Named Ranges
              {:else if activePanel === 'structure'}Rows and Columns
              {:else if activePanel === 'templates'}Templates
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
                  <span>
                    <span class="function-category">{fn.category}</span>
                    <span class="function-syntax">{functionHelp(fn.name).syntax}</span>
                    <span class="function-description">{functionHelp(fn.name).description}</span>
                    <span class="function-example">{functionHelp(fn.name).example}</span>
                  </span>
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
            <label>Label column
              <select class="panel-input" bind:value={chartXColumn}>
                {#each chartColumns as col}
                  <option value={String(col.col)}>{col.name || colLabel(col.col)}</option>
                {/each}
              </select>
            </label>
            <label>Value column
              <select class="panel-input" bind:value={chartYColumn} onchange={() => chartSeriesName = chartColumns.find((col) => String(col.col) === chartYColumn)?.name ?? chartSeriesName}>
                {#each chartColumns as col}
                  <option value={String(col.col)}>{col.name || colLabel(col.col)}</option>
                {/each}
              </select>
            </label>
            <label>Series name<input class="panel-input" bind:value={chartSeriesName} /></label>
            <label>Legend
              <select class="panel-input" bind:value={chartLegendPosition}>
                <option>Bottom</option>
                <option>Top</option>
                <option>Left</option>
                <option>Right</option>
                <option>None</option>
              </select>
            </label>
            <p class="panel-note">Uses the selected range as a table with headers. Current range: {rangeLabel(currentRange)}.</p>
            <div class="panel-actions">
              <button type="button" class="primary-btn" onclick={runChart}>Create Chart</button>
            </div>
            {#if chartSvg}
              <div class="chart-preview">{@html chartSvg}</div>
            {/if}
          </div>
        {:else if activePanel === 'pivot'}
          <div class="panel-body form-grid">
            <p class="panel-note">Builds a pivot sheet from the selected table. Current range: {rangeLabel(currentRange)}.</p>
            <label>Rows
              <select class="panel-input" bind:value={pivotRowColumn}>
                {#each pivotColumns as col}
                  <option value={String(col.col)}>{col.name || colLabel(col.col)}</option>
                {/each}
              </select>
            </label>
            <label>Columns
              <select class="panel-input" bind:value={pivotColumnColumn}>
                <option value="">None</option>
                {#each pivotColumns as col}
                  <option value={String(col.col)}>{col.name || colLabel(col.col)}</option>
                {/each}
              </select>
            </label>
            <label>Values
              <select class="panel-input" bind:value={pivotValueColumn}>
                {#each pivotColumns as col}
                  <option value={String(col.col)}>{col.name || colLabel(col.col)}</option>
                {/each}
              </select>
            </label>
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
            <label>Filter field
              <select class="panel-input" bind:value={pivotFilterColumn}>
                <option value="">None</option>
                {#each pivotColumns as col}
                  <option value={String(col.col)}>{col.name || colLabel(col.col)}</option>
                {/each}
              </select>
            </label>
            <label>Filter values<input class="panel-input" bind:value={pivotFilterValue} placeholder="Comma-separated values" /></label>
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
            <label>Rule name<input class="panel-input" bind:value={validationRuleName} /></label>
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
              <button type="button" class="secondary-btn" onclick={saveValidationRule}>Save Rule</button>
            </div>
            {#if validationRules.length > 0}
              <div class="rule-list">
                {#each validationRules as rule}
                  <div class="rule-row">
                    <button type="button" class="result-row" onclick={() => selectRange(rule.range)}>
                      <span>{rangeLabel(rule.range)}</span>
                      <span>{validationLabel(rule)}</span>
                    </button>
                    <button type="button" class="icon-btn small" onclick={() => removeValidationRule(rule.id)} aria-label="Remove validation rule">×</button>
                  </div>
                {/each}
              </div>
            {/if}
            <div class="result-list">
              {#each validationResults as result}
                <button type="button" class="result-row" onclick={() => jumpToResult(result)}>
                  <span>{cellKey(result.row, result.col)}</span>
                  <span>{result.error}</span>
                </button>
              {/each}
            </div>
          </div>
        {:else if activePanel === 'filter'}
          <div class="panel-body form-grid">
            <p class="panel-note">Filters the current selected range: {rangeLabel(currentRange)}.</p>
            <label class="check-row"><input type="checkbox" bind:checked={filterHasHeader} /> First row is headers</label>
            <label>Column
              <select class="panel-input" bind:value={filterColumn}>
                {#each Array(currentRange.endCol - currentRange.startCol + 1) as _, offset}
                  {@const col = currentRange.startCol + offset}
                  <option value={String(col)}>{colLabel(col)} {getCellValue(currentRange.startRow, col) ? `- ${getCellValue(currentRange.startRow, col)}` : ''}</option>
                {/each}
              </select>
            </label>
            <label>Condition
              <select class="panel-input" bind:value={filterCondition}>
                <option>Contains</option>
                <option>Equals</option>
                <option>NotEquals</option>
                <option>StartsWith</option>
                <option>EndsWith</option>
                <option>GreaterThan</option>
                <option>LessThan</option>
                <option>Empty</option>
                <option>NotEmpty</option>
              </select>
            </label>
            {#if filterCondition !== 'Empty' && filterCondition !== 'NotEmpty'}
              <label>Value<input class="panel-input" bind:value={filterValue} placeholder="Filter value" /></label>
            {/if}
            <div class="panel-actions">
              <button type="button" class="primary-btn" onclick={applyFilter}>Apply Filter</button>
              <button type="button" class="secondary-btn" onclick={clearFilter}>Clear Filter</button>
            </div>
            {#if activeFilterLabel}
              <p class="panel-note">Active filter: {activeFilterLabel}</p>
            {/if}
          </div>
        {:else if activePanel === 'namedRanges'}
          <div class="panel-body form-grid">
            <p class="panel-note">Current selection: {rangeLabel(currentRange)}.</p>
            <label>Name<input class="panel-input" bind:value={namedRangeName} placeholder="Revenue_Q1" /></label>
            <div class="panel-actions">
              <button type="button" class="primary-btn" onclick={addNamedRange}>Add Named Range</button>
            </div>
            {#if namedRanges.length > 0}
              <div class="rule-list">
                {#each namedRanges as named}
                  <div class="rule-row">
                    <button type="button" class="result-row" onclick={() => selectNamedRange(named)}>
                      <span>{named.name}</span>
                      <span>{rangeLabel(named.range)}</span>
                    </button>
                    <button type="button" class="secondary-btn" onclick={() => insertNamedRangeReference(named)}>Insert Ref</button>
                    <button type="button" class="icon-btn small" onclick={() => removeNamedRange(named.id)} aria-label="Remove named range">×</button>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {:else if activePanel === 'structure'}
          <div class="panel-body form-grid">
            <p class="panel-note">Active cell: {cellKey(selectedRow, selectedCol)}. Row and column operations shift existing cells and are undoable.</p>
            <div class="panel-actions">
              <button type="button" class="primary-btn" onclick={insertRowAbove}>Insert Row Above</button>
              <button type="button" class="secondary-btn" onclick={deleteSelectedRow}>Delete Row</button>
              <button type="button" class="primary-btn" onclick={insertColumnLeft}>Insert Column Left</button>
              <button type="button" class="secondary-btn" onclick={deleteSelectedColumn}>Delete Column</button>
            </div>
            <div class="panel-actions">
              <button type="button" class="secondary-btn" onclick={freezePanesAtSelection}>Freeze Panes at Selection</button>
              <button type="button" class="secondary-btn" onclick={unfreezePanes}>Unfreeze Panes</button>
            </div>
            <p class="panel-note">Frozen rows: {frozenRowCount}. Frozen columns: {frozenColCount}.</p>
          </div>
        {:else if activePanel === 'templates'}
          <div class="panel-body form-grid">
            <p class="panel-note">Templates insert practical starter tables at the current selection.</p>
            <div class="template-grid">
              {#each Object.entries(TEMPLATES) as [key, template]}
                <button type="button" class="template-card" onclick={() => applyTemplate(key)}>
                  <span>{template.title}</span>
                  <small>{template.rows.length} rows × {template.rows[0].length} columns</small>
                </button>
              {/each}
            </div>
          </div>
        {:else if activePanel === 'conditional'}
          <div class="panel-body form-grid">
            <label>Rule name<input class="panel-input" bind:value={conditionalRuleName} /></label>
            <label>Condition
              <select class="panel-input" bind:value={conditionalType}>
                <option>CellValue</option>
                <option>TextContains</option>
                <option>TextNotContains</option>
                <option>TextBeginsWith</option>
                <option>TextEndsWith</option>
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
              <button type="button" class="primary-btn" onclick={applyConditionalFormat}>Save Live Rule</button>
            </div>
            <p class="panel-note">{conditionalMatches.length} current cell{conditionalMatches.length === 1 ? '' : 's'} match the last saved rule.</p>
            {#if conditionalRules.length > 0}
              <div class="rule-list">
                {#each conditionalRules as stored}
                  <div class="rule-row">
                    <button type="button" class="result-row" onclick={() => selectRange(tupleToRange(stored.rule.range))}>
                      <span>{rangeLabel(tupleToRange(stored.rule.range))}</span>
                      <span>{stored.label}: {stored.rule.condition_type}</span>
                    </button>
                    <button type="button" class="icon-btn small" onclick={() => removeConditionalRule(stored.id)} aria-label="Remove conditional rule">×</button>
                  </div>
                {/each}
              </div>
            {/if}
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
