# Sprint 13: Internationalization & Accessibility

## Goal
Add locale management, translations, locale-aware formatting, and accessibility support.

## What Was Built
- **Crate**: `sheets-i18n`
- **Locale management**: 22 supported locales (en, es, fr, de, it, pt, ru, zh, ja, ko, ar, hi, tr, nl, sv, pl, fa, he, th, vi, id, uk)
- **Translations**: 88 translation keys covering UI and accessibility
- `TranslationProvider` for locale-specific text lookup
- **Formatting**: locale-aware number, currency, percentage, date, and time formatting
- **RTL support**: Arabic, Hebrew, Persian, Urdu
- **Accessibility**: `AccessibilityLabel` struct, ARIA labels, screen reader support, keyboard navigation metadata
- Excel 1900 leap year bug handled in date serial conversion
- Tauri IPC: `get_available_locales`, `get_translations`, `translate_key`, `format_number_i18n`, `format_currency_i18n`, `format_percentage_i18n`, `format_date_i18n`, `format_time_i18n`, `get_cell_accessibility_label`, `get_selected_cell_label`, `get_editing_cell_label`, `get_navigation_direction_name`

## Tests
44 unit tests covering locales, translations, formatting, and accessibility.

## Status: Complete ✅
