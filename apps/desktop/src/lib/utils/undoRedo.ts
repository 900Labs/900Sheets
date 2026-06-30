import type { HistoryEntry } from '../types'

export class UndoRedoStack {
  private undoStack: HistoryEntry[][] = []
  private redoStack: HistoryEntry[][] = []
  private maxStack: number = 100

  push(entries: HistoryEntry[]) {
    if (entries.length === 0) return
    this.undoStack.push(entries)
    if (this.undoStack.length > this.maxStack) {
      this.undoStack.shift()
    }
    this.redoStack = []
  }

  undo(): HistoryEntry[] | null {
    const entries = this.undoStack.pop()
    if (entries) {
      this.redoStack.push(entries)
      return entries
    }
    return null
  }

  redo(): HistoryEntry[] | null {
    const entries = this.redoStack.pop()
    if (entries) {
      this.undoStack.push(entries)
      return entries
    }
    return null
  }

  canUndo(): boolean {
    return this.undoStack.length > 0
  }

  canRedo(): boolean {
    return this.redoStack.length > 0
  }

  clear() {
    this.undoStack = []
    this.redoStack = []
  }
}
