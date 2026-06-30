use serde::{Deserialize, Serialize};
use sheets_core::sheet::Sheet;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Aggregation {
    Sum,
    Count,
    Average,
    Min,
    Max,
    CountNumbers,
    CountNonEmpty,
    Product,
}

impl Aggregation {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "sum" => Some(Self::Sum),
            "count" => Some(Self::Count),
            "counta" => Some(Self::CountNonEmpty),
            "countnum" => Some(Self::CountNumbers),
            "average" | "avg" | "mean" => Some(Self::Average),
            "min" => Some(Self::Min),
            "max" => Some(Self::Max),
            "product" | "prod" => Some(Self::Product),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Sum => "Sum",
            Self::Count => "Count",
            Self::CountNonEmpty => "CountA",
            Self::CountNumbers => "CountNum",
            Self::Average => "Average",
            Self::Min => "Min",
            Self::Max => "Max",
            Self::Product => "Product",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PivotField {
    pub column: u32,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PivotValueField {
    pub column: u32,
    pub label: String,
    pub aggregation: Aggregation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PivotConfig {
    pub source_sheet: usize,
    pub data_range: (u32, u32, u32, u32), // (start_row, start_col, end_row, end_col)
    pub header_row: u32,
    pub row_fields: Vec<PivotField>,
    pub column_fields: Vec<PivotField>,
    pub value_fields: Vec<PivotValueField>,
    pub filter_field: Option<PivotField>,
    pub filter_values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PivotResult {
    pub row_headers: Vec<String>,
    pub column_headers: Vec<String>,
    pub data: Vec<Vec<Vec<f64>>>, // [row][col][value_field]
    pub grand_totals_row: Vec<f64>,
    pub grand_totals_col: Vec<Vec<f64>>, // [row][value_field]
    pub value_field_labels: Vec<String>,
}

#[derive(Debug, Error, Serialize)]
pub enum PivotError {
    #[error("Invalid data range: {0}")]
    InvalidRange(String),
    #[error("No data rows found")]
    NoData,
    #[error("Column index {0} out of range")]
    ColumnOutOfRange(u32),
    #[error("No row fields specified")]
    NoRowFields,
    #[error("No value fields specified")]
    NoValueFields,
}

type RowData = Vec<String>;

fn extract_column_name(sheet: &Sheet, row: u32, col: u32) -> String {
    sheet
        .cell_value(row, col)
        .unwrap_or_else(|| format!("Col{}", col + 1))
}

fn extract_cell_text(sheet: &Sheet, row: u32, col: u32) -> String {
    sheet.cell_value(row, col).unwrap_or_default()
}

fn extract_cell_number(sheet: &Sheet, row: u32, col: u32) -> Option<f64> {
    sheet.cell(row, col).and_then(|c| c.as_number())
}

fn passes_filter(
    sheet: &Sheet,
    row: u32,
    filter: &Option<PivotField>,
    filter_values: &[String],
) -> bool {
    if let Some(f) = filter {
        if filter_values.is_empty() {
            return true;
        }
        let val = extract_cell_text(sheet, row, f.column);
        return filter_values.iter().any(|fv| fv == &val);
    }
    true
}

fn aggregate(values: &[f64], agg: &Aggregation) -> f64 {
    match agg {
        Aggregation::Sum => values.iter().sum(),
        Aggregation::Count => values.len() as f64,
        Aggregation::CountNumbers => values.len() as f64,
        Aggregation::CountNonEmpty => values.len() as f64,
        Aggregation::Average => {
            if values.is_empty() {
                0.0
            } else {
                values.iter().sum::<f64>() / values.len() as f64
            }
        }
        Aggregation::Min => values.iter().cloned().fold(f64::INFINITY, f64::min),
        Aggregation::Max => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        Aggregation::Product => values.iter().product(),
    }
}

pub fn build_pivot(sheet: &Sheet, config: &PivotConfig) -> Result<PivotResult, PivotError> {
    if config.row_fields.is_empty() {
        return Err(PivotError::NoRowFields);
    }
    if config.value_fields.is_empty() {
        return Err(PivotError::NoValueFields);
    }

    let (start_row, start_col, end_row, end_col) = config.data_range;
    if start_row > end_row || start_col > end_col {
        return Err(PivotError::InvalidRange(format!(
            "start ({},{}) > end ({},{})",
            start_row, start_col, end_row, end_col
        )));
    }

    let data_start_row = config.header_row + 1;

    let mut row_keys: Vec<RowData> = Vec::new();
    let mut row_key_index: HashMap<RowData, usize> = HashMap::new();
    let mut col_keys: Vec<RowData> = Vec::new();
    let mut col_key_index: HashMap<RowData, usize> = HashMap::new();

    let mut buckets: HashMap<(usize, usize), Vec<Vec<f64>>> = HashMap::new();

    for row in data_start_row..=end_row {
        if !passes_filter(sheet, row, &config.filter_field, &config.filter_values) {
            continue;
        }

        let row_key: RowData = config
            .row_fields
            .iter()
            .map(|f| extract_cell_text(sheet, row, f.column))
            .collect();

        let col_key: RowData = config
            .column_fields
            .iter()
            .map(|f| extract_cell_text(sheet, row, f.column))
            .collect();

        let row_idx = if let Some(&idx) = row_key_index.get(&row_key) {
            idx
        } else {
            let idx = row_keys.len();
            row_key_index.insert(row_key.clone(), idx);
            row_keys.push(row_key);
            idx
        };

        let col_idx = if let Some(&idx) = col_key_index.get(&col_key) {
            idx
        } else {
            let idx = col_keys.len();
            col_key_index.insert(col_key.clone(), idx);
            col_keys.push(col_key);
            idx
        };

        let entry = buckets
            .entry((row_idx, col_idx))
            .or_insert_with(|| vec![Vec::new(); config.value_fields.len()]);

        for (vi, vf) in config.value_fields.iter().enumerate() {
            if let Some(n) = extract_cell_number(sheet, row, vf.column) {
                entry[vi].push(n);
            }
        }
    }

    if row_keys.is_empty() {
        return Err(PivotError::NoData);
    }

    let num_rows = row_keys.len();
    let num_cols = if col_keys.is_empty() {
        1
    } else {
        col_keys.len()
    };
    let num_values = config.value_fields.len();

    let mut data = vec![vec![vec![0.0; num_values]; num_cols]; num_rows];

    for ((ri, ci), values) in &buckets {
        for vi in 0..num_values {
            if vi < values.len() {
                data[*ri][*ci][vi] = aggregate(&values[vi], &config.value_fields[vi].aggregation);
            }
        }
    }

    let mut grand_totals_row = vec![0.0; num_values];
    let mut grand_totals_col = vec![vec![0.0; num_values]; num_rows];

    for (ri, row_data) in data.iter().enumerate() {
        for col_data in row_data.iter() {
            for (vi, &val) in col_data.iter().enumerate() {
                grand_totals_row[vi] += val;
                grand_totals_col[ri][vi] += val;
            }
        }
    }

    if config.value_fields.len() == 1
        && matches!(config.value_fields[0].aggregation, Aggregation::Average)
    {
        for (vi, total) in grand_totals_row.iter_mut().enumerate() {
            let total_count: usize = buckets
                .values()
                .map(|v| v.get(vi).map(|s| s.len()).unwrap_or(0))
                .sum();
            if total_count > 0 {
                *total /= total_count as f64;
            }
        }
    }

    let row_headers: Vec<String> = row_keys
        .iter()
        .map(|rk| {
            if rk.len() == 1 {
                rk[0].clone()
            } else {
                rk.join(" / ")
            }
        })
        .collect();

    let column_headers: Vec<String> = if col_keys.is_empty() {
        config
            .value_fields
            .iter()
            .map(|vf| format!("{} of {}", vf.aggregation.label(), vf.label))
            .collect()
    } else {
        let mut headers = Vec::new();
        for ck in &col_keys {
            let col_label = if ck.len() == 1 {
                ck[0].clone()
            } else {
                ck.join(" / ")
            };
            for vf in &config.value_fields {
                headers.push(format!(
                    "{} of {} ({})",
                    vf.aggregation.label(),
                    vf.label,
                    col_label
                ));
            }
        }
        headers
    };

    let value_field_labels: Vec<String> = config
        .value_fields
        .iter()
        .map(|vf| format!("{} of {}", vf.aggregation.label(), vf.label))
        .collect();

    Ok(PivotResult {
        row_headers,
        column_headers,
        data,
        grand_totals_row,
        grand_totals_col,
        value_field_labels,
    })
}

pub fn auto_detect_columns(
    sheet: &Sheet,
    header_row: u32,
    start_col: u32,
    end_col: u32,
) -> Vec<(u32, String)> {
    let mut cols = Vec::new();
    for col in start_col..=end_col {
        let name = extract_column_name(sheet, header_row, col);
        if !name.is_empty() {
            cols.push((col, name));
        }
    }
    cols
}

pub fn write_pivot_to_sheet(
    sheet: &mut Sheet,
    result: &PivotResult,
    start_row: u32,
    start_col: u32,
) {
    let mut r = start_row;

    if !result.column_headers.is_empty() {
        sheet.set_cell_value(r, start_col, "".into());
        for (ci, ch) in result.column_headers.iter().enumerate() {
            sheet.set_cell_value(r, start_col + 1 + ci as u32, ch.clone());
        }
        r += 1;
    }

    for (ri, rh) in result.row_headers.iter().enumerate() {
        sheet.set_cell_value(r, start_col, rh.clone());
        for ci in 0..result.column_headers.len() {
            if ri < result.data.len() && ci < result.data[ri].len() {
                for vi in 0..result.data[ri][ci].len() {
                    let val = result.data[ri][ci][vi];
                    let col_offset = if result.value_field_labels.len() == 1 {
                        ci as u32
                    } else {
                        (ci * result.value_field_labels.len() + vi) as u32
                    };
                    sheet.set_cell_value(r, start_col + 1 + col_offset, val.to_string());
                }
            }
        }
        r += 1;
    }

    sheet.set_cell_value(r, start_col, "Grand Total".into());
    for (vi, &total) in result.grand_totals_row.iter().enumerate() {
        sheet.set_cell_value(r, start_col + 1 + vi as u32, total.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sheets_core::workbook::Workbook;

    fn make_test_sheet() -> Sheet {
        let mut sheet = Sheet::new("Sales");
        // Headers
        sheet.set_cell_value(0, 0, "Region".into());
        sheet.set_cell_value(0, 1, "Product".into());
        sheet.set_cell_value(0, 2, "Amount".into());
        sheet.set_cell_value(0, 3, "Quantity".into());

        // Data
        sheet.set_cell_value(1, 0, "North".into());
        sheet.set_cell_value(1, 1, "Widget".into());
        sheet.set_cell_value(1, 2, "100".into());
        sheet.set_cell_value(1, 3, "5".into());

        sheet.set_cell_value(2, 0, "North".into());
        sheet.set_cell_value(2, 1, "Gadget".into());
        sheet.set_cell_value(2, 2, "200".into());
        sheet.set_cell_value(2, 3, "10".into());

        sheet.set_cell_value(3, 0, "South".into());
        sheet.set_cell_value(3, 1, "Widget".into());
        sheet.set_cell_value(3, 2, "150".into());
        sheet.set_cell_value(3, 3, "7".into());

        sheet.set_cell_value(4, 0, "South".into());
        sheet.set_cell_value(4, 1, "Gadget".into());
        sheet.set_cell_value(4, 2, "300".into());
        sheet.set_cell_value(4, 3, "15".into());

        sheet
    }

    #[test]
    fn test_aggregation_parse() {
        assert_eq!(Aggregation::parse("sum"), Some(Aggregation::Sum));
        assert_eq!(Aggregation::parse("AVERAGE"), Some(Aggregation::Average));
        assert_eq!(Aggregation::parse("xyz"), None);
    }

    #[test]
    fn test_aggregate_sum() {
        let vals = vec![1.0, 2.0, 3.0];
        assert_eq!(aggregate(&vals, &Aggregation::Sum), 6.0);
    }

    #[test]
    fn test_aggregate_average() {
        let vals = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(aggregate(&vals, &Aggregation::Average), 2.5);
    }

    #[test]
    fn test_aggregate_min_max() {
        let vals = vec![3.0, 1.0, 4.0, 1.0, 5.0];
        assert_eq!(aggregate(&vals, &Aggregation::Min), 1.0);
        assert_eq!(aggregate(&vals, &Aggregation::Max), 5.0);
    }

    #[test]
    fn test_aggregate_product() {
        let vals = vec![2.0, 3.0, 4.0];
        assert_eq!(aggregate(&vals, &Aggregation::Product), 24.0);
    }

    #[test]
    fn test_pivot_simple_sum() {
        let sheet = make_test_sheet();
        let config = PivotConfig {
            source_sheet: 0,
            data_range: (0, 0, 4, 3),
            header_row: 0,
            row_fields: vec![PivotField {
                column: 0,
                label: "Region".into(),
            }],
            column_fields: vec![PivotField {
                column: 1,
                label: "Product".into(),
            }],
            value_fields: vec![PivotValueField {
                column: 2,
                label: "Amount".into(),
                aggregation: Aggregation::Sum,
            }],
            filter_field: None,
            filter_values: vec![],
        };

        let result = build_pivot(&sheet, &config).unwrap();
        assert_eq!(result.row_headers.len(), 2);
        assert!(result.row_headers.contains(&"North".to_string()));
        assert!(result.row_headers.contains(&"South".to_string()));
        assert_eq!(result.column_headers.len(), 2);
        assert_eq!(result.grand_totals_row[0], 750.0);
    }

    #[test]
    fn test_pivot_with_filter() {
        let sheet = make_test_sheet();
        let config = PivotConfig {
            source_sheet: 0,
            data_range: (0, 0, 4, 3),
            header_row: 0,
            row_fields: vec![PivotField {
                column: 0,
                label: "Region".into(),
            }],
            column_fields: vec![],
            value_fields: vec![PivotValueField {
                column: 2,
                label: "Amount".into(),
                aggregation: Aggregation::Sum,
            }],
            filter_field: Some(PivotField {
                column: 1,
                label: "Product".into(),
            }),
            filter_values: vec!["Widget".into()],
        };

        let result = build_pivot(&sheet, &config).unwrap();
        assert_eq!(result.row_headers.len(), 2);
        // North Widget = 100, South Widget = 150
        assert_eq!(result.grand_totals_row[0], 250.0);
    }

    #[test]
    fn test_pivot_no_row_fields() {
        let sheet = make_test_sheet();
        let config = PivotConfig {
            source_sheet: 0,
            data_range: (0, 0, 4, 3),
            header_row: 0,
            row_fields: vec![],
            column_fields: vec![],
            value_fields: vec![PivotValueField {
                column: 2,
                label: "Amount".into(),
                aggregation: Aggregation::Sum,
            }],
            filter_field: None,
            filter_values: vec![],
        };

        let result = build_pivot(&sheet, &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_pivot_multiple_value_fields() {
        let sheet = make_test_sheet();
        let config = PivotConfig {
            source_sheet: 0,
            data_range: (0, 0, 4, 3),
            header_row: 0,
            row_fields: vec![PivotField {
                column: 0,
                label: "Region".into(),
            }],
            column_fields: vec![PivotField {
                column: 1,
                label: "Product".into(),
            }],
            value_fields: vec![
                PivotValueField {
                    column: 2,
                    label: "Amount".into(),
                    aggregation: Aggregation::Sum,
                },
                PivotValueField {
                    column: 3,
                    label: "Quantity".into(),
                    aggregation: Aggregation::Sum,
                },
            ],
            filter_field: None,
            filter_values: vec![],
        };

        let result = build_pivot(&sheet, &config).unwrap();
        assert_eq!(result.value_field_labels.len(), 2);
        assert_eq!(result.grand_totals_row.len(), 2);
    }

    #[test]
    fn test_pivot_write_to_sheet() {
        let sheet = make_test_sheet();
        let config = PivotConfig {
            source_sheet: 0,
            data_range: (0, 0, 4, 3),
            header_row: 0,
            row_fields: vec![PivotField {
                column: 0,
                label: "Region".into(),
            }],
            column_fields: vec![PivotField {
                column: 1,
                label: "Product".into(),
            }],
            value_fields: vec![PivotValueField {
                column: 2,
                label: "Amount".into(),
                aggregation: Aggregation::Sum,
            }],
            filter_field: None,
            filter_values: vec![],
        };

        let result = build_pivot(&sheet, &config).unwrap();
        let mut wb = Workbook::new();
        let pivot_sheet = wb.sheet_mut(0).unwrap();
        write_pivot_to_sheet(pivot_sheet, &result, 0, 0);
        assert!(pivot_sheet.cell(0, 1).is_some());
        assert!(pivot_sheet.cell(3, 0).is_some());
    }

    #[test]
    fn test_auto_detect_columns() {
        let sheet = make_test_sheet();
        let cols = auto_detect_columns(&sheet, 0, 0, 3);
        assert_eq!(cols.len(), 4);
        assert_eq!(cols[0].1, "Region");
        assert_eq!(cols[2].1, "Amount");
    }
}
