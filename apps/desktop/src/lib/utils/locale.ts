export type AppLocale = 'en' | 'sv' | 'es'

export const APP_LOCALES: Array<{ code: AppLocale; label: string }> = [
  { code: 'en', label: 'English' },
  { code: 'sv', label: 'Svenska' },
  { code: 'es', label: 'Español' },
]

type TranslationKey =
  | 'activeCell'
  | 'blank'
  | 'cell'
  | 'goTo'
  | 'goToHint'
  | 'grid'
  | 'language'
  | 'notSelected'
  | 'selected'
  | 'settingsSaved'

const TRANSLATIONS: Record<AppLocale, Record<TranslationKey, string>> = {
  en: {
    activeCell: 'Active cell',
    blank: 'blank',
    cell: 'Cell',
    goTo: 'Go to cell or range',
    goToHint: 'Enter an address from A1 to XFD1000000',
    grid: 'Spreadsheet grid',
    language: 'Interface language',
    notSelected: 'not selected',
    selected: 'selected',
    settingsSaved: 'Language preference saved',
  },
  sv: {
    activeCell: 'Aktiv cell',
    blank: 'tom',
    cell: 'Cell',
    goTo: 'Gå till cell eller område',
    goToHint: 'Ange en adress från A1 till XFD1000000',
    grid: 'Kalkylblad',
    language: 'Gränssnittsspråk',
    notSelected: 'inte markerad',
    selected: 'markerad',
    settingsSaved: 'Språkinställningen har sparats',
  },
  es: {
    activeCell: 'Celda activa',
    blank: 'vacía',
    cell: 'Celda',
    goTo: 'Ir a celda o rango',
    goToHint: 'Introduce una dirección entre A1 y XFD1000000',
    grid: 'Hoja de cálculo',
    language: 'Idioma de la interfaz',
    notSelected: 'no seleccionada',
    selected: 'seleccionada',
    settingsSaved: 'Preferencia de idioma guardada',
  },
}

export function isAppLocale(value: string | null): value is AppLocale {
  return value === 'en' || value === 'sv' || value === 'es'
}

export function translate(locale: AppLocale, key: TranslationKey): string {
  return TRANSLATIONS[locale][key]
}
