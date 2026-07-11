import { expect, type Page, test } from '@playwright/test'

interface MockOptions {
  recoveries?: Array<{ id: string; modified_millis: number }>
  discardFailures?: number
  discardFailuresAfter?: number
}

async function installTauriMock(page: Page, options: MockOptions = {}) {
  await page.addInitScript((options: MockOptions) => {
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
    const formats = new Map<string, Record<string, unknown>>()
    const comments = new Map<string, { row: number; col: number; text: string; author: string }>()
    const callbacks = new Map<number, (data: unknown) => unknown>()
    const initialSheets = [
      { id: 0, stable_id: 1, name: 'Sheet1' },
      { id: 1, stable_id: 2, name: 'Sheet2' },
    ]
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
    }
    ;(window as Window & { __RECOVERY_TEST__?: typeof recoveryEvents }).__RECOVERY_TEST__ = recoveryEvents
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
          case 'plugin:dialog|save':
            return '/tmp/mock-workbook.900sheets'
          case 'plugin:event|listen':
            return 1
          case 'plugin:event|unlisten':
            return null
          case 'list_recovery_snapshots':
            return cloneJson(options.recoveries ?? [])
          case 'write_recovery_snapshot':
            recoveryEvents.writes.push(cloneJson((args.metadata as Record<string, unknown>) ?? {}))
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
            formats.clear()
            sheets.splice(0, sheets.length, ...cloneJson(initialSheets))
            undoStack.length = 0
            redoStack.length = 0
            return sheets
          case 'set_active_sheet':
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
            savedNative = snapshot((args.metadata as Record<string, unknown>) ?? {})
            recoveryEvents.nativeSaves.push(cloneJson(savedNative.metadata))
            return null
          case 'import_native_file':
            if (!savedNative) throw new Error('No saved native workbook')
            restore(savedNative)
            undoStack.length = 0
            redoStack.length = 0
            return {
              sheets: cloneJson(sheets),
              metadata: cloneJson(savedNative.metadata),
            }
          case 'get_sheet_data':
            return Array.from(cells.entries()).map(([key, cell]) => {
              const [row, col] = key.split(':').map(Number)
              return {
                row,
                col,
                value: cell.value,
                display: cell.display,
                cell_type: cell.value.startsWith('=') ? 'formula' : 'text',
                format: formats.get(key) ?? null,
              }
            })
          case 'set_cell': {
            const value = String(args.value ?? '')
            cells.set(keyFor(args.row, args.col), { value, display: value })
            return null
          }
          case 'batch_set_cells': {
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

  const formulaInput = page.locator('.formula-bar input')
  await formulaInput.click()
  await formulaInput.press('h')
  await formulaInput.press('i')

  await expect(formulaInput).toHaveValue('hi')
  await expect(page.locator('input.cell-input')).toHaveCount(0)

  await formulaInput.press('Enter')
  await expect(cell(page, 'A1')).toHaveText('hi')
  await expect(page.locator('input.cell-input')).toHaveCount(0)
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
  const nameInput = page.getByLabel('Name')
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

test('explicit recovery discard removes only the selected snapshot', async ({ page }) => {
  await installTauriMock(page, {
    recoveries: [
      { id: 'discard-me', modified_millis: 2_000 },
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
  expect(events?.discardAttempts).toEqual(['discard-me'])
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
  expect(attempts).toHaveLength(3)
  expect(attempts[1]).toBe(attempts[2])
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
  expect(attempts).toHaveLength(3)
  expect(attempts[1]).toBe(attempts[2])
})

test('delete active sheet undo redo save and reopen does not resurrect stale metadata', async ({ page }) => {
  await openWorkbook(page)
  await page.locator('button.sheet-tab-add').click()
  await expect(page.getByRole('button', { name: 'Sheet3', exact: true })).toHaveClass(/active/)

  await page.locator('.menu-bar button').filter({ hasText: /^Data$/ }).click()
  await page.getByRole('button', { name: 'Named Ranges...' }).click()
  await page.getByLabel('Name').fill('TransientRange')
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
