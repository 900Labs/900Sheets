use serde::{Deserialize, Serialize};
use thiserror::Error;

/// An Excel table object. Tables map a named, structured range to a worksheet
/// region, carrying column definitions, an optional totals row, and style
/// information. 900Sheets preserves this subset for XLSX round trips without
/// yet evaluating structured references in its formula engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    /// Table name. Equal to the OOXML `name` attribute.
    pub name: String,
    /// Display name used by Excel in structured references. Equal to the OOXML
    /// `displayName` attribute. Falls back to `name` when omitted.
    pub display_name: String,
    /// Worksheet region the table covers: `(start_row, start_col, end_row, end_col)`.
    pub range: (u32, u32, u32, u32),
    /// Number of header rows. Defaults to 1. Excel reserves 0 for tables
    /// authored without a header row.
    pub header_row_count: u32,
    /// Whether the totals row is visible.
    pub totals_row_shown: bool,
    /// Column definitions, ordered left to right.
    pub columns: Vec<TableColumn>,
    /// Built-in or custom table style.
    pub style: TableStyleInfo,
    /// Active filter range. Excel stores this as an `autoFilter` child whose
    /// range matches or is a subset of `range`.
    pub auto_filter_range: Option<(u32, u32, u32, u32)>,
}

impl Table {
    /// Create a table covering `range` with no columns and the default style.
    pub fn new(name: impl Into<String>, range: (u32, u32, u32, u32)) -> Self {
        let name = name.into();
        Self {
            display_name: name.clone(),
            name,
            range,
            header_row_count: 1,
            totals_row_shown: false,
            columns: Vec::new(),
            style: TableStyleInfo::default(),
            auto_filter_range: Some(range),
        }
    }

    /// Validate that the table is internally consistent and within workbook bounds.
    pub fn validate(&self, max_rows: u32, max_cols: u32) -> Result<(), TableError> {
        let (start_row, start_col, end_row, end_col) = self.range;
        if start_row > end_row || start_col > end_col {
            return Err(TableError::InvalidRange(self.range));
        }
        if end_row >= max_rows || end_col >= max_cols {
            return Err(TableError::OutOfRange);
        }
        if self.name.is_empty() {
            return Err(TableError::MissingName);
        }
        if self.display_name.is_empty() {
            return Err(TableError::MissingName);
        }
        let spanned_cols = end_col.saturating_sub(start_col) + 1;
        if !self.columns.is_empty() && self.columns.len() as u32 != spanned_cols {
            return Err(TableError::ColumnCountMismatch {
                columns: self.columns.len(),
                spanned: spanned_cols,
            });
        }
        let data_rows = end_row.saturating_sub(start_row) + 1;
        let header = self.header_row_count;
        let overhead = header + if self.totals_row_shown { 1 } else { 0 };
        if overhead > data_rows {
            return Err(TableError::HeaderTotalsExceedRange);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableColumn {
    /// Stable column identifier within the table. Equal to the OOXML `id`.
    pub id: u32,
    /// Column header text. Equal to the OOXML `name`.
    pub name: String,
    /// Aggregate applied in the totals row, if any.
    pub totals_row_function: Option<TotalsRowFunction>,
    /// Free-text label shown in the totals row when no function is set.
    pub totals_row_label: Option<String>,
}

/// The totals-row functions that round-trip through OOXML.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TotalsRowFunction {
    Sum,
    Average,
    Count,
    CountNumbers,
    Max,
    Min,
    StdDev,
    Var,
    Custom,
}

impl TotalsRowFunction {
    /// Map to the OOXML `totalsRowFunction` attribute value.
    pub fn as_xml(&self) -> &'static str {
        match self {
            Self::Sum => "sum",
            Self::Average => "average",
            Self::Count => "count",
            Self::CountNumbers => "countNums",
            Self::Max => "max",
            Self::Min => "min",
            Self::StdDev => "stdDev",
            Self::Var => "var",
            Self::Custom => "custom",
        }
    }

    /// Parse an OOXML `totalsRowFunction` attribute value.
    pub fn from_xml(value: &str) -> Option<Self> {
        Some(match value {
            "sum" => Self::Sum,
            "average" => Self::Average,
            "count" => Self::Count,
            "countNums" => Self::CountNumbers,
            "max" => Self::Max,
            "min" => Self::Min,
            "stdDev" => Self::StdDev,
            "var" => Self::Var,
            "custom" => Self::Custom,
            _ => return None,
        })
    }
}

/// Built-in or custom table style, mirroring the OOXML `tableStyleInfo` child.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableStyleInfo {
    /// Built-in style name, such as `TableStyleMedium2`.
    pub name: Option<String>,
    pub show_first_column: bool,
    pub show_last_column: bool,
    pub show_row_stripes: bool,
    pub show_column_stripes: bool,
}

impl Default for TableStyleInfo {
    fn default() -> Self {
        // Excel's default for an inserted table is row stripes on.
        Self {
            name: Some("TableStyleMedium2".to_string()),
            show_first_column: false,
            show_last_column: false,
            show_row_stripes: true,
            show_column_stripes: false,
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum TableError {
    #[error("table range {0:?} is not normalized")]
    InvalidRange((u32, u32, u32, u32)),
    #[error("table range is outside workbook bounds")]
    OutOfRange,
    #[error("table name is required")]
    MissingName,
    #[error("column count {columns} does not match the {spanned} spanned columns")]
    ColumnCountMismatch { columns: usize, spanned: u32 },
    #[error("header and totals rows exceed the table range")]
    HeaderTotalsExceedRange,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range() -> (u32, u32, u32, u32) {
        // A1:C3
        (0, 0, 2, 2)
    }

    #[test]
    fn new_table_defaults_to_matching_auto_filter_and_stripe_style() {
        let table = Table::new("Sales", range());
        assert_eq!(table.name, "Sales");
        assert_eq!(table.display_name, "Sales");
        assert_eq!(table.auto_filter_range, Some(range()));
        assert!(table.style.show_row_stripes);
        assert_eq!(table.style.name.as_deref(), Some("TableStyleMedium2"));
    }

    #[test]
    fn validate_accepts_a_well_formed_table_with_columns() {
        let mut table = Table::new("Sales", range());
        table.columns = vec![
            TableColumn {
                id: 1,
                name: "Region".into(),
                totals_row_function: None,
                totals_row_label: None,
            },
            TableColumn {
                id: 2,
                name: "Item".into(),
                totals_row_function: None,
                totals_row_label: None,
            },
            TableColumn {
                id: 3,
                name: "Total".into(),
                totals_row_function: Some(TotalsRowFunction::Sum),
                totals_row_label: None,
            },
        ];
        table.totals_row_shown = true;
        assert!(table.validate(1_000_000, 16_384).is_ok());
    }

    #[test]
    fn validate_rejects_unnormalized_range() {
        let table = Table::new("Sales", (5, 5, 0, 0));
        assert_eq!(
            table.validate(1_000_000, 16_384),
            Err(TableError::InvalidRange((5, 5, 0, 0)))
        );
    }

    #[test]
    fn validate_rejects_out_of_bounds_range() {
        let table = Table::new("Sales", (0, 0, 0, 16_384));
        assert_eq!(
            table.validate(1_000_000, 16_384),
            Err(TableError::OutOfRange)
        );
    }

    #[test]
    fn validate_rejects_missing_name() {
        let mut table = Table::new("Sales", range());
        table.name.clear();
        assert_eq!(
            table.validate(1_000_000, 16_384),
            Err(TableError::MissingName)
        );
    }

    #[test]
    fn validate_rejects_column_count_mismatch() {
        let mut table = Table::new("Sales", range());
        table.columns = vec![TableColumn {
            id: 1,
            name: "Only".into(),
            totals_row_function: None,
            totals_row_label: None,
        }];
        assert!(matches!(
            table.validate(1_000_000, 16_384),
            Err(TableError::ColumnCountMismatch {
                columns: 1,
                spanned: 3
            })
        ));
    }

    #[test]
    fn totals_row_function_round_trips_through_xml() {
        for function in [
            TotalsRowFunction::Sum,
            TotalsRowFunction::Average,
            TotalsRowFunction::Count,
            TotalsRowFunction::CountNumbers,
            TotalsRowFunction::Max,
            TotalsRowFunction::Min,
            TotalsRowFunction::StdDev,
            TotalsRowFunction::Var,
            TotalsRowFunction::Custom,
        ] {
            assert_eq!(
                TotalsRowFunction::from_xml(function.as_xml()),
                Some(function)
            );
        }
        assert!(TotalsRowFunction::from_xml("nope").is_none());
    }

    #[test]
    fn header_and_totals_overhead_is_checked_against_range() {
        // A single-row table cannot hold a header plus a totals row.
        let mut table = Table::new("Sales", (0, 0, 0, 0));
        table.totals_row_shown = true;
        assert_eq!(
            table.validate(1_000_000, 16_384),
            Err(TableError::HeaderTotalsExceedRange)
        );
    }
}
