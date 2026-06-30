use crate::error::CoreError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CellAddress {
    pub sheet: Option<String>,
    pub row: u32,
    pub col: u32,
    pub abs_row: bool,
    pub abs_col: bool,
}

impl CellAddress {
    pub fn new(row: u32, col: u32) -> Self {
        Self {
            sheet: None,
            row,
            col,
            abs_row: false,
            abs_col: false,
        }
    }

    pub fn with_sheet(mut self, sheet: impl Into<String>) -> Self {
        self.sheet = Some(sheet.into());
        self
    }
}

impl std::fmt::Display for CellAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref s) = self.sheet {
            write!(f, "{}!", s)?;
        }
        if self.abs_col {
            write!(f, "$")?;
        }
        write!(f, "{}", col_to_label(self.col))?;
        if self.abs_row {
            write!(f, "$")?;
        }
        write!(f, "{}", self.row + 1)
    }
}

pub fn col_to_label(col: u32) -> String {
    let mut label = String::new();
    let mut c = col;
    loop {
        label.insert(0, char::from_u32(65 + (c % 26)).unwrap_or('A'));
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    label
}

pub fn label_to_col(label: &str) -> Result<u32, CoreError> {
    let label = label.to_uppercase();
    if label.is_empty() || !label.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(CoreError::AddressParseError(label));
    }
    let mut col: u32 = 0;
    for ch in label.chars() {
        let val = (ch as u32) - 64;
        col = col * 26 + val;
    }
    Ok(col - 1)
}

pub fn cell_address_to_index(addr: &str) -> Result<(u32, u32), CoreError> {
    let addr = addr.trim();
    if addr.is_empty() {
        return Err(CoreError::AddressParseError(addr.to_string()));
    }

    let mut col_part = String::new();
    let mut row_part = String::new();

    for ch in addr.chars() {
        if ch.is_ascii_alphabetic() {
            if !row_part.is_empty() {
                return Err(CoreError::AddressParseError(addr.to_string()));
            }
            col_part.push(ch);
        } else if ch.is_ascii_digit() {
            row_part.push(ch);
        } else {
            return Err(CoreError::AddressParseError(addr.to_string()));
        }
    }

    if col_part.is_empty() || row_part.is_empty() {
        return Err(CoreError::AddressParseError(addr.to_string()));
    }

    let col = label_to_col(&col_part)?;
    let row: u32 = row_part
        .parse()
        .map_err(|_| CoreError::AddressParseError(addr.to_string()))?;

    if row == 0 {
        return Err(CoreError::AddressParseError(addr.to_string()));
    }

    Ok((row - 1, col))
}

pub fn index_to_cell_address(row: u32, col: u32) -> String {
    format!("{}{}", col_to_label(col), row + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_col_to_label() {
        assert_eq!(col_to_label(0), "A");
        assert_eq!(col_to_label(25), "Z");
        assert_eq!(col_to_label(26), "AA");
        assert_eq!(col_to_label(27), "AB");
        assert_eq!(col_to_label(51), "AZ");
        assert_eq!(col_to_label(52), "BA");
    }

    #[test]
    fn test_label_to_col() {
        assert_eq!(label_to_col("A").unwrap(), 0);
        assert_eq!(label_to_col("Z").unwrap(), 25);
        assert_eq!(label_to_col("AA").unwrap(), 26);
        assert_eq!(label_to_col("AB").unwrap(), 27);
    }

    #[test]
    fn test_cell_address_roundtrip() {
        for (row, col) in [(0, 0), (0, 25), (99, 0), (0, 26), (999, 51)] {
            let addr = index_to_cell_address(row, col);
            let (r, c) = cell_address_to_index(&addr).unwrap();
            assert_eq!(r, row);
            assert_eq!(c, col);
        }
    }

    #[test]
    fn test_cell_address_display() {
        let addr = CellAddress::new(0, 0);
        assert_eq!(addr.to_string(), "A1");

        let addr = CellAddress::new(9, 2);
        assert_eq!(addr.to_string(), "C10");
    }
}
