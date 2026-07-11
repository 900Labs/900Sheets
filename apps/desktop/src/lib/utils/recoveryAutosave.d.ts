export interface RecoveryAutosaveOptions {
  delay?: number
  flush: () => Promise<unknown>
  write: () => Promise<unknown>
  onError?: (error: unknown) => void
}

export class RecoveryAutosave {
  constructor(options: RecoveryAutosaveOptions)
  schedule(): void
  cancel(): void
  cancelAndWait(): Promise<void>
  runNow(): Promise<void>
}
