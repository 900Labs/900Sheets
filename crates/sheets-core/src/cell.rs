use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CellType {
    Number,
    Text,
    Boolean,
    Error,
    Empty,
    Formula,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellValue {
    #[serde(rename = "type")]
    pub cell_type: CellType,
    pub raw: String,
    pub display: String,
}

impl CellValue {
    pub fn empty() -> Self {
        Self {
            cell_type: CellType::Empty,
            raw: String::new(),
            display: String::new(),
        }
    }

    pub fn text(value: impl Into<String>) -> Self {
        let raw = value.into();
        let display = raw.clone();
        Self {
            cell_type: CellType::Text,
            raw,
            display,
        }
    }

    pub fn number(value: f64) -> Self {
        if !value.is_finite() {
            return Self::error("#NUM!: non-finite number");
        }
        let raw = value.to_string();
        let display = format_number(value);
        Self {
            cell_type: CellType::Number,
            raw,
            display,
        }
    }

    pub fn boolean(value: bool) -> Self {
        Self {
            cell_type: CellType::Boolean,
            raw: value.to_string(),
            display: if value { "TRUE".into() } else { "FALSE".into() },
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        let msg = message.into();
        Self {
            cell_type: CellType::Error,
            raw: msg.clone(),
            display: msg,
        }
    }

    pub fn formula(expression: impl Into<String>) -> Self {
        let raw = expression.into();
        Self {
            cell_type: CellType::Formula,
            raw,
            display: String::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.cell_type == CellType::Empty
    }

    pub fn is_formula(&self) -> bool {
        self.cell_type == CellType::Formula
    }

    pub fn as_number(&self) -> Option<f64> {
        match &self.cell_type {
            CellType::Number => self.raw.parse().ok(),
            CellType::Text => self.raw.parse().ok(),
            CellType::Boolean => Some(if self.raw == "true" { 1.0 } else { 0.0 }),
            _ => None,
        }
    }
}

impl Default for CellValue {
    fn default() -> Self {
        Self::empty()
    }
}

fn format_number(value: f64) -> String {
    if !value.is_finite() {
        return "#NUM!".into();
    }
    if value == value.trunc() && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{}", value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let cell = CellValue::empty();
        assert!(cell.is_empty());
        assert_eq!(cell.raw, "");
    }

    #[test]
    fn test_text() {
        let cell = CellValue::text("Hello");
        assert_eq!(cell.cell_type, CellType::Text);
        assert_eq!(cell.raw, "Hello");
        assert_eq!(cell.display, "Hello");
    }

    #[test]
    fn test_number() {
        let cell = CellValue::number(42.0);
        assert_eq!(cell.cell_type, CellType::Number);
        assert_eq!(cell.display, "42");
    }

    #[test]
    fn test_boolean() {
        let cell = CellValue::boolean(true);
        assert_eq!(cell.cell_type, CellType::Boolean);
        assert_eq!(cell.display, "TRUE");
    }

    #[test]
    fn test_as_number() {
        assert_eq!(CellValue::number(42.0).as_number(), Some(42.0));
        assert_eq!(CellValue::text("3.14").as_number(), Some(314.0 / 100.0));
        assert_eq!(CellValue::text("abc").as_number(), None);
    }

    #[test]
    fn test_non_finite_number_becomes_error() {
        let cell = CellValue::number(f64::NAN);
        assert_eq!(cell.cell_type, CellType::Error);
    }
}
