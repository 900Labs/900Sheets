use serde::Serialize;
use sheets_core::{Sheet, Workbook};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExportKind {
    Csv,
    Json,
    Pdf,
    Xlsx,
}

impl ExportKind {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "csv" => Ok(Self::Csv),
            "json" => Ok(Self::Json),
            "pdf" => Ok(Self::Pdf),
            "xlsx" => Ok(Self::Xlsx),
            _ => Err(format!("Unsupported export format: {value}")),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Csv => "CSV",
            Self::Json => "JSON",
            Self::Pdf => "PDF",
            Self::Xlsx => "XLSX",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ExportPreflight {
    pub(crate) format: String,
    pub(crate) estimated_dense_cells: u64,
    pub(crate) populated_cells: usize,
    pub(crate) max_row: Option<u32>,
    pub(crate) max_col: Option<u32>,
    pub(crate) limit: Option<u64>,
    pub(crate) blocked: bool,
    pub(crate) message: String,
}

fn sheet_extent(sheet: &Sheet, include_formats: bool) -> (u64, usize, Option<u32>, Option<u32>) {
    let mut populated_cells = sheet.cell_count();
    let mut max_row: Option<u32> = None;
    let mut max_col: Option<u32> = None;
    for ((row, col), _) in sheet.iter_cells() {
        max_row = Some(max_row.map_or(row, |current: u32| current.max(row)));
        max_col = Some(max_col.map_or(col, |current: u32| current.max(col)));
    }
    if include_formats {
        for ((row, col), _) in sheet.iter_formats() {
            if sheet.cell(row, col).is_none() {
                populated_cells = populated_cells.saturating_add(1);
            }
            max_row = Some(max_row.map_or(row, |current: u32| current.max(row)));
            max_col = Some(max_col.map_or(col, |current: u32| current.max(col)));
        }
    }
    let dense = match (max_row, max_col) {
        (Some(row), Some(col)) => u64::from(row + 1).saturating_mul(u64::from(col + 1)),
        _ => 0,
    };
    (dense, populated_cells, max_row, max_col)
}

pub(crate) fn preflight_workbook(
    workbook: &Workbook,
    kind: ExportKind,
    sheet_index: Option<usize>,
    print_area: Option<[u32; 4]>,
) -> Result<ExportPreflight, String> {
    let selected: Vec<&Sheet> = match kind {
        ExportKind::Csv | ExportKind::Pdf => vec![workbook
            .sheet(sheet_index.ok_or_else(|| "A sheet is required for this export".to_string())?)
            .ok_or_else(|| "Sheet not found".to_string())?],
        ExportKind::Json | ExportKind::Xlsx => workbook.sheets().iter().collect(),
    };

    let mut estimated_dense_cells = 0u64;
    let mut populated_cells = 0usize;
    let mut max_row: Option<u32> = None;
    let mut max_col: Option<u32> = None;
    let mut largest_sheet_area = 0u64;
    for sheet in selected {
        let (mut dense, populated, mut row, mut col) =
            sheet_extent(sheet, kind == ExportKind::Xlsx);
        if kind == ExportKind::Pdf {
            if let Some([start_row, start_col, end_row, end_col]) = print_area {
                if start_row > end_row || start_col > end_col {
                    return Err("Print area start must not be after its end".into());
                }
                if end_row >= sheet.max_rows() || end_col >= sheet.max_cols() {
                    return Err("Print area is outside the workbook grid".into());
                }
                dense = (u64::from(end_row) - u64::from(start_row) + 1)
                    .saturating_mul(u64::from(end_col) - u64::from(start_col) + 1);
                row = Some(end_row);
                col = Some(end_col);
            } else if let Some((start_row, start_col, end_row, end_col)) =
                sheets_print::used_range(sheet)
            {
                dense = (u64::from(end_row) - u64::from(start_row) + 1)
                    .saturating_mul(u64::from(end_col) - u64::from(start_col) + 1);
                row = Some(end_row);
                col = Some(end_col);
            } else {
                dense = 0;
            }
        }
        if kind == ExportKind::Xlsx {
            dense = 0;
        }
        estimated_dense_cells = estimated_dense_cells.saturating_add(dense);
        populated_cells = populated_cells.saturating_add(populated);
        largest_sheet_area = largest_sheet_area.max(dense);
        max_row = match (max_row, row) {
            (Some(current), Some(value)) => Some(current.max(value)),
            (None, value) => value,
            (value, None) => value,
        };
        max_col = match (max_col, col) {
            (Some(current), Some(value)) => Some(current.max(value)),
            (None, value) => value,
            (value, None) => value,
        };
    }

    let limit = match kind {
        ExportKind::Csv => Some(sheets_csv::MAX_DENSE_EXPORT_CELLS as u64),
        ExportKind::Json => Some(sheets_json::MAX_DENSE_EXPORT_CELLS as u64),
        ExportKind::Pdf => Some(sheets_print::MAX_PRINT_CELLS),
        ExportKind::Xlsx => None,
    };
    let compared_area = if kind == ExportKind::Json {
        largest_sheet_area
    } else {
        estimated_dense_cells
    };
    let blocked = limit.is_some_and(|maximum| compared_area > maximum);
    let message = if blocked {
        format!(
            "{} export would expand a sheet to {compared_area} cells, above the {} cell safety limit. Move or clear high-coordinate cells before exporting.",
            kind.label(),
            limit.expect("blocked exports have a limit")
        )
    } else if kind == ExportKind::Xlsx {
        format!(
            "XLSX export will write {populated_cells} stored cell or format coordinates without dense grid expansion."
        )
    } else if estimated_dense_cells >= 1_000_000 {
        format!(
            "{} export may process approximately {estimated_dense_cells} cells. Continue only if that size is expected.",
            kind.label()
        )
    } else {
        format!(
            "{} export size is within the configured safety limit.",
            kind.label()
        )
    };

    Ok(ExportPreflight {
        format: kind.label().to_string(),
        estimated_dense_cells,
        populated_cells,
        max_row,
        max_col,
        limit,
        blocked,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_high_coordinate_csv_is_blocked_before_materialization() {
        let mut workbook = Workbook::new();
        workbook
            .sheet_mut(0)
            .unwrap()
            .set_cell_value(999_999, 10, "far away".into());

        let result = preflight_workbook(&workbook, ExportKind::Csv, Some(0), None).unwrap();

        assert!(result.blocked);
        assert_eq!(result.populated_cells, 1);
        assert_eq!(result.estimated_dense_cells, 11_000_000);
    }

    #[test]
    fn xlsx_reports_sparse_extent_without_applying_a_dense_limit() {
        let mut workbook = Workbook::new();
        workbook
            .sheet_mut(0)
            .unwrap()
            .set_cell_value(999_999, 16_383, "edge".into());

        let result = preflight_workbook(&workbook, ExportKind::Xlsx, None, None).unwrap();

        assert!(!result.blocked);
        assert_eq!(result.limit, None);
        assert_eq!(result.estimated_dense_cells, 0);
        assert_eq!(result.max_row, Some(999_999));
        assert_eq!(result.max_col, Some(16_383));
    }

    #[test]
    fn json_compares_each_sheet_to_its_per_sheet_backend_limit() {
        let mut workbook = Workbook::new();
        workbook
            .sheet_mut(0)
            .unwrap()
            .set_cell_value(999, 999, "first".into());
        let second = workbook.add_sheet("Second").unwrap();
        workbook
            .sheet_mut(second)
            .unwrap()
            .set_cell_value(999, 999, "second".into());

        let result = preflight_workbook(&workbook, ExportKind::Json, None, None).unwrap();

        assert!(!result.blocked);
        assert_eq!(result.estimated_dense_cells, 2_000_000);
    }

    #[test]
    fn formatted_blank_does_not_false_block_dense_data_exports() {
        let mut workbook = Workbook::new();
        workbook.sheet_mut(0).unwrap().set_format(
            999_999,
            16_383,
            sheets_core::CellFormat {
                bold: Some(true),
                ..Default::default()
            },
        );

        let csv = preflight_workbook(&workbook, ExportKind::Csv, Some(0), None).unwrap();
        let json = preflight_workbook(&workbook, ExportKind::Json, None, None).unwrap();

        assert!(!csv.blocked);
        assert!(!json.blocked);
        assert_eq!(csv.estimated_dense_cells, 0);
        assert_eq!(json.estimated_dense_cells, 0);
    }

    #[test]
    fn explicit_pdf_print_area_ignores_unrelated_far_cell() {
        let mut workbook = Workbook::new();
        workbook
            .sheet_mut(0)
            .unwrap()
            .set_cell_value(999_999, 16_383, "far".into());

        let result =
            preflight_workbook(&workbook, ExportKind::Pdf, Some(0), Some([0, 0, 49, 9])).unwrap();

        assert!(!result.blocked);
        assert_eq!(result.estimated_dense_cells, 500);
    }

    #[test]
    fn implicit_pdf_area_uses_the_true_sparse_bounding_rectangle() {
        let mut workbook = Workbook::new();
        workbook
            .sheet_mut(0)
            .unwrap()
            .set_cell_value(999_999, 10, "far".into());

        let result = preflight_workbook(&workbook, ExportKind::Pdf, Some(0), None).unwrap();

        assert!(!result.blocked);
        assert_eq!(result.estimated_dense_cells, 1);
    }

    #[test]
    fn pdf_area_rejects_coordinates_outside_the_grid_without_overflow() {
        let workbook = Workbook::new();
        assert!(preflight_workbook(
            &workbook,
            ExportKind::Pdf,
            Some(0),
            Some([0, 0, u32::MAX, u32::MAX]),
        )
        .is_err());
    }
}
