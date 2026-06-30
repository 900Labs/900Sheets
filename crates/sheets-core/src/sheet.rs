use crate::cell::CellValue;
use crate::format::CellFormat;
use std::collections::HashMap;

const DEFAULT_MAX_ROWS: u32 = 1_000_000;
const DEFAULT_MAX_COLS: u32 = 16_384;

pub struct Sheet {
    name: String,
    cells: HashMap<(u32, u32), CellValue>,
    formats: HashMap<(u32, u32), CellFormat>,
    max_rows: u32,
    max_cols: u32,
}

impl Sheet {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            cells: HashMap::new(),
            formats: HashMap::new(),
            max_rows: DEFAULT_MAX_ROWS,
            max_cols: DEFAULT_MAX_COLS,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn rename(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    pub fn max_rows(&self) -> u32 {
        self.max_rows
    }

    pub fn max_cols(&self) -> u32 {
        self.max_cols
    }

    pub fn cell_value(&self, row: u32, col: u32) -> Option<String> {
        self.cells
            .get(&(row, col))
            .filter(|c| !c.is_empty())
            .map(|c| c.raw.clone())
    }

    pub fn cell(&self, row: u32, col: u32) -> Option<&CellValue> {
        self.cells.get(&(row, col))
    }

    pub fn set_cell_value(&mut self, row: u32, col: u32, value: String) {
        if !self.in_bounds(row, col) {
            return;
        }
        if value.is_empty() {
            self.cells.remove(&(row, col));
            return;
        }

        let cell = if value.starts_with('=') {
            CellValue::formula(value)
        } else if let Ok(n) = value.parse::<f64>() {
            if !n.is_finite() {
                CellValue::text(value)
            } else {
                CellValue::number(n)
            }
        } else if value.eq_ignore_ascii_case("true") {
            CellValue::boolean(true)
        } else if value.eq_ignore_ascii_case("false") {
            CellValue::boolean(false)
        } else {
            CellValue::text(value)
        };

        self.cells.insert((row, col), cell);
    }

    pub fn in_bounds(&self, row: u32, col: u32) -> bool {
        row < self.max_rows && col < self.max_cols
    }

    pub fn set_cell(&mut self, row: u32, col: u32, value: CellValue) {
        if !self.in_bounds(row, col) {
            return;
        }
        if value.is_empty() {
            self.cells.remove(&(row, col));
        } else {
            self.cells.insert((row, col), value);
        }
    }

    pub fn clear_cell(&mut self, row: u32, col: u32) {
        if !self.in_bounds(row, col) {
            return;
        }
        self.cells.remove(&(row, col));
        self.formats.remove(&(row, col));
    }

    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    pub fn iter_cells(&self) -> impl Iterator<Item = ((u32, u32), &CellValue)> {
        self.cells.iter().map(|(&k, v)| (k, v))
    }

    pub fn set_format(&mut self, row: u32, col: u32, format: CellFormat) {
        if !self.in_bounds(row, col) {
            return;
        }
        if format.is_empty() {
            self.formats.remove(&(row, col));
        } else {
            self.formats.insert((row, col), format);
        }
    }

    pub fn get_format(&self, row: u32, col: u32) -> Option<&CellFormat> {
        self.formats.get(&(row, col))
    }

    pub fn clear_format(&mut self, row: u32, col: u32) {
        if !self.in_bounds(row, col) {
            return;
        }
        self.formats.remove(&(row, col));
    }

    pub fn iter_formats(&self) -> impl Iterator<Item = ((u32, u32), &CellFormat)> {
        self.formats.iter().map(|(&k, v)| (k, v))
    }

    pub fn format_count(&self) -> usize {
        self.formats.len()
    }
}

impl Default for Sheet {
    fn default() -> Self {
        Self::new("Sheet1")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_sheet() {
        let sheet = Sheet::new("MySheet");
        assert_eq!(sheet.name(), "MySheet");
        assert_eq!(sheet.cell_count(), 0);
    }

    #[test]
    fn test_set_and_get_text() {
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_cell_value(0, 0, "Hello".into());
        assert_eq!(sheet.cell_value(0, 0), Some("Hello".into()));
    }

    #[test]
    fn test_set_and_get_number() {
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_cell_value(0, 0, "42".into());
        let cell = sheet.cell(0, 0).unwrap();
        assert_eq!(cell.cell_type, crate::cell::CellType::Number);
    }

    #[test]
    fn test_non_finite_string_is_text() {
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_cell_value(0, 0, "NaN".into());
        let cell = sheet.cell(0, 0).unwrap();
        assert_eq!(cell.cell_type, crate::cell::CellType::Text);
    }

    #[test]
    fn test_out_of_bounds_cell_is_not_stored() {
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_cell_value(sheet.max_rows(), 0, "outside".into());
        sheet.set_cell_value(0, sheet.max_cols(), "outside".into());
        assert_eq!(sheet.cell_count(), 0);
    }

    #[test]
    fn test_set_formula() {
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_cell_value(0, 0, "=SUM(A1:A10)".into());
        let cell = sheet.cell(0, 0).unwrap();
        assert!(cell.is_formula());
    }

    #[test]
    fn test_clear_cell() {
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_cell_value(0, 0, "Hello".into());
        sheet.clear_cell(0, 0);
        assert_eq!(sheet.cell_value(0, 0), None);
    }

    #[test]
    fn test_empty_string_removes_cell() {
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_cell_value(0, 0, "Hello".into());
        sheet.set_cell_value(0, 0, "".into());
        assert_eq!(sheet.cell_value(0, 0), None);
    }

    #[test]
    fn test_set_and_get_format() {
        let mut sheet = Sheet::new("Sheet1");
        let fmt = CellFormat::new().bold(true).font_size(14.0);
        sheet.set_format(0, 0, fmt);
        let retrieved = sheet.get_format(0, 0).unwrap();
        assert_eq!(retrieved.bold, Some(true));
        assert_eq!(retrieved.font_size, Some(14.0));
    }

    #[test]
    fn test_clear_format() {
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_format(0, 0, CellFormat::new().bold(true));
        assert!(sheet.get_format(0, 0).is_some());
        sheet.clear_format(0, 0);
        assert!(sheet.get_format(0, 0).is_none());
    }

    #[test]
    fn test_clear_cell_also_clears_format() {
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_cell_value(0, 0, "Hello".into());
        sheet.set_format(0, 0, CellFormat::new().bold(true));
        sheet.clear_cell(0, 0);
        assert!(sheet.get_format(0, 0).is_none());
    }

    #[test]
    fn test_empty_format_not_stored() {
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_format(0, 0, CellFormat::default());
        assert_eq!(sheet.format_count(), 0);
    }

    #[test]
    fn test_iter_formats() {
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_format(0, 0, CellFormat::new().bold(true));
        sheet.set_format(1, 1, CellFormat::new().italic(true));
        assert_eq!(sheet.format_count(), 2);
        let count = sheet.iter_formats().count();
        assert_eq!(count, 2);
    }
}
