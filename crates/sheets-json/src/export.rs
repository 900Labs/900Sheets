use crate::error::JsonError;
use serde_json::{json, Value};
use sheets_core::cell::CellType;
use sheets_core::workbook::Workbook;

pub fn export_workbook_json(workbook: &Workbook) -> Result<String, JsonError> {
    let mut sheets_arr = Vec::new();

    for i in 0..workbook.sheet_count() {
        if let Some(sheet) = workbook.sheet(i) {
            let data = export_sheet_data(sheet);
            sheets_arr.push(json!({
                "name": sheet.name(),
                "data": data,
            }));
        }
    }

    let result = json!({ "sheets": sheets_arr });
    Ok(serde_json::to_string_pretty(&result)?)
}

fn export_sheet_data(sheet: &sheets_core::sheet::Sheet) -> Value {
    let mut rows: std::collections::BTreeMap<u32, std::collections::BTreeMap<u32, Value>> =
        std::collections::BTreeMap::new();

    let mut max_col: u32 = 0;
    let mut max_row: u32 = 0;

    for ((row, col), cell) in sheet.iter_cells() {
        if !cell.is_empty() {
            rows.entry(row)
                .or_default()
                .insert(col, cell_to_json(cell.cell_type.clone(), &cell.raw));
            if col > max_col {
                max_col = col;
            }
            if row > max_row {
                max_row = row;
            }
        }
    }

    if rows.is_empty() {
        return Value::Array(Vec::new());
    }

    let mut result = Vec::new();
    for row in 0..=max_row {
        let mut row_arr = Vec::new();
        if let Some(row_data) = rows.get(&row) {
            for col in 0..=max_col {
                if let Some(v) = row_data.get(&col) {
                    row_arr.push(v.clone());
                } else {
                    row_arr.push(Value::Null);
                }
            }
        } else {
            for _ in 0..=max_col {
                row_arr.push(Value::Null);
            }
        }
        result.push(Value::Array(row_arr));
    }

    Value::Array(result)
}

fn cell_to_json(cell_type: CellType, raw: &str) -> Value {
    match cell_type {
        CellType::Number => {
            if let Ok(n) = raw.parse::<f64>() {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    json!(n as i64)
                } else {
                    json!(n)
                }
            } else {
                json!(raw)
            }
        }
        CellType::Text => json!(raw),
        CellType::Boolean => json!(raw.eq_ignore_ascii_case("true")),
        CellType::Formula => json!({"formula": raw}),
        CellType::Error => json!(raw),
        CellType::Empty => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_empty_workbook() {
        let wb = Workbook::new();
        let json_str = export_workbook_json(&wb).unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert!(parsed["sheets"].is_array());
        assert_eq!(parsed["sheets"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_export_with_data() {
        let mut wb = Workbook::new();
        let sheet = wb.sheet_mut(0).unwrap();
        sheet.set_cell_value(0, 0, "1".into());
        sheet.set_cell_value(0, 1, "hello".into());
        sheet.set_cell_value(1, 0, "3.14".into());

        let json_str = export_workbook_json(&wb).unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        let data = &parsed["sheets"][0]["data"];
        assert_eq!(data[0][0], json!(1));
        assert_eq!(data[0][1], json!("hello"));
        assert_eq!(data[1][0], json!(314.0 / 100.0));
    }

    #[test]
    fn test_export_multiple_sheets() {
        let mut wb = Workbook::new();
        wb.rename_sheet(0, "First");
        wb.add_sheet("Second");
        wb.sheet_mut(0).unwrap().set_cell_value(0, 0, "a".into());
        wb.sheet_mut(1).unwrap().set_cell_value(0, 0, "b".into());

        let json_str = export_workbook_json(&wb).unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["sheets"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["sheets"][0]["name"], "First");
        assert_eq!(parsed["sheets"][1]["name"], "Second");
        assert_eq!(parsed["sheets"][0]["data"][0][0], json!("a"));
        assert_eq!(parsed["sheets"][1]["data"][0][0], json!("b"));
    }

    #[test]
    fn test_json_roundtrip() {
        let mut wb = Workbook::new();
        let sheet = wb.sheet_mut(0).unwrap();
        sheet.set_cell_value(0, 0, "name".into());
        sheet.set_cell_value(0, 1, "42".into());
        sheet.set_cell_value(1, 0, "Alice".into());
        sheet.set_cell_value(1, 1, "30".into());

        let json_str = export_workbook_json(&wb).unwrap();
        let wb2 = crate::import_workbook_json(&json_str).unwrap();

        assert_eq!(wb2.sheet_count(), 1);
        assert_eq!(wb2.sheets()[0].cell_value(0, 0), Some("name".into()));
        assert_eq!(wb2.sheets()[0].cell_value(0, 1), Some("42".into()));
        assert_eq!(wb2.sheets()[0].cell_value(1, 0), Some("Alice".into()));
        assert_eq!(wb2.sheets()[0].cell_value(1, 1), Some("30".into()));
    }

    #[test]
    fn test_export_booleans() {
        let mut wb = Workbook::new();
        wb.sheet_mut(0).unwrap().set_cell_value(0, 0, "true".into());
        wb.sheet_mut(0)
            .unwrap()
            .set_cell_value(0, 1, "false".into());

        let json_str = export_workbook_json(&wb).unwrap();
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["sheets"][0]["data"][0][0], json!(true));
        assert_eq!(parsed["sheets"][0]["data"][0][1], json!(false));
    }
}
