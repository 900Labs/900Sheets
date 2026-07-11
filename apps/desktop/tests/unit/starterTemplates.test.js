import assert from 'node:assert/strict'
import test from 'node:test'
import {
  completeFirstRun,
  FIRST_RUN_STEPS,
  FIRST_RUN_STORAGE_KEY,
  getStarterTemplate,
  shouldShowFirstRun,
  STARTER_TEMPLATES,
  templateCellChanges,
  templateDimensions,
} from '../../src/lib/utils/starterTemplates.js'

test('catalog includes the four community starter templates', () => {
  assert.deepEqual(STARTER_TEMPLATES.map(({ id }) => id), [
    'school-budget',
    'small-business-accounts',
    'community-project',
    'household-planning',
  ])
  for (const template of STARTER_TEMPLATES) {
    const dimensions = templateDimensions(template)
    assert.ok(dimensions.rows >= 6)
    assert.ok(dimensions.columns >= 4)
    assert.ok(template.tasks.length >= 3)
    assert.ok(template.rows.every((row) => row.length === dimensions.columns))
  }
})

test('catalog uses only invented, publishable examples', () => {
  const serialized = JSON.stringify(STARTER_TEMPLATES)
  assert.doesNotMatch(serialized, /(?:\/Users\/|[A-Z]:\\Users\\|@(?:gmail|outlook)\.com)/i)
  assert.match(serialized, /Invented sample/)
})

test('template cells are rebased when inserted away from A1', () => {
  const changes = templateCellChanges('school-budget', 9, 2)
  assert.deepEqual(changes.find(({ row, col }) => row === 11 && col === 5), {
    row: 11,
    col: 5,
    value: '=D12-E12',
  })
  assert.equal(changes.find(({ row, col }) => row === 15 && col === 3)?.value, '=SUM(D12:D15)')
})

test('template lookup and coordinate validation fail safely', () => {
  assert.equal(getStarterTemplate('missing'), null)
  assert.throws(() => templateCellChanges('missing'), /Unknown starter template/)
  assert.throws(() => templateCellChanges('school-budget', -1, 0), /startRow/)
  assert.throws(() => templateCellChanges('school-budget', 0, 1.5), /startCol/)
})

test('template insertion is all-or-nothing at the workbook edges', () => {
  const school = getStarterTemplate('school-budget')
  const dimensions = templateDimensions(school)
  const lastFit = templateCellChanges(
    'school-budget',
    1_000_000 - dimensions.rows,
    16_384 - dimensions.columns,
  )
  assert.equal(lastFit.length, dimensions.rows * dimensions.columns)
  assert.equal(lastFit.at(-1).row, 999_999)
  assert.equal(lastFit.at(-1).col, 16_383)

  assert.throws(
    () => templateCellChanges('school-budget', 1_000_000 - dimensions.rows + 1, 0),
    /does not fit/,
  )
  assert.throws(
    () => templateCellChanges('school-budget', 0, 16_384 - dimensions.columns + 1),
    /does not fit/,
  )
})

test('catalog and nested template values cannot be mutated', () => {
  assert.equal(Object.isFrozen(STARTER_TEMPLATES), true)
  assert.equal(Object.isFrozen(STARTER_TEMPLATES[0]), true)
  assert.equal(Object.isFrozen(STARTER_TEMPLATES[0].rows[0]), true)
})

test('first-run completion is local, explicit, and repeatable', () => {
  const values = new Map()
  const storage = {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  }

  assert.ok(FIRST_RUN_STEPS.length >= 3)
  assert.equal(shouldShowFirstRun(storage), true)
  assert.equal(completeFirstRun(storage), true)
  assert.equal(values.get(FIRST_RUN_STORAGE_KEY), 'complete')
  assert.equal(shouldShowFirstRun(storage), false)
})

test('first-run helpers stay usable when storage is unavailable', () => {
  const unavailableStorage = {
    getItem: () => { throw new Error('unavailable') },
    setItem: () => { throw new Error('unavailable') },
  }
  assert.equal(shouldShowFirstRun(unavailableStorage), true)
  assert.equal(completeFirstRun(unavailableStorage), false)
})
