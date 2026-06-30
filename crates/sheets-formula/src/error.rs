use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum FormulaError {
    ParseError(String),
    EvalError(String),
    CircularReference,
    UnknownFunction(String),
    DivisionByZero,
    TypeMismatch { expected: String, actual: String },
    InvalidArguments(String),
    RefError(String),
    NumError(String),
    ValueError(String),
    NotAvailable(String),
}

impl fmt::Display for FormulaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormulaError::ParseError(msg) => write!(f, "#ERROR!: {}", msg),
            FormulaError::EvalError(msg) => write!(f, "#ERROR!: {}", msg),
            FormulaError::CircularReference => write!(f, "#REF!: Circular reference"),
            FormulaError::UnknownFunction(name) => write!(f, "#NAME?: {}", name),
            FormulaError::DivisionByZero => write!(f, "#DIV/0!"),
            FormulaError::TypeMismatch { expected, actual } => {
                write!(f, "#VALUE!: expected {}, got {}", expected, actual)
            }
            FormulaError::InvalidArguments(msg) => write!(f, "#VALUE!: {}", msg),
            FormulaError::RefError(msg) => write!(f, "#REF!: {}", msg),
            FormulaError::NumError(msg) => write!(f, "#NUM!: {}", msg),
            FormulaError::ValueError(msg) => write!(f, "#VALUE!: {}", msg),
            FormulaError::NotAvailable(msg) => write!(f, "#N/A: {}", msg),
        }
    }
}

impl std::error::Error for FormulaError {}

impl From<std::num::ParseFloatError> for FormulaError {
    fn from(e: std::num::ParseFloatError) -> Self {
        FormulaError::ValueError(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        assert_eq!(FormulaError::DivisionByZero.to_string(), "#DIV/0!");
        assert_eq!(
            FormulaError::UnknownFunction("FOO".into()).to_string(),
            "#NAME?: FOO"
        );
    }
}
