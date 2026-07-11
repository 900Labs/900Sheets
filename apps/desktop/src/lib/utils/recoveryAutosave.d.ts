export interface RecoveryAutosaveOptions {
  delay?: number
  flush: () => Promise<unknown>
  write: () => Promise<unknown>
  onError?: (error: unknown) => void
  onSuccess?: () => void
}

export class RecoveryAutosave {
  constructor(options: RecoveryAutosaveOptions)
  setDelay(delay: number): void
  getDelay(): number
  schedule(): void
  cancel(): void
  cancelAndWait(): Promise<void>
  runNow(): Promise<void>
}
