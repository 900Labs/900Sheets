use thiserror::Error;

#[derive(Debug, Error)]
pub enum CsvError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid CSV: {0}")]
    InvalidFormat(String),
    #[error("Encoding error: {0}")]
    Encoding(String),
    #[error("CSV data too large: {0} bytes (max {1})")]
    FileTooLarge(usize, usize),
}
