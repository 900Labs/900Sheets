use crate::error::JsonError;
use sheets_core::cell::CellValue;
use sheets_core::sheet::Sheet;
use sheets_core::workbook::Workbook;

const MAX_JSON_SIZE: usize = 50 * 1024 * 1024;
const MAX_CELLS: usize = 10_000_000;
const MAX_DEPTH: usize = 64;
const MAX_ROWS: usize = 1_000_000;
const MAX_COLS: usize = 16_384;

#[derive(Clone, Copy)]
struct ImportLimits {
    max_bytes: usize,
    max_cells: usize,
    max_depth: usize,
    max_rows: usize,
    max_cols: usize,
}

impl Default for ImportLimits {
    fn default() -> Self {
        Self {
            max_bytes: MAX_JSON_SIZE,
            max_cells: MAX_CELLS,
            max_depth: MAX_DEPTH,
            max_rows: MAX_ROWS,
            max_cols: MAX_COLS,
        }
    }
}

#[derive(Default)]
struct ImportBudget {
    cells: usize,
}

pub fn import_workbook_json(data: &str) -> Result<Workbook, JsonError> {
    import_workbook_json_with_limits(data, ImportLimits::default())
}

fn import_workbook_json_with_limits(
    data: &str,
    limits: ImportLimits,
) -> Result<Workbook, JsonError> {
    if data.len() > limits.max_bytes {
        return Err(JsonError::FileTooLarge(data.len(), limits.max_bytes));
    }
    let json: serde_json::Value = serde_json::from_str(data)?;
    validate_json_depth(&json, 0, limits.max_depth)?;
    let mut budget = ImportBudget::default();
    import_from_json(&json, limits, &mut budget)
}

fn validate_json_depth(
    value: &serde_json::Value,
    depth: usize,
    max_depth: usize,
) -> Result<(), JsonError> {
    if depth > max_depth {
        return Err(JsonError::TooDeep(depth, max_depth));
    }
    match value {
        serde_json::Value::Array(values) => {
            for child in values {
                validate_json_depth(child, depth + 1, max_depth)?;
            }
        }
        serde_json::Value::Object(values) => {
            for child in values.values() {
                validate_json_depth(child, depth + 1, max_depth)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn track_cells(
    budget: &mut ImportBudget,
    count: usize,
    limits: ImportLimits,
) -> Result<(), JsonError> {
    budget.cells = budget.cells.saturating_add(count);
    if budget.cells > limits.max_cells {
        return Err(JsonError::TooManyCells(budget.cells, limits.max_cells));
    }
    Ok(())
}

fn checked_cell_ref(row: usize, col: usize, limits: ImportLimits) -> Result<(u32, u32), JsonError> {
    if row >= limits.max_rows {
        return Err(JsonError::InvalidStructure(format!(
            "row {} exceeds maximum row {}",
            row + 1,
            limits.max_rows
        )));
    }
    if col >= limits.max_cols {
        return Err(JsonError::InvalidStructure(format!(
            "column {} exceeds maximum column {}",
            col + 1,
            limits.max_cols
        )));
    }
    Ok((row as u32, col as u32))
}

fn import_from_json(
    json: &serde_json::Value,
    limits: ImportLimits,
    budget: &mut ImportBudget,
) -> Result<Workbook, JsonError> {
    let mut workbook = Workbook::new();

    match json {
        serde_json::Value::Array(rows) => {
            import_rows_into_sheet(rows, &mut workbook, 0, limits, budget)?;
        }
        serde_json::Value::Object(obj) => {
            if let Some(sheets) = obj.get("sheets").and_then(|v| v.as_array()) {
                if sheets.is_empty() {
                    return Ok(workbook);
                }
                workbook.rename_sheet(
                    0,
                    sheets[0]
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Sheet1"),
                );
                for (_i, sheet_json) in sheets.iter().enumerate().skip(1) {
                    let name = sheet_json
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Sheet");
                    workbook.add_sheet(name);
                }
                for (i, sheet_json) in sheets.iter().enumerate() {
                    if let Some(rows) = sheet_json.get("data").and_then(|v| v.as_array()) {
                        if let Some(sheet) = workbook.sheet_mut(i) {
                            import_rows(rows, sheet, limits, budget)?;
                        }
                    }
                }
            } else if let Some(rows) = obj.get("data").and_then(|v| v.as_array()) {
                import_rows_into_sheet(rows, &mut workbook, 0, limits, budget)?;
            } else {
                import_object_rows(obj, &mut workbook, 0, limits, budget)?;
            }
        }
        _ => {
            return Err(JsonError::InvalidStructure(
                "Expected array or object".into(),
            ));
        }
    }

    Ok(workbook)
}

fn import_rows_into_sheet(
    rows: &[serde_json::Value],
    workbook: &mut Workbook,
    sheet_idx: usize,
    limits: ImportLimits,
    budget: &mut ImportBudget,
) -> Result<(), JsonError> {
    if let Some(sheet) = workbook.sheet_mut(sheet_idx) {
        import_rows(rows, sheet, limits, budget)?;
    }
    Ok(())
}

fn import_rows(
    rows: &[serde_json::Value],
    sheet: &mut Sheet,
    limits: ImportLimits,
    budget: &mut ImportBudget,
) -> Result<(), JsonError> {
    for (row_idx, row_value) in rows.iter().enumerate() {
        match row_value {
            serde_json::Value::Array(cells) => {
                track_cells(budget, cells.len(), limits)?;
                for (col_idx, cell_value) in cells.iter().enumerate() {
                    let (row, col) = checked_cell_ref(row_idx, col_idx, limits)?;
                    let cv = json_to_cell_value(cell_value);
                    if !cv.is_empty() {
                        sheet.set_cell(row, col, cv);
                    }
                }
            }
            serde_json::Value::Object(obj) => {
                track_cells(budget, obj.len().saturating_mul(2), limits)?;
                for (col_idx, (key, value)) in obj.iter().enumerate() {
                    let key_col = col_idx * 2;
                    let val_col = key_col + 1;
                    let (key_row, key_col) = checked_cell_ref(row_idx, key_col, limits)?;
                    let (val_row, val_col) = checked_cell_ref(row_idx, val_col, limits)?;
                    if !key.is_empty() {
                        sheet.set_cell_value(key_row, key_col, key.clone());
                    }
                    let cv = json_to_cell_value(value);
                    if !cv.is_empty() {
                        sheet.set_cell(val_row, val_col, cv);
                    }
                }
            }
            _ => {
                track_cells(budget, 1, limits)?;
                let (row, col) = checked_cell_ref(row_idx, 0, limits)?;
                let cv = json_to_cell_value(row_value);
                if !cv.is_empty() {
                    sheet.set_cell(row, col, cv);
                }
            }
        }
    }
    Ok(())
}

fn import_object_rows(
    obj: &serde_json::Map<String, serde_json::Value>,
    workbook: &mut Workbook,
    sheet_idx: usize,
    limits: ImportLimits,
    budget: &mut ImportBudget,
) -> Result<(), JsonError> {
    if let Some(sheet) = workbook.sheet_mut(sheet_idx) {
        track_cells(budget, obj.len().saturating_mul(2), limits)?;
        for (row_idx, (key, value)) in obj.iter().enumerate() {
            let (key_row, key_col) = checked_cell_ref(row_idx, 0, limits)?;
            let (val_row, val_col) = checked_cell_ref(row_idx, 1, limits)?;
            sheet.set_cell_value(key_row, key_col, key.clone());
            let cv = json_to_cell_value(value);
            if !cv.is_empty() {
                sheet.set_cell(val_row, val_col, cv);
            }
        }
    }
    Ok(())
}

fn json_to_cell_value(value: &serde_json::Value) -> CellValue {
    match value {
        serde_json::Value::Null => CellValue::empty(),
        serde_json::Value::Bool(b) => CellValue::boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                CellValue::number(f)
            } else {
                CellValue::text(n.to_string())
            }
        }
        serde_json::Value::String(s) => CellValue::text(s.clone()),
        serde_json::Value::Object(obj) => {
            if let Some(formula) = obj.get("formula").and_then(|v| v.as_str()) {
                CellValue::formula(formula.to_string())
            } else {
                CellValue::text(value.to_string())
            }
        }
        serde_json::Value::Array(_) => CellValue::text(value.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_import_array_of_arrays() {
        let json = r#"[[1,2,3],[4,5,6]]"#;
        let wb = import_workbook_json(json).unwrap();
        let sheet = &wb.sheets()[0];
        assert_eq!(sheet.cell_value(0, 0), Some("1".into()));
        assert_eq!(sheet.cell_value(0, 2), Some("3".into()));
        assert_eq!(sheet.cell_value(1, 1), Some("5".into()));
    }

    #[test]
    fn test_import_array_of_objects() {
        let json = r#"[{"name":"Alice","age":30},{"name":"Bob","age":25}]"#;
        let wb = import_workbook_json(json).unwrap();
        let sheet = &wb.sheets()[0];
        assert_eq!(sheet.cell_value(0, 0), Some("name".into()));
        assert_eq!(sheet.cell_value(0, 1), Some("Alice".into()));
        assert_eq!(sheet.cell_value(1, 0), Some("name".into()));
        assert_eq!(sheet.cell_value(1, 1), Some("Bob".into()));
    }

    #[test]
    fn test_import_object_key_values() {
        let json = r#"{"name":"Alice","age":30}"#;
        let wb = import_workbook_json(json).unwrap();
        let sheet = &wb.sheets()[0];
        assert_eq!(sheet.cell_value(0, 0), Some("name".into()));
        assert_eq!(sheet.cell_value(0, 1), Some("Alice".into()));
        assert_eq!(sheet.cell_value(1, 0), Some("age".into()));
        assert_eq!(sheet.cell_value(1, 1), Some("30".into()));
    }

    #[test]
    fn test_import_with_sheets_structure() {
        let json = r#"{"sheets":[{"name":"Data","data":[[1,2],[3,4]]}]}"#;
        let wb = import_workbook_json(json).unwrap();
        assert_eq!(wb.sheet_count(), 1);
        assert_eq!(wb.sheets()[0].name(), "Data");
        assert_eq!(wb.sheets()[0].cell_value(0, 0), Some("1".into()));
        assert_eq!(wb.sheets()[0].cell_value(1, 1), Some("4".into()));
    }

    #[test]
    fn test_import_multiple_sheets() {
        let json = r#"{"sheets":[{"name":"A","data":[[1]]},{"name":"B","data":[[2]]}]}"#;
        let wb = import_workbook_json(json).unwrap();
        assert_eq!(wb.sheet_count(), 2);
        assert_eq!(wb.sheets()[0].name(), "A");
        assert_eq!(wb.sheets()[1].name(), "B");
        assert_eq!(wb.sheets()[0].cell_value(0, 0), Some("1".into()));
        assert_eq!(wb.sheets()[1].cell_value(0, 0), Some("2".into()));
    }

    #[test]
    fn test_import_booleans() {
        let json = r#"[[true,false]]"#;
        let wb = import_workbook_json(json).unwrap();
        let sheet = &wb.sheets()[0];
        assert_eq!(sheet.cell_value(0, 0), Some("true".into()));
        assert_eq!(sheet.cell_value(0, 1), Some("false".into()));
    }

    #[test]
    fn test_import_strings() {
        let json = r#"[["hello","world"]]"#;
        let wb = import_workbook_json(json).unwrap();
        let sheet = &wb.sheets()[0];
        assert_eq!(sheet.cell_value(0, 0), Some("hello".into()));
        assert_eq!(sheet.cell_value(0, 1), Some("world".into()));
    }

    #[test]
    fn test_import_null_is_empty() {
        let json = r#"[[1,null,3]]"#;
        let wb = import_workbook_json(json).unwrap();
        let sheet = &wb.sheets()[0];
        assert_eq!(sheet.cell_value(0, 0), Some("1".into()));
        assert_eq!(sheet.cell_value(0, 1), None);
        assert_eq!(sheet.cell_value(0, 2), Some("3".into()));
    }

    #[test]
    fn test_import_invalid_json() {
        let result = import_workbook_json("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_import_rejects_too_many_cells() {
        let limits = ImportLimits {
            max_cells: 2,
            ..ImportLimits::default()
        };
        let result = import_workbook_json_with_limits("[[1,2,3]]", limits);
        assert!(matches!(result, Err(JsonError::TooManyCells(3, 2))));
    }

    #[test]
    fn test_import_rejects_out_of_bounds_rows() {
        let limits = ImportLimits {
            max_rows: 1,
            ..ImportLimits::default()
        };
        let result = import_workbook_json_with_limits("[[1],[2]]", limits);
        assert!(matches!(result, Err(JsonError::InvalidStructure(_))));
    }

    #[test]
    fn test_import_rejects_nested_json_past_limit() {
        let limits = ImportLimits {
            max_depth: 1,
            ..ImportLimits::default()
        };
        let result = import_workbook_json_with_limits("[[[1]]]", limits);
        assert!(matches!(result, Err(JsonError::TooDeep(_, 1))));
    }

    #[test]
    fn test_import_rejects_oversized_json_before_parse() {
        let limits = ImportLimits {
            max_bytes: 4,
            ..ImportLimits::default()
        };
        let result = import_workbook_json_with_limits("[1,2]", limits);
        assert!(matches!(result, Err(JsonError::FileTooLarge(5, 4))));
    }
}
