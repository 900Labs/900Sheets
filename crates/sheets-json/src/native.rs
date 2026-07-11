use crate::error::JsonError;
use serde::{Deserialize, Serialize};
use sheets_core::cell::CellValue;
use sheets_core::format::CellFormat;
use sheets_core::workbook::Workbook;

const FORMAT_VERSION: u32 = 1;
const MAX_NATIVE_SIZE: usize = 100 * 1024 * 1024;
const MAX_CELLS_AND_FORMATS: usize = 10_000_000;

#[derive(Serialize, Deserialize)]
struct NativeWorkbook {
    format: String,
    version: u32,
    active_sheet: usize,
    sheets: Vec<NativeSheet>,
    #[serde(default)]
    metadata: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
struct NativeSheet {
    #[serde(default)]
    stable_id: u64,
    name: String,
    cells: Vec<NativeCell>,
    formats: Vec<NativeFormat>,
}

#[derive(Serialize, Deserialize)]
struct NativeCell {
    row: u32,
    col: u32,
    value: CellValue,
}

#[derive(Serialize, Deserialize)]
struct NativeFormat {
    row: u32,
    col: u32,
    format: CellFormat,
}

pub fn export_native_workbook(workbook: &Workbook) -> Result<String, JsonError> {
    export_native_workbook_with_metadata(workbook, serde_json::Value::Null)
}

pub fn export_native_workbook_with_metadata(
    workbook: &Workbook,
    metadata: serde_json::Value,
) -> Result<String, JsonError> {
    let sheets = workbook
        .sheets()
        .iter()
        .map(|sheet| NativeSheet {
            stable_id: sheet.stable_id(),
            name: sheet.name().to_string(),
            cells: sheet
                .iter_cells()
                .map(|((row, col), value)| NativeCell {
                    row,
                    col,
                    value: value.clone(),
                })
                .collect(),
            formats: sheet
                .iter_formats()
                .map(|((row, col), format)| NativeFormat {
                    row,
                    col,
                    format: format.clone(),
                })
                .collect(),
        })
        .collect();
    Ok(serde_json::to_string_pretty(&NativeWorkbook {
        format: "900Sheets".to_string(),
        version: FORMAT_VERSION,
        active_sheet: workbook.active_sheet(),
        sheets,
        metadata,
    })?)
}

pub fn import_native_workbook(data: &str) -> Result<Workbook, JsonError> {
    import_native_workbook_with_metadata(data).map(|(workbook, _)| workbook)
}

pub fn import_native_workbook_with_metadata(
    data: &str,
) -> Result<(Workbook, serde_json::Value), JsonError> {
    if data.len() > MAX_NATIVE_SIZE {
        return Err(JsonError::FileTooLarge(data.len(), MAX_NATIVE_SIZE));
    }
    let native: NativeWorkbook = serde_json::from_str(data)?;
    if native.format != "900Sheets" || native.version != FORMAT_VERSION {
        return Err(JsonError::InvalidStructure(format!(
            "unsupported 900Sheets format version {}",
            native.version
        )));
    }
    if native.sheets.is_empty() {
        return Err(JsonError::InvalidStructure(
            "a workbook must contain at least one sheet".to_string(),
        ));
    }
    let item_count: usize = native
        .sheets
        .iter()
        .map(|sheet| sheet.cells.len().saturating_add(sheet.formats.len()))
        .sum();
    if item_count > MAX_CELLS_AND_FORMATS {
        return Err(JsonError::TooManyCells(item_count, MAX_CELLS_AND_FORMATS));
    }

    let mut workbook = Workbook::new();
    workbook.rename_sheet(0, &native.sheets[0].name);
    if native.sheets[0].stable_id != 0 {
        workbook.set_sheet_stable_id(0, native.sheets[0].stable_id);
    }
    for sheet in native.sheets.iter().skip(1) {
        let index = workbook.add_sheet(&sheet.name);
        if sheet.stable_id != 0 {
            workbook.set_sheet_stable_id(index, sheet.stable_id);
        }
    }
    let active_sheet = native.active_sheet;
    let metadata = native.metadata;
    for (index, source) in native.sheets.into_iter().enumerate() {
        let target = workbook.sheet_mut(index).ok_or_else(|| {
            JsonError::InvalidStructure(format!("sheet {index} could not be created"))
        })?;
        for cell in source.cells {
            if !target.in_bounds(cell.row, cell.col) {
                return Err(JsonError::InvalidStructure(format!(
                    "cell {},{} is outside workbook limits",
                    cell.row, cell.col
                )));
            }
            target.set_cell(cell.row, cell.col, cell.value);
        }
        for format in source.formats {
            if !target.in_bounds(format.row, format.col) {
                return Err(JsonError::InvalidStructure(format!(
                    "format {},{} is outside workbook limits",
                    format.row, format.col
                )));
            }
            target.set_format(format.row, format.col, format.format);
        }
    }
    workbook.set_active_sheet(active_sheet.min(workbook.sheet_count() - 1));
    Ok((workbook, metadata))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_roundtrip_preserves_sparse_cells_formulas_formats_and_active_sheet() {
        let mut workbook = Workbook::new();
        workbook.rename_sheet(0, "Data");
        workbook.add_sheet("Report");
        workbook.set_active_sheet(1);
        workbook
            .sheet_mut(0)
            .unwrap()
            .set_cell_value(999_999, 16_383, "edge".into());
        workbook
            .sheet_mut(1)
            .unwrap()
            .set_cell_value(0, 0, "=1+1".into());
        workbook.sheet_mut(1).unwrap().set_format(
            4,
            4,
            CellFormat::new().bold(true).bg_color("#112233"),
        );

        let data = export_native_workbook(&workbook).unwrap();
        assert!(data.len() < 2_000);
        let imported = import_native_workbook(&data).unwrap();

        assert_eq!(imported.active_sheet(), 1);
        assert_eq!(
            imported.sheet(0).unwrap().cell_value(999_999, 16_383),
            Some("edge".into())
        );
        assert!(imported.sheet(1).unwrap().cell(0, 0).unwrap().is_formula());
        assert_eq!(
            imported.sheet(1).unwrap().get_format(4, 4),
            Some(&CellFormat::new().bold(true).bg_color("#112233"))
        );
    }

    #[test]
    fn native_roundtrip_preserves_feature_metadata() {
        let mut workbook = Workbook::new();
        workbook.add_sheet("Second");
        let first_id = workbook.sheet(0).unwrap().stable_id();
        let second_id = workbook.sheet(1).unwrap().stable_id();
        let metadata = serde_json::json!({
            "sheet_states": {
                first_id.to_string(): {"namedRanges": [{"name": "Revenue"}], "frozenRowCount": 1},
                second_id.to_string(): {"namedRanges": [{"name": "Costs"}], "frozenRowCount": 2}
            }
        });
        let data = export_native_workbook_with_metadata(&workbook, metadata.clone()).unwrap();
        let (restored_workbook, restored) = import_native_workbook_with_metadata(&data).unwrap();
        assert_eq!(restored, metadata);
        assert_eq!(restored_workbook.sheet(0).unwrap().stable_id(), first_id);
        assert_eq!(restored_workbook.sheet(1).unwrap().stable_id(), second_id);
    }
}
