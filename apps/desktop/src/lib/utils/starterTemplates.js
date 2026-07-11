/**
 * @typedef {{
 *   id: string,
 *   title: string,
 *   summary: string,
 *   audience: string,
 *   tasks: string[],
 *   rows: string[][],
 * }} StarterTemplate
 * @typedef {{getItem: (key: string) => string | null, setItem: (key: string, value: string) => void}} KeyValueStorage
 */

/** @type {StarterTemplate[]} */
const TEMPLATE_CATALOG = [
  {
    id: 'school-budget',
    title: 'School Budget',
    summary: 'Plan a term budget and compare each category with actual spending.',
    audience: 'Schools, parent groups, and education programs',
    tasks: ['Set the available budget', 'Record actual costs', 'Review remaining funds'],
    rows: [
      ['School Budget', '', '', ''],
      ['Category', 'Planned', 'Actual', 'Remaining'],
      ['Learning materials', '1200', '1080', '=B3-C3'],
      ['Meals', '850', '815', '=B4-C4'],
      ['Transport', '600', '640', '=B5-C5'],
      ['Activities', '350', '290', '=B6-C6'],
      ['Total', '=SUM(B3:B6)', '=SUM(C3:C6)', '=SUM(D3:D6)'],
    ],
  },
  {
    id: 'small-business-accounts',
    title: 'Small Business Accounts',
    summary: 'Track money in, money out, and a running balance for a small organization.',
    audience: 'Small businesses, cooperatives, and sole traders',
    tasks: ['Replace the opening balance', 'Add each transaction', 'Check the running balance'],
    rows: [
      ['Small Business Accounts', '', '', '', '', ''],
      ['Date', 'Reference', 'Description', 'Money In', 'Money Out', 'Balance'],
      ['2026-01-01', 'OPEN', 'Opening balance', '500', '0', '=D3-E3'],
      ['2026-01-03', 'SALE-001', 'Invented sample sale', '180', '0', '=F3+D4-E4'],
      ['2026-01-05', 'COST-001', 'Invented sample supplies', '0', '65', '=F4+D5-E5'],
      ['2026-01-07', 'COST-002', 'Invented sample transport', '0', '20', '=F5+D6-E6'],
    ],
  },
  {
    id: 'community-project',
    title: 'Community Project',
    summary: 'Coordinate tasks, owners, dates, and spending for a community project.',
    audience: 'Community groups, clubs, and volunteer teams',
    tasks: ['Assign each task', 'Update the status', 'Compare budget with spending'],
    rows: [
      ['Community Project Plan', '', '', '', '', '', ''],
      ['Task', 'Owner', 'Status', 'Due Date', 'Budget', 'Spent', 'Remaining'],
      ['Confirm venue', 'Coordinator', 'In progress', '2026-02-10', '200', '50', '=E3-F3'],
      ['Prepare materials', 'Volunteer team', 'Not started', '2026-02-15', '150', '0', '=E4-F4'],
      ['Run event', 'Event team', 'Not started', '2026-02-20', '400', '0', '=E5-F5'],
      ['Follow up', 'Coordinator', 'Not started', '2026-02-25', '50', '0', '=E6-F6'],
      ['Total', '', '', '', '=SUM(E3:E6)', '=SUM(F3:F6)', '=SUM(G3:G6)'],
    ],
  },
  {
    id: 'household-planning',
    title: 'Household Planning',
    summary: 'Compare a monthly household plan with actual costs and available funds.',
    audience: 'Households and shared homes',
    tasks: ['Set planned amounts', 'Enter actual costs', 'Review each difference'],
    rows: [
      ['Household Plan', '', '', ''],
      ['Category', 'Planned', 'Actual', 'Difference'],
      ['Food', '300', '275', '=B3-C3'],
      ['Transport', '100', '115', '=B4-C4'],
      ['Utilities', '180', '172', '=B5-C5'],
      ['Savings', '120', '100', '=B6-C6'],
      ['Total', '=SUM(B3:B6)', '=SUM(C3:C6)', '=SUM(D3:D6)'],
    ],
  },
]

const MAX_GRID_ROWS = 1_000_000
const MAX_GRID_COLS = 16_384

/**
 * @template T
 * @param {T} value
 * @returns {T}
 */
function freezeCatalog(value) {
  if (value !== null && typeof value === 'object') {
    Object.freeze(value)
    for (const nested of Object.values(value)) freezeCatalog(nested)
  }
  return value
}

export const STARTER_TEMPLATES = freezeCatalog(TEMPLATE_CATALOG)

/** @param {string} id */
export function getStarterTemplate(id) {
  return STARTER_TEMPLATES.find((template) => template.id === id) ?? null
}

/** @param {StarterTemplate} template */
export function templateDimensions(template) {
  return {
    rows: template.rows.length,
    columns: Math.max(0, ...template.rows.map((row) => row.length)),
  }
}

/**
 * @param {string} id
 * @param {number} [startRow]
 * @param {number} [startCol]
 */
export function templateCellChanges(id, startRow = 0, startCol = 0) {
  const template = getStarterTemplate(id)
  if (!template) throw new Error(`Unknown starter template: ${id}`)
  assertGridCoordinate(startRow, 'startRow')
  assertGridCoordinate(startCol, 'startCol')
  const dimensions = templateDimensions(template)
  if (startRow + dimensions.rows > MAX_GRID_ROWS || startCol + dimensions.columns > MAX_GRID_COLS) {
    throw new RangeError('Starter template does not fit inside the workbook grid at this location')
  }

  return template.rows.flatMap((row, rowOffset) => row.map((value, colOffset) => ({
    row: startRow + rowOffset,
    col: startCol + colOffset,
    value: rebaseFormula(value, startRow, startCol),
  })))
}

/** @param {number} value @param {string} label */
function assertGridCoordinate(value, label) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new TypeError(`${label} must be a non-negative safe integer`)
  }
}

/** @param {string} value @param {number} rowOffset @param {number} colOffset */
function rebaseFormula(value, rowOffset, colOffset) {
  if (!value.startsWith('=') || rowOffset === 0 && colOffset === 0) return value
  return value.replace(/(\$?)([A-Z]+)(\$?)([1-9][0-9]*)/g, (_match, fixedCol, col, fixedRow, row) => {
    const nextCol = fixedCol ? col : columnLabel(columnIndex(col) + colOffset)
    const nextRow = fixedRow ? row : String(Number(row) + rowOffset)
    return `${fixedCol}${nextCol}${fixedRow}${nextRow}`
  })
}

/** @param {string} label */
function columnIndex(label) {
  let index = 0
  for (const character of label) index = index * 26 + character.charCodeAt(0) - 64
  return index - 1
}

/** @param {number} index */
function columnLabel(index) {
  if (!Number.isSafeInteger(index) || index < 0) throw new RangeError('Column is outside the supported grid')
  let label = ''
  let current = index
  do {
    label = String.fromCharCode(65 + current % 26) + label
    current = Math.floor(current / 26) - 1
  } while (current >= 0)
  return label
}

export const FIRST_RUN_STORAGE_KEY = '900sheets.first-run.v1'

export const FIRST_RUN_STEPS = freezeCatalog([
  {
    id: 'choose-template',
    title: 'Start with a useful table',
    detail: 'Choose a starter template or open a blank workbook.',
  },
  {
    id: 'edit-cells',
    title: 'Add your own information',
    detail: 'Select a cell and type. Formulas begin with an equals sign.',
  },
  {
    id: 'save-locally',
    title: 'Save on your computer',
    detail: '900Sheets works locally. Save a native workbook to keep supported features.',
  },
])

/** @param {KeyValueStorage} storage */
export function shouldShowFirstRun(storage) {
  try {
    return storage.getItem(FIRST_RUN_STORAGE_KEY) !== 'complete'
  } catch {
    return true
  }
}

/** @param {KeyValueStorage} storage */
export function completeFirstRun(storage) {
  try {
    storage.setItem(FIRST_RUN_STORAGE_KEY, 'complete')
    return true
  } catch {
    return false
  }
}
