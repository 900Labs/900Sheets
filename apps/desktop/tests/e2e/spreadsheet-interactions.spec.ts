import { expect, type Page, test } from '@playwright/test'

async function installTauriMock(page: Page) {
  await page.addInitScript(() => {
    type CellRecord = { value: string; display: string }
    type TauriWindow = Window & {
      __TAURI_INTERNALS__?: {
        invoke?: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>
        transformCallback?: (callback?: (data: unknown) => unknown, once?: boolean) => number
        unregisterCallback?: (id: number) => void
        runCallback?: (id: number, data: unknown) => unknown
        callbacks?: Map<number, (data: unknown) => unknown>
      }
    }

    const cells = new Map<string, CellRecord>()
    const formats = new Map<string, Record<string, unknown>>()
    const callbacks = new Map<number, (data: unknown) => unknown>()
    const sheets = [{ id: 1, name: 'Sheet1' }]
    let callbackId = 1

    const keyFor = (row: unknown, col: unknown) => `${Number(row)}:${Number(col)}`

    const tauriWindow = window as TauriWindow
    tauriWindow.__TAURI_INTERNALS__ = {
      ...(tauriWindow.__TAURI_INTERNALS__ ?? {}),
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
          case 'new_workbook':
            cells.clear()
            formats.clear()
            return sheets
          case 'get_sheet_data':
            return Array.from(cells.entries()).map(([key, cell]) => {
              const [row, col] = key.split(':').map(Number)
              return { row, col, value: cell.value, display: cell.display }
            })
          case 'set_cell': {
            const value = String(args.value ?? '')
            cells.set(keyFor(args.row, args.col), { value, display: value })
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
          default:
            return null
        }
      },
    }
  })
}

async function openWorkbook(page: Page) {
  await installTauriMock(page)
  await page.goto('/')
  await expect(page.locator('button.cell[aria-label="A1"]')).toBeVisible()
}

function cell(page: Page, label: string) {
  return page.locator(`button.cell[aria-label="${label}"]`)
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
