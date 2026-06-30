use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// ============================================================================
// Locale Management
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Locale {
    EnUS,
    EnGB,
    FrFR,
    DeDE,
    EsES,
    ItIT,
    PtBR,
    JaJP,
    ZhCN,
    ZhTW,
    KoKR,
    RuRU,
    NlNL,
    SvSE,
    DaDK,
    FiFI,
    NoNO,
    PlPL,
    TrTR,
    ArSA,
    HiIN,
    ThTH,
}

impl Locale {
    pub fn code(&self) -> &'static str {
        match self {
            Locale::EnUS => "en-US",
            Locale::EnGB => "en-GB",
            Locale::FrFR => "fr-FR",
            Locale::DeDE => "de-DE",
            Locale::EsES => "es-ES",
            Locale::ItIT => "it-IT",
            Locale::PtBR => "pt-BR",
            Locale::JaJP => "ja-JP",
            Locale::ZhCN => "zh-CN",
            Locale::ZhTW => "zh-TW",
            Locale::KoKR => "ko-KR",
            Locale::RuRU => "ru-RU",
            Locale::NlNL => "nl-NL",
            Locale::SvSE => "sv-SE",
            Locale::DaDK => "da-DK",
            Locale::FiFI => "fi-FI",
            Locale::NoNO => "nb-NO",
            Locale::PlPL => "pl-PL",
            Locale::TrTR => "tr-TR",
            Locale::ArSA => "ar-SA",
            Locale::HiIN => "hi-IN",
            Locale::ThTH => "th-TH",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "en-US" | "en_US" => Some(Locale::EnUS),
            "en-GB" | "en_GB" => Some(Locale::EnGB),
            "fr-FR" | "fr_FR" => Some(Locale::FrFR),
            "de-DE" | "de_DE" => Some(Locale::DeDE),
            "es-ES" | "es_ES" => Some(Locale::EsES),
            "it-IT" | "it_IT" => Some(Locale::ItIT),
            "pt-BR" | "pt_BR" => Some(Locale::PtBR),
            "ja-JP" | "ja_JP" => Some(Locale::JaJP),
            "zh-CN" | "zh_CN" => Some(Locale::ZhCN),
            "zh-TW" | "zh_TW" => Some(Locale::ZhTW),
            "ko-KR" | "ko_KR" => Some(Locale::KoKR),
            "ru-RU" | "ru_RU" => Some(Locale::RuRU),
            "nl-NL" | "nl_NL" => Some(Locale::NlNL),
            "sv-SE" | "sv_SE" => Some(Locale::SvSE),
            "da-DK" | "da_DK" => Some(Locale::DaDK),
            "fi-FI" | "fi_FI" => Some(Locale::FiFI),
            "nb-NO" | "no-NO" | "nb_NO" | "no_NO" => Some(Locale::NoNO),
            "pl-PL" | "pl_PL" => Some(Locale::PlPL),
            "tr-TR" | "tr_TR" => Some(Locale::TrTR),
            "ar-SA" | "ar_SA" => Some(Locale::ArSA),
            "hi-IN" | "hi_IN" => Some(Locale::HiIN),
            "th-TH" | "th_TH" => Some(Locale::ThTH),
            _ => None,
        }
    }

    pub fn language(&self) -> &'static str {
        match self {
            Locale::EnUS | Locale::EnGB => "English",
            Locale::FrFR => "Français",
            Locale::DeDE => "Deutsch",
            Locale::EsES => "Español",
            Locale::ItIT => "Italiano",
            Locale::PtBR => "Português",
            Locale::JaJP => "日本語",
            Locale::ZhCN | Locale::ZhTW => "中文",
            Locale::KoKR => "한국어",
            Locale::RuRU => "Русский",
            Locale::NlNL => "Nederlands",
            Locale::SvSE => "Svenska",
            Locale::DaDK => "Dansk",
            Locale::FiFI => "Suomi",
            Locale::NoNO => "Norsk",
            Locale::PlPL => "Polski",
            Locale::TrTR => "Türkçe",
            Locale::ArSA => "العربية",
            Locale::HiIN => "हिन्दी",
            Locale::ThTH => "ไทย",
        }
    }

    pub fn is_rtl(&self) -> bool {
        matches!(self, Locale::ArSA)
    }

    pub fn all() -> Vec<Locale> {
        vec![
            Locale::EnUS,
            Locale::EnGB,
            Locale::FrFR,
            Locale::DeDE,
            Locale::EsES,
            Locale::ItIT,
            Locale::PtBR,
            Locale::JaJP,
            Locale::ZhCN,
            Locale::ZhTW,
            Locale::KoKR,
            Locale::RuRU,
            Locale::NlNL,
            Locale::SvSE,
            Locale::DaDK,
            Locale::FiFI,
            Locale::NoNO,
            Locale::PlPL,
            Locale::TrTR,
            Locale::ArSA,
            Locale::HiIN,
            Locale::ThTH,
        ]
    }

    /// Returns the locale's default date format pattern
    pub fn date_format(&self) -> &'static str {
        match self {
            Locale::EnUS | Locale::PtBR => "MM/DD/YYYY",
            Locale::JaJP | Locale::ZhCN | Locale::ZhTW | Locale::KoKR => "YYYY/MM/DD",
            _ => "DD/MM/YYYY",
        }
    }

    /// Returns the locale's default number format (decimal separator, thousands separator)
    pub fn number_format(&self) -> (char, char) {
        match self {
            Locale::EnUS
            | Locale::EnGB
            | Locale::JaJP
            | Locale::ZhCN
            | Locale::ZhTW
            | Locale::KoKR
            | Locale::ThTH
            | Locale::HiIN => ('.', ','),
            _ => (',', '.'),
        }
    }

    /// Returns the locale's default currency symbol
    pub fn currency_symbol(&self) -> &'static str {
        match self {
            Locale::EnUS => "$",
            Locale::EnGB => "£",
            Locale::FrFR
            | Locale::DeDE
            | Locale::EsES
            | Locale::ItIT
            | Locale::NlNL
            | Locale::SvSE
            | Locale::DaDK
            | Locale::FiFI
            | Locale::NoNO
            | Locale::PlPL
            | Locale::PtBR => "€",
            Locale::JaJP => "¥",
            Locale::ZhCN | Locale::ZhTW => "¥",
            Locale::KoKR => "₩",
            Locale::RuRU => "₽",
            Locale::TrTR => "₺",
            Locale::ArSA => "ر.س",
            Locale::HiIN => "₹",
            Locale::ThTH => "฿",
        }
    }

    /// Returns the locale's first day of week (0 = Sunday, 1 = Monday)
    pub fn first_day_of_week(&self) -> u32 {
        match self {
            Locale::EnUS
            | Locale::PtBR
            | Locale::JaJP
            | Locale::ZhCN
            | Locale::ZhTW
            | Locale::KoKR
            | Locale::ThTH
            | Locale::HiIN
            | Locale::ArSA => 0,
            _ => 1,
        }
    }
}

// ============================================================================
// Translation Keys
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TranslationKey {
    // Menu / Toolbar
    File,
    Edit,
    View,
    Insert,
    Format,
    Tools,
    Help,
    New,
    Open,
    Save,
    SaveAs,
    Export,
    Import,
    Print,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Find,
    Replace,
    Sort,
    Filter,
    // Cell operations
    Cell,
    Row,
    Column,
    InsertRow,
    InsertColumn,
    DeleteRow,
    DeleteColumn,
    HideRow,
    HideColumn,
    AutoFit,
    // Sheet operations
    Sheet,
    NewSheet,
    DeleteSheet,
    RenameSheet,
    DuplicateSheet,
    // Formula
    Formula,
    Sum,
    Average,
    Count,
    Max,
    Min,
    FunctionWizard,
    // Formatting
    Bold,
    Italic,
    Underline,
    Strikethrough,
    FontSize,
    FontColor,
    BackgroundColor,
    Alignment,
    WrapText,
    NumberFormat,
    Currency,
    Percentage,
    Date,
    Time,
    // Data Validation
    DataValidation,
    ConditionalFormatting,
    // Charts
    Chart,
    BarChart,
    LineChart,
    PieChart,
    ScatterChart,
    // Pivot Tables
    PivotTable,
    // Errors
    ErrorCircularReference,
    ErrorDivisionByZero,
    ErrorInvalidFormula,
    ErrorValueNotFound,
    ErrorTypeMismatch,
    // Status
    Ready,
    Calculating,
    Saving,
    Loading,
    // Accessibility
    CellLabel,
    RowLabel,
    ColumnLabel,
    SelectedCell,
    EditingCell,
    // Dialogs
    ConfirmDelete,
    ConfirmClose,
    Ok,
    Cancel,
    Yes,
    No,
}

impl TranslationKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            TranslationKey::File => "file",
            TranslationKey::Edit => "edit",
            TranslationKey::View => "view",
            TranslationKey::Insert => "insert",
            TranslationKey::Format => "format",
            TranslationKey::Tools => "tools",
            TranslationKey::Help => "help",
            TranslationKey::New => "new",
            TranslationKey::Open => "open",
            TranslationKey::Save => "save",
            TranslationKey::SaveAs => "save_as",
            TranslationKey::Export => "export",
            TranslationKey::Import => "import",
            TranslationKey::Print => "print",
            TranslationKey::Undo => "undo",
            TranslationKey::Redo => "redo",
            TranslationKey::Cut => "cut",
            TranslationKey::Copy => "copy",
            TranslationKey::Paste => "paste",
            TranslationKey::Find => "find",
            TranslationKey::Replace => "replace",
            TranslationKey::Sort => "sort",
            TranslationKey::Filter => "filter",
            TranslationKey::Cell => "cell",
            TranslationKey::Row => "row",
            TranslationKey::Column => "column",
            TranslationKey::InsertRow => "insert_row",
            TranslationKey::InsertColumn => "insert_column",
            TranslationKey::DeleteRow => "delete_row",
            TranslationKey::DeleteColumn => "delete_column",
            TranslationKey::HideRow => "hide_row",
            TranslationKey::HideColumn => "hide_column",
            TranslationKey::AutoFit => "auto_fit",
            TranslationKey::Sheet => "sheet",
            TranslationKey::NewSheet => "new_sheet",
            TranslationKey::DeleteSheet => "delete_sheet",
            TranslationKey::RenameSheet => "rename_sheet",
            TranslationKey::DuplicateSheet => "duplicate_sheet",
            TranslationKey::Formula => "formula",
            TranslationKey::Sum => "sum",
            TranslationKey::Average => "average",
            TranslationKey::Count => "count",
            TranslationKey::Max => "max",
            TranslationKey::Min => "min",
            TranslationKey::FunctionWizard => "function_wizard",
            TranslationKey::Bold => "bold",
            TranslationKey::Italic => "italic",
            TranslationKey::Underline => "underline",
            TranslationKey::Strikethrough => "strikethrough",
            TranslationKey::FontSize => "font_size",
            TranslationKey::FontColor => "font_color",
            TranslationKey::BackgroundColor => "background_color",
            TranslationKey::Alignment => "alignment",
            TranslationKey::WrapText => "wrap_text",
            TranslationKey::NumberFormat => "number_format",
            TranslationKey::Currency => "currency",
            TranslationKey::Percentage => "percentage",
            TranslationKey::Date => "date",
            TranslationKey::Time => "time",
            TranslationKey::DataValidation => "data_validation",
            TranslationKey::ConditionalFormatting => "conditional_formatting",
            TranslationKey::Chart => "chart",
            TranslationKey::BarChart => "bar_chart",
            TranslationKey::LineChart => "line_chart",
            TranslationKey::PieChart => "pie_chart",
            TranslationKey::ScatterChart => "scatter_chart",
            TranslationKey::PivotTable => "pivot_table",
            TranslationKey::ErrorCircularReference => "error_circular_ref",
            TranslationKey::ErrorDivisionByZero => "error_div_zero",
            TranslationKey::ErrorInvalidFormula => "error_invalid_formula",
            TranslationKey::ErrorValueNotFound => "error_value_not_found",
            TranslationKey::ErrorTypeMismatch => "error_type_mismatch",
            TranslationKey::Ready => "ready",
            TranslationKey::Calculating => "calculating",
            TranslationKey::Saving => "saving",
            TranslationKey::Loading => "loading",
            TranslationKey::CellLabel => "cell_label",
            TranslationKey::RowLabel => "row_label",
            TranslationKey::ColumnLabel => "column_label",
            TranslationKey::SelectedCell => "selected_cell",
            TranslationKey::EditingCell => "editing_cell",
            TranslationKey::ConfirmDelete => "confirm_delete",
            TranslationKey::ConfirmClose => "confirm_close",
            TranslationKey::Ok => "ok",
            TranslationKey::Cancel => "cancel",
            TranslationKey::Yes => "yes",
            TranslationKey::No => "no",
        }
    }
}

// ============================================================================
// Translation Provider
// ============================================================================

#[derive(Debug, Error, Clone, PartialEq)]
pub enum I18nError {
    #[error("Locale not found: {0}")]
    LocaleNotFound(String),
    #[error("Translation key not found: {0}")]
    KeyNotFound(String),
}

#[derive(Debug, Clone)]
pub struct TranslationProvider {
    locale: Locale,
    translations: HashMap<TranslationKey, String>,
}

impl TranslationProvider {
    pub fn new(locale: Locale) -> Self {
        let translations = get_translations(locale);
        Self {
            locale,
            translations,
        }
    }

    pub fn locale(&self) -> Locale {
        self.locale
    }

    pub fn translate(&self, key: TranslationKey) -> &str {
        self.translations
            .get(&key)
            .map(|s| s.as_str())
            .unwrap_or_else(|| {
                // Fallback to English
                get_english_translation(key)
            })
    }

    pub fn translate_with_args(&self, key: TranslationKey, args: &[(&str, &str)]) -> String {
        let template = self.translate(key).to_string();
        let mut result = template;
        for (placeholder, value) in args {
            result = result.replace(&format!("{{{}}}", placeholder), value);
        }
        result
    }

    /// Get all available translations as key-value pairs for the frontend
    pub fn all_translations(&self) -> Vec<(String, String)> {
        self.translations
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.clone()))
            .collect()
    }
}

// ============================================================================
// Number / Date / Currency Formatting
// ============================================================================

/// Format a number according to locale settings
pub fn format_number_locale(value: f64, locale: Locale) -> String {
    let (decimal_sep, thousands_sep) = locale.number_format();

    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "∞" } else { "-∞" }.to_string();
    }

    let abs = value.abs();
    let is_negative = value < 0.0;

    // Split into integer and fractional parts
    let formatted = if abs == abs.trunc() && abs < 1e15 {
        format_integer(abs as i64, thousands_sep)
    } else {
        // Format with up to 10 decimal places
        let s = format!("{:.10}", abs);
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() == 2 {
            let int_part = parts[0].parse::<i64>().unwrap_or(0);
            let mut frac = parts[1].to_string();
            // Trim trailing zeros
            while frac.ends_with('0') {
                frac.pop();
            }
            if frac.is_empty() {
                format_integer(int_part, thousands_sep)
            } else {
                format!(
                    "{}{}{}",
                    format_integer(int_part, thousands_sep),
                    decimal_sep,
                    frac
                )
            }
        } else {
            format_integer(abs as i64, thousands_sep)
        }
    };

    if is_negative {
        format!("-{}", formatted)
    } else {
        formatted
    }
}

/// Format a number as currency according to locale
pub fn format_currency(value: f64, locale: Locale) -> String {
    let symbol = locale.currency_symbol();
    let formatted = format_number_locale(value, locale);

    match locale {
        Locale::EnUS | Locale::EnGB | Locale::PtBR => format!("{}{}", symbol, formatted),
        Locale::JaJP | Locale::ZhCN | Locale::ZhTW | Locale::KoKR => {
            format!("{}{}", formatted, symbol)
        }
        _ => format!("{} {}", formatted, symbol),
    }
}

/// Format a number as percentage according to locale
pub fn format_percentage(value: f64, locale: Locale) -> String {
    let pct = value * 100.0;
    let (decimal_sep, _) = locale.number_format();

    let formatted = if pct == pct.trunc() {
        format!("{}", pct as i64)
    } else {
        let s = format!("{:.2}", pct);
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() == 2 {
            let mut frac = parts[1].to_string();
            while frac.ends_with('0') {
                frac.pop();
            }
            if frac.is_empty() {
                parts[0].to_string()
            } else {
                format!("{}{}{}", parts[0], decimal_sep, frac)
            }
        } else {
            s
        }
    };

    match locale {
        Locale::FrFR
        | Locale::DeDE
        | Locale::EsES
        | Locale::ItIT
        | Locale::PtBR
        | Locale::NlNL
        | Locale::SvSE
        | Locale::DaDK
        | Locale::FiFI
        | Locale::NoNO
        | Locale::PlPL
        | Locale::TrTR
        | Locale::RuRU => format!("{} %", formatted),
        _ => format!("{}%", formatted),
    }
}

/// Format a date serial (Excel-style, day 1 = 1900-01-01) according to locale
pub fn format_date(serial: f64, locale: Locale) -> String {
    let date = serial_to_ymd(serial);
    let fmt = locale.date_format();

    let day = format!("{:02}", date.2);
    let month = format!("{:02}", date.1);
    let year = format!("{}", date.0);

    fmt.replace("YYYY", &year)
        .replace("MM", &month)
        .replace("DD", &day)
}

/// Format a time serial (fraction of a day) according to locale
pub fn format_time(serial: f64, locale: Locale) -> String {
    let frac = serial - serial.trunc();
    let total_seconds = (frac * 86400.0).round() as u32;
    let hours = total_seconds / 3600;
    let _minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    let (sep, _) = locale.number_format();

    match locale {
        Locale::EnUS => {
            let h12 = if hours == 0 {
                12
            } else if hours > 12 {
                hours - 12
            } else {
                hours
            };
            let ampm = if hours < 12 { "AM" } else { "PM" };
            format!("{:02}:{}:{:02} {}", h12, sep, seconds, ampm)
        }
        _ => format!("{:02}:{}:{:02}", hours, sep, seconds),
    }
}

/// Get localized weekday name
pub fn weekday_name(day: u32, locale: Locale) -> &'static str {
    let names = weekday_names(locale);
    names.get(day as usize).copied().unwrap_or("")
}

/// Get localized month name
pub fn month_name(month: u32, locale: Locale) -> &'static str {
    let names = month_names(locale);
    names.get(month as usize - 1).copied().unwrap_or("")
}

/// Get localized abbreviated weekday name
pub fn weekday_abbr(day: u32, locale: Locale) -> &'static str {
    let names = weekday_abbrs(locale);
    names.get(day as usize).copied().unwrap_or("")
}

/// Get localized abbreviated month name
pub fn month_abbr(month: u32, locale: Locale) -> &'static str {
    let names = month_abbrs(locale);
    names.get(month as usize - 1).copied().unwrap_or("")
}

// ============================================================================
// Accessibility
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccessibilityLabel {
    pub role: String,
    pub label: String,
    pub description: Option<String>,
}

/// Generate an accessible label for a cell
pub fn cell_label(row: u32, col: u32, value: &str, locale: Locale) -> AccessibilityLabel {
    let col_letter = col_to_letter(col);
    let row_num = row + 1;
    let cell_ref = format!("{}{}", col_letter, row_num);

    let row_label = translate(locale, TranslationKey::RowLabel);
    let col_label = translate(locale, TranslationKey::ColumnLabel);
    let cell_word = translate(locale, TranslationKey::Cell);

    let label = if value.is_empty() {
        format!(
            "{} {} {} {}, {}",
            cell_word, cell_ref, col_label, col_letter, row_label
        )
    } else {
        format!(
            "{} {} {} {}, {}, {}",
            cell_word, cell_ref, col_label, col_letter, row_label, value
        )
    };

    AccessibilityLabel {
        role: "gridcell".to_string(),
        label,
        description: Some(format!("{} {}", cell_ref, value)),
    }
}

/// Generate an accessible label for a row header
pub fn row_header_label(row: u32, locale: Locale) -> AccessibilityLabel {
    let row_label = translate(locale, TranslationKey::RowLabel);
    AccessibilityLabel {
        role: "rowheader".to_string(),
        label: format!("{} {}", row_label, row + 1),
        description: None,
    }
}

/// Generate an accessible label for a column header
pub fn col_header_label(col: u32, locale: Locale) -> AccessibilityLabel {
    let col_label = translate(locale, TranslationKey::ColumnLabel);
    let letter = col_to_letter(col);
    AccessibilityLabel {
        role: "columnheader".to_string(),
        label: format!("{} {}", col_label, letter),
        description: None,
    }
}

/// Generate an accessible label for the selected cell
pub fn selected_cell_label(row: u32, col: u32, value: &str, locale: Locale) -> String {
    let selected = translate(locale, TranslationKey::SelectedCell);
    let base = cell_label(row, col, value, locale);
    format!("{}. {}", selected, base.label)
}

/// Generate an accessible label for editing a cell
pub fn editing_cell_label(row: u32, col: u32, locale: Locale) -> String {
    let editing = translate(locale, TranslationKey::EditingCell);
    let col_letter = col_to_letter(col);
    let cell_ref = format!("{}{}", col_letter, row + 1);
    format!("{}. {}", editing, cell_ref)
}

/// Keyboard navigation directions for accessibility
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum NavigationDirection {
    Up,
    Down,
    Left,
    Right,
    Next,
    Previous,
    Home,
    End,
    PageUp,
    PageDown,
}

/// Get localized navigation direction name
pub fn navigation_direction_name(dir: NavigationDirection, locale: Locale) -> &'static str {
    let en = match dir {
        NavigationDirection::Up => "Up",
        NavigationDirection::Down => "Down",
        NavigationDirection::Left => "Left",
        NavigationDirection::Right => "Right",
        NavigationDirection::Next => "Next",
        NavigationDirection::Previous => "Previous",
        NavigationDirection::Home => "Home",
        NavigationDirection::End => "End",
        NavigationDirection::PageUp => "Page Up",
        NavigationDirection::PageDown => "Page Down",
    };
    // For now, navigation direction names are in English
    // In a full implementation, these would be translated per locale
    let _ = locale;
    en
}

/// Accessibility configuration for the spreadsheet
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccessibilityConfig {
    /// Enable screen reader announcements
    pub screen_reader_enabled: bool,
    /// Enable high contrast mode
    pub high_contrast: bool,
    /// Enable large text mode
    pub large_text: bool,
    /// Enable keyboard navigation indicators
    pub keyboard_indicators: bool,
    /// ARIA live region politeness level
    pub live_region_politeness: LiveRegionPoliteness,
    /// Focus ring thickness in pixels
    pub focus_ring_thickness: u32,
}

impl Default for AccessibilityConfig {
    fn default() -> Self {
        Self {
            screen_reader_enabled: true,
            high_contrast: false,
            large_text: false,
            keyboard_indicators: true,
            live_region_politeness: LiveRegionPoliteness::Polite,
            focus_ring_thickness: 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LiveRegionPoliteness {
    Off,
    Polite,
    Assertive,
}

impl LiveRegionPoliteness {
    pub fn as_str(&self) -> &'static str {
        match self {
            LiveRegionPoliteness::Off => "off",
            LiveRegionPoliteness::Polite => "polite",
            LiveRegionPoliteness::Assertive => "assertive",
        }
    }
}

// ============================================================================
// Internal Helpers
// ============================================================================

fn format_integer(n: i64, thousands_sep: char) -> String {
    let s = n.abs().to_string();
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::new();
    let len = chars.len();

    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            result.push(thousands_sep);
        }
        result.push(*c);
    }

    result
}

fn serial_to_ymd(serial: f64) -> (i32, u32, u32) {
    // Excel epoch: 1900-01-01 = serial 1
    // Excel treats 1900 as a leap year (it's not), so serial 60 = phantom Feb 29 1900
    // For serials < 61, we need to add 1 to compensate
    let adjusted_serial = if serial < 61.0 { serial + 1.0 } else { serial };
    let days_since_epoch = adjusted_serial as i64 - 25569;

    // Simple date calculation (handles negative days for dates before 1970)
    let mut year = 1970i32;
    let mut remaining = days_since_epoch;

    if remaining >= 0 {
        while remaining >= if is_leap(year) { 366 } else { 365 } {
            remaining -= if is_leap(year) { 366 } else { 365 };
            year += 1;
        }
    } else {
        while remaining < 0 {
            year -= 1;
            remaining += if is_leap(year) { 366 } else { 365 };
        }
    }

    let month_days = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u32;
    for &dim in &month_days {
        if remaining < dim as i64 {
            break;
        }
        remaining -= dim as i64;
        month += 1;
    }

    let day = remaining as u32 + 1;
    (year, month, day)
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

fn col_to_letter(col: u32) -> String {
    let mut result = String::new();
    let mut c = col;
    loop {
        result.insert(0, (b'A' + (c % 26) as u8) as char);
        if c < 26 {
            break;
        }
        c = (c / 26) - 1;
    }
    result
}

fn translate(locale: Locale, key: TranslationKey) -> &'static str {
    get_translation_static(locale, key)
}

fn get_translation_static(locale: Locale, key: TranslationKey) -> &'static str {
    match locale {
        Locale::EnUS | Locale::EnGB => get_english_translation(key),
        Locale::FrFR => get_french_translation(key),
        Locale::DeDE => get_german_translation(key),
        Locale::EsES => get_spanish_translation(key),
        Locale::ItIT => get_italian_translation(key),
        Locale::PtBR => get_portuguese_translation(key),
        Locale::JaJP => get_japanese_translation(key),
        Locale::ZhCN => get_chinese_simplified_translation(key),
        Locale::ZhTW => get_chinese_traditional_translation(key),
        Locale::KoKR => get_korean_translation(key),
        Locale::RuRU => get_russian_translation(key),
        Locale::NlNL => get_dutch_translation(key),
        Locale::SvSE => get_swedish_translation(key),
        Locale::DaDK => get_danish_translation(key),
        Locale::FiFI => get_finnish_translation(key),
        Locale::NoNO => get_norwegian_translation(key),
        Locale::PlPL => get_polish_translation(key),
        Locale::TrTR => get_turkish_translation(key),
        Locale::ArSA => get_arabic_translation(key),
        Locale::HiIN => get_hindi_translation(key),
        Locale::ThTH => get_thai_translation(key),
    }
}

fn get_translations(locale: Locale) -> HashMap<TranslationKey, String> {
    let mut map = HashMap::new();
    for key in all_keys() {
        map.insert(key, get_translation_static(locale, key).to_string());
    }
    map
}

fn all_keys() -> Vec<TranslationKey> {
    vec![
        TranslationKey::File,
        TranslationKey::Edit,
        TranslationKey::View,
        TranslationKey::Insert,
        TranslationKey::Format,
        TranslationKey::Tools,
        TranslationKey::Help,
        TranslationKey::New,
        TranslationKey::Open,
        TranslationKey::Save,
        TranslationKey::SaveAs,
        TranslationKey::Export,
        TranslationKey::Import,
        TranslationKey::Print,
        TranslationKey::Undo,
        TranslationKey::Redo,
        TranslationKey::Cut,
        TranslationKey::Copy,
        TranslationKey::Paste,
        TranslationKey::Find,
        TranslationKey::Replace,
        TranslationKey::Sort,
        TranslationKey::Filter,
        TranslationKey::Cell,
        TranslationKey::Row,
        TranslationKey::Column,
        TranslationKey::InsertRow,
        TranslationKey::InsertColumn,
        TranslationKey::DeleteRow,
        TranslationKey::DeleteColumn,
        TranslationKey::HideRow,
        TranslationKey::HideColumn,
        TranslationKey::AutoFit,
        TranslationKey::Sheet,
        TranslationKey::NewSheet,
        TranslationKey::DeleteSheet,
        TranslationKey::RenameSheet,
        TranslationKey::DuplicateSheet,
        TranslationKey::Formula,
        TranslationKey::Sum,
        TranslationKey::Average,
        TranslationKey::Count,
        TranslationKey::Max,
        TranslationKey::Min,
        TranslationKey::FunctionWizard,
        TranslationKey::Bold,
        TranslationKey::Italic,
        TranslationKey::Underline,
        TranslationKey::Strikethrough,
        TranslationKey::FontSize,
        TranslationKey::FontColor,
        TranslationKey::BackgroundColor,
        TranslationKey::Alignment,
        TranslationKey::WrapText,
        TranslationKey::NumberFormat,
        TranslationKey::Currency,
        TranslationKey::Percentage,
        TranslationKey::Date,
        TranslationKey::Time,
        TranslationKey::DataValidation,
        TranslationKey::ConditionalFormatting,
        TranslationKey::Chart,
        TranslationKey::BarChart,
        TranslationKey::LineChart,
        TranslationKey::PieChart,
        TranslationKey::ScatterChart,
        TranslationKey::PivotTable,
        TranslationKey::ErrorCircularReference,
        TranslationKey::ErrorDivisionByZero,
        TranslationKey::ErrorInvalidFormula,
        TranslationKey::ErrorValueNotFound,
        TranslationKey::ErrorTypeMismatch,
        TranslationKey::Ready,
        TranslationKey::Calculating,
        TranslationKey::Saving,
        TranslationKey::Loading,
        TranslationKey::CellLabel,
        TranslationKey::RowLabel,
        TranslationKey::ColumnLabel,
        TranslationKey::SelectedCell,
        TranslationKey::EditingCell,
        TranslationKey::ConfirmDelete,
        TranslationKey::ConfirmClose,
        TranslationKey::Ok,
        TranslationKey::Cancel,
        TranslationKey::Yes,
        TranslationKey::No,
    ]
}

// ============================================================================
// Translation Tables
// ============================================================================

fn get_english_translation(key: TranslationKey) -> &'static str {
    match key {
        TranslationKey::File => "File",
        TranslationKey::Edit => "Edit",
        TranslationKey::View => "View",
        TranslationKey::Insert => "Insert",
        TranslationKey::Format => "Format",
        TranslationKey::Tools => "Tools",
        TranslationKey::Help => "Help",
        TranslationKey::New => "New",
        TranslationKey::Open => "Open",
        TranslationKey::Save => "Save",
        TranslationKey::SaveAs => "Save As",
        TranslationKey::Export => "Export",
        TranslationKey::Import => "Import",
        TranslationKey::Print => "Print",
        TranslationKey::Undo => "Undo",
        TranslationKey::Redo => "Redo",
        TranslationKey::Cut => "Cut",
        TranslationKey::Copy => "Copy",
        TranslationKey::Paste => "Paste",
        TranslationKey::Find => "Find",
        TranslationKey::Replace => "Replace",
        TranslationKey::Sort => "Sort",
        TranslationKey::Filter => "Filter",
        TranslationKey::Cell => "Cell",
        TranslationKey::Row => "Row",
        TranslationKey::Column => "Column",
        TranslationKey::InsertRow => "Insert Row",
        TranslationKey::InsertColumn => "Insert Column",
        TranslationKey::DeleteRow => "Delete Row",
        TranslationKey::DeleteColumn => "Delete Column",
        TranslationKey::HideRow => "Hide Row",
        TranslationKey::HideColumn => "Hide Column",
        TranslationKey::AutoFit => "Auto Fit",
        TranslationKey::Sheet => "Sheet",
        TranslationKey::NewSheet => "New Sheet",
        TranslationKey::DeleteSheet => "Delete Sheet",
        TranslationKey::RenameSheet => "Rename Sheet",
        TranslationKey::DuplicateSheet => "Duplicate Sheet",
        TranslationKey::Formula => "Formula",
        TranslationKey::Sum => "Sum",
        TranslationKey::Average => "Average",
        TranslationKey::Count => "Count",
        TranslationKey::Max => "Max",
        TranslationKey::Min => "Min",
        TranslationKey::FunctionWizard => "Function Wizard",
        TranslationKey::Bold => "Bold",
        TranslationKey::Italic => "Italic",
        TranslationKey::Underline => "Underline",
        TranslationKey::Strikethrough => "Strikethrough",
        TranslationKey::FontSize => "Font Size",
        TranslationKey::FontColor => "Font Color",
        TranslationKey::BackgroundColor => "Background Color",
        TranslationKey::Alignment => "Alignment",
        TranslationKey::WrapText => "Wrap Text",
        TranslationKey::NumberFormat => "Number Format",
        TranslationKey::Currency => "Currency",
        TranslationKey::Percentage => "Percentage",
        TranslationKey::Date => "Date",
        TranslationKey::Time => "Time",
        TranslationKey::DataValidation => "Data Validation",
        TranslationKey::ConditionalFormatting => "Conditional Formatting",
        TranslationKey::Chart => "Chart",
        TranslationKey::BarChart => "Bar Chart",
        TranslationKey::LineChart => "Line Chart",
        TranslationKey::PieChart => "Pie Chart",
        TranslationKey::ScatterChart => "Scatter Chart",
        TranslationKey::PivotTable => "Pivot Table",
        TranslationKey::ErrorCircularReference => "Circular reference detected",
        TranslationKey::ErrorDivisionByZero => "Division by zero",
        TranslationKey::ErrorInvalidFormula => "Invalid formula",
        TranslationKey::ErrorValueNotFound => "Value not found",
        TranslationKey::ErrorTypeMismatch => "Type mismatch",
        TranslationKey::Ready => "Ready",
        TranslationKey::Calculating => "Calculating...",
        TranslationKey::Saving => "Saving...",
        TranslationKey::Loading => "Loading...",
        TranslationKey::CellLabel => "Cell",
        TranslationKey::RowLabel => "Row",
        TranslationKey::ColumnLabel => "Column",
        TranslationKey::SelectedCell => "Selected",
        TranslationKey::EditingCell => "Editing",
        TranslationKey::ConfirmDelete => "Are you sure you want to delete?",
        TranslationKey::ConfirmClose => {
            "Are you sure you want to close? Unsaved changes will be lost."
        }
        TranslationKey::Ok => "OK",
        TranslationKey::Cancel => "Cancel",
        TranslationKey::Yes => "Yes",
        TranslationKey::No => "No",
    }
}

fn get_french_translation(key: TranslationKey) -> &'static str {
    match key {
        TranslationKey::File => "Fichier",
        TranslationKey::Edit => "Édition",
        TranslationKey::View => "Affichage",
        TranslationKey::Insert => "Insertion",
        TranslationKey::Format => "Format",
        TranslationKey::Tools => "Outils",
        TranslationKey::Help => "Aide",
        TranslationKey::New => "Nouveau",
        TranslationKey::Open => "Ouvrir",
        TranslationKey::Save => "Enregistrer",
        TranslationKey::SaveAs => "Enregistrer sous",
        TranslationKey::Export => "Exporter",
        TranslationKey::Import => "Importer",
        TranslationKey::Print => "Imprimer",
        TranslationKey::Undo => "Annuler",
        TranslationKey::Redo => "Rétablir",
        TranslationKey::Cut => "Couper",
        TranslationKey::Copy => "Copier",
        TranslationKey::Paste => "Coller",
        TranslationKey::Find => "Rechercher",
        TranslationKey::Replace => "Remplacer",
        TranslationKey::Sort => "Trier",
        TranslationKey::Filter => "Filtrer",
        TranslationKey::Cell => "Cellule",
        TranslationKey::Row => "Ligne",
        TranslationKey::Column => "Colonne",
        TranslationKey::InsertRow => "Insérer une ligne",
        TranslationKey::InsertColumn => "Insérer une colonne",
        TranslationKey::DeleteRow => "Supprimer la ligne",
        TranslationKey::DeleteColumn => "Supprimer la colonne",
        TranslationKey::HideRow => "Masquer la ligne",
        TranslationKey::HideColumn => "Masquer la colonne",
        TranslationKey::AutoFit => "Ajustement automatique",
        TranslationKey::Sheet => "Feuille",
        TranslationKey::NewSheet => "Nouvelle feuille",
        TranslationKey::DeleteSheet => "Supprimer la feuille",
        TranslationKey::RenameSheet => "Renommer la feuille",
        TranslationKey::DuplicateSheet => "Dupliquer la feuille",
        TranslationKey::Formula => "Formule",
        TranslationKey::Sum => "Somme",
        TranslationKey::Average => "Moyenne",
        TranslationKey::Count => "Nombre",
        TranslationKey::Max => "Max",
        TranslationKey::Min => "Min",
        TranslationKey::FunctionWizard => "Assistant de fonction",
        TranslationKey::Bold => "Gras",
        TranslationKey::Italic => "Italique",
        TranslationKey::Underline => "Souligné",
        TranslationKey::Strikethrough => "Barré",
        TranslationKey::FontSize => "Taille de police",
        TranslationKey::FontColor => "Couleur de police",
        TranslationKey::BackgroundColor => "Couleur d'arrière-plan",
        TranslationKey::Alignment => "Alignement",
        TranslationKey::WrapText => "Renvoyer à la ligne",
        TranslationKey::NumberFormat => "Format de nombre",
        TranslationKey::Currency => "Monnaie",
        TranslationKey::Percentage => "Pourcentage",
        TranslationKey::Date => "Date",
        TranslationKey::Time => "Heure",
        TranslationKey::DataValidation => "Validation des données",
        TranslationKey::ConditionalFormatting => "Mise en forme conditionnelle",
        TranslationKey::Chart => "Graphique",
        TranslationKey::BarChart => "Graphique à barres",
        TranslationKey::LineChart => "Graphique en courbes",
        TranslationKey::PieChart => "Graphique à secteurs",
        TranslationKey::ScatterChart => "Graphique en nuage de points",
        TranslationKey::PivotTable => "Tableau croisé dynamique",
        TranslationKey::ErrorCircularReference => "Référence circulaire détectée",
        TranslationKey::ErrorDivisionByZero => "Division par zéro",
        TranslationKey::ErrorInvalidFormula => "Formule invalide",
        TranslationKey::ErrorValueNotFound => "Valeur introuvable",
        TranslationKey::ErrorTypeMismatch => "Incompatibilité de type",
        TranslationKey::Ready => "Prêt",
        TranslationKey::Calculating => "Calcul...",
        TranslationKey::Saving => "Enregistrement...",
        TranslationKey::Loading => "Chargement...",
        TranslationKey::CellLabel => "Cellule",
        TranslationKey::RowLabel => "Ligne",
        TranslationKey::ColumnLabel => "Colonne",
        TranslationKey::SelectedCell => "Sélectionné",
        TranslationKey::EditingCell => "Édition",
        TranslationKey::ConfirmDelete => "Êtes-vous sûr de vouloir supprimer ?",
        TranslationKey::ConfirmClose => {
            "Êtes-vous sûr de vouloir fermer ? Les modifications non enregistrées seront perdues."
        }
        TranslationKey::Ok => "OK",
        TranslationKey::Cancel => "Annuler",
        TranslationKey::Yes => "Oui",
        TranslationKey::No => "Non",
    }
}

fn get_german_translation(key: TranslationKey) -> &'static str {
    match key {
        TranslationKey::File => "Datei",
        TranslationKey::Edit => "Bearbeiten",
        TranslationKey::View => "Ansicht",
        TranslationKey::Insert => "Einfügen",
        TranslationKey::Format => "Format",
        TranslationKey::Tools => "Extras",
        TranslationKey::Help => "Hilfe",
        TranslationKey::New => "Neu",
        TranslationKey::Open => "Öffnen",
        TranslationKey::Save => "Speichern",
        TranslationKey::SaveAs => "Speichern unter",
        TranslationKey::Export => "Exportieren",
        TranslationKey::Import => "Importieren",
        TranslationKey::Print => "Drucken",
        TranslationKey::Undo => "Rückgängig",
        TranslationKey::Redo => "Wiederholen",
        TranslationKey::Cut => "Ausschneiden",
        TranslationKey::Copy => "Kopieren",
        TranslationKey::Paste => "Einfügen",
        TranslationKey::Find => "Suchen",
        TranslationKey::Replace => "Ersetzen",
        TranslationKey::Sort => "Sortieren",
        TranslationKey::Filter => "Filtern",
        TranslationKey::Cell => "Zelle",
        TranslationKey::Row => "Zeile",
        TranslationKey::Column => "Spalte",
        TranslationKey::InsertRow => "Zeile einfügen",
        TranslationKey::InsertColumn => "Spalte einfügen",
        TranslationKey::DeleteRow => "Zeile löschen",
        TranslationKey::DeleteColumn => "Spalte löschen",
        TranslationKey::HideRow => "Zeile ausblenden",
        TranslationKey::HideColumn => "Spalte ausblenden",
        TranslationKey::AutoFit => "Automatische Anpassung",
        TranslationKey::Sheet => "Blatt",
        TranslationKey::NewSheet => "Neues Blatt",
        TranslationKey::DeleteSheet => "Blatt löschen",
        TranslationKey::RenameSheet => "Blatt umbenennen",
        TranslationKey::DuplicateSheet => "Blatt duplizieren",
        TranslationKey::Formula => "Formel",
        TranslationKey::Sum => "Summe",
        TranslationKey::Average => "Durchschnitt",
        TranslationKey::Count => "Anzahl",
        TranslationKey::Max => "Max",
        TranslationKey::Min => "Min",
        TranslationKey::FunctionWizard => "Funktions-Assistent",
        TranslationKey::Bold => "Fett",
        TranslationKey::Italic => "Kursiv",
        TranslationKey::Underline => "Unterstrichen",
        TranslationKey::Strikethrough => "Durchgestrichen",
        TranslationKey::FontSize => "Schriftgröße",
        TranslationKey::FontColor => "Schriftfarbe",
        TranslationKey::BackgroundColor => "Hintergrundfarbe",
        TranslationKey::Alignment => "Ausrichtung",
        TranslationKey::WrapText => "Zeilenumbruch",
        TranslationKey::NumberFormat => "Zahlenformat",
        TranslationKey::Currency => "Währung",
        TranslationKey::Percentage => "Prozent",
        TranslationKey::Date => "Datum",
        TranslationKey::Time => "Zeit",
        TranslationKey::DataValidation => "Datenüberprüfung",
        TranslationKey::ConditionalFormatting => "Bedingte Formatierung",
        TranslationKey::Chart => "Diagramm",
        TranslationKey::BarChart => "Balkendiagramm",
        TranslationKey::LineChart => "Liniendiagramm",
        TranslationKey::PieChart => "Kreisdiagramm",
        TranslationKey::ScatterChart => "Streudiagramm",
        TranslationKey::PivotTable => "Pivot-Tabelle",
        TranslationKey::ErrorCircularReference => "Zirkelbezug erkannt",
        TranslationKey::ErrorDivisionByZero => "Division durch Null",
        TranslationKey::ErrorInvalidFormula => "Ungültige Formel",
        TranslationKey::ErrorValueNotFound => "Wert nicht gefunden",
        TranslationKey::ErrorTypeMismatch => "Typkonflikt",
        TranslationKey::Ready => "Bereit",
        TranslationKey::Calculating => "Berechnung...",
        TranslationKey::Saving => "Speichern...",
        TranslationKey::Loading => "Laden...",
        TranslationKey::CellLabel => "Zelle",
        TranslationKey::RowLabel => "Zeile",
        TranslationKey::ColumnLabel => "Spalte",
        TranslationKey::SelectedCell => "Ausgewählt",
        TranslationKey::EditingCell => "Bearbeiten",
        TranslationKey::ConfirmDelete => "Möchten Sie wirklich löschen?",
        TranslationKey::ConfirmClose => {
            "Möchten Sie wirklich schließen? Nicht gespeicherte Änderungen gehen verloren."
        }
        TranslationKey::Ok => "OK",
        TranslationKey::Cancel => "Abbrechen",
        TranslationKey::Yes => "Ja",
        TranslationKey::No => "Nein",
    }
}

fn get_spanish_translation(key: TranslationKey) -> &'static str {
    match key {
        TranslationKey::File => "Archivo",
        TranslationKey::Edit => "Editar",
        TranslationKey::View => "Ver",
        TranslationKey::Insert => "Insertar",
        TranslationKey::Format => "Formato",
        TranslationKey::Tools => "Herramientas",
        TranslationKey::Help => "Ayuda",
        TranslationKey::New => "Nuevo",
        TranslationKey::Open => "Abrir",
        TranslationKey::Save => "Guardar",
        TranslationKey::SaveAs => "Guardar como",
        TranslationKey::Export => "Exportar",
        TranslationKey::Import => "Importar",
        TranslationKey::Print => "Imprimir",
        TranslationKey::Undo => "Deshacer",
        TranslationKey::Redo => "Rehacer",
        TranslationKey::Cut => "Cortar",
        TranslationKey::Copy => "Copiar",
        TranslationKey::Paste => "Pegar",
        TranslationKey::Find => "Buscar",
        TranslationKey::Replace => "Reemplazar",
        TranslationKey::Sort => "Ordenar",
        TranslationKey::Filter => "Filtrar",
        TranslationKey::Cell => "Celda",
        TranslationKey::Row => "Fila",
        TranslationKey::Column => "Columna",
        TranslationKey::InsertRow => "Insertar fila",
        TranslationKey::InsertColumn => "Insertar columna",
        TranslationKey::DeleteRow => "Eliminar fila",
        TranslationKey::DeleteColumn => "Eliminar columna",
        TranslationKey::HideRow => "Ocultar fila",
        TranslationKey::HideColumn => "Ocultar columna",
        TranslationKey::AutoFit => "Autoajustar",
        TranslationKey::Sheet => "Hoja",
        TranslationKey::NewSheet => "Nueva hoja",
        TranslationKey::DeleteSheet => "Eliminar hoja",
        TranslationKey::RenameSheet => "Renombrar hoja",
        TranslationKey::DuplicateSheet => "Duplicar hoja",
        TranslationKey::Formula => "Fórmula",
        TranslationKey::Sum => "Suma",
        TranslationKey::Average => "Promedio",
        TranslationKey::Count => "Contar",
        TranslationKey::Max => "Máx",
        TranslationKey::Min => "Mín",
        TranslationKey::FunctionWizard => "Asistente de funciones",
        TranslationKey::Bold => "Negrita",
        TranslationKey::Italic => "Cursiva",
        TranslationKey::Underline => "Subrayado",
        TranslationKey::Strikethrough => "Tachado",
        TranslationKey::FontSize => "Tamaño de fuente",
        TranslationKey::FontColor => "Color de fuente",
        TranslationKey::BackgroundColor => "Color de fondo",
        TranslationKey::Alignment => "Alineación",
        TranslationKey::WrapText => "Ajustar texto",
        TranslationKey::NumberFormat => "Formato de número",
        TranslationKey::Currency => "Moneda",
        TranslationKey::Percentage => "Porcentaje",
        TranslationKey::Date => "Fecha",
        TranslationKey::Time => "Hora",
        TranslationKey::DataValidation => "Validación de datos",
        TranslationKey::ConditionalFormatting => "Formato condicional",
        TranslationKey::Chart => "Gráfico",
        TranslationKey::BarChart => "Gráfico de barras",
        TranslationKey::LineChart => "Gráfico de líneas",
        TranslationKey::PieChart => "Gráfico circular",
        TranslationKey::ScatterChart => "Gráfico de dispersión",
        TranslationKey::PivotTable => "Tabla dinámica",
        TranslationKey::ErrorCircularReference => "Referencia circular detectada",
        TranslationKey::ErrorDivisionByZero => "División por cero",
        TranslationKey::ErrorInvalidFormula => "Fórmula inválida",
        TranslationKey::ErrorValueNotFound => "Valor no encontrado",
        TranslationKey::ErrorTypeMismatch => "Incompatibilidad de tipo",
        TranslationKey::Ready => "Listo",
        TranslationKey::Calculating => "Calculando...",
        TranslationKey::Saving => "Guardando...",
        TranslationKey::Loading => "Cargando...",
        TranslationKey::CellLabel => "Celda",
        TranslationKey::RowLabel => "Fila",
        TranslationKey::ColumnLabel => "Columna",
        TranslationKey::SelectedCell => "Seleccionado",
        TranslationKey::EditingCell => "Editando",
        TranslationKey::ConfirmDelete => "¿Está seguro de que desea eliminar?",
        TranslationKey::ConfirmClose => {
            "¿Está seguro de que desea cerrar? Se perderán los cambios no guardados."
        }
        TranslationKey::Ok => "Aceptar",
        TranslationKey::Cancel => "Cancelar",
        TranslationKey::Yes => "Sí",
        TranslationKey::No => "No",
    }
}

fn get_italian_translation(key: TranslationKey) -> &'static str {
    match key {
        TranslationKey::File => "File",
        TranslationKey::Edit => "Modifica",
        TranslationKey::View => "Visualizza",
        TranslationKey::Insert => "Inserisci",
        TranslationKey::Format => "Formato",
        TranslationKey::Tools => "Strumenti",
        TranslationKey::Help => "Guida",
        TranslationKey::New => "Nuovo",
        TranslationKey::Open => "Apri",
        TranslationKey::Save => "Salva",
        TranslationKey::SaveAs => "Salva con nome",
        TranslationKey::Export => "Esporta",
        TranslationKey::Import => "Importa",
        TranslationKey::Print => "Stampa",
        TranslationKey::Undo => "Annulla",
        TranslationKey::Redo => "Ripristina",
        TranslationKey::Cut => "Taglia",
        TranslationKey::Copy => "Copia",
        TranslationKey::Paste => "Incolla",
        TranslationKey::Find => "Trova",
        TranslationKey::Replace => "Sostituisci",
        TranslationKey::Sort => "Ordina",
        TranslationKey::Filter => "Filtra",
        TranslationKey::Cell => "Cella",
        TranslationKey::Row => "Riga",
        TranslationKey::Column => "Colonna",
        TranslationKey::InsertRow => "Inserisci riga",
        TranslationKey::InsertColumn => "Inserisci colonna",
        TranslationKey::DeleteRow => "Elimina riga",
        TranslationKey::DeleteColumn => "Elimina colonna",
        TranslationKey::HideRow => "Nascondi riga",
        TranslationKey::HideColumn => "Nascondi colonna",
        TranslationKey::AutoFit => "Adattamento automatico",
        TranslationKey::Sheet => "Foglio",
        TranslationKey::NewSheet => "Nuovo foglio",
        TranslationKey::DeleteSheet => "Elimina foglio",
        TranslationKey::RenameSheet => "Rinomina foglio",
        TranslationKey::DuplicateSheet => "Duplica foglio",
        TranslationKey::Formula => "Formula",
        TranslationKey::Sum => "Somma",
        TranslationKey::Average => "Media",
        TranslationKey::Count => "Conta",
        TranslationKey::Max => "Max",
        TranslationKey::Min => "Min",
        TranslationKey::FunctionWizard => "Procedura guidata funzione",
        TranslationKey::Bold => "Grassetto",
        TranslationKey::Italic => "Corsivo",
        TranslationKey::Underline => "Sottolineato",
        TranslationKey::Strikethrough => "Barrato",
        TranslationKey::FontSize => "Dimensione carattere",
        TranslationKey::FontColor => "Colore carattere",
        TranslationKey::BackgroundColor => "Colore sfondo",
        TranslationKey::Alignment => "Allineamento",
        TranslationKey::WrapText => "Testo a capo",
        TranslationKey::NumberFormat => "Formato numero",
        TranslationKey::Currency => "Valuta",
        TranslationKey::Percentage => "Percentuale",
        TranslationKey::Date => "Data",
        TranslationKey::Time => "Ora",
        TranslationKey::DataValidation => "Convalida dati",
        TranslationKey::ConditionalFormatting => "Formattazione condizionale",
        TranslationKey::Chart => "Grafico",
        TranslationKey::BarChart => "Grafico a barre",
        TranslationKey::LineChart => "Grafico a linee",
        TranslationKey::PieChart => "Grafico a torta",
        TranslationKey::ScatterChart => "Grafico a dispersione",
        TranslationKey::PivotTable => "Tabella pivot",
        TranslationKey::ErrorCircularReference => "Riferimento circolare rilevato",
        TranslationKey::ErrorDivisionByZero => "Divisione per zero",
        TranslationKey::ErrorInvalidFormula => "Formula non valida",
        TranslationKey::ErrorValueNotFound => "Valore non trovato",
        TranslationKey::ErrorTypeMismatch => "Incompatibilità di tipo",
        TranslationKey::Ready => "Pronto",
        TranslationKey::Calculating => "Calcolo...",
        TranslationKey::Saving => "Salvataggio...",
        TranslationKey::Loading => "Caricamento...",
        TranslationKey::CellLabel => "Cella",
        TranslationKey::RowLabel => "Riga",
        TranslationKey::ColumnLabel => "Colonna",
        TranslationKey::SelectedCell => "Selezionato",
        TranslationKey::EditingCell => "Modifica",
        TranslationKey::ConfirmDelete => "Sei sicuro di voler eliminare?",
        TranslationKey::ConfirmClose => {
            "Sei sicuro di voler chiudere? Le modifiche non salvate andranno perse."
        }
        TranslationKey::Ok => "OK",
        TranslationKey::Cancel => "Annulla",
        TranslationKey::Yes => "Sì",
        TranslationKey::No => "No",
    }
}

fn get_portuguese_translation(key: TranslationKey) -> &'static str {
    match key {
        TranslationKey::File => "Arquivo",
        TranslationKey::Edit => "Editar",
        TranslationKey::View => "Exibir",
        TranslationKey::Insert => "Inserir",
        TranslationKey::Format => "Formatar",
        TranslationKey::Tools => "Ferramentas",
        TranslationKey::Help => "Ajuda",
        TranslationKey::New => "Novo",
        TranslationKey::Open => "Abrir",
        TranslationKey::Save => "Salvar",
        TranslationKey::SaveAs => "Salvar como",
        TranslationKey::Export => "Exportar",
        TranslationKey::Import => "Importar",
        TranslationKey::Print => "Imprimir",
        TranslationKey::Undo => "Desfazer",
        TranslationKey::Redo => "Refazer",
        TranslationKey::Cut => "Recortar",
        TranslationKey::Copy => "Copiar",
        TranslationKey::Paste => "Colar",
        TranslationKey::Find => "Localizar",
        TranslationKey::Replace => "Substituir",
        TranslationKey::Sort => "Classificar",
        TranslationKey::Filter => "Filtrar",
        TranslationKey::Cell => "Célula",
        TranslationKey::Row => "Linha",
        TranslationKey::Column => "Coluna",
        TranslationKey::InsertRow => "Inserir linha",
        TranslationKey::InsertColumn => "Inserir coluna",
        TranslationKey::DeleteRow => "Excluir linha",
        TranslationKey::DeleteColumn => "Excluir coluna",
        TranslationKey::HideRow => "Ocultar linha",
        TranslationKey::HideColumn => "Ocultar coluna",
        TranslationKey::AutoFit => "Autoajustar",
        TranslationKey::Sheet => "Planilha",
        TranslationKey::NewSheet => "Nova planilha",
        TranslationKey::DeleteSheet => "Excluir planilha",
        TranslationKey::RenameSheet => "Renomear planilha",
        TranslationKey::DuplicateSheet => "Duplicar planilha",
        TranslationKey::Formula => "Fórmula",
        TranslationKey::Sum => "Soma",
        TranslationKey::Average => "Média",
        TranslationKey::Count => "Contar",
        TranslationKey::Max => "Máx",
        TranslationKey::Min => "Mín",
        TranslationKey::FunctionWizard => "Assistente de função",
        TranslationKey::Bold => "Negrito",
        TranslationKey::Italic => "Itálico",
        TranslationKey::Underline => "Sublinhado",
        TranslationKey::Strikethrough => "Tachado",
        TranslationKey::FontSize => "Tamanho da fonte",
        TranslationKey::FontColor => "Cor da fonte",
        TranslationKey::BackgroundColor => "Cor de fundo",
        TranslationKey::Alignment => "Alinhamento",
        TranslationKey::WrapText => "Quebrar texto",
        TranslationKey::NumberFormat => "Formato de número",
        TranslationKey::Currency => "Moeda",
        TranslationKey::Percentage => "Porcentagem",
        TranslationKey::Date => "Data",
        TranslationKey::Time => "Hora",
        TranslationKey::DataValidation => "Validação de dados",
        TranslationKey::ConditionalFormatting => "Formatação condicional",
        TranslationKey::Chart => "Gráfico",
        TranslationKey::BarChart => "Gráfico de barras",
        TranslationKey::LineChart => "Gráfico de linhas",
        TranslationKey::PieChart => "Gráfico de pizza",
        TranslationKey::ScatterChart => "Gráfico de dispersão",
        TranslationKey::PivotTable => "Tabela dinâmica",
        TranslationKey::ErrorCircularReference => "Referência circular detectada",
        TranslationKey::ErrorDivisionByZero => "Divisão por zero",
        TranslationKey::ErrorInvalidFormula => "Fórmula inválida",
        TranslationKey::ErrorValueNotFound => "Valor não encontrado",
        TranslationKey::ErrorTypeMismatch => "Incompatibilidade de tipo",
        TranslationKey::Ready => "Pronto",
        TranslationKey::Calculating => "Calculando...",
        TranslationKey::Saving => "Salvando...",
        TranslationKey::Loading => "Carregando...",
        TranslationKey::CellLabel => "Célula",
        TranslationKey::RowLabel => "Linha",
        TranslationKey::ColumnLabel => "Coluna",
        TranslationKey::SelectedCell => "Selecionado",
        TranslationKey::EditingCell => "Editando",
        TranslationKey::ConfirmDelete => "Tem certeza de que deseja excluir?",
        TranslationKey::ConfirmClose => {
            "Tem certeza de que deseja fechar? As alterações não salvas serão perdidas."
        }
        TranslationKey::Ok => "OK",
        TranslationKey::Cancel => "Cancelar",
        TranslationKey::Yes => "Sim",
        TranslationKey::No => "Não",
    }
}

fn get_japanese_translation(key: TranslationKey) -> &'static str {
    match key {
        TranslationKey::File => "ファイル",
        TranslationKey::Edit => "編集",
        TranslationKey::View => "表示",
        TranslationKey::Insert => "挿入",
        TranslationKey::Format => "書式",
        TranslationKey::Tools => "ツール",
        TranslationKey::Help => "ヘルプ",
        TranslationKey::New => "新規",
        TranslationKey::Open => "開く",
        TranslationKey::Save => "保存",
        TranslationKey::SaveAs => "名前を付けて保存",
        TranslationKey::Export => "エクスポート",
        TranslationKey::Import => "インポート",
        TranslationKey::Print => "印刷",
        TranslationKey::Undo => "元に戻す",
        TranslationKey::Redo => "やり直し",
        TranslationKey::Cut => "切り取り",
        TranslationKey::Copy => "コピー",
        TranslationKey::Paste => "貼り付け",
        TranslationKey::Find => "検索",
        TranslationKey::Replace => "置換",
        TranslationKey::Sort => "並べ替え",
        TranslationKey::Filter => "フィルター",
        TranslationKey::Cell => "セル",
        TranslationKey::Row => "行",
        TranslationKey::Column => "列",
        TranslationKey::InsertRow => "行を挿入",
        TranslationKey::InsertColumn => "列を挿入",
        TranslationKey::DeleteRow => "行を削除",
        TranslationKey::DeleteColumn => "列を削除",
        TranslationKey::HideRow => "行を非表示",
        TranslationKey::HideColumn => "列を非表示",
        TranslationKey::AutoFit => "自動調整",
        TranslationKey::Sheet => "シート",
        TranslationKey::NewSheet => "新しいシート",
        TranslationKey::DeleteSheet => "シートを削除",
        TranslationKey::RenameSheet => "シート名の変更",
        TranslationKey::DuplicateSheet => "シートを複製",
        TranslationKey::Formula => "数式",
        TranslationKey::Sum => "合計",
        TranslationKey::Average => "平均",
        TranslationKey::Count => "カウント",
        TranslationKey::Max => "最大",
        TranslationKey::Min => "最小",
        TranslationKey::FunctionWizard => "関数ウィザード",
        TranslationKey::Bold => "太字",
        TranslationKey::Italic => "斜体",
        TranslationKey::Underline => "下線",
        TranslationKey::Strikethrough => "取り消し線",
        TranslationKey::FontSize => "フォントサイズ",
        TranslationKey::FontColor => "フォントの色",
        TranslationKey::BackgroundColor => "背景色",
        TranslationKey::Alignment => "配置",
        TranslationKey::WrapText => "折り返して全体を表示",
        TranslationKey::NumberFormat => "数値の書式",
        TranslationKey::Currency => "通貨",
        TranslationKey::Percentage => "パーセンテージ",
        TranslationKey::Date => "日付",
        TranslationKey::Time => "時刻",
        TranslationKey::DataValidation => "データの入力規則",
        TranslationKey::ConditionalFormatting => "条件付き書式",
        TranslationKey::Chart => "グラフ",
        TranslationKey::BarChart => "棒グラフ",
        TranslationKey::LineChart => "折れ線グラフ",
        TranslationKey::PieChart => "円グラフ",
        TranslationKey::ScatterChart => "散布図",
        TranslationKey::PivotTable => "ピボットテーブル",
        TranslationKey::ErrorCircularReference => "循環参照が検出されました",
        TranslationKey::ErrorDivisionByZero => "ゼロ除算",
        TranslationKey::ErrorInvalidFormula => "無効な数式",
        TranslationKey::ErrorValueNotFound => "値が見つかりません",
        TranslationKey::ErrorTypeMismatch => "型の不一致",
        TranslationKey::Ready => "準備完了",
        TranslationKey::Calculating => "計算中...",
        TranslationKey::Saving => "保存中...",
        TranslationKey::Loading => "読み込み中...",
        TranslationKey::CellLabel => "セル",
        TranslationKey::RowLabel => "行",
        TranslationKey::ColumnLabel => "列",
        TranslationKey::SelectedCell => "選択済み",
        TranslationKey::EditingCell => "編集中",
        TranslationKey::ConfirmDelete => "本当に削除しますか？",
        TranslationKey::ConfirmClose => "本当に閉じますか？未保存の変更は失われます。",
        TranslationKey::Ok => "OK",
        TranslationKey::Cancel => "キャンセル",
        TranslationKey::Yes => "はい",
        TranslationKey::No => "いいえ",
    }
}

fn get_chinese_simplified_translation(key: TranslationKey) -> &'static str {
    match key {
        TranslationKey::File => "文件",
        TranslationKey::Edit => "编辑",
        TranslationKey::View => "视图",
        TranslationKey::Insert => "插入",
        TranslationKey::Format => "格式",
        TranslationKey::Tools => "工具",
        TranslationKey::Help => "帮助",
        TranslationKey::New => "新建",
        TranslationKey::Open => "打开",
        TranslationKey::Save => "保存",
        TranslationKey::SaveAs => "另存为",
        TranslationKey::Export => "导出",
        TranslationKey::Import => "导入",
        TranslationKey::Print => "打印",
        TranslationKey::Undo => "撤销",
        TranslationKey::Redo => "重做",
        TranslationKey::Cut => "剪切",
        TranslationKey::Copy => "复制",
        TranslationKey::Paste => "粘贴",
        TranslationKey::Find => "查找",
        TranslationKey::Replace => "替换",
        TranslationKey::Sort => "排序",
        TranslationKey::Filter => "筛选",
        TranslationKey::Cell => "单元格",
        TranslationKey::Row => "行",
        TranslationKey::Column => "列",
        TranslationKey::InsertRow => "插入行",
        TranslationKey::InsertColumn => "插入列",
        TranslationKey::DeleteRow => "删除行",
        TranslationKey::DeleteColumn => "删除列",
        TranslationKey::HideRow => "隐藏行",
        TranslationKey::HideColumn => "隐藏列",
        TranslationKey::AutoFit => "自动调整",
        TranslationKey::Sheet => "工作表",
        TranslationKey::NewSheet => "新建工作表",
        TranslationKey::DeleteSheet => "删除工作表",
        TranslationKey::RenameSheet => "重命名工作表",
        TranslationKey::DuplicateSheet => "复制工作表",
        TranslationKey::Formula => "公式",
        TranslationKey::Sum => "求和",
        TranslationKey::Average => "平均值",
        TranslationKey::Count => "计数",
        TranslationKey::Max => "最大值",
        TranslationKey::Min => "最小值",
        TranslationKey::FunctionWizard => "函数向导",
        TranslationKey::Bold => "加粗",
        TranslationKey::Italic => "斜体",
        TranslationKey::Underline => "下划线",
        TranslationKey::Strikethrough => "删除线",
        TranslationKey::FontSize => "字号",
        TranslationKey::FontColor => "字体颜色",
        TranslationKey::BackgroundColor => "背景色",
        TranslationKey::Alignment => "对齐",
        TranslationKey::WrapText => "自动换行",
        TranslationKey::NumberFormat => "数字格式",
        TranslationKey::Currency => "货币",
        TranslationKey::Percentage => "百分比",
        TranslationKey::Date => "日期",
        TranslationKey::Time => "时间",
        TranslationKey::DataValidation => "数据验证",
        TranslationKey::ConditionalFormatting => "条件格式",
        TranslationKey::Chart => "图表",
        TranslationKey::BarChart => "柱形图",
        TranslationKey::LineChart => "折线图",
        TranslationKey::PieChart => "饼图",
        TranslationKey::ScatterChart => "散点图",
        TranslationKey::PivotTable => "数据透视表",
        TranslationKey::ErrorCircularReference => "检测到循环引用",
        TranslationKey::ErrorDivisionByZero => "除以零",
        TranslationKey::ErrorInvalidFormula => "无效公式",
        TranslationKey::ErrorValueNotFound => "未找到值",
        TranslationKey::ErrorTypeMismatch => "类型不匹配",
        TranslationKey::Ready => "就绪",
        TranslationKey::Calculating => "计算中...",
        TranslationKey::Saving => "保存中...",
        TranslationKey::Loading => "加载中...",
        TranslationKey::CellLabel => "单元格",
        TranslationKey::RowLabel => "行",
        TranslationKey::ColumnLabel => "列",
        TranslationKey::SelectedCell => "已选中",
        TranslationKey::EditingCell => "编辑中",
        TranslationKey::ConfirmDelete => "确定要删除吗？",
        TranslationKey::ConfirmClose => "确定要关闭吗？未保存的更改将丢失。",
        TranslationKey::Ok => "确定",
        TranslationKey::Cancel => "取消",
        TranslationKey::Yes => "是",
        TranslationKey::No => "否",
    }
}

// For locales without full translations, fall back to English
fn get_chinese_traditional_translation(key: TranslationKey) -> &'static str {
    get_chinese_simplified_translation(key)
}

fn get_korean_translation(key: TranslationKey) -> &'static str {
    match key {
        TranslationKey::File => "파일",
        TranslationKey::Edit => "편집",
        TranslationKey::View => "보기",
        TranslationKey::Insert => "삽입",
        TranslationKey::Format => "서식",
        TranslationKey::Tools => "도구",
        TranslationKey::Help => "도움말",
        TranslationKey::New => "새로 만들기",
        TranslationKey::Open => "열기",
        TranslationKey::Save => "저장",
        TranslationKey::SaveAs => "다른 이름으로 저장",
        TranslationKey::Export => "내보내기",
        TranslationKey::Import => "가져오기",
        TranslationKey::Print => "인쇄",
        TranslationKey::Undo => "실행 취소",
        TranslationKey::Redo => "다시 실행",
        TranslationKey::Cut => "잘라내기",
        TranslationKey::Copy => "복사",
        TranslationKey::Paste => "붙여넣기",
        TranslationKey::Find => "찾기",
        TranslationKey::Replace => "바꾸기",
        TranslationKey::Sort => "정렬",
        TranslationKey::Filter => "필터",
        TranslationKey::Cell => "셀",
        TranslationKey::Row => "행",
        TranslationKey::Column => "열",
        TranslationKey::InsertRow => "행 삽입",
        TranslationKey::InsertColumn => "열 삽입",
        TranslationKey::DeleteRow => "행 삭제",
        TranslationKey::DeleteColumn => "열 삭제",
        TranslationKey::HideRow => "행 숨기기",
        TranslationKey::HideColumn => "열 숨기기",
        TranslationKey::AutoFit => "자동 맞춤",
        TranslationKey::Sheet => "시트",
        TranslationKey::NewSheet => "새 시트",
        TranslationKey::DeleteSheet => "시트 삭제",
        TranslationKey::RenameSheet => "시트 이름 변경",
        TranslationKey::DuplicateSheet => "시트 복사",
        TranslationKey::Formula => "수식",
        TranslationKey::Sum => "합계",
        TranslationKey::Average => "평균",
        TranslationKey::Count => "개수",
        TranslationKey::Max => "최대",
        TranslationKey::Min => "최소",
        TranslationKey::FunctionWizard => "함수 마법사",
        TranslationKey::Bold => "굵게",
        TranslationKey::Italic => "기울임",
        TranslationKey::Underline => "밑줄",
        TranslationKey::Strikethrough => "취소선",
        TranslationKey::FontSize => "글꼴 크기",
        TranslationKey::FontColor => "글꼴 색",
        TranslationKey::BackgroundColor => "배경색",
        TranslationKey::Alignment => "맞춤",
        TranslationKey::WrapText => "텍스트 줄 바꿈",
        TranslationKey::NumberFormat => "숫자 서식",
        TranslationKey::Currency => "통화",
        TranslationKey::Percentage => "백분율",
        TranslationKey::Date => "날짜",
        TranslationKey::Time => "시간",
        TranslationKey::DataValidation => "데이터 유효성 검사",
        TranslationKey::ConditionalFormatting => "조건부 서식",
        TranslationKey::Chart => "차트",
        TranslationKey::BarChart => "막대 차트",
        TranslationKey::LineChart => "꺾은선 차트",
        TranslationKey::PieChart => "원형 차트",
        TranslationKey::ScatterChart => "분산형 차트",
        TranslationKey::PivotTable => "피벗 테이블",
        TranslationKey::ErrorCircularReference => "순환 참조가 감지되었습니다",
        TranslationKey::ErrorDivisionByZero => "0으로 나누기",
        TranslationKey::ErrorInvalidFormula => "잘못된 수식",
        TranslationKey::ErrorValueNotFound => "값을 찾을 수 없습니다",
        TranslationKey::ErrorTypeMismatch => "형식이 일치하지 않습니다",
        TranslationKey::Ready => "준비",
        TranslationKey::Calculating => "계산 중...",
        TranslationKey::Saving => "저장 중...",
        TranslationKey::Loading => "로드 중...",
        TranslationKey::CellLabel => "셀",
        TranslationKey::RowLabel => "행",
        TranslationKey::ColumnLabel => "열",
        TranslationKey::SelectedCell => "선택됨",
        TranslationKey::EditingCell => "편집 중",
        TranslationKey::ConfirmDelete => "정말 삭제하시겠습니까?",
        TranslationKey::ConfirmClose => {
            "정말 닫으시겠습니까? 저장되지 않은 변경 사항이 손실됩니다."
        }
        TranslationKey::Ok => "확인",
        TranslationKey::Cancel => "취소",
        TranslationKey::Yes => "예",
        TranslationKey::No => "아니오",
    }
}

fn get_russian_translation(key: TranslationKey) -> &'static str {
    match key {
        TranslationKey::File => "Файл",
        TranslationKey::Edit => "Правка",
        TranslationKey::View => "Вид",
        TranslationKey::Insert => "Вставка",
        TranslationKey::Format => "Формат",
        TranslationKey::Tools => "Сервис",
        TranslationKey::Help => "Справка",
        TranslationKey::New => "Создать",
        TranslationKey::Open => "Открыть",
        TranslationKey::Save => "Сохранить",
        TranslationKey::SaveAs => "Сохранить как",
        TranslationKey::Export => "Экспорт",
        TranslationKey::Import => "Импорт",
        TranslationKey::Print => "Печать",
        TranslationKey::Undo => "Отменить",
        TranslationKey::Redo => "Повторить",
        TranslationKey::Cut => "Вырезать",
        TranslationKey::Copy => "Копировать",
        TranslationKey::Paste => "Вставить",
        TranslationKey::Find => "Найти",
        TranslationKey::Replace => "Заменить",
        TranslationKey::Sort => "Сортировка",
        TranslationKey::Filter => "Фильтр",
        TranslationKey::Cell => "Ячейка",
        TranslationKey::Row => "Строка",
        TranslationKey::Column => "Столбец",
        TranslationKey::InsertRow => "Вставить строку",
        TranslationKey::InsertColumn => "Вставить столбец",
        TranslationKey::DeleteRow => "Удалить строку",
        TranslationKey::DeleteColumn => "Удалить столбец",
        TranslationKey::HideRow => "Скрыть строку",
        TranslationKey::HideColumn => "Скрыть столбец",
        TranslationKey::AutoFit => "Автоподбор",
        TranslationKey::Sheet => "Лист",
        TranslationKey::NewSheet => "Новый лист",
        TranslationKey::DeleteSheet => "Удалить лист",
        TranslationKey::RenameSheet => "Переименовать лист",
        TranslationKey::DuplicateSheet => "Дублировать лист",
        TranslationKey::Formula => "Формула",
        TranslationKey::Sum => "Сумма",
        TranslationKey::Average => "Среднее",
        TranslationKey::Count => "Количество",
        TranslationKey::Max => "Макс",
        TranslationKey::Min => "Мин",
        TranslationKey::FunctionWizard => "Мастер функций",
        TranslationKey::Bold => "Полужирный",
        TranslationKey::Italic => "Курсив",
        TranslationKey::Underline => "Подчёркнутый",
        TranslationKey::Strikethrough => "Зачёркнутый",
        TranslationKey::FontSize => "Размер шрифта",
        TranslationKey::FontColor => "Цвет шрифта",
        TranslationKey::BackgroundColor => "Цвет фона",
        TranslationKey::Alignment => "Выравнивание",
        TranslationKey::WrapText => "Перенос текста",
        TranslationKey::NumberFormat => "Формат числа",
        TranslationKey::Currency => "Валюта",
        TranslationKey::Percentage => "Процент",
        TranslationKey::Date => "Дата",
        TranslationKey::Time => "Время",
        TranslationKey::DataValidation => "Проверка данных",
        TranslationKey::ConditionalFormatting => "Условное форматирование",
        TranslationKey::Chart => "Диаграмма",
        TranslationKey::BarChart => "Гистограмма",
        TranslationKey::LineChart => "Линейная диаграмма",
        TranslationKey::PieChart => "Круговая диаграмма",
        TranslationKey::ScatterChart => "Точечная диаграмма",
        TranslationKey::PivotTable => "Сводная таблица",
        TranslationKey::ErrorCircularReference => "Обнаружена циклическая ссылка",
        TranslationKey::ErrorDivisionByZero => "Деление на ноль",
        TranslationKey::ErrorInvalidFormula => "Недопустимая формула",
        TranslationKey::ErrorValueNotFound => "Значение не найдено",
        TranslationKey::ErrorTypeMismatch => "Несоответствие типов",
        TranslationKey::Ready => "Готово",
        TranslationKey::Calculating => "Вычисление...",
        TranslationKey::Saving => "Сохранение...",
        TranslationKey::Loading => "Загрузка...",
        TranslationKey::CellLabel => "Ячейка",
        TranslationKey::RowLabel => "Строка",
        TranslationKey::ColumnLabel => "Столбец",
        TranslationKey::SelectedCell => "Выбрано",
        TranslationKey::EditingCell => "Редактирование",
        TranslationKey::ConfirmDelete => "Вы уверены, что хотите удалить?",
        TranslationKey::ConfirmClose => {
            "Вы уверены, что хотите закрыть? Несохранённые изменения будут потеряны."
        }
        TranslationKey::Ok => "OK",
        TranslationKey::Cancel => "Отмена",
        TranslationKey::Yes => "Да",
        TranslationKey::No => "Нет",
    }
}

// Remaining locales fall back to English for now
fn get_dutch_translation(key: TranslationKey) -> &'static str {
    get_english_translation(key)
}
fn get_swedish_translation(key: TranslationKey) -> &'static str {
    get_english_translation(key)
}
fn get_danish_translation(key: TranslationKey) -> &'static str {
    get_english_translation(key)
}
fn get_finnish_translation(key: TranslationKey) -> &'static str {
    get_english_translation(key)
}
fn get_norwegian_translation(key: TranslationKey) -> &'static str {
    get_english_translation(key)
}
fn get_polish_translation(key: TranslationKey) -> &'static str {
    get_english_translation(key)
}
fn get_turkish_translation(key: TranslationKey) -> &'static str {
    get_english_translation(key)
}
fn get_arabic_translation(key: TranslationKey) -> &'static str {
    get_english_translation(key)
}
fn get_hindi_translation(key: TranslationKey) -> &'static str {
    get_english_translation(key)
}
fn get_thai_translation(key: TranslationKey) -> &'static str {
    get_english_translation(key)
}

// ============================================================================
// Weekday / Month Names
// ============================================================================

fn weekday_names(locale: Locale) -> &'static [&'static str] {
    match locale {
        Locale::FrFR => &[
            "Dimanche", "Lundi", "Mardi", "Mercredi", "Jeudi", "Vendredi", "Samedi",
        ],
        Locale::DeDE => &[
            "Sonntag",
            "Montag",
            "Dienstag",
            "Mittwoch",
            "Donnerstag",
            "Freitag",
            "Samstag",
        ],
        Locale::EsES => &[
            "Domingo",
            "Lunes",
            "Martes",
            "Miércoles",
            "Jueves",
            "Viernes",
            "Sábado",
        ],
        Locale::ItIT => &[
            "Domenica",
            "Lunedì",
            "Martedì",
            "Mercoledì",
            "Giovedì",
            "Venerdì",
            "Sabato",
        ],
        Locale::PtBR => &[
            "Domingo", "Segunda", "Terça", "Quarta", "Quinta", "Sexta", "Sábado",
        ],
        Locale::JaJP => &[
            "日曜日",
            "月曜日",
            "火曜日",
            "水曜日",
            "木曜日",
            "金曜日",
            "土曜日",
        ],
        Locale::ZhCN | Locale::ZhTW => &[
            "星期日",
            "星期一",
            "星期二",
            "星期三",
            "星期四",
            "星期五",
            "星期六",
        ],
        Locale::KoKR => &[
            "일요일",
            "월요일",
            "화요일",
            "수요일",
            "목요일",
            "금요일",
            "토요일",
        ],
        Locale::RuRU => &[
            "Воскресенье",
            "Понедельник",
            "Вторник",
            "Среда",
            "Четверг",
            "Пятница",
            "Суббота",
        ],
        _ => &[
            "Sunday",
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
        ],
    }
}

fn weekday_abbrs(locale: Locale) -> &'static [&'static str] {
    match locale {
        Locale::FrFR => &["Dim", "Lun", "Mar", "Mer", "Jeu", "Ven", "Sam"],
        Locale::DeDE => &["So", "Mo", "Di", "Mi", "Do", "Fr", "Sa"],
        Locale::EsES => &["Dom", "Lun", "Mar", "Mié", "Jue", "Vie", "Sáb"],
        Locale::ItIT => &["Dom", "Lun", "Mar", "Mer", "Gio", "Ven", "Sab"],
        Locale::PtBR => &["Dom", "Seg", "Ter", "Qua", "Qui", "Sex", "Sáb"],
        Locale::JaJP => &["日", "月", "火", "水", "木", "金", "土"],
        Locale::ZhCN | Locale::ZhTW => &["日", "一", "二", "三", "四", "五", "六"],
        Locale::KoKR => &["일", "월", "화", "수", "목", "금", "토"],
        Locale::RuRU => &["Вс", "Пн", "Вт", "Ср", "Чт", "Пт", "Сб"],
        _ => &["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
    }
}

fn month_names(locale: Locale) -> &'static [&'static str] {
    match locale {
        Locale::FrFR => &[
            "Janvier",
            "Février",
            "Mars",
            "Avril",
            "Mai",
            "Juin",
            "Juillet",
            "Août",
            "Septembre",
            "Octobre",
            "Novembre",
            "Décembre",
        ],
        Locale::DeDE => &[
            "Januar",
            "Februar",
            "März",
            "April",
            "Mai",
            "Juni",
            "Juli",
            "August",
            "September",
            "Oktober",
            "November",
            "Dezember",
        ],
        Locale::EsES => &[
            "Enero",
            "Febrero",
            "Marzo",
            "Abril",
            "Mayo",
            "Junio",
            "Julio",
            "Agosto",
            "Septiembre",
            "Octubre",
            "Noviembre",
            "Diciembre",
        ],
        Locale::ItIT => &[
            "Gennaio",
            "Febbraio",
            "Marzo",
            "Aprile",
            "Maggio",
            "Giugno",
            "Luglio",
            "Agosto",
            "Settembre",
            "Ottobre",
            "Novembre",
            "Dicembre",
        ],
        Locale::PtBR => &[
            "Janeiro",
            "Fevereiro",
            "Março",
            "Abril",
            "Maio",
            "Junho",
            "Julho",
            "Agosto",
            "Setembro",
            "Outubro",
            "Novembro",
            "Dezembro",
        ],
        Locale::JaJP => &[
            "1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月",
        ],
        Locale::ZhCN | Locale::ZhTW => &[
            "一月",
            "二月",
            "三月",
            "四月",
            "五月",
            "六月",
            "七月",
            "八月",
            "九月",
            "十月",
            "十一月",
            "十二月",
        ],
        Locale::KoKR => &[
            "1월", "2월", "3월", "4월", "5월", "6월", "7월", "8월", "9월", "10월", "11월", "12월",
        ],
        Locale::RuRU => &[
            "Январь",
            "Февраль",
            "Март",
            "Апрель",
            "Май",
            "Июнь",
            "Июль",
            "Август",
            "Сентябрь",
            "Октябрь",
            "Ноябрь",
            "Декабрь",
        ],
        _ => &[
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ],
    }
}

fn month_abbrs(locale: Locale) -> &'static [&'static str] {
    match locale {
        Locale::FrFR => &[
            "Jan", "Fév", "Mar", "Avr", "Mai", "Juin", "Juil", "Aoû", "Sep", "Oct", "Nov", "Déc",
        ],
        Locale::DeDE => &[
            "Jan", "Feb", "Mär", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez",
        ],
        Locale::EsES => &[
            "Ene", "Feb", "Mar", "Abr", "May", "Jun", "Jul", "Ago", "Sep", "Oct", "Nov", "Dic",
        ],
        Locale::ItIT => &[
            "Gen", "Feb", "Mar", "Apr", "Mag", "Giu", "Lug", "Ago", "Set", "Ott", "Nov", "Dic",
        ],
        Locale::PtBR => &[
            "Jan", "Fev", "Mar", "Abr", "Mai", "Jun", "Jul", "Ago", "Set", "Out", "Nov", "Dez",
        ],
        Locale::JaJP => &[
            "1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月",
        ],
        Locale::ZhCN | Locale::ZhTW => &[
            "1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月",
        ],
        Locale::KoKR => &[
            "1월", "2월", "3월", "4월", "5월", "6월", "7월", "8월", "9월", "10월", "11월", "12월",
        ],
        Locale::RuRU => &[
            "Янв", "Фев", "Мар", "Апр", "Май", "Июн", "Июл", "Авг", "Сен", "Окт", "Ноя", "Дек",
        ],
        _ => &[
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ],
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locale_code() {
        assert_eq!(Locale::EnUS.code(), "en-US");
        assert_eq!(Locale::FrFR.code(), "fr-FR");
        assert_eq!(Locale::JaJP.code(), "ja-JP");
    }

    #[test]
    fn test_locale_from_code() {
        assert_eq!(Locale::from_code("en-US"), Some(Locale::EnUS));
        assert_eq!(Locale::from_code("fr_FR"), Some(Locale::FrFR));
        assert_eq!(Locale::from_code("xx-XX"), None);
    }

    #[test]
    fn test_locale_all() {
        let all = Locale::all();
        assert!(all.len() >= 22);
        assert!(all.contains(&Locale::EnUS));
        assert!(all.contains(&Locale::JaJP));
    }

    #[test]
    fn test_rtl() {
        assert!(Locale::ArSA.is_rtl());
        assert!(!Locale::EnUS.is_rtl());
    }

    #[test]
    fn test_date_format() {
        assert_eq!(Locale::EnUS.date_format(), "MM/DD/YYYY");
        assert_eq!(Locale::FrFR.date_format(), "DD/MM/YYYY");
        assert_eq!(Locale::JaJP.date_format(), "YYYY/MM/DD");
    }

    #[test]
    fn test_number_format() {
        let (dec, thou) = Locale::EnUS.number_format();
        assert_eq!(dec, '.');
        assert_eq!(thou, ',');

        let (dec, thou) = Locale::FrFR.number_format();
        assert_eq!(dec, ',');
        assert_eq!(thou, '.');
    }

    #[test]
    fn test_currency_symbol() {
        assert_eq!(Locale::EnUS.currency_symbol(), "$");
        assert_eq!(Locale::EnGB.currency_symbol(), "£");
        assert_eq!(Locale::JaJP.currency_symbol(), "¥");
        assert_eq!(Locale::FrFR.currency_symbol(), "€");
    }

    #[test]
    fn test_first_day_of_week() {
        assert_eq!(Locale::EnUS.first_day_of_week(), 0);
        assert_eq!(Locale::FrFR.first_day_of_week(), 1);
    }

    #[test]
    fn test_translation_provider_english() {
        let provider = TranslationProvider::new(Locale::EnUS);
        assert_eq!(provider.translate(TranslationKey::File), "File");
        assert_eq!(provider.translate(TranslationKey::Save), "Save");
        assert_eq!(provider.translate(TranslationKey::Ok), "OK");
    }

    #[test]
    fn test_translation_provider_french() {
        let provider = TranslationProvider::new(Locale::FrFR);
        assert_eq!(provider.translate(TranslationKey::File), "Fichier");
        assert_eq!(provider.translate(TranslationKey::Save), "Enregistrer");
        assert_eq!(provider.translate(TranslationKey::Ok), "OK");
    }

    #[test]
    fn test_translation_provider_german() {
        let provider = TranslationProvider::new(Locale::DeDE);
        assert_eq!(provider.translate(TranslationKey::File), "Datei");
        assert_eq!(provider.translate(TranslationKey::Save), "Speichern");
    }

    #[test]
    fn test_translation_provider_spanish() {
        let provider = TranslationProvider::new(Locale::EsES);
        assert_eq!(provider.translate(TranslationKey::File), "Archivo");
        assert_eq!(provider.translate(TranslationKey::Save), "Guardar");
    }

    #[test]
    fn test_translation_provider_japanese() {
        let provider = TranslationProvider::new(Locale::JaJP);
        assert_eq!(provider.translate(TranslationKey::File), "ファイル");
        assert_eq!(provider.translate(TranslationKey::Save), "保存");
    }

    #[test]
    fn test_translation_provider_chinese() {
        let provider = TranslationProvider::new(Locale::ZhCN);
        assert_eq!(provider.translate(TranslationKey::File), "文件");
        assert_eq!(provider.translate(TranslationKey::Save), "保存");
    }

    #[test]
    fn test_translation_provider_korean() {
        let provider = TranslationProvider::new(Locale::KoKR);
        assert_eq!(provider.translate(TranslationKey::File), "파일");
        assert_eq!(provider.translate(TranslationKey::Save), "저장");
    }

    #[test]
    fn test_translation_provider_russian() {
        let provider = TranslationProvider::new(Locale::RuRU);
        assert_eq!(provider.translate(TranslationKey::File), "Файл");
        assert_eq!(provider.translate(TranslationKey::Save), "Сохранить");
    }

    #[test]
    fn test_translation_fallback() {
        let provider = TranslationProvider::new(Locale::NlNL);
        // Dutch falls back to English
        assert_eq!(provider.translate(TranslationKey::File), "File");
    }

    #[test]
    fn test_translate_with_args() {
        let provider = TranslationProvider::new(Locale::EnUS);
        // No template args in current keys, but test the mechanism
        let result = provider.translate_with_args(TranslationKey::ConfirmDelete, &[]);
        assert_eq!(result, "Are you sure you want to delete?");
    }

    #[test]
    fn test_all_translations() {
        let provider = TranslationProvider::new(Locale::EnUS);
        let all = provider.all_translations();
        assert!(all.len() >= 80);
    }

    #[test]
    fn test_format_number_us() {
        assert_eq!(format_number_locale(1234.56, Locale::EnUS), "1,234.56");
        assert_eq!(format_number_locale(1000000.0, Locale::EnUS), "1,000,000");
        assert_eq!(format_number_locale(42.0, Locale::EnUS), "42");
    }

    #[test]
    fn test_format_number_fr() {
        assert_eq!(format_number_locale(1234.56, Locale::FrFR), "1.234,56");
        assert_eq!(format_number_locale(1000000.0, Locale::FrFR), "1.000.000");
    }

    #[test]
    fn test_format_number_negative() {
        assert_eq!(format_number_locale(-1234.56, Locale::EnUS), "-1,234.56");
    }

    #[test]
    fn test_format_currency_us() {
        let result = format_currency(1234.56, Locale::EnUS);
        assert!(result.starts_with('$'));
        assert!(result.contains("1,234.56"));
    }

    #[test]
    fn test_format_currency_eu() {
        let result = format_currency(1234.56, Locale::FrFR);
        assert!(result.contains("€"));
        assert!(result.contains("1.234,56"));
    }

    #[test]
    fn test_format_currency_jp() {
        let result = format_currency(1000.0, Locale::JaJP);
        assert!(result.ends_with("¥"));
    }

    #[test]
    fn test_format_percentage_us() {
        assert_eq!(format_percentage(0.5, Locale::EnUS), "50%");
        assert_eq!(format_percentage(0.125, Locale::EnUS), "12.5%");
    }

    #[test]
    fn test_format_percentage_fr() {
        let result = format_percentage(0.5, Locale::FrFR);
        assert!(result.contains("50"));
        assert!(result.contains("%"));
    }

    #[test]
    fn test_format_date() {
        // Serial 1 = 1900-01-01
        let us = format_date(1.0, Locale::EnUS);
        assert_eq!(us, "01/01/1900");

        let fr = format_date(1.0, Locale::FrFR);
        assert_eq!(fr, "01/01/1900");

        let jp = format_date(1.0, Locale::JaJP);
        assert_eq!(jp, "1900/01/01");
    }

    #[test]
    fn test_format_date_known() {
        // Serial 45292 = 2024-01-01
        let us = format_date(45292.0, Locale::EnUS);
        assert_eq!(us, "01/01/2024");
    }

    #[test]
    fn test_format_time() {
        let us = format_time(0.5, Locale::EnUS);
        assert!(us.contains("12"));
        assert!(us.contains("AM") || us.contains("PM"));

        let eu = format_time(0.5, Locale::FrFR);
        assert!(eu.contains("12"));
    }

    #[test]
    fn test_weekday_names() {
        assert_eq!(weekday_name(0, Locale::EnUS), "Sunday");
        assert_eq!(weekday_name(1, Locale::EnUS), "Monday");
        assert_eq!(weekday_name(0, Locale::FrFR), "Dimanche");
        assert_eq!(weekday_name(1, Locale::JaJP), "月曜日");
    }

    #[test]
    fn test_weekday_abbr() {
        assert_eq!(weekday_abbr(0, Locale::EnUS), "Sun");
        assert_eq!(weekday_abbr(1, Locale::EnUS), "Mon");
        assert_eq!(weekday_abbr(0, Locale::FrFR), "Dim");
    }

    #[test]
    fn test_month_names() {
        assert_eq!(month_name(1, Locale::EnUS), "January");
        assert_eq!(month_name(12, Locale::EnUS), "December");
        assert_eq!(month_name(1, Locale::FrFR), "Janvier");
        assert_eq!(month_name(1, Locale::JaJP), "1月");
    }

    #[test]
    fn test_month_abbr() {
        assert_eq!(month_abbr(1, Locale::EnUS), "Jan");
        assert_eq!(month_abbr(12, Locale::EnUS), "Dec");
        assert_eq!(month_abbr(1, Locale::FrFR), "Jan");
    }

    #[test]
    fn test_col_to_letter() {
        assert_eq!(col_to_letter(0), "A");
        assert_eq!(col_to_letter(25), "Z");
        assert_eq!(col_to_letter(26), "AA");
        assert_eq!(col_to_letter(27), "AB");
    }

    // --- Accessibility Tests ---

    #[test]
    fn test_cell_label() {
        let label = cell_label(0, 0, "Hello", Locale::EnUS);
        assert_eq!(label.role, "gridcell");
        assert!(label.label.contains("A1"));
        assert!(label.label.contains("Hello"));
    }

    #[test]
    fn test_cell_label_empty() {
        let label = cell_label(5, 3, "", Locale::EnUS);
        assert_eq!(label.role, "gridcell");
        assert!(label.label.contains("D6"));
    }

    #[test]
    fn test_row_header_label() {
        let label = row_header_label(0, Locale::EnUS);
        assert_eq!(label.role, "rowheader");
        assert!(label.label.contains("1"));
    }

    #[test]
    fn test_col_header_label() {
        let label = col_header_label(0, Locale::EnUS);
        assert_eq!(label.role, "columnheader");
        assert!(label.label.contains("A"));
    }

    #[test]
    fn test_selected_cell_label() {
        let label = selected_cell_label(0, 0, "42", Locale::EnUS);
        assert!(label.contains("Selected"));
        assert!(label.contains("A1"));
        assert!(label.contains("42"));
    }

    #[test]
    fn test_editing_cell_label() {
        let label = editing_cell_label(2, 3, Locale::EnUS);
        assert!(label.contains("Editing"));
        assert!(label.contains("D3"));
    }

    #[test]
    fn test_accessibility_config_default() {
        let config = AccessibilityConfig::default();
        assert!(config.screen_reader_enabled);
        assert!(!config.high_contrast);
        assert!(config.keyboard_indicators);
        assert_eq!(config.live_region_politeness, LiveRegionPoliteness::Polite);
    }

    #[test]
    fn test_live_region_politeness() {
        assert_eq!(LiveRegionPoliteness::Off.as_str(), "off");
        assert_eq!(LiveRegionPoliteness::Polite.as_str(), "polite");
        assert_eq!(LiveRegionPoliteness::Assertive.as_str(), "assertive");
    }

    #[test]
    fn test_navigation_direction_name() {
        assert_eq!(
            navigation_direction_name(NavigationDirection::Up, Locale::EnUS),
            "Up"
        );
        assert_eq!(
            navigation_direction_name(NavigationDirection::Down, Locale::EnUS),
            "Down"
        );
        assert_eq!(
            navigation_direction_name(NavigationDirection::Left, Locale::EnUS),
            "Left"
        );
        assert_eq!(
            navigation_direction_name(NavigationDirection::Right, Locale::EnUS),
            "Right"
        );
    }
}
