use crate::error::CsvError;
use sheets_core::sheet::Sheet;

pub fn export_sheet_csv(sheet: &Sheet, delimiter: char) -> Result<String, CsvError> {
    let mut rows: std::collections::BTreeMap<u32, std::collections::BTreeMap<u32, String>> =
        std::collections::BTreeMap::new();

    let mut max_col: u32 = 0;

    for ((row, col), cell) in sheet.iter_cells() {
        if !cell.is_empty() {
            rows.entry(row).or_default().insert(col, cell.raw.clone());
            if col > max_col {
                max_col = col;
            }
        }
    }

    let mut output = String::new();

    if rows.is_empty() {
        return Ok(output);
    }

    let max_row = *rows.last_key_value().unwrap().0;

    for row in 0..=max_row {
        let row_data = rows.get(&row);
        for col in 0..=max_col {
            if col > 0 {
                output.push(delimiter);
            }
            if let Some(cell_map) = row_data {
                if let Some(value) = cell_map.get(&col) {
                    output.push_str(&escape_csv_field(value, delimiter));
                }
            }
        }
        output.push('\n');
    }

    Ok(output)
}

fn escape_csv_field(value: &str, delimiter: char) -> String {
    let needs_quoting = value.contains(delimiter)
        || value.contains('"')
        || value.contains('\n')
        || value.contains('\r');

    if needs_quoting {
        let escaped = value.replace('"', "\"\"");
        format!("\"{}\"", escaped)
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_empty_sheet() {
        let sheet = Sheet::new("Sheet1");
        let csv = export_sheet_csv(&sheet, ',').unwrap();
        assert_eq!(csv, "");
    }

    #[test]
    fn test_export_simple() {
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_cell_value(0, 0, "a".into());
        sheet.set_cell_value(0, 1, "b".into());
        sheet.set_cell_value(1, 0, "1".into());
        sheet.set_cell_value(1, 1, "2".into());
        let csv = export_sheet_csv(&sheet, ',').unwrap();
        assert_eq!(csv, "a,b\n1,2\n");
    }

    #[test]
    fn test_export_with_commas() {
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_cell_value(0, 0, "hello,world".into());
        let csv = export_sheet_csv(&sheet, ',').unwrap();
        assert_eq!(csv, "\"hello,world\"\n");
    }

    #[test]
    fn test_export_with_quotes() {
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_cell_value(0, 0, "say \"hi\"".into());
        let csv = export_sheet_csv(&sheet, ',').unwrap();
        assert_eq!(csv, "\"say \"\"hi\"\"\"\n");
    }

    #[test]
    fn test_export_with_newlines() {
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_cell_value(0, 0, "line1\nline2".into());
        let csv = export_sheet_csv(&sheet, ',').unwrap();
        assert_eq!(csv, "\"line1\nline2\"\n");
    }

    #[test]
    fn test_export_with_gaps() {
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_cell_value(0, 0, "a".into());
        sheet.set_cell_value(0, 2, "c".into());
        let csv = export_sheet_csv(&sheet, ',').unwrap();
        assert_eq!(csv, "a,,c\n");
    }

    #[test]
    fn test_export_tab_delimited() {
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_cell_value(0, 0, "a".into());
        sheet.set_cell_value(0, 1, "b".into());
        let csv = export_sheet_csv(&sheet, '\t').unwrap();
        assert_eq!(csv, "a\tb\n");
    }

    #[test]
    fn test_csv_roundtrip() {
        let original = "name,age,city\nAlice,30,NYC\nBob,25,LA";
        let sheet = crate::import_csv(original, ',').unwrap();
        let exported = export_sheet_csv(&sheet, ',').unwrap();
        assert_eq!(exported, "name,age,city\nAlice,30,NYC\nBob,25,LA\n");
    }
}
