import { expect, type Page, test } from '@playwright/test'

interface MockOptions {
  recoveries?: Array<{ id: string; modified_millis: number }>
  discardFailures?: number
  discardFailuresAfter?: number
  populatedCellCount?: number
  nativeSaveDelayMs?: number
  nativeOpenDelayMs?: number
  dialogOpenResult?: string | null
  sheetDataById?: Record<string, Record<string, string>>
  sheetDataDelayMsById?: Record<string, number>
  setActiveSheetDelayMs?: number
  recoveryWriteDelayMs?: number
  recoveryRestoreFails?: boolean
  showFirstRun?: boolean
  cellEvaluations?: Record<string, { numeric_value: number | null; display: string }>
}

async function installTauriMock(page: Page, options: MockOptions = {}) {
  await page.addInitScript((options: MockOptions) => {
    if (!options.showFirstRun) localStorage.setItem('900sheets.first-run.v1', 'complete')
    type CellRecord = { value: string; display: string }
    type TauriWindow = Window & {
      __TAURI_INTERNALS__?: {
        metadata?: {
          currentWindow: { label: string }
          currentWebview: { label: string }
        }
        invoke?: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>
        transformCallback?: (callback?: (data: unknown) => unknown, once?: boolean) => number
        unregisterCallback?: (id: number) => void
        runCallback?: (id: number, data: unknown) => unknown
        callbacks?: Map<number, (data: unknown) => unknown>
      }
    }

    const cells = new Map<string, CellRecord>()
    const sheetCells = new Map<string, Map<string, CellRecord>>()
    for (const [sheetId, values] of Object.entries(options.sheetDataById ?? {})) {
      sheetCells.set(sheetId, new Map(Object.entries(values).map(([key, value]) => [key, { value, display: value }])))
    }
    const formats = new Map<string, Record<string, unknown>>()
    const comments = new Map<string, { row: number; col: number; text: string; author: string }>()
    const callbacks = new Map<number, (data: unknown) => unknown>()
    const initialSheets = [
      { id: 0, stable_id: 1, name: 'Sheet1' },
      { id: 1, stable_id: 2, name: 'Sheet2' },
    ]
    for (let index = 0; index < (options.populatedCellCount ?? 0); index++) {
      cells.set(`${Math.floor(index / 100)}:${index % 100}`, { value: String(index), display: String(index) })
    }
    const sheets = structuredClone(initialSheets)
    comments.set('0:0:1', { row: 0, col: 1, text: 'existing B1 comment', author: 'tester' })
    let callbackId = 1
    type MockSnapshot = {
      cells: Array<[string, CellRecord]>
      formats: Array<[string, Record<string, unknown>]>
      comments: Array<[string, { row: number; col: number; text: string; author: string }]>
      sheets: Array<{ id: number; stable_id: number; name: string }>
      metadata: Record<string, unknown>
    }
    let pending: MockSnapshot | null = null
    let savedNative: MockSnapshot | null = null
    let discardFailures = options.discardFailures ?? 0
    const recoveryEvents = {
      discardAttempts: [] as string[],
      restored: [] as string[],
      writes: [] as Array<Record<string, unknown>>,
      nativeSaves: [] as Array<Record<string, unknown>>,
      destroyed: 0,
    }
    ;(window as Window & { __RECOVERY_TEST__?: typeof recoveryEvents }).__RECOVERY_TEST__ = recoveryEvents
    let closeCallbackId: number | null = null
    ;(window as Window & { __TRIGGER_CLOSE__?: () => void }).__TRIGGER_CLOSE__ = () => {
      if (closeCallbackId !== null) callbacks.get(closeCallbackId)?.({ id: 1, event: 'tauri://close-requested', payload: null })
    }
    const undoStack: Array<{ before: MockSnapshot; after: MockSnapshot }> = []
    const redoStack: Array<{ before: MockSnapshot; after: MockSnapshot }> = []
    const cloneJson = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T
    const snapshot = (metadata: Record<string, unknown> = {}): MockSnapshot => ({
      cells: cloneJson(Array.from(cells.entries())),
      formats: cloneJson(Array.from(formats.entries())),
      comments: cloneJson(Array.from(comments.entries())),
      sheets: cloneJson(sheets),
      metadata: cloneJson(metadata),
    })
    const restore = (value: MockSnapshot) => {
      cells.clear()
      for (const [key, cell] of value.cells) cells.set(key, cloneJson(cell))
      formats.clear()
      for (const [key, format] of value.formats) formats.set(key, cloneJson(format))
      comments.clear()
      for (const [key, comment] of value.comments) comments.set(key, cloneJson(comment))
      sheets.splice(0, sheets.length, ...cloneJson(value.sheets))
    }

    const keyFor = (row: unknown, col: unknown) => `${Number(row)}:${Number(col)}`
    const commentKeyFor = (sheetId: unknown, row: unknown, col: unknown) => `${Number(sheetId)}:${Number(row)}:${Number(col)}`

    const tauriWindow = window as TauriWindow
    tauriWindow.__TAURI_INTERNALS__ = {
      ...(tauriWindow.__TAURI_INTERNALS__ ?? {}),
      metadata: {
        currentWindow: { label: 'main' },
        currentWebview: { label: 'main' },
      },
      callbacks,
      transformCallback: (callback?: (data: unknown) => unknown, once = false) => {
        const id = callbackId++
        callbacks.set(id, (data: unknown) => {
          if (once) callbacks.delete(id)
          return callback?.(data)
        })
        return id
      },
      unregisterCallback: (id: number) => {
        callbacks.delete(id)
      },
      runCallback: (id: number, data: unknown) => callbacks.get(id)?.(data),
      invoke: async (cmd: string, args: Record<string, unknown> = {}) => {
        switch (cmd) {
          case 'plugin:dialog|open':
            return options.dialogOpenResult === undefined ? '/tmp/mock-workbook.900sheets' : options.dialogOpenResult
          case 'plugin:dialog|save':
            return '/tmp/mock-workbook.900sheets'
          case 'plugin:event|listen':
            if (args.event === 'tauri://close-requested') closeCallbackId = Number(args.handler)
            return 1
          case 'plugin:event|unlisten':
            return null
          case 'list_recovery_snapshots':
            return cloneJson(options.recoveries ?? [])
          case 'list_native_backups':
            return []
          case 'get_export_preflight':
            return {
              format: String(args.format ?? '').toUpperCase(),
              estimated_dense_cells: 0,
              populated_cells: cells.size,
              max_row: null,
              max_col: null,
              limit: null,
              blocked: false,
              message: 'Export is within the configured safety limit.',
            }
          case 'write_recovery_snapshot':
            if ((options.recoveryWriteDelayMs ?? 0) > 0) {
              await new Promise((resolve) => setTimeout(resolve, options.recoveryWriteDelayMs))
            }
            recoveryEvents.writes.push(cloneJson((args.metadata as Record<string, unknown>) ?? {}))
            return null
          case 'plugin:window|destroy':
            recoveryEvents.destroyed += 1
            return null
          case 'discard_recovery_snapshot':
            recoveryEvents.discardAttempts.push(String(args.recoveryId))
            if (
              recoveryEvents.discardAttempts.length > (options.discardFailuresAfter ?? 0)
              && discardFailures > 0
            ) {
              discardFailures -= 1
              throw new Error('injected recovery cleanup failure')
            }
            return null
          case 'restore_recovery_snapshot':
            recoveryEvents.restored.push(String(args.recoveryId))
            if (options.recoveryRestoreFails) throw new Error('injected recovery restore failure')
            return {
              sheets: cloneJson(sheets),
              metadata: { sheet_states: {} },
            }
          case 'begin_workbook_transaction':
            pending = snapshot((args.metadata as Record<string, unknown>) ?? {})
            return { can_undo: undoStack.length > 0, can_redo: redoStack.length > 0 }
          case 'commit_workbook_transaction': {
            if (!pending) throw new Error('No transaction is active')
            undoStack.push({
              before: pending,
              after: snapshot((args.metadata as Record<string, unknown>) ?? {}),
            })
            pending = null
            redoStack.length = 0
            return { can_undo: true, can_redo: false }
          }
          case 'abort_workbook_transaction':
            if (pending) restore(pending)
            pending = null
            return { can_undo: undoStack.length > 0, can_redo: redoStack.length > 0 }
          case 'undo_workbook_transaction': {
            const transaction = undoStack.pop()
            if (!transaction) throw new Error('Nothing to undo')
            restore(transaction.before)
            redoStack.push(transaction)
            return {
              sheets: cloneJson(sheets),
              metadata: cloneJson(transaction.before.metadata),
              can_undo: undoStack.length > 0,
              can_redo: true,
            }
          }
          case 'redo_workbook_transaction': {
            const transaction = redoStack.pop()
            if (!transaction) throw new Error('Nothing to redo')
            restore(transaction.after)
            undoStack.push(transaction)
            return {
              sheets: cloneJson(sheets),
              metadata: cloneJson(transaction.after.metadata),
              can_undo: true,
              can_redo: redoStack.length > 0,
            }
          }
          case 'new_workbook':
            cells.clear()
            for (let index = 0; index < (options.populatedCellCount ?? 0); index++) {
              cells.set(`${Math.floor(index / 100)}:${index % 100}`, { value: String(index), display: String(index) })
            }
            formats.clear()
            sheets.splice(0, sheets.length, ...cloneJson(initialSheets))
            undoStack.length = 0
            redoStack.length = 0
            return sheets
          case 'set_active_sheet':
            if ((options.setActiveSheetDelayMs ?? 0) > 0) {
              await new Promise((resolve) => setTimeout(resolve, options.setActiveSheetDelayMs))
            }
            return null
          case 'add_generated_sheet': {
            const nextId = sheets.length
            const usedNames = new Set(sheets.map((sheet) => sheet.name.toLowerCase()))
            let suffix = sheets.length + 1
            while (usedNames.has(`sheet${suffix}`)) suffix++
            sheets.push({ id: nextId, stable_id: nextId + 1, name: `Sheet${suffix}` })
            return sheets
          }
          case 'delete_sheet': {
            const index = Number(args.sheetId)
            if (sheets.length <= 1 || index < 0 || index >= sheets.length) {
              throw new Error('Cannot delete sheet')
            }
            sheets.splice(index, 1)
            sheets.forEach((sheet, sheetIndex) => { sheet.id = sheetIndex })
            return cloneJson(sheets)
          }
          case 'export_native_file':
            {
              const captured = snapshot((args.metadata as Record<string, unknown>) ?? {})
              if ((options.nativeSaveDelayMs ?? 0) > 0) {
                await new Promise((resolve) => setTimeout(resolve, options.nativeSaveDelayMs))
              }
              savedNative = captured
              recoveryEvents.nativeSaves.push(cloneJson(savedNative.metadata))
              return { backup: { id: 'backup-1' } }
            }
          case 'import_native_file':
            if (!savedNative) throw new Error('No saved native workbook')
            if ((options.nativeOpenDelayMs ?? 0) > 0) {
              await new Promise((resolve) => setTimeout(resolve, options.nativeOpenDelayMs))
            }
            restore(savedNative)
            undoStack.length = 0
            redoStack.length = 0
            return {
              sheets: cloneJson(sheets),
              metadata: cloneJson(savedNative.metadata),
            }
          case 'get_sheet_data': {
            const sheetId = String(args.sheetId ?? 0)
            const delay = options.sheetDataDelayMsById?.[sheetId] ?? 0
            if (delay > 0) await new Promise((resolve) => setTimeout(resolve, delay))
            const source = new Map(sheetCells.get(sheetId) ?? cells)
            for (const key of formats.keys()) {
              if (!source.has(key)) source.set(key, { value: '', display: '' })
            }
            return Array.from(source.entries()).map(([key, cell]) => {
              const [row, col] = key.split(':').map(Number)
              return {
                row,
                col,
                value: cell.value,
                display: cell.display,
                cell_type: cell.value.startsWith('=') ? 'formula' : 'text',
                format: formats.get(key) ?? null,
                ...(options.cellEvaluations?.[key] ?? {}),
              }
            })
          }
          case 'set_cell': {
            const value = String(args.value ?? '')
            const source = sheetCells.get(String(args.sheetId ?? 0)) ?? cells
            source.set(keyFor(args.row, args.col), { value, display: value })
            return null
          }
          case 'move_cells':
          case 'batch_set_cells': {
            if (cmd === 'move_cells') {
              if ([cells, ...sheetCells.values()].some((source) => Array.from(source.values()).some((cell) => cell.value.startsWith('=')))) {
                throw new Error('Cut cannot preserve formula dependencies yet; use Copy instead')
              }
              const source = args.source as Array<{ row: number; col: number; value: string }>
              if (source.some((cell) => (cells.get(keyFor(cell.row, cell.col))?.value ?? '') !== cell.value)) {
                throw new Error('The cut source changed; source cells have been kept')
              }
            }
            const changes = (args.changes as Array<{ row: number; col: number; value: string }>) ?? []
            for (const change of changes) {
              const key = keyFor(change.row, change.col)
              if (change.value) {
                cells.set(key, { value: change.value, display: change.value })
              } else {
                cells.delete(key)
              }
            }
            return null
          }
          case 'workbook_has_formulas':
            return [cells, ...sheetCells.values()].some((source) => Array.from(source.values()).some((cell) => cell.value.startsWith('=')))
          case 'clear_cell':
            cells.delete(keyFor(args.row, args.col))
            formats.delete(keyFor(args.row, args.col))
            return null
          case 'get_cell_format':
            return formats.get(keyFor(args.row, args.col)) ?? null
          case 'set_cell_format':
            formats.set(keyFor(args.row, args.col), (args.format as Record<string, unknown>) ?? {})
            return null
          case 'batch_set_formats': {
            const changes = (args.changes as Array<{ row: number; col: number; format: Record<string, unknown> }>) ?? []
            for (const change of changes) {
              formats.set(keyFor(change.row, change.col), change.format)
            }
            return null
          }
          case 'get_cell_comment':
            return comments.get(commentKeyFor(args.sheetId, args.row, args.col)) ?? null
          case 'find_conditional_format_matches':
            return []
          case 'list_comments':
            return Array.from(comments.entries())
              .filter(([key]) => key.startsWith(`${Number(args.sheetId)}:`))
              .map(([, comment]) => comment)
          case 'add_cell_comment':
            comments.set(commentKeyFor(args.sheetId, args.row, args.col), {
              row: Number(args.row),
              col: Number(args.col),
              text: String(args.text ?? ''),
              author: String(args.author ?? ''),
            })
            return null
          case 'remove_cell_comment':
            return comments.delete(commentKeyFor(args.sheetId, args.row, args.col))
          default:
            throw new Error(`Unrecognized Tauri command in test mock: ${cmd}`)
        }
      },
    }
  }, options)
}

async function openWorkbook(page: Page) {
  await installTauriMock(page)
  await page.goto('/')
  await expect(page.locator('button.cell[aria-label^="A1,"]')).toBeVisible()
}

function cell(page: Page, label: string) {
  return page.locator(`button.cell[aria-label^="${label},"]`)
}

async function enterCellText(page: Page, label: string, value: string) {
  const target = cell(page, label)
  await target.click()
  await target.press(value[0])

  const editor = page.locator('input.cell-input')
  await expect(editor).toHaveValue(value[0])

  for (const char of value.slice(1)) {
    await editor.press(char)
  }

  await expect(editor).toHaveValue(value)
  await editor.press('Enter')
  await expect(target).toHaveText(value)
}

test('typing into a selected cell preserves the first character', async ({ page }) => {
  await openWorkbook(page)

  const a1 = cell(page, 'A1')
  await expect(a1).toHaveAccessibleName('A1, blank, selected')
  await a1.click()
  await a1.press('1')

  const editor = page.locator('input.cell-input')
  await expect(editor).toHaveValue('1')
  await expect(editor).toHaveJSProperty('selectionStart', 1)
  await expect(editor).toHaveJSProperty('selectionEnd', 1)

  await editor.press('2')
  await expect(editor).toHaveValue('12')

  await editor.press('Enter')
  await expect(a1).toHaveText('12')
  await expect(a1).toHaveAccessibleName('A1, 12, not selected')
})

test('first run offers local templates and records completion without an account', async ({ page }) => {
  await installTauriMock(page, { showFirstRun: true })
  await page.goto('/')

  await expect(page.getByRole('heading', { name: 'Make a useful workbook' })).toBeVisible()
  await page.getByRole('button', { name: /Household Planning/ }).click()
  await expect(page.getByRole('heading', { name: 'Make a useful workbook' })).toHaveCount(0)
  await expect(cell(page, 'A1')).toHaveText('Household Plan')
  await expect.poll(() => page.evaluate(() => localStorage.getItem('900sheets.first-run.v1'))).toBe('complete')
})

test('an edit committed during a slow save remains dirty and recoverable', async ({ page }) => {
  await installTauriMock(page, { nativeSaveDelayMs: 1_500 })
  await page.goto('/')
  await expect(cell(page, 'A1')).toBeVisible()
  await enterCellText(page, 'A1', 'before save')

  await page.keyboard.press('Control+S')
  await page.waitForTimeout(50)
  await enterCellText(page, 'B1', 'newer edit')

  await expect(page.locator('.toolbar-status')).toContainText('newer edits pending')
  await expect(page.locator('.app-title')).toContainText('•')
  await expect.poll(() => page.evaluate(() => {
    const writes = (window as Window & { __RECOVERY_TEST__?: { writes: unknown[] } })
      .__RECOVERY_TEST__?.writes ?? []
    return writes.length
  }), { timeout: 3_000 }).toBeGreaterThan(0)
  await expect(cell(page, 'B1')).toHaveText('newer edit')
})

test('a slow save cannot take ownership of a replacement workbook session', async ({ page }) => {
  await installTauriMock(page, { nativeSaveDelayMs: 1_500 })
  await page.goto('/')
  await expect(cell(page, 'A1')).toBeVisible()
  await enterCellText(page, 'A1', 'old workbook')
  const cleanupCountBefore = await page.evaluate(() => {
    const events = (window as Window & { __RECOVERY_TEST__?: { discardAttempts: string[] } })
      .__RECOVERY_TEST__
    return events?.discardAttempts.length ?? 0
  })

  await page.keyboard.press('Control+S')
  await page.waitForTimeout(50)
  await page.locator('.menu-bar button').filter({ hasText: /^File$/ }).click()
  const confirmation = page.waitForEvent('dialog')
  const newWorkbook = page.getByRole('button', { name: 'New Workbook' }).click()
  const dialog = await confirmation
  await dialog.accept()
  await newWorkbook

  await expect(cell(page, 'A1')).toHaveText('')
  await expect(page.locator('.toolbar-status')).toContainText('previous workbook finished saving')
  await expect(page.locator('.app-title')).not.toContainText('•')
  await expect.poll(() => page.evaluate(() => {
    const events = (window as Window & { __RECOVERY_TEST__?: { discardAttempts: string[] } })
      .__RECOVERY_TEST__
    return events?.discardAttempts.length ?? 0
  })).toBe(cleanupCountBefore + 1)
})

test('new sheet uses the backend generated name path', async ({ page }) => {
  await openWorkbook(page)

  await page.locator('button.sheet-tab-add').click()

  await expect(page.getByRole('button', { name: 'Sheet3', exact: true })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Sheet3', exact: true })).toHaveClass(/active/)
})

test('delete and backspace clear the selected cell visibly', async ({ page }) => {
  await openWorkbook(page)

  const a1 = cell(page, 'A1')
  await enterCellText(page, 'A1', 'abc')

  await a1.click()
  await a1.press('Delete')
  await expect(a1).toHaveText('')

  await enterCellText(page, 'A1', 'xyz')

  await a1.click()
  await a1.press('Backspace')
  await expect(a1).toHaveText('')
})

test('formula bar typing and enter do not leak into inline cell editing', async ({ page }) => {
  await openWorkbook(page)

  const formulaInput = page.getByRole('textbox', { name: 'Formula bar' })
  await formulaInput.click()
  await formulaInput.press('h')
  await formulaInput.press('i')

  await expect(formulaInput).toHaveValue('hi')
  await expect(page.locator('input.cell-input')).toHaveCount(0)

  await formulaInput.press('Enter')
  await expect(cell(page, 'A1')).toHaveText('hi')
  await expect(page.locator('input.cell-input')).toHaveCount(0)
})

test('formula bar drafts commit before navigation and native save', async ({ page }) => {
  await openWorkbook(page)

  const formulaInput = page.getByRole('textbox', { name: 'Formula bar' })
  await formulaInput.fill('draft for A1')
  await cell(page, 'B1').click()
  await expect(cell(page, 'A1')).toHaveText('draft for A1')

  await formulaInput.fill('saved from B1')
  await page.keyboard.press('Control+S')
  await expect(cell(page, 'B1')).toHaveText('saved from B1')
  await expect(page.locator('.toolbar-status')).toContainText('Saved mock-workbook.900sheets')
})

test('sheet switching never writes a stale formula bar value into the destination', async ({ page }) => {
  await installTauriMock(page, {
    sheetDataById: {
      0: { '0:0': 'one' },
      1: { '0:0': 'two' },
    },
  })
  await page.goto('/')
  await expect(cell(page, 'A1')).toHaveText('one')

  await page.getByRole('button', { name: 'Sheet2', exact: true }).click()
  await expect(cell(page, 'A1')).toHaveText('two')
  await page.getByRole('button', { name: 'Sheet1', exact: true }).click()
  await expect(cell(page, 'A1')).toHaveText('one')
  await page.getByRole('button', { name: 'Sheet2', exact: true }).click()
  await expect(cell(page, 'A1')).toHaveText('two')
})

test('canceling Open after a delayed sheet switch keeps tab and cells paired', async ({ page }) => {
  await installTauriMock(page, {
    dialogOpenResult: null,
    sheetDataById: {
      0: { '0:0': 'one' },
      1: { '0:0': 'two' },
    },
    sheetDataDelayMsById: { 1: 350 },
  })
  await page.goto('/')
  await expect(cell(page, 'A1')).toHaveText('one')

  await page.getByRole('button', { name: 'Sheet2', exact: true }).click()
  await page.locator('.menu-bar button').filter({ hasText: /^File$/ }).click()
  await page.getByRole('button', { name: 'Open 900Sheets Workbook...' }).click()

  await expect(page.getByRole('button', { name: 'Sheet2', exact: true })).toHaveClass(/active/)
  await expect(cell(page, 'A1')).toHaveText('two')
  await expect(page.getByRole('status', { name: 'Workbook replacement in progress' })).toHaveCount(0)
})

test('slow replacement blocks edits until the restored workbook is coherent', async ({ page }) => {
  await installTauriMock(page, { nativeOpenDelayMs: 700 })
  await page.goto('/')
  await enterCellText(page, 'A1', 'saved baseline')
  await page.keyboard.press('Control+S')
  await expect(page.locator('.toolbar-status')).toContainText('Saved mock-workbook.900sheets')
  await enterCellText(page, 'A1', 'discard this')

  await page.locator('.menu-bar button').filter({ hasText: /^File$/ }).click()
  const confirmation = page.waitForEvent('dialog')
  const opening = page.getByRole('button', { name: 'Open 900Sheets Workbook...' }).click()
  const dialog = await confirmation
  await dialog.accept()
  await opening
  await expect(page.getByRole('status', { name: 'Workbook replacement in progress' })).toBeVisible()
  await page.keyboard.press('x')

  await expect(page.getByRole('status', { name: 'Workbook replacement in progress' })).toHaveCount(0)
  await expect(cell(page, 'A1')).toHaveText('saved baseline')
  await expect(page.locator('input.cell-input')).toHaveCount(0)
})

test('invalid inline edits block navigation and native save', async ({ page }) => {
  await openWorkbook(page)
  await page.locator('.menu-bar button').filter({ hasText: /^Data$/ }).click()
  await page.getByRole('button', { name: 'Data Validation...' }).click()
  await page.getByLabel('Rule type').selectOption('List')
  await page.getByLabel('Allowed values').fill('Allowed')
  await page.getByRole('button', { name: 'Save Rule' }).click()
  await page.getByRole('button', { name: 'Close' }).click()

  await cell(page, 'A1').dblclick()
  await page.locator('input.cell-input').fill('Blocked')
  await page.locator('input.cell-input').press('Enter')
  await expect(page.locator('input.cell-input')).toHaveValue('Blocked')
  await expect(page.locator('.toolbar-status')).toContainText('Validation failed')

  await cell(page, 'B1').click()
  await expect(page.locator('input.cell-input')).toHaveValue('Blocked')
  await page.keyboard.press('Control+S')
  await expect.poll(() => page.evaluate(() =>
    (window as Window & { __RECOVERY_TEST__?: { nativeSaves: unknown[] } }).__RECOVERY_TEST__?.nativeSaves.length ?? 0
  )).toBe(0)
})

test('close barrier blocks late edits while the final recovery snapshot is written', async ({ page }) => {
  await installTauriMock(page, { recoveryWriteDelayMs: 600 })
  await page.goto('/')
  await enterCellText(page, 'A1', 'preserve me')

  await page.evaluate(() => (window as Window & { __TRIGGER_CLOSE__?: () => void }).__TRIGGER_CLOSE__?.())
  await expect(page.getByRole('status', { name: 'Workbook close in progress' })).toBeVisible()
  await page.keyboard.press('x')
  await expect(page.locator('input.cell-input')).toHaveCount(0)
  await expect.poll(() => page.evaluate(() =>
    (window as Window & { __RECOVERY_TEST__?: { destroyed: number } }).__RECOVERY_TEST__?.destroyed ?? 0
  )).toBe(1)
})

test('Go To reaches the maximum grid address with a bounded DOM', async ({ page }) => {
  await openWorkbook(page)

  const grid = page.getByRole('grid', { name: 'Spreadsheet grid' })
  await expect(grid).toHaveAttribute('aria-rowcount', '1000000')
  await expect(grid).toHaveAttribute('aria-colcount', '16384')

  for (let step = 0; step < 5; step++) {
    await page.locator('.menu-bar button').filter({ hasText: /^View$/ }).click()
    await page.locator('.app-menu').getByRole('button', { name: 'Zoom In', exact: true }).click()
  }
  await expect(page.locator('.status-bar')).toContainText('150%')

  await page.keyboard.press('Control+G')
  const goTo = page.getByRole('textbox', { name: 'Go to cell or range' })
  await expect(goTo).toBeFocused()
  await goTo.fill('XFD1000000')
  await goTo.press('Enter')

  await expect(grid).toHaveAttribute('aria-activedescendant', 'grid-cell-999999-16383')
  await expect(grid).toBeFocused()
  const lastCell = page.locator('#grid-cell-999999-16383')
  await expect(lastCell).toBeVisible()
  await expect(lastCell).toHaveAttribute('aria-rowindex', '1000000')
  await expect(lastCell).toHaveAttribute('aria-colindex', '16384')
  expect(await page.locator('button.cell').count()).toBeLessThan(2500)
  await page.keyboard.press('F2')
  await expect(page.locator('input.cell-input')).toHaveAttribute('aria-label', 'Cell XFD1000000')
  await page.keyboard.press('Escape')
})

test('Go To accepts ranges and rejects addresses outside the workbook bounds', async ({ page }) => {
  await openWorkbook(page)
  const goTo = page.getByRole('textbox', { name: 'Go to cell or range' })

  for (const invalid of ['XFE1', 'A1000001', 'A0']) {
    await goTo.fill(invalid)
    await goTo.press('Enter')
    await expect(page.locator('.toolbar-status')).toContainText('Go To failed')
    await expect(goTo).toBeFocused()
  }

  await goTo.fill('D20:B10')
  await goTo.press('Enter')
  await expect(page.locator('.status-bar')).toContainText('B10:D20')
  await expect(page.getByRole('grid')).toHaveAttribute('aria-activedescendant', 'grid-cell-9-1')
})

test('grid uses one keyboard focus target across virtualized navigation and zoom', async ({ page }) => {
  await openWorkbook(page)
  const grid = page.getByRole('grid')
  await grid.focus()

  await page.keyboard.press('ArrowRight')
  await page.keyboard.press('ArrowDown')
  await expect(grid).toBeFocused()
  await expect(grid).toHaveAttribute('aria-activedescendant', 'grid-cell-1-1')
  await expect(page.locator('button.cell[tabindex="0"]')).toHaveCount(0)

  await page.locator('.menu-bar button').filter({ hasText: /^View$/ }).click()
  await page.locator('.app-menu').getByRole('button', { name: 'Zoom In', exact: true }).click()
  await page.locator('.menu-bar button').filter({ hasText: /^View$/ }).click()
  await page.locator('.app-menu').getByRole('button', { name: 'Zoom In', exact: true }).click()
  await grid.focus()
  await page.keyboard.press('PageDown')
  await expect(grid).toBeFocused()
  await expect(page.locator('.cell.active')).toBeVisible()

  await page.keyboard.press('Shift+Tab')
  await expect(page.getByRole('textbox', { name: 'Formula bar' })).toBeFocused()

  await grid.focus()
  await page.keyboard.press('Tab')
  await expect(grid).not.toBeFocused()
})

test('single-cell keyboard navigation stays responsive with 50,000 populated cells', async ({ page }) => {
  await installTauriMock(page, { populatedCellCount: 50_000 })
  await page.goto('/')
  const grid = page.getByRole('grid')
  await expect(cell(page, 'A1')).toHaveText('0')
  await grid.focus()

  const started = Date.now()
  for (let step = 0; step < 50; step++) await page.keyboard.press('ArrowDown')
  expect(Date.now() - started).toBeLessThan(3_000)
  await expect(grid).toHaveAttribute('aria-activedescendant', 'grid-cell-50-0')
  expect(await page.locator('button.cell').count()).toBeLessThan(2500)
})

test('locale setting persists verified grid accessibility labels', async ({ page }) => {
  await openWorkbook(page)

  await page.locator('.menu-bar button').filter({ hasText: /^Tools$/ }).click()
  await page.getByRole('button', { name: 'Locale Settings...' }).click()
  const dialog = page.getByRole('dialog', { name: 'Locale Settings' })
  await expect(dialog).toBeVisible()
  await page.getByLabel('Interface language').selectOption('sv')
  await expect(page.getByRole('grid', { name: 'Kalkylblad' })).toBeVisible()
  await expect(cell(page, 'A1')).toHaveAccessibleName('A1, tom, markerad')
  await page.keyboard.press('Escape')
  await expect(dialog).toHaveCount(0)

  await page.reload()
  await expect(page.getByRole('grid', { name: 'Kalkylblad' })).toBeVisible()
})

test('dense operations fail quickly for selections above the shared safety budget', async ({ page }) => {
  await openWorkbook(page)
  const goTo = page.getByRole('textbox', { name: 'Go to cell or range' })
  await goTo.fill('A1:XFD1000000')
  await goTo.press('Enter')

  await page.keyboard.press('Control+C')
  await expect(page.locator('.toolbar-status')).toContainText('Copy selection unavailable')
  await expect(page.locator('.toolbar-status')).toContainText('200,000 cells')

  await page.keyboard.press('Control+B')
  await expect(page.locator('.toolbar-status')).toContainText('Format selection unavailable')

  await page.locator('.menu-bar button').filter({ hasText: /^Data$/ }).click()
  await page.getByRole('button', { name: 'Data Validation...' }).click()
  await page.getByRole('button', { name: /^Validate A1:XFD1000000$/ }).click()
  await expect(page.locator('.toolbar-status')).toContainText('Validate range unavailable')
})

test('freeze panes leaves a scrollable area in the current viewport', async ({ page }) => {
  await openWorkbook(page)
  const goTo = page.getByRole('textbox', { name: 'Go to cell or range' })

  await goTo.fill('C3')
  await goTo.press('Enter')
  await page.locator('.menu-bar button').filter({ hasText: /^View$/ }).click()
  await page.getByRole('button', { name: 'Freeze Panes at Selection' }).click()
  await expect(page.locator('.status-bar')).toContainText('Frozen 2R/2C')
  expect(await page.locator('button.cell').count()).toBeLessThan(2500)

  await goTo.fill('U21')
  await goTo.press('Enter')
  await page.locator('.menu-bar button').filter({ hasText: /^View$/ }).click()
  await page.getByRole('button', { name: 'Freeze Panes at Selection' }).click()
  await expect(page.locator('.toolbar-status')).toContainText('Freeze panes failed')
  await expect(page.locator('.status-bar')).toContainText('Frozen 2R/2C')
})

test('fx menu inserts functions with the cursor ready for arguments', async ({ page }) => {
  await openWorkbook(page)

  await cell(page, 'A1').click()
  await page.locator('button[title="Insert function"]').click()

  const formulaMenu = page.locator('.formula-menu')
  await expect(formulaMenu).toBeVisible()
  await formulaMenu.getByRole('button', { name: 'SUM', exact: true }).click()

  const editor = page.locator('input.cell-input')
  await expect(editor).toHaveValue('=SUM()')
  await expect(editor).toHaveJSProperty('selectionStart', 5)
  await expect(editor).toHaveJSProperty('selectionEnd', 5)

  await editor.press('A')
  await expect(editor).toHaveValue('=SUM(A)')
})

test('copy paste undo and redo update visible cells', async ({ page }) => {
  await openWorkbook(page)

  await enterCellText(page, 'A1', 'copy-me')

  const a1 = cell(page, 'A1')
  const b1 = cell(page, 'B1')

  await a1.click()
  await page.keyboard.press('Control+C')

  await b1.click()
  await page.keyboard.press('Control+V')
  await expect(b1).toHaveText('copy-me')

  await page.keyboard.press('Control+Z')
  await expect(b1).toHaveText('')

  await page.keyboard.press('Control+Y')
  await expect(b1).toHaveText('copy-me')
})

test('compact toolbar menus stay inside the viewport', async ({ page }) => {
  await page.setViewportSize({ width: 900, height: 700 })
  await openWorkbook(page)

  const dataMenu = page.locator('button[title="Data tools"]')
  await expect(dataMenu).toBeVisible()
  await dataMenu.click()

  const popover = page.locator('.toolbar-popover')
  await expect(popover).toBeVisible()
  await expect(popover).toContainText('Remove Duplicates')

  const box = await popover.boundingBox()
  const viewport = page.viewportSize()
  expect(box).not.toBeNull()
  expect(viewport).not.toBeNull()
  expect(box!.x).toBeGreaterThanOrEqual(0)
  expect(box!.y).toBeGreaterThanOrEqual(0)
  expect(box!.x + box!.width).toBeLessThanOrEqual(viewport!.width)
  expect(box!.y + box!.height).toBeLessThanOrEqual(viewport!.height)
})

test('named range metadata stays isolated by stable sheet identity', async ({ page }) => {
  await openWorkbook(page)

  await page.locator('.menu-bar button').filter({ hasText: /^Data$/ }).click()
  await page.getByRole('button', { name: 'Named Ranges...' }).click()
  const nameInput = page.getByLabel('Name', { exact: true })
  await nameInput.fill('Revenue')
  await page.getByRole('button', { name: 'Add Named Range' }).click()
  await expect(page.getByText('Revenue', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Close' }).click()

  await page.getByRole('button', { name: 'Sheet2', exact: true }).click()
  await page.locator('.menu-bar button').filter({ hasText: /^Data$/ }).click()
  await page.getByRole('button', { name: 'Named Ranges...' }).click()
  await expect(page.getByText('Revenue', { exact: true })).toHaveCount(0)
  await page.getByRole('button', { name: 'Close' }).click()

  await page.getByRole('button', { name: 'Sheet1', exact: true }).click()
  await page.locator('.menu-bar button').filter({ hasText: /^Data$/ }).click()
  await page.getByRole('button', { name: 'Named Ranges...' }).click()
  await expect(page.getByText('Revenue', { exact: true })).toBeVisible()
})

test('view and print settings are dirty, undoable, and included in save and recovery metadata', async ({ page }) => {
  await openWorkbook(page)
  const app = page.locator('.app')
  const title = page.locator('.app-title')
  const openPrintPanel = async () => {
    await page.keyboard.press('Control+P')
    await expect(page.getByLabel('Page size')).toBeVisible()
  }

  await page.locator('.menu-bar button').filter({ hasText: /^View$/ }).click()
  await page.getByRole('button', { name: 'Toggle Gridlines' }).click()
  await expect(app).toHaveClass(/no-gridlines/)
  await expect(title).toContainText('•')

  await openPrintPanel()
  await page.getByLabel('Page size').selectOption('A4')
  await page.getByLabel('Orientation').selectOption('Landscape')
  await expect(page.getByLabel('Page size')).toHaveValue('A4')
  await expect(page.getByLabel('Orientation')).toHaveValue('Landscape')
  await page.getByRole('button', { name: 'Close' }).click()

  await page.keyboard.press('Control+Z')
  await openPrintPanel()
  await expect(page.getByLabel('Page size')).toHaveValue('A4')
  await expect(page.getByLabel('Orientation')).toHaveValue('Portrait')
  await page.getByRole('button', { name: 'Close' }).click()

  await page.keyboard.press('Control+Z')
  await openPrintPanel()
  await expect(page.getByLabel('Page size')).toHaveValue('Letter')
  await expect(page.getByLabel('Orientation')).toHaveValue('Portrait')
  await page.getByRole('button', { name: 'Close' }).click()

  await page.keyboard.press('Control+Z')
  await expect(app).not.toHaveClass(/no-gridlines/)

  await page.keyboard.press('Control+Y')
  await page.keyboard.press('Control+Y')
  await page.keyboard.press('Control+Y')
  await expect(app).toHaveClass(/no-gridlines/)
  await openPrintPanel()
  await expect(page.getByLabel('Page size')).toHaveValue('A4')
  await expect(page.getByLabel('Orientation')).toHaveValue('Landscape')
  await page.getByRole('button', { name: 'Close' }).click()

  await expect.poll(() => page.evaluate(() => {
    const writes = (window as Window & { __RECOVERY_TEST__?: { writes: Array<Record<string, unknown>> } })
      .__RECOVERY_TEST__?.writes ?? []
    const metadata = writes.at(-1) ?? {}
    const settings = (metadata.sheet_states as Record<string, Record<string, unknown>>)?.['1']
    return settings
      ? [settings.showGridlines, settings.printPageSize, settings.printOrientation]
      : null
  })).toEqual([false, 'A4', 'Landscape'])

  const recoverySettings = await page.evaluate(() => {
    const writes = (window as Window & { __RECOVERY_TEST__?: { writes: Array<Record<string, unknown>> } })
      .__RECOVERY_TEST__?.writes ?? []
    const metadata = writes.at(-1) ?? {}
    return (metadata.sheet_states as Record<string, Record<string, unknown>>)?.['1']
  })
  expect(recoverySettings).toMatchObject({
    showGridlines: false,
    printPageSize: 'A4',
    printOrientation: 'Landscape',
  })

  await page.locator('.menu-bar button').filter({ hasText: /^File$/ }).click()
  await page.getByRole('button', { name: 'Save Workbook' }).click()
  await expect(title).not.toContainText('•')
  const savedSettings = await page.evaluate(() => {
    const saves = (window as Window & { __RECOVERY_TEST__?: { nativeSaves: Array<Record<string, unknown>> } })
      .__RECOVERY_TEST__?.nativeSaves ?? []
    const metadata = saves.at(-1) ?? {}
    return (metadata.sheet_states as Record<string, Record<string, unknown>>)?.['1']
  })
  expect(savedSettings).toMatchObject({
    showGridlines: false,
    printPageSize: 'A4',
    printOrientation: 'Landscape',
  })
})

test('Open XLSX is replacement wording and requires discard confirmation', async ({ page }) => {
  await openWorkbook(page)
  await enterCellText(page, 'A1', 'unsaved')

  await page.locator('.menu-bar button').filter({ hasText: /^File$/ }).click()
  const openXlsx = page.getByRole('button', { name: 'Open XLSX...' })
  await expect(openXlsx).toBeVisible()
  const confirmation = page.waitForEvent('dialog')
  const click = openXlsx.click()
  const dialog = await confirmation
  expect(dialog.message()).toContain('unsaved changes')
  expect(dialog.message()).toContain('Discard')
  await dialog.dismiss()
  await click

  await expect(cell(page, 'A1')).toHaveText('unsaved')
})

test('restoring one recovery preserves every unselected snapshot', async ({ page }) => {
  await installTauriMock(page, {
    recoveries: [
      { id: 'newest', modified_millis: 2_000 },
      { id: 'older', modified_millis: 1_000 },
    ],
  })
  page.on('dialog', async (dialog) => dialog.accept())
  await page.goto('/')
  await expect(page.locator('.toolbar-status')).toContainText('Restored autosaved recovery')

  const events = await page.evaluate(() =>
    (window as Window & { __RECOVERY_TEST__?: { discardAttempts: string[]; restored: string[] } })
      .__RECOVERY_TEST__
  )
  expect(events?.restored).toEqual(['newest'])
  expect(events?.discardAttempts).toEqual([])
})

test('deferring a startup recovery preserves it while restoring another', async ({ page }) => {
  await installTauriMock(page, {
    recoveries: [
      { id: 'keep-for-later', modified_millis: 2_000 },
      { id: 'restore-me', modified_millis: 1_000 },
    ],
  })
  let confirmation = 0
  page.on('dialog', async (dialog) => {
    if (dialog.type() === 'alert') return dialog.accept()
    confirmation += 1
    if (confirmation === 1) return dialog.dismiss()
    return dialog.accept()
  })
  await page.goto('/')
  await expect(page.locator('.toolbar-status')).toContainText('Restored autosaved recovery')

  const events = await page.evaluate(() =>
    (window as Window & { __RECOVERY_TEST__?: { discardAttempts: string[]; restored: string[] } })
      .__RECOVERY_TEST__
  )
  expect(events?.discardAttempts).toEqual([])
  expect(events?.restored).toEqual(['restore-me'])
})

test('replacement cleanup failure retains identity and Save retries cleanup', async ({ page }) => {
  await installTauriMock(page, { discardFailures: 1, discardFailuresAfter: 1 })
  await page.goto('/')
  await expect(cell(page, 'A1')).toBeVisible()
  await enterCellText(page, 'A1', 'replace-me')

  page.once('dialog', async (dialog) => dialog.accept())
  await page.locator('.menu-bar button').filter({ hasText: /^File$/ }).click()
  await page.getByRole('button', { name: 'New Workbook' }).click()
  await expect(page.locator('.toolbar-status')).toContainText('Replacement succeeded, but recovery cleanup failed')
  await expect(page.locator('.toolbar-status')).toContainText('Save Workbook to retry')

  await page.locator('.menu-bar button').filter({ hasText: /^File$/ }).click()
  await page.getByRole('button', { name: 'Save Workbook' }).click()
  await expect(page.locator('.toolbar-status')).toContainText('Saved mock-workbook.900sheets')
  const attempts = await page.evaluate(() =>
    (window as Window & { __RECOVERY_TEST__?: { discardAttempts: string[] } })
      .__RECOVERY_TEST__?.discardAttempts ?? []
  )
  expect(attempts).toHaveLength(4)
  expect(attempts[1]).toBe(attempts[2])
  expect(attempts[3]).not.toBe(attempts[2])
})

test('save cleanup failure is visible and retryable under the same recovery identity', async ({ page }) => {
  await installTauriMock(page, { discardFailures: 1, discardFailuresAfter: 1 })
  await page.goto('/')
  await expect(cell(page, 'A1')).toBeVisible()
  await enterCellText(page, 'A1', 'save-me')

  await page.locator('.menu-bar button').filter({ hasText: /^File$/ }).click()
  await page.getByRole('button', { name: 'Save Workbook' }).click()
  await expect(page.locator('.toolbar-status')).toContainText('Workbook saved, but recovery cleanup failed')
  await expect(page.locator('.toolbar-status')).toContainText('Save Workbook to retry')

  await page.locator('.menu-bar button').filter({ hasText: /^File$/ }).click()
  await page.getByRole('button', { name: 'Save Workbook' }).click()
  await expect(page.locator('.toolbar-status')).toContainText('Saved mock-workbook.900sheets')
  const attempts = await page.evaluate(() =>
    (window as Window & { __RECOVERY_TEST__?: { discardAttempts: string[] } })
      .__RECOVERY_TEST__?.discardAttempts ?? []
  )
  expect(attempts).toHaveLength(4)
  expect(attempts[1]).toBe(attempts[2])
  expect(attempts[3]).not.toBe(attempts[2])
})

test('delete active sheet undo redo save and reopen does not resurrect stale metadata', async ({ page }) => {
  await openWorkbook(page)
  await page.locator('button.sheet-tab-add').click()
  await expect(page.getByRole('button', { name: 'Sheet3', exact: true })).toHaveClass(/active/)

  await page.locator('.menu-bar button').filter({ hasText: /^Data$/ }).click()
  await page.getByRole('button', { name: 'Named Ranges...' }).click()
  await page.getByLabel('Name', { exact: true }).fill('TransientRange')
  await page.getByRole('button', { name: 'Add Named Range' }).click()
  await expect(page.getByText('TransientRange', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Close' }).click()

  await page.locator('button.sheet-tab-delete[title="Delete sheet"]').click()
  await expect(page.getByRole('button', { name: 'Sheet3', exact: true })).toHaveCount(0)

  await page.keyboard.press('Control+Z')
  await expect(page.getByRole('button', { name: 'Sheet3', exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Sheet3', exact: true }).click()
  await page.locator('.menu-bar button').filter({ hasText: /^Data$/ }).click()
  await page.getByRole('button', { name: 'Named Ranges...' }).click()
  await expect(page.getByText('TransientRange', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Close' }).click()

  await page.keyboard.press('Control+Y')
  await expect(page.getByRole('button', { name: 'Sheet3', exact: true })).toHaveCount(0)

  await page.locator('.menu-bar button').filter({ hasText: /^File$/ }).click()
  await page.getByRole('button', { name: 'Save Workbook' }).click()
  await expect(page.locator('.toolbar-status')).toContainText('Saved mock-workbook.900sheets')
  await page.locator('.menu-bar button').filter({ hasText: /^File$/ }).click()
  await page.getByRole('button', { name: 'New Workbook' }).click()
  await page.locator('.menu-bar button').filter({ hasText: /^File$/ }).click()
  await page.getByRole('button', { name: 'Open 900Sheets Workbook...' }).click()

  await expect(page.getByRole('button', { name: 'Sheet3', exact: true })).toHaveCount(0)
  await page.locator('.menu-bar button').filter({ hasText: /^Data$/ }).click()
  await page.getByRole('button', { name: 'Named Ranges...' }).click()
  await expect(page.getByText('TransientRange', { exact: true })).toHaveCount(0)
})

test('comment drafts cannot follow cell or sheet selection changes', async ({ page }) => {
  await openWorkbook(page)

  await page.locator('.menu-bar button').filter({ hasText: /^Insert$/ }).click()
  await page.getByRole('button', { name: 'Comment...' }).click()
  const commentInput = page.locator('textarea.panel-input')
  await expect(commentInput).toBeVisible()
  await commentInput.fill('draft for A1')

  await page.locator('.result-row').filter({ hasText: 'B1' }).click()
  await expect(commentInput).toHaveCount(0)

  await page.locator('.menu-bar button').filter({ hasText: /^Insert$/ }).click()
  await page.getByRole('button', { name: 'Comment...' }).click()
  await expect(page.locator('textarea.panel-input')).toHaveValue('existing B1 comment')
  await page.locator('textarea.panel-input').fill('draft for Sheet1 B1')

  const sheet2 = page.getByRole('button', { name: 'Sheet2', exact: true })
  await sheet2.evaluate((button: HTMLButtonElement) => button.click())
  await expect(page.locator('textarea.panel-input')).toHaveCount(0)
  await expect(sheet2).toHaveClass(/active/)

  await page.locator('.menu-bar button').filter({ hasText: /^Insert$/ }).click()
  await page.getByRole('button', { name: 'Comment...' }).click()
  await expect(page.locator('textarea.panel-input')).toHaveValue('')
})

for (const viewport of [{ width: 800, height: 500 }, { width: 1024, height: 768 }, { width: 1280, height: 720 }]) {
  test(`workbook controls remain usable at ${viewport.width}x${viewport.height}`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport)
    await openWorkbook(page)
    for (const locator of [page.locator('.workbook-header'), page.locator('.sheet-tabs'), page.locator('.status-bar'), page.getByRole('button', { name: 'Open', exact: true }), page.getByRole('button', { name: 'Save', exact: true })]) {
      const box = await locator.boundingBox()
      expect(box).not.toBeNull()
      expect(box!.x).toBeGreaterThanOrEqual(0)
      expect(box!.y).toBeGreaterThanOrEqual(0)
      expect(box!.x + box!.width).toBeLessThanOrEqual(viewport.width)
      expect(box!.y + box!.height).toBeLessThanOrEqual(viewport.height)
    }
    expect(await page.locator('.grid-container').evaluate((grid) => grid.clientHeight)).toBeGreaterThan(viewport.height - 240)
    if (viewport.width >= 1024) {
      expect(await page.locator('.format-toolbar').evaluate((toolbar) => toolbar.scrollWidth <= toolbar.clientWidth)).toBe(true)
    }
    await page.getByRole('button', { name: 'Zoom in', exact: true }).click()
    await expect(page.locator('.zoom-value')).toHaveText('110%')
    await page.getByTitle('Reset zoom to 100%').click()
    await expect(page.locator('.zoom-value')).toHaveText('100%')
    await page.locator('button[title="Data tools"]').click()
    await page.locator('.toolbar-menu-item').filter({ hasText: /^Find$/ }).click()
    const panel = await page.getByRole('dialog').boundingBox()
    expect(panel!.x + panel!.width).toBeLessThanOrEqual(viewport.width)
    expect(panel!.y + panel!.height).toBeLessThanOrEqual(viewport.height)
    await page.getByRole('button', { name: 'Close', exact: true }).click()
    await page.screenshot({ path: testInfo.outputPath(`900sheets-ui-${viewport.width}.png`) })
  })
}

test('format controls follow selection and undo instead of retaining the last chosen value', async ({ page }) => {
  await openWorkbook(page)
  await enterCellText(page, 'A1', '123.45')
  await cell(page, 'A1').click()
  const numberFormat = page.getByRole('combobox', { name: 'Number format', exact: true })
  await numberFormat.selectOption('#,##0.00')
  await page.getByRole('button', { name: 'Bold', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Bold', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await page.getByRole('spinbutton', { name: 'Font size', exact: true }).fill('18')
  await page.getByRole('spinbutton', { name: 'Font size', exact: true }).press('Tab')
  await expect(cell(page, 'A1')).toHaveCSS('font-size', '18px')
  await cell(page, 'B1').click()
  await expect(numberFormat).toHaveValue('')
  await expect(page.getByRole('button', { name: 'Bold', exact: true })).toHaveAttribute('aria-pressed', 'false')
  await expect(page.getByRole('spinbutton', { name: 'Font size', exact: true })).toHaveValue('13')
  await cell(page, 'A1').click()
  await expect(numberFormat).toHaveValue('#,##0.00')
  await expect(page.getByRole('spinbutton', { name: 'Font size', exact: true })).toHaveValue('18')
  await page.keyboard.press('Control+Z')
  await expect(page.getByRole('spinbutton', { name: 'Font size', exact: true })).toHaveValue('13')
  await expect(page.getByRole('button', { name: 'Bold', exact: true })).toHaveAttribute('aria-pressed', 'true')
})

test('local first-run template editing and save state work with browser networking offline', async ({ page, context }) => {
  await installTauriMock(page, { showFirstRun: true })
  const externalRequests: string[] = []
  page.on('request', (request) => {
    if (!request.url().startsWith('http://127.0.0.1:1422/') && !request.url().startsWith('data:')) externalRequests.push(request.url())
  })
  await page.goto('/')
  await expect(page.getByRole('dialog')).toBeVisible()
  await context.setOffline(true)
  await page.locator('.template-card').first().click()
  await expect(page.locator('.save-state')).toHaveText('Unsaved changes')
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await expect(page.locator('.workbook-name')).toHaveText('mock-workbook.900sheets')
  await expect(page.locator('.save-state')).toHaveText('Saved to this computer')
  const formula = page.getByRole('textbox', { name: 'Formula bar', exact: true })
  await formula.fill('An offline draft')
  await expect(page.locator('.save-state')).toHaveText('Unsaved changes')
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await expect(page.locator('.save-state')).toHaveText('Saved to this computer')
  await expect(cell(page, 'A1')).toHaveText('An offline draft')
  expect(externalRequests).toEqual([])
})

async function useClipboardReadback(page: Page) {
  await page.evaluate(() => {
    let text = ''
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: {
      writeText: async (value: string) => { text = value },
      readText: async () => text,
    } })
  })
}

test('formula copy rebases mixed references even when system clipboard reads succeed', async ({ page }) => {
  await openWorkbook(page)
  await useClipboardReadback(page)
  await cell(page, 'B2').click()
  const formula = page.getByRole('textbox', { name: 'Formula bar', exact: true })
  await formula.fill('=A1+$B2+C$3+$D$4+LOG10(A1)+"A1"')
  await formula.press('Enter')
  await cell(page, 'B2').click()
  await page.keyboard.press('Control+C')
  await expect(page.locator('.toolbar-status')).toContainText('Copied B2')
  await cell(page, 'C3').click()
  await page.keyboard.press('Control+V')
  await expect(cell(page, 'C3')).toHaveText('=B2+$B3+D$3+$D$4+LOG10(B2)+"A1"')
  await expect(cell(page, 'B2')).toHaveText('=A1+$B2+C$3+$D$4+LOG10(A1)+"A1"')
})

test('cut keeps the source until paste succeeds and the complete move undoes together', async ({ page }) => {
  await openWorkbook(page)
  await useClipboardReadback(page)
  await enterCellText(page, 'A1', 'move-me')
  await cell(page, 'A1').click()
  await page.keyboard.press('Control+X')
  await expect(page.locator('.toolbar-status')).toContainText('Cut ready')
  await expect(cell(page, 'A1')).toHaveText('move-me')
  await cell(page, 'B1').click()
  await page.keyboard.press('Control+V')
  await expect(cell(page, 'B1')).toHaveText('move-me')
  await expect(cell(page, 'A1')).toHaveText('')
  await page.keyboard.press('Control+Z')
  await expect(cell(page, 'A1')).toHaveText('move-me')
  await expect(cell(page, 'B1')).toHaveText('')
})

test('a cut paste backend failure restores source and destination and allows retry', async ({ page }) => {
  await openWorkbook(page)
  await useClipboardReadback(page)
  await enterCellText(page, 'A1', 'keep-me')
  await enterCellText(page, 'B1', 'existing')
  await cell(page, 'A1').click()
  await page.keyboard.press('Control+X')
  await expect(page.locator('.toolbar-status')).toContainText('Cut ready')
  await page.evaluate(() => {
    const runtime = (window as any).__TAURI_INTERNALS__
    const original = runtime.invoke
    let failNextBatch = true
    runtime.invoke = async (command: string, args: Record<string, unknown>) => {
      const result = await original(command, args)
      if (command === 'move_cells' && failNextBatch) {
        failNextBatch = false
        throw new Error('injected paste failure after applying cells')
      }
      return result
    }
  })
  await cell(page, 'B1').click()
  await page.keyboard.press('Control+V')
  await expect(page.locator('.toolbar-status')).toContainText('Unable to paste cells')
  await expect(cell(page, 'A1')).toHaveText('keep-me')
  await expect(cell(page, 'B1')).toHaveText('existing')
  await page.keyboard.press('Control+V')
  await expect(cell(page, 'B1')).toHaveText('keep-me')
  await expect(cell(page, 'A1')).toHaveText('')
})

test('overlapping cut ranges preserve all destination values in order', async ({ page }) => {
  await openWorkbook(page)
  await useClipboardReadback(page)
  await enterCellText(page, 'A1', 'first')
  await enterCellText(page, 'A2', 'second')
  const goTo = page.getByRole('textbox', { name: 'Go to cell or range' })
  await goTo.fill('A1:A2')
  await goTo.press('Enter')
  await page.keyboard.press('Control+X')
  await expect(page.locator('.toolbar-status')).toContainText('Cut ready')
  await cell(page, 'A2').click()
  await page.keyboard.press('Control+V')
  await expect(cell(page, 'A1')).toHaveText('')
  await expect(cell(page, 'A2')).toHaveText('first')
  await expect(cell(page, 'A3')).toHaveText('second')
})

test('cut rejects formula dependencies on another sheet without clearing the source', async ({ page }) => {
  await installTauriMock(page, { sheetDataById: { '1': { '0:0': '=Sheet1!A1' } } })
  await page.goto('/')
  await expect(cell(page, 'A1')).toBeVisible()
  await useClipboardReadback(page)
  await enterCellText(page, 'A1', 'source')
  await cell(page, 'A1').click()
  await page.keyboard.press('Control+X')
  await expect(page.locator('.toolbar-status')).toContainText('Cut ready')
  await cell(page, 'B1').click()
  await page.keyboard.press('Control+V')
  await expect(page.locator('.toolbar-status')).toContainText('contains formulas; use Copy instead')
  await expect(cell(page, 'A1')).toHaveText('source')
  await expect(cell(page, 'B1')).toHaveText('')
})

test('cut rejects a changed source without erasing the newer edit', async ({ page }) => {
  await openWorkbook(page)
  await useClipboardReadback(page)
  await enterCellText(page, 'A1', 'original')
  await cell(page, 'A1').click()
  await page.keyboard.press('Control+X')
  await expect(page.locator('.toolbar-status')).toContainText('Cut ready')
  await enterCellText(page, 'A1', 'newer')
  await cell(page, 'B1').click()
  await page.keyboard.press('Control+V')
  await expect(page.locator('.toolbar-status')).toContainText('cut source changed')
  await expect(cell(page, 'A1')).toHaveText('newer')
  await expect(cell(page, 'B1')).toHaveText('')
})

test('structural edits reject named range metadata before dispatch and preserve it', async ({ page }) => {
  await openWorkbook(page)
  await enterCellText(page, 'A1', 'retained')
  await page.locator('.menu-bar button').filter({ hasText: /^Data$/ }).click()
  await page.getByRole('button', { name: 'Named Ranges...' }).click()
  await page.getByLabel('Name', { exact: true }).fill('Revenue')
  await page.getByRole('button', { name: 'Add Named Range' }).click()
  await expect(page.getByText('Revenue', { exact: true })).toBeVisible()
  await page.getByRole('button', { name: 'Close', exact: true }).click()
  await page.evaluate(() => {
    const runtime = (window as any).__TAURI_INTERNALS__
    const original = runtime.invoke
    ;(window as any).__STRUCTURE_CALLS__ = []
    runtime.invoke = async (command: string, args: Record<string, unknown>) => {
      if (command === 'edit_sheet_structure') (window as any).__STRUCTURE_CALLS__.push(args)
      return original(command, args)
    }
  })
  for (const action of ['Insert Row Above', 'Delete Row', 'Insert Column Left', 'Delete Column']) {
    await page.locator('.menu-bar button').filter({ hasText: /^Insert$/ }).click()
    await page.getByRole('button', { name: action, exact: true }).click()
    await expect(page.locator('.toolbar-status')).toContainText('cannot preserve these features yet')
  }
  expect(await page.evaluate(() => (window as any).__STRUCTURE_CALLS__)).toEqual([])
  await expect(cell(page, 'A1')).toHaveText('retained')
  await page.locator('.menu-bar button').filter({ hasText: /^Data$/ }).click()
  await page.getByRole('button', { name: 'Named Ranges...' }).click()
  await expect(page.getByText('Revenue', { exact: true })).toBeVisible()
})

test('structural commands flush the formula bar draft before changing coordinates', async ({ page }) => {
  await openWorkbook(page)
  await page.evaluate(() => {
    const runtime = (window as any).__TAURI_INTERNALS__
    const original = runtime.invoke
    ;(window as any).__EDIT_ORDER__ = []
    runtime.invoke = async (command: string, args: Record<string, unknown>) => {
      if (command === 'batch_set_cells' || command === 'edit_sheet_structure') {
        ;(window as any).__EDIT_ORDER__.push({ command, args })
      }
      if (command === 'edit_sheet_structure') return null
      return original(command, args)
    }
  })
  await page.getByRole('textbox', { name: 'Formula bar', exact: true }).fill('draft before insertion')
  await page.locator('.menu-bar button').filter({ hasText: /^Insert$/ }).click()
  await page.getByRole('button', { name: 'Insert Row Above', exact: true }).click()
  await expect(page.locator('.toolbar-status')).toContainText('Inserted row above')
  const edits = await page.evaluate(() => (window as any).__EDIT_ORDER__)
  expect(edits.map((entry: { command: string }) => entry.command)).toEqual(['batch_set_cells', 'edit_sheet_structure'])
  expect(edits[0].args.changes).toEqual([{ row: 0, col: 0, value: 'draft before insertion' }])
})

test('a failed startup recovery remains available after opening a blank workbook', async ({ page }) => {
  await installTauriMock(page, { recoveries: [{ id: 'failed-restore', modified_millis: 2_000 }], recoveryRestoreFails: true })
  page.on('dialog', async (dialog) => dialog.accept())
  await page.goto('/')
  await expect(cell(page, 'A1')).toBeVisible()
  const events = await page.evaluate(() => (window as any).__RECOVERY_TEST__)
  expect(events.restored).toEqual(['failed-restore'])
  expect(events.discardAttempts).not.toContain('failed-restore')
})

test('cut revalidates dependencies inside the move when the preflight becomes stale', async ({ page }) => {
  await openWorkbook(page)
  await useClipboardReadback(page)
  await enterCellText(page, 'A1', 'retained')
  await cell(page, 'A1').click()
  await page.keyboard.press('Control+X')
  await expect(page.locator('.toolbar-status')).toContainText('Cut ready')
  await page.evaluate(() => {
    const runtime = (window as any).__TAURI_INTERNALS__
    const original = runtime.invoke
    runtime.invoke = async (command: string, args: Record<string, unknown>) => {
      const result = await original(command, args)
      if (command === 'workbook_has_formulas') {
        // A different edit lands after the advisory preflight, before move begins.
        await new Promise((resolve) => setTimeout(resolve, 25))
        await original('set_cell', { sheetId: 0, row: 0, col: 2, value: '=A1' })
      }
      return result
    }
  })
  await cell(page, 'B1').click()
  await page.keyboard.press('Control+V')
  await expect(page.locator('.toolbar-status')).toContainText('formula dependencies')
  await expect(cell(page, 'A1')).toHaveText('retained')
  await expect(cell(page, 'B1')).toHaveText('')
  await expect(cell(page, 'C1')).toHaveText('=A1')
})

test('deleting the active sheet commits its draft before sheet indexes change', async ({ page }) => {
  await openWorkbook(page)
  await page.evaluate(() => {
    const runtime = (window as any).__TAURI_INTERNALS__
    const original = runtime.invoke
    ;(window as any).__DELETE_ORDER__ = []
    runtime.invoke = async (command: string, args: Record<string, unknown>) => {
      if (command === 'batch_set_cells' || command === 'delete_sheet') {
        ;(window as any).__DELETE_ORDER__.push({ command, args })
      }
      return original(command, args)
    }
  })
  await page.getByRole('textbox', { name: 'Formula bar', exact: true }).fill('draft belongs to deleted sheet')
  await page.locator('button.sheet-tab-delete[title="Delete sheet"]').click()
  await expect(page.getByRole('button', { name: 'Sheet1', exact: true })).toHaveCount(0)
  const edits = await page.evaluate(() => (window as any).__DELETE_ORDER__)
  expect(edits.map((entry: { command: string }) => entry.command)).toEqual(['batch_set_cells', 'delete_sheet'])
  expect(edits[0].args).toEqual({ sheetId: 0, changes: [{ row: 0, col: 0, value: 'draft belongs to deleted sheet' }] })
})

test('selection statistics and numeric conditional rules use evaluated values before display formatting', async ({ page }) => {
  await installTauriMock(page, {
    sheetDataById: { '0': { '0:0': '=SUM(B1:C1)', '0:1': '10', '0:2': '20', '0:3': 'text', '0:4': '0012', '0:5': '=1/0' } },
    cellEvaluations: {
      '0:0': { numeric_value: 30, display: '$30.00' },
      '0:1': { numeric_value: 10, display: '1,000%' },
      '0:2': { numeric_value: 20, display: '20.00' },
      '0:3': { numeric_value: null, display: 'text' },
      '0:4': { numeric_value: null, display: '0012' },
      '0:5': { numeric_value: null, display: '#DIV/0!' },
    },
  })
  await page.goto('/')
  await expect(cell(page, 'A1')).toHaveText('$30.00')
  await expect(cell(page, 'A1')).toHaveCSS('text-align', 'right')
  await expect(cell(page, 'E1')).toHaveCSS('text-align', 'left')
  await expect(page.locator('.status-bar')).toContainText('Sum 30')
  await page.getByRole('textbox', { name: 'Go to cell or range' }).fill('A1:F1')
  await page.getByRole('textbox', { name: 'Go to cell or range' }).press('Enter')
  await expect(page.locator('.status-bar')).toContainText('Count 6')
  await expect(page.locator('.status-bar')).toContainText('Sum 60')
  await expect(page.locator('.status-bar')).toContainText('Avg 20')
  await page.locator('.menu-bar button').filter({ hasText: /^Format$/ }).click()
  await page.getByRole('button', { name: 'Conditional Formatting...' }).click()
  await page.getByRole('textbox', { name: 'Value', exact: true }).fill('25')
  await page.getByRole('button', { name: 'Save Live Rule' }).click()
  await expect(page.locator('.toolbar-status')).toContainText('Saved conditional rule')
  await page.getByRole('button', { name: 'Close', exact: true }).click()
  await expect(cell(page, 'A1')).toHaveCSS('background-color', 'rgb(254, 243, 199)')
  await expect(cell(page, 'B1')).not.toHaveCSS('background-color', 'rgb(254, 243, 199)')
  await expect(cell(page, 'E1')).not.toHaveCSS('background-color', 'rgb(254, 243, 199)')
  await expect(cell(page, 'F1')).not.toHaveCSS('background-color', 'rgb(254, 243, 199)')
})

test('successful workbook replacements clear undo and redo controls', async ({ page }) => {
  await openWorkbook(page)
  const undo = page.getByTitle('Undo (Ctrl+Z)', { exact: true })
  const redo = page.getByTitle('Redo (Ctrl+Y)', { exact: true })
  await enterCellText(page, 'A1', 'first')
  await enterCellText(page, 'A1', 'second')
  await undo.click()
  await expect(redo).toBeEnabled()
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await expect(page.locator('.save-state')).toHaveText('Saved to this computer')
  await page.getByRole('button', { name: 'Open', exact: true }).click()
  await expect(page.locator('.toolbar-status')).toContainText('Opened')
  await expect(undo).toBeDisabled()
  await expect(redo).toBeDisabled()
  await enterCellText(page, 'B1', 'new session')
  await expect(undo).toBeEnabled()
  page.once('dialog', (dialog) => dialog.accept())
  await page.locator('.menu-bar button').filter({ hasText: /^File$/ }).click()
  await page.locator('.app-menu').getByRole('button', { name: 'New Workbook' }).click()
  await expect(page.locator('.toolbar-status')).toContainText('New workbook')
  await expect(undo).toBeDisabled()
  await expect(redo).toBeDisabled()
})

test('formatting an empty cell remains visible in controls before entering data', async ({ page }) => {
  await openWorkbook(page)
  const bold = page.getByRole('button', { name: 'Bold', exact: true })
  await bold.click()
  await expect(bold).toHaveAttribute('aria-pressed', 'true')
  await cell(page, 'B1').click()
  await expect(bold).toHaveAttribute('aria-pressed', 'false')
  await cell(page, 'A1').click()
  await expect(bold).toHaveAttribute('aria-pressed', 'true')
  await page.getByRole('button', { name: 'Save', exact: true }).click()
  await expect(page.locator('.save-state')).toHaveText('Saved to this computer')
  await page.getByRole('button', { name: 'Open', exact: true }).click()
  await expect(bold).toHaveAttribute('aria-pressed', 'true')
  await expect(cell(page, 'A1')).toHaveText('')
})
