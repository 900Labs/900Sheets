export interface StarterTemplate {
  readonly id: string
  readonly title: string
  readonly summary: string
  readonly audience: string
  readonly tasks: readonly string[]
  readonly rows: readonly (readonly string[])[]
}

export interface TemplateCellChange {
  row: number
  col: number
  value: string
}

export interface FirstRunStep {
  readonly id: string
  readonly title: string
  readonly detail: string
}

export interface KeyValueStorage {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
}

export const STARTER_TEMPLATES: readonly StarterTemplate[]
export const FIRST_RUN_STORAGE_KEY: '900sheets.first-run.v1'
export const FIRST_RUN_STEPS: readonly FirstRunStep[]

export function getStarterTemplate(id: string): StarterTemplate | null
export function templateDimensions(template: StarterTemplate): { rows: number; columns: number }
export function templateCellChanges(id: string, startRow?: number, startCol?: number): TemplateCellChange[]
export function shouldShowFirstRun(storage: KeyValueStorage): boolean
export function completeFirstRun(storage: KeyValueStorage): boolean
