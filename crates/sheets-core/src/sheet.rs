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
        self.apply_structure_edit_scoped(edit, true, None);
    }

    pub(crate) fn apply_structure_edit_scoped(
        &mut self,
        edit: StructureEdit,
        rewrite_unqualified: bool,
        target_sheet: Option<&str>,
    ) {
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
                    cell.raw = rewrite_formula_references_scoped(
                        &cell.raw,
                        edit,
                        rewrite_unqualified,
                        target_sheet,
                    );
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

    pub(crate) fn rewrite_structure_references(&mut self, edit: StructureEdit, target_sheet: &str) {
        for cell in self.cells.values_mut().filter(|cell| cell.is_formula()) {
            cell.raw =
                rewrite_formula_references_scoped(&cell.raw, edit, false, Some(target_sheet));
        }
    }

    pub(crate) fn rewrite_sheet_references(&mut self, old_name: &str, new_name: Option<&str>) {
        for cell in self.cells.values_mut().filter(|cell| cell.is_formula()) {
            cell.raw = rewrite_sheet_qualifiers(&cell.raw, old_name, new_name);
        }
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

#[cfg(test)]
fn rewrite_formula_references(formula: &str, edit: StructureEdit) -> String {
    rewrite_formula_references_scoped(formula, edit, true, None)
}

fn rewrite_formula_references_scoped(
    formula: &str,
    edit: StructureEdit,
    rewrite_unqualified: bool,
    target_sheet: Option<&str>,
) -> String {
    let mut output = String::with_capacity(formula.len());
    let mut index = 0;
    while index < formula.len() {
        if let Some(end) = protected_literal_end(formula, index) {
            output.push_str(&formula[index..end]);
            index = end;
            continue;
        }
        if let Some(reference) = parse_formula_reference(formula, index) {
            let sheet = reference.first.sheet.as_deref();
            let should_rewrite = match sheet {
                Some(name) => target_sheet.is_some_and(|target| sheet_names_equal(name, target)),
                None => rewrite_unqualified,
            };
            if !should_rewrite {
                output.push_str(&formula[index..reference.end]);
            } else if let Some(second) = &reference.second {
                if let Some((first_position, second_position)) =
                    rewrite_range_positions(&reference.first, second, edit)
                {
                    write_rewritten_cell(&mut output, formula, &reference.first, first_position);
                    output.push(':');
                    write_rewritten_cell(&mut output, formula, second, second_position);
                } else {
                    output.push_str("#REF!");
                }
            } else if let Some(position) = rewrite_cell_position(&reference.first, edit) {
                write_rewritten_cell(&mut output, formula, &reference.first, position);
            } else {
                output.push_str("#REF!");
            }
            index = reference.end;
            continue;
        }
        push_next_char(formula, &mut output, &mut index);
    }
    output
}

fn rewrite_sheet_qualifiers(formula: &str, old_name: &str, new_name: Option<&str>) -> String {
    let mut output = String::with_capacity(formula.len());
    let mut index = 0;
    while index < formula.len() {
        if let Some(end) = protected_literal_end(formula, index) {
            output.push_str(&formula[index..end]);
            index = end;
            continue;
        }
        if let Some(reference) = parse_formula_reference(formula, index) {
            if reference
                .first
                .sheet
                .as_deref()
                .is_some_and(|sheet| sheet_names_equal(sheet, old_name))
            {
                if let Some(new_name) = new_name {
                    write_renamed_reference(&mut output, formula, &reference, old_name, new_name);
                } else {
                    output.push_str("#REF!");
                }
            } else {
                output.push_str(&formula[index..reference.end]);
            }
            index = reference.end;
            continue;
        }
        push_next_char(formula, &mut output, &mut index);
    }
    output
}

#[derive(Clone)]
struct ParsedCellReference {
    start: usize,
    coordinate_start: usize,
    end: usize,
    sheet: Option<String>,
    col: u32,
    row: u32,
    abs_col: bool,
    abs_row: bool,
}

struct ParsedFormulaReference {
    first: ParsedCellReference,
    second: Option<ParsedCellReference>,
    end: usize,
}

fn parse_formula_reference(formula: &str, start: usize) -> Option<ParsedFormulaReference> {
    let first = parse_single_reference(formula, start)?;
    let mut end = first.end;
    let second = if formula.as_bytes().get(end) == Some(&b':') {
        let mut second = parse_single_reference(formula, end + 1)?;
        if second.sheet.is_none() {
            second.sheet.clone_from(&first.sheet);
        }
        if first.sheet.is_some() && second.sheet != first.sheet {
            return None;
        }
        end = second.end;
        Some(second)
    } else {
        None
    };
    Some(ParsedFormulaReference { first, second, end })
}

fn parse_single_reference(formula: &str, start: usize) -> Option<ParsedCellReference> {
    if start >= formula.len()
        || (start > 0
            && (formula.as_bytes()[start - 1].is_ascii_alphanumeric()
                || formula.as_bytes()[start - 1] == b'_'))
    {
        return None;
    }
    let mut coordinate_start = start;
    let mut sheet = None;
    if formula.as_bytes()[start] == b'\'' {
        let (name, after_quote) = parse_quoted_sheet_name(formula, start)?;
        if formula.as_bytes().get(after_quote) != Some(&b'!') {
            return None;
        }
        sheet = Some(name);
        coordinate_start = after_quote + 1;
    } else if formula.as_bytes()[start].is_ascii_alphabetic() || formula.as_bytes()[start] == b'_' {
        let mut candidate_end = start;
        while candidate_end < formula.len()
            && (formula.as_bytes()[candidate_end].is_ascii_alphanumeric()
                || matches!(formula.as_bytes()[candidate_end], b'_' | b'.'))
        {
            candidate_end += 1;
        }
        if formula.as_bytes().get(candidate_end) == Some(&b'!') {
            sheet = Some(formula[start..candidate_end].to_string());
            coordinate_start = candidate_end + 1;
        }
    }

    let bytes = formula.as_bytes();
    let mut index = coordinate_start;
    let abs_col = bytes.get(index) == Some(&b'$');
    if abs_col {
        index += 1;
    }
    let column_start = index;
    while index < bytes.len() && bytes[index].is_ascii_alphabetic() {
        index += 1;
    }
    if index == column_start || index - column_start > 3 {
        return None;
    }
    let column_end = index;
    let abs_row = bytes.get(index) == Some(&b'$');
    if abs_row {
        index += 1;
    }
    let row_start = index;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if row_start == index
        || (index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_'))
        || bytes.get(index) == Some(&b'(')
    {
        return None;
    }
    let col = column_label_to_index(&formula[column_start..column_end])?;
    let row_number = formula[row_start..index].parse::<u32>().ok()?;
    if row_number == 0 {
        return None;
    }
    Some(ParsedCellReference {
        start,
        coordinate_start,
        end: index,
        sheet,
        col,
        row: row_number - 1,
        abs_col,
        abs_row,
    })
}

fn parse_quoted_sheet_name(formula: &str, start: usize) -> Option<(String, usize)> {
    let mut name = String::new();
    let mut index = start + 1;
    while index < formula.len() {
        let ch = formula[index..].chars().next()?;
        if ch == '\'' {
            let next = index + ch.len_utf8();
            if formula[next..].starts_with('\'') {
                name.push('\'');
                index = next + 1;
            } else {
                return Some((name, next));
            }
        } else {
            name.push(ch);
            index += ch.len_utf8();
        }
    }
    None
}

fn protected_literal_end(formula: &str, start: usize) -> Option<usize> {
    let bytes = formula.as_bytes();
    if bytes.get(start) == Some(&b'"') {
        let mut index = start + 1;
        while index < bytes.len() {
            if bytes[index] == b'"' {
                index += 1;
                if bytes.get(index) == Some(&b'"') {
                    index += 1;
                } else {
                    break;
                }
            } else {
                index += formula[index..].chars().next()?.len_utf8();
            }
        }
        return Some(index);
    }
    if bytes.get(start) == Some(&b'#') {
        let mut index = start + 1;
        while index < bytes.len() && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'/')
        {
            index += 1;
        }
        if index < bytes.len() && matches!(bytes[index], b'!' | b'?') {
            index += 1;
        }
        return Some(index);
    }
    None
}

fn push_next_char(formula: &str, output: &mut String, index: &mut usize) {
    let ch = formula[*index..]
        .chars()
        .next()
        .expect("valid char boundary");
    output.push(ch);
    *index += ch.len_utf8();
}

fn rewrite_cell_position(
    reference: &ParsedCellReference,
    edit: StructureEdit,
) -> Option<(u32, u32)> {
    let (mut row, mut col) = (reference.row, reference.col);
    match edit {
        StructureEdit::InsertRow(at) if row >= at => row = row.saturating_add(1),
        StructureEdit::DeleteRow(at) if row == at => return None,
        StructureEdit::DeleteRow(at) if row > at => row -= 1,
        StructureEdit::InsertColumn(at) if col >= at => col = col.saturating_add(1),
        StructureEdit::DeleteColumn(at) if col == at => return None,
        StructureEdit::DeleteColumn(at) if col > at => col -= 1,
        _ => {}
    }
    Some((row, col))
}

fn rewrite_range_positions(
    first: &ParsedCellReference,
    second: &ParsedCellReference,
    edit: StructureEdit,
) -> Option<((u32, u32), (u32, u32))> {
    let (mut first_row, mut second_row) = (first.row, second.row);
    let (mut first_col, mut second_col) = (first.col, second.col);
    match edit {
        StructureEdit::InsertRow(at) => insert_into_axis(&mut first_row, &mut second_row, at),
        StructureEdit::DeleteRow(at) => delete_from_axis(&mut first_row, &mut second_row, at)?,
        StructureEdit::InsertColumn(at) => insert_into_axis(&mut first_col, &mut second_col, at),
        StructureEdit::DeleteColumn(at) => delete_from_axis(&mut first_col, &mut second_col, at)?,
    }
    Some(((first_row, first_col), (second_row, second_col)))
}

fn insert_into_axis(first: &mut u32, second: &mut u32, at: u32) {
    let (low, high) = ((*first).min(*second), (*first).max(*second));
    if at <= low {
        *first = first.saturating_add(1);
        *second = second.saturating_add(1);
    } else if at <= high {
        if *first <= *second {
            *second = second.saturating_add(1);
        } else {
            *first = first.saturating_add(1);
        }
    }
}

fn delete_from_axis(first: &mut u32, second: &mut u32, at: u32) -> Option<()> {
    let (low, high) = ((*first).min(*second), (*first).max(*second));
    if at < low {
        *first -= 1;
        *second -= 1;
    } else if at <= high {
        if low == high {
            return None;
        }
        if *first <= *second {
            *second -= 1;
        } else {
            *first -= 1;
        }
    }
    Some(())
}

fn write_rewritten_cell(
    output: &mut String,
    formula: &str,
    reference: &ParsedCellReference,
    (row, col): (u32, u32),
) {
    output.push_str(&formula[reference.start..reference.coordinate_start]);
    if reference.abs_col {
        output.push('$');
    }
    output.push_str(&index_to_column_label(col));
    if reference.abs_row {
        output.push('$');
    }
    output.push_str(&(row + 1).to_string());
}

fn write_renamed_reference(
    output: &mut String,
    formula: &str,
    reference: &ParsedFormulaReference,
    old_name: &str,
    new_name: &str,
) {
    output.push_str(&format_sheet_name(new_name));
    output.push('!');
    output.push_str(&formula[reference.first.coordinate_start..reference.first.end]);
    if let Some(second) = &reference.second {
        output.push(':');
        if second
            .sheet
            .as_deref()
            .is_some_and(|sheet| sheet_names_equal(sheet, old_name))
            && second.coordinate_start > second.start
        {
            output.push_str(&format_sheet_name(new_name));
            output.push('!');
        } else {
            output.push_str(&formula[second.start..second.coordinate_start]);
        }
        output.push_str(&formula[second.coordinate_start..second.end]);
    }
}

fn format_sheet_name(name: &str) -> String {
    if !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.'))
        && !name.starts_with(|ch: char| ch.is_ascii_digit())
    {
        name.to_string()
    } else {
        format!("'{}'", name.replace('\'', "''"))
    }
}

fn sheet_names_equal(left: &str, right: &str) -> bool {
    left.trim().to_lowercase() == right.trim().to_lowercase()
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

    #[test]
    fn structure_range_deletion_shrinks_endpoints_or_refs_entire_range() {
        assert_eq!(
            rewrite_formula_references("=SUM(A1:A3)", StructureEdit::DeleteRow(0)),
            "=SUM(A1:A2)"
        );
        assert_eq!(
            rewrite_formula_references("=SUM(A1:A3)", StructureEdit::DeleteRow(1)),
            "=SUM(A1:A2)"
        );
        assert_eq!(
            rewrite_formula_references("=SUM(A1:B1)", StructureEdit::DeleteRow(0)),
            "=SUM(#REF!)"
        );
        assert_eq!(
            rewrite_formula_references("=SUM(A1:C2)", StructureEdit::DeleteColumn(1)),
            "=SUM(A1:B2)"
        );
    }

    #[test]
    fn structural_rewrite_preserves_unicode_text() {
        assert_eq!(
            rewrite_formula_references("=\"Årsöversikt\"&A1", StructureEdit::InsertRow(0)),
            "=\"Årsöversikt\"&A2"
        );
    }
}
