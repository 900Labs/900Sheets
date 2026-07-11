use crate::cell::CellValue;
use crate::format::CellFormat;
use std::collections::HashMap;

const DEFAULT_MAX_ROWS: u32 = 1_000_000;
const DEFAULT_MAX_COLS: u32 = 16_384;

#[derive(Debug, Clone, Copy)]
pub enum StructureEdit {
    InsertRow(u32),
    DeleteRow(u32),
    InsertColumn(u32),
    DeleteColumn(u32),
}

#[derive(Clone)]
pub struct Sheet {
    stable_id: u64,
    name: String,
    cells: HashMap<(u32, u32), CellValue>,
    formats: HashMap<(u32, u32), CellFormat>,
    max_rows: u32,
    max_cols: u32,
}

impl Sheet {
    pub fn new(name: impl Into<String>) -> Self {
        Self::with_stable_id(0, name)
    }

    pub fn with_stable_id(stable_id: u64, name: impl Into<String>) -> Self {
        Self {
            stable_id,
            name: name.into(),
            cells: HashMap::new(),
            formats: HashMap::new(),
            max_rows: DEFAULT_MAX_ROWS,
            max_cols: DEFAULT_MAX_COLS,
        }
    }

    pub fn stable_id(&self) -> u64 {
        self.stable_id
    }

    pub fn set_stable_id(&mut self, stable_id: u64) {
        self.stable_id = stable_id;
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

    pub fn clear_value(&mut self, row: u32, col: u32) {
        if self.in_bounds(row, col) {
            self.cells.remove(&(row, col));
        }
    }

    pub fn replace_contents(
        &mut self,
        cells: HashMap<(u32, u32), CellValue>,
        formats: HashMap<(u32, u32), CellFormat>,
    ) {
        self.cells = cells
            .into_iter()
            .filter(|((row, col), value)| self.in_bounds(*row, *col) && !value.is_empty())
            .collect();
        self.formats = formats
            .into_iter()
            .filter(|((row, col), format)| self.in_bounds(*row, *col) && !format.is_empty())
            .collect();
    }

    pub fn apply_structure_edit(&mut self, edit: StructureEdit) {
        let max_rows = self.max_rows;
        let max_cols = self.max_cols;
        let move_position = |row: u32, col: u32| -> Option<(u32, u32)> {
            match edit {
                StructureEdit::InsertRow(at) if row >= at => {
                    (row + 1 < max_rows).then_some((row + 1, col))
                }
                StructureEdit::DeleteRow(at) if row == at => None,
                StructureEdit::DeleteRow(at) if row > at => Some((row - 1, col)),
                StructureEdit::InsertColumn(at) if col >= at => {
                    (col + 1 < max_cols).then_some((row, col + 1))
                }
                StructureEdit::DeleteColumn(at) if col == at => None,
                StructureEdit::DeleteColumn(at) if col > at => Some((row, col - 1)),
                _ => Some((row, col)),
            }
        };

        let mut cells: HashMap<(u32, u32), CellValue> = self
            .cells
            .drain()
            .filter_map(|((row, col), mut cell)| {
                if cell.is_formula() {
                    cell.raw = rewrite_formula_references(&cell.raw, edit);
                }
                move_position(row, col).map(|position| (position, cell))
            })
            .collect();
        let formats = self
            .formats
            .drain()
            .filter_map(|((row, col), format)| {
                move_position(row, col).map(|position| (position, format))
            })
            .collect();
        self.cells.clear();
        self.cells.extend(cells.drain());
        self.formats = formats;
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

fn rewrite_formula_references(formula: &str, edit: StructureEdit) -> String {
    let bytes = formula.as_bytes();
    let mut output = String::with_capacity(formula.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            let start = index;
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'"' {
                    index += 1;
                    if index < bytes.len() && bytes[index] == b'"' {
                        index += 1;
                        continue;
                    }
                    break;
                }
                index += 1;
            }
            output.push_str(&formula[start..index]);
            continue;
        }
        if bytes[index] == b'#' {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'/')
            {
                index += 1;
            }
            if index < bytes.len() && matches!(bytes[index], b'!' | b'?') {
                index += 1;
            }
            output.push_str(&formula[start..index]);
            continue;
        }
        let start = index;
        if bytes[index] == b'$' {
            index += 1;
        }
        let column_start = index;
        while index < bytes.len() && bytes[index].is_ascii_alphabetic() {
            index += 1;
        }
        let column_end = index;
        if column_end == column_start || column_end - column_start > 3 {
            output.push(bytes[start] as char);
            index = start + 1;
            continue;
        }
        if index < bytes.len() && bytes[index] == b'$' {
            index += 1;
        }
        let row_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        let boundary_before =
            start == 0 || (!bytes[start - 1].is_ascii_alphanumeric() && bytes[start - 1] != b'_');
        let boundary_after =
            index == bytes.len() || (!bytes[index].is_ascii_alphanumeric() && bytes[index] != b'_');
        if row_start == index || !boundary_before || !boundary_after {
            output.push(bytes[start] as char);
            index = start + 1;
            continue;
        }
        if index < bytes.len() && bytes[index] == b'(' {
            output.push_str(&formula[start..index]);
            continue;
        }

        let column_label = &formula[column_start..column_end];
        let Some(mut column) = column_label_to_index(column_label) else {
            output.push_str(&formula[start..index]);
            continue;
        };
        let Ok(row_number) = formula[row_start..index].parse::<u32>() else {
            output.push_str(&formula[start..index]);
            continue;
        };
        if row_number == 0 {
            output.push_str(&formula[start..index]);
            continue;
        }
        let mut row = row_number - 1;
        let deleted = match edit {
            StructureEdit::InsertRow(at) if row >= at => {
                row = row.saturating_add(1);
                false
            }
            StructureEdit::DeleteRow(at) if row == at => true,
            StructureEdit::DeleteRow(at) if row > at => {
                row -= 1;
                false
            }
            StructureEdit::InsertColumn(at) if column >= at => {
                column = column.saturating_add(1);
                false
            }
            StructureEdit::DeleteColumn(at) if column == at => true,
            StructureEdit::DeleteColumn(at) if column > at => {
                column -= 1;
                false
            }
            _ => false,
        };
        if deleted {
            output.push_str("#REF!");
        } else {
            if formula.as_bytes()[start] == b'$' {
                output.push('$');
            }
            output.push_str(&index_to_column_label(column));
            if row_start > column_end {
                output.push('$');
            }
            output.push_str(&(row + 1).to_string());
        }
    }
    output
}

fn column_label_to_index(label: &str) -> Option<u32> {
    let mut column = 0u32;
    for byte in label.bytes() {
        let value = u32::from(byte.to_ascii_uppercase().checked_sub(b'A')?) + 1;
        column = column.checked_mul(26)?.checked_add(value)?;
    }
    column.checked_sub(1)
}

fn index_to_column_label(mut column: u32) -> String {
    let mut label = String::new();
    loop {
        label.insert(0, (b'A' + (column % 26) as u8) as char);
        if column < 26 {
            break;
        }
        column = column / 26 - 1;
    }
    label
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

    #[test]
    fn test_structure_edits_move_cells_formats_and_rewrite_formula_references() {
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_cell_value(0, 0, "10".into());
        sheet.set_cell_value(0, 1, "=A1+$A$1".into());
        sheet.set_format(0, 0, CellFormat::new().bold(true));

        sheet.apply_structure_edit(StructureEdit::InsertRow(0));
        assert_eq!(sheet.cell_value(1, 0), Some("10".into()));
        assert_eq!(sheet.cell_value(1, 1), Some("=A2+$A$2".into()));
        assert_eq!(sheet.get_format(1, 0).unwrap().bold, Some(true));

        sheet.apply_structure_edit(StructureEdit::DeleteColumn(0));
        assert_eq!(sheet.cell_value(1, 0), Some("=#REF!+#REF!".into()));
    }

    #[test]
    fn test_structure_edit_updates_ranges_and_preserves_absolute_markers() {
        assert_eq!(
            rewrite_formula_references("=SUM(A1:$B$2)", StructureEdit::InsertColumn(1)),
            "=SUM(A1:$C$2)"
        );
        assert_eq!(
            rewrite_formula_references("=A1+A2", StructureEdit::DeleteRow(0)),
            "=#REF!+A1"
        );
        assert_eq!(
            rewrite_formula_references("=LOG10(A1)", StructureEdit::InsertRow(0)),
            "=LOG10(A2)"
        );
        assert_eq!(
            rewrite_formula_references("=\"A1\"&A1", StructureEdit::InsertRow(0)),
            "=\"A1\"&A2"
        );
        assert_eq!(
            rewrite_formula_references("=#REF!&A1", StructureEdit::InsertRow(0)),
            "=#REF!&A2"
        );
    }
}
