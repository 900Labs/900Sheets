use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Sheet index {0} out of bounds")]
    SheetOutOfBounds(usize),
    #[error("Cell address parse error: {0}")]
    AddressParseError(String),
    #[error("Row index {row} out of bounds (max {max_rows})")]
    RowOutOfBounds { row: u32, max_rows: u32 },
    #[error("Column index {col} out of bounds (max {max_cols})")]
    ColOutOfBounds { col: u32, max_cols: u32 },
    #[error("Sheet name cannot be empty")]
    InvalidSheetName,
    #[error("Sheet name already exists: {0}")]
    DuplicateSheetName(String),
    #[error("Stable sheet IDs must be nonzero and unique")]
    InvalidStableSheetIds,
}
