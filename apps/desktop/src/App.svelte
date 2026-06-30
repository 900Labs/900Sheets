<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { open, save } from '@tauri-apps/plugin-dialog'
  import { onMount } from 'svelte'
  import type { SheetInfo, CellData, CellRange, ClipboardData, HistoryEntry } from './lib/types'
  import { colLabel, cellKey, normalizeRange, rangeContains, rangeSize, rangeLabel, parseCellKey } from './lib/utils/grid'
  import { UndoRedoStack } from './lib/utils/undoRedo'

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

  function filename(path: string): string {
    return path.split(/[\\/]/).pop() || path
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
      for (const cell of data) {
        contents[cellKey(cell.row, cell.col)] = cell.value
        displays[cellKey(cell.row, cell.col)] = cell.display
      }
      cellContents = contents
      cellDisplays = displays
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
        } else {
          delete cellContents[key]
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

    if (ctrl && e.key === 'z' && !e.shiftKey) {
      e.preventDefault()
      doUndo()
      return
    }
    if ((ctrl && e.key === 'y') || (ctrl && e.shiftKey && e.key === 'Z')) {
      e.preventDefault()
      doRedo()
      return
    }
    if (ctrl && e.key === 'c') {
      e.preventDefault()
      copySelection(false)
      return
    }
    if (ctrl && e.key === 'x') {
      e.preventDefault()
      copySelection(true)
      return
    }
    if (ctrl && e.key === 'v') {
      e.preventDefault()
      pasteFromSystemClipboard()
      return
    }
    if (ctrl && e.key === 'a') {
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

  onMount(async () => {
    await handleNewWorkbook()
  })
</script>

<svelte:window onkeydown={handleKeydown} onmouseup={handleMouseUp} />

<div class="app">
  <div class="toolbar">
    <span class="app-title">900Sheets</span>
    <div class="toolbar-actions">
      <button type="button" class="toolbar-btn text" onclick={handleNewWorkbook} title="New workbook">New</button>
      <button type="button" class="toolbar-btn text" onclick={handleOpenXlsx} title="Open XLSX">Open</button>
      <button type="button" class="toolbar-btn text" onclick={handleImportCsv} title="Import CSV or TSV into the active sheet">CSV</button>
      <button type="button" class="toolbar-btn text" onclick={handleImportJson} title="Import JSON workbook">JSON</button>
      <button type="button" class="toolbar-btn text" onclick={handleSaveXlsx} title="Save workbook as XLSX">Save</button>
      <button type="button" class="toolbar-btn text" onclick={handleExportCsv} title="Export active sheet as CSV">Export CSV</button>
      <button type="button" class="toolbar-btn text" onclick={handleExportJson} title="Export workbook as JSON">Export JSON</button>
      <button type="button" class="toolbar-btn" onclick={doUndo} disabled={!canUndo} title="Undo (Ctrl+Z)">↶</button>
      <button type="button" class="toolbar-btn" onclick={doRedo} disabled={!canRedo} title="Redo (Ctrl+Y)">↷</button>
    </div>
    <div class="toolbar-status" class:error={!!errorMessage}>
      {errorMessage || statusMessage}
    </div>
  </div>

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

  <div class="grid-container" role="grid" onscroll={handleScroll}>
    <div
      class="grid"
      style="grid-template-columns: {COL_WIDTH * 0.6}px repeat({COLS}, {COL_WIDTH}px); height: {HEADER_HEIGHT + ROWS * ROW_HEIGHT}px;"
    >
      <div class="corner-cell"></div>
      {#each Array(COLS) as _, c}
        <div class="col-header">{colLabel(c)}</div>
      {/each}

      {#each Array(visibleRowEnd - visibleRowStart) as _, i}
        {@const r = visibleRowStart + i}
        <div class="row-header">{r + 1}</div>
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
            style="height: {ROW_HEIGHT}px;"
          >
            {#if editingCell === cellKey(r, c)}
              <input
                type="text"
                bind:value={editValue}
                onblur={commitEdit}
                class="cell-input"
              />
            {:else}
              <span class="cell-value">{getCellDisplay(r, c)}</span>
            {/if}
          </button>
        {/each}
      {/each}
    </div>
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
</div>
