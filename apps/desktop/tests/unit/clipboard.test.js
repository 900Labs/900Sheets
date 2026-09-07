import assert from 'node:assert/strict'
import test from 'node:test'
import { rebaseCopiedFormula, structuralMetadataBlockers } from '../../src/lib/utils/clipboard.js'

test('copies relative, absolute, and mixed references independently', () => {
  assert.equal(rebaseCopiedFormula('=A1+$B2+C$3+$D$4', 2, 3), '=D3+$B4+F$3+$D$4')
  assert.equal(rebaseCopiedFormula('=SUM(A1:B2)', 1, 1), '=SUM(B2:C3)')
})

test('preserves functions, identifiers, and escaped quoted text', () => {
  assert.equal(
    rebaseCopiedFormula('=LOG10(A1)+LOG10 (B2)+A1_total+rate.A1+"A1 and ""B2"""', 1, 1),
    '=LOG10(B2)+LOG10 (C3)+A1_total+rate.A1+"A1 and ""B2"""',
  )
  assert.equal(rebaseCopiedFormula('=ΔA1+ZZZ1+A1000001', 1, 1), '=ΔA1+ZZZ1+A1000001')
})

test('copies qualified references while leaving sheet names intact', () => {
  assert.equal(
    rebaseCopiedFormula("=Sheet1!A1+'O''Brien A1'!$B2+A1!C$3", 1, 2),
    "=Sheet1!C2+'O''Brien A1'!$B3+A1!E$3",
  )
})

test('produces REF errors at grid edges and keeps absolute references', () => {
  assert.equal(rebaseCopiedFormula('=A1+$A$1+A$2+$B1', -1, -1), '=#REF!+$A$1+#REF!+#REF!')
  assert.equal(rebaseCopiedFormula('=XFD1000000+$XFD$1000000', 1, 1), '=#REF!+$XFD$1000000')
})

test('preserves plain text and zero-offset copies', () => {
  assert.equal(rebaseCopiedFormula('A1', 1, 1), 'A1')
  assert.equal(rebaseCopiedFormula('=A1', 0, 0), '=A1')
})

test('rejects unsupported external and table references without rewriting them', () => {
  assert.throws(() => rebaseCopiedFormula('=[Book.xlsx]Sheet1!A1', 1, 1), /external workbook/)
  assert.throws(() => rebaseCopiedFormula("='[Book.xlsx]Sheet 1'!A1", 1, 1), /external workbook/)
  assert.throws(() => rebaseCopiedFormula('=SUM(Table1[Amount])', 1, 1), /structured table/)
  assert.equal(rebaseCopiedFormula('="[A1]"&A1', 1, 1), '="[A1]"&B2')
})

test('rejects unsupported whole-axis and multiple-sheet reference rewrites', () => {
  assert.throws(() => rebaseCopiedFormula('=SUM(A:A)', 1, 1), /whole-row or whole-column/)
  assert.throws(() => rebaseCopiedFormula('=SUM($1:$2)', 1, 1), /whole-row or whole-column/)
  assert.throws(() => rebaseCopiedFormula('=SUM(Sheet1:Sheet2!A1)', 1, 1), /multiple sheets/)
  assert.throws(() => rebaseCopiedFormula("=SUM('Sheet 1:Sheet 2'!A1)", 1, 1), /multiple sheets/)
  assert.throws(() => rebaseCopiedFormula("=SUM(Sheet1:'Sheet 2'!A1)", 1, 1), /multiple sheets/)
})

test('rejects incomplete quotes rather than silently changing a malformed formula', () => {
  assert.throws(() => rebaseCopiedFormula('="A1', 1, 1), /quoted text/)
})

test('structure guard identifies every coordinate-bearing feature without changing it', () => {
  const state = {
    validationRules: [{}], conditionalRules: [{}], namedRanges: [{}], tables: [{}], charts: [{}],
    frozenRowCount: 1, frozenColCount: 1, hiddenRows: { 4: true }, activeFilterLabel: 'Filter', chartSvg: '<svg/>',
  }
  const before = structuredClone(state)
  assert.deepEqual(structuralMetadataBlockers(state), [
    'data validation', 'conditional formatting', 'named ranges', 'tables', 'charts', 'frozen panes', 'filters',
  ])
  assert.deepEqual(state, before)
})

test('structure guard allows sheets without coordinate-bearing metadata', () => {
  assert.deepEqual(structuralMetadataBlockers({ hiddenRows: { 4: false } }), [])
  assert.deepEqual(structuralMetadataBlockers({ chartSvg: '<svg/>' }), ['charts'])
})
