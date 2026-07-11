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

    let mut assigned_ids = Vec::with_capacity(native.sheets.len());
    let mut used_ids: std::collections::HashSet<u64> = native
        .sheets
        .iter()
        .filter_map(|sheet| (sheet.stable_id != 0).then_some(sheet.stable_id))
        .collect();
    let nonzero_count = native
        .sheets
        .iter()
        .filter(|sheet| sheet.stable_id != 0)
        .count();
    if used_ids.len() != nonzero_count {
        return Err(JsonError::InvalidStructure(
            "native workbook contains duplicate stable sheet IDs".into(),
        ));
    }
    let mut next_generated_id = 1u64;
    for sheet in &native.sheets {
        if sheet.stable_id != 0 {
            assigned_ids.push(sheet.stable_id);
        } else {
            while used_ids.contains(&next_generated_id) {
                next_generated_id = next_generated_id.saturating_add(1);
            }
            assigned_ids.push(next_generated_id);
            used_ids.insert(next_generated_id);
            next_generated_id = next_generated_id.saturating_add(1);
        }
    }

    let mut workbook = Workbook::new();
    workbook
        .rename_sheet(0, &native.sheets[0].name)
        .map_err(|error| JsonError::InvalidStructure(error.to_string()))?;
    for sheet in native.sheets.iter().skip(1) {
        workbook
            .add_sheet(&sheet.name)
            .map_err(|error| JsonError::InvalidStructure(error.to_string()))?;
    }
    workbook
        .replace_sheet_stable_ids(&assigned_ids)
        .map_err(|error| JsonError::InvalidStructure(error.to_string()))?;
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
        workbook.rename_sheet(0, "Data").unwrap();
        workbook.add_sheet("Report").unwrap();
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
        workbook.add_sheet("Second").unwrap();
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

    #[test]
    fn native_roundtrip_preserves_cross_sheet_formula_text_and_stable_ids() {
        let mut workbook = Workbook::new();
        workbook.rename_sheet(0, "Annual Budget").unwrap();
        let report = workbook.add_sheet("Report").unwrap();
        workbook.sheet_mut(report).unwrap().set_cell_value(
            0,
            0,
            "=SUM('Annual Budget'!$A$1:$A$2)".into(),
        );
        let source_id = workbook.sheet(0).unwrap().stable_id();

        let restored = import_native_workbook(&export_native_workbook(&workbook).unwrap()).unwrap();
        assert_eq!(restored.sheet(0).unwrap().stable_id(), source_id);
        assert_eq!(
            restored.sheet(1).unwrap().cell_value(0, 0),
            Some("=SUM('Annual Budget'!$A$1:$A$2)".into())
        );
    }

    #[test]
    fn native_import_rejects_duplicate_nonzero_stable_ids() {
        let data = serde_json::json!({
            "format": "900Sheets",
            "version": 1,
            "active_sheet": 0,
            "sheets": [
                {"stable_id": 7, "name": "First", "cells": [], "formats": []},
                {"stable_id": 7, "name": "Second", "cells": [], "formats": []}
            ]
        })
        .to_string();
        assert!(matches!(
            import_native_workbook(&data),
            Err(JsonError::InvalidStructure(message)) if message.contains("duplicate stable")
        ));
    }

    #[test]
    fn native_import_remaps_legacy_zero_ids_without_colliding_with_explicit_ids() {
        let data = serde_json::json!({
            "format": "900Sheets",
            "version": 1,
            "active_sheet": 0,
            "sheets": [
                {"stable_id": 0, "name": "Legacy", "cells": [], "formats": []},
                {"stable_id": 1, "name": "Explicit", "cells": [], "formats": []}
            ]
        })
        .to_string();
        let workbook = import_native_workbook(&data).unwrap();
        assert_eq!(workbook.sheet(1).unwrap().stable_id(), 1);
        assert_ne!(workbook.sheet(0).unwrap().stable_id(), 1);
        assert_ne!(workbook.sheet(0).unwrap().stable_id(), 0);
    }
}
