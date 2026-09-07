const MAX_ROWS = 1_000_000
const MAX_COLS = 16_384

/** @param {string} label */
function columnIndex(label) {
  let result = 0
  for (const char of label.toUpperCase()) result = result * 26 + char.charCodeAt(0) - 64
  return result - 1
}

/** @param {number} index */
function columnLabel(index) {
  let label = ''
  for (let value = index + 1; value > 0; value = Math.floor((value - 1) / 26)) {
    label = String.fromCharCode(65 + (value - 1) % 26) + label
  }
  return label
}

/**
 * Translate supported A1 references while preserving strings, sheet names,
 * function names, and absolute axes. Reject unsupported reference syntax before
 * a paste can change any cells.
 * @param {string} value
 * @param {number} rowDelta
 * @param {number} colDelta
 */
export function rebaseCopiedFormula(value, rowDelta, colDelta) {
  if (!value.startsWith('=') || (rowDelta === 0 && colDelta === 0)) return value
  let result = ''
  let index = 0
  while (index < value.length) {
    const character = value[index]
    if (character === '"' || character === "'") {
      const start = index++
      let closed = false
      while (index < value.length) {
        if (value[index++] !== character) continue
        if (value[index] === character) { index++; continue }
        closed = true
        break
      }
      if (!closed) throw new Error('Finish the quoted text in the formula before copying it.')
      if (character === "'" && value.slice(start, index).includes('[')) {
        throw new Error('Copying formulas with external workbook references is not supported yet.')
      }
      if (character === "'" && (value.slice(start, index).includes(':') || /^\s*:/.test(value.slice(index)) || /:\s*$/.test(result))) {
        throw new Error('Copying formulas with references spanning multiple sheets is not supported yet.')
      }
      result += value.slice(start, index)
      continue
    }
    if (character === '[' || character === ']') {
      throw new Error('Copying formulas with external workbook or structured table references is not supported yet.')
    }
    const boundaryBefore = index === 0 || !/[\p{L}\p{N}_.$]/u.test(value[index - 1])
    if (boundaryBefore && /^(?:\$?[A-Za-z]{1,3}:\$?[A-Za-z]{1,3}|\$?[1-9]\d*:\$?[1-9]\d*)(?![\p{L}\p{N}_.$])/u.test(value.slice(index))) {
      throw new Error('Copying formulas with whole-row or whole-column references is not supported yet. Use a bounded cell range.')
    }
    if (boundaryBefore && /^[A-Za-z_][A-Za-z0-9_.]*:[A-Za-z_][A-Za-z0-9_.]*!/.test(value.slice(index))) {
      throw new Error('Copying formulas with references spanning multiple sheets is not supported yet.')
    }
    const reference = boundaryBefore ? /^(\$?)([A-Za-z]{1,3})(\$?)([1-9]\d*)/.exec(value.slice(index)) : null
    if (reference) {
      const [token, absoluteCol, column, absoluteRow, row] = reference
      const suffix = value.slice(index + token.length)
      const boundaryAfter = !/^[\p{L}\p{N}_.$]/u.test(suffix)
      if (boundaryAfter && !/^\s*[(!]/.test(suffix)) {
        const originalRow = Number(row) - 1
        const originalCol = columnIndex(column)
        // Out-of-grid names are identifiers, not A1 cell references.
        if (originalRow < MAX_ROWS && originalCol < MAX_COLS) {
          const nextRow = originalRow + (absoluteRow ? 0 : rowDelta)
          const nextCol = originalCol + (absoluteCol ? 0 : colDelta)
          result += nextRow < 0 || nextRow >= MAX_ROWS || nextCol < 0 || nextCol >= MAX_COLS
            ? '#REF!'
            : `${absoluteCol}${columnLabel(nextCol)}${absoluteRow}${nextRow + 1}`
          index += token.length
          continue
        }
      }
    }
    result += character
    index++
  }
  return result
}

/**
 * @param {{ validationRules?: unknown[], conditionalRules?: unknown[], namedRanges?: unknown[], tables?: unknown[], charts?: unknown[], frozenRowCount?: number, frozenColCount?: number, hiddenRows?: Record<string, boolean>, activeFilterLabel?: string, chartSvg?: string }} state
 */
export function structuralMetadataBlockers(state) {
  const blockers = []
  if (state.validationRules?.length) blockers.push('data validation')
  if (state.conditionalRules?.length) blockers.push('conditional formatting')
  if (state.namedRanges?.length) blockers.push('named ranges')
  if (state.tables?.length) blockers.push('tables')
  if (state.charts?.length || state.chartSvg) blockers.push('charts')
  if (state.frozenRowCount || state.frozenColCount) blockers.push('frozen panes')
  if (state.activeFilterLabel || Object.values(state.hiddenRows ?? {}).some(Boolean)) blockers.push('filters')
  return blockers
}
