use thiserror::Error;

#[derive(Debug, Error)]
pub enum JsonError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("Invalid JSON structure: {0}")]
    InvalidStructure(String),
    #[error("JSON input too large: {0} bytes (max {1})")]
    FileTooLarge(usize, usize),
    #[error("Too many cells: {0} (max {1})")]
    TooManyCells(usize, usize),
    #[error("JSON nesting too deep: {0} levels (max {1})")]
    TooDeep(usize, usize),
}
