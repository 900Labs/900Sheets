use thiserror::Error;

#[derive(Debug, Error)]
pub enum XlsxError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("XML parse error: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("Invalid XLSX: {0}")]
    InvalidFormat(String),
    #[error("File too large: {0} bytes (max {1})")]
    FileTooLarge(u64, u64),
    #[error("Too many cells: {0} (max {1})")]
    TooManyCells(usize, usize),
}
