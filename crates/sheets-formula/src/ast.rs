use crate::error::FormulaError;

pub const MAX_ROWS: u32 = 1_000_000;
pub const MAX_COLS: u32 = 16_384;
pub const MAX_EXPANDED_REFERENCES: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QualifiedCellRef {
    pub sheet: Option<String>,
    pub row: u32,
    pub col: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    String(String),
    Boolean(bool),
    CellRef {
        sheet: Option<String>,
        col: u32,
        row: u32,
        abs_col: bool,
        abs_row: bool,
    },
    RangeRef {
        sheet: Option<String>,
        start_col: u32,
        start_row: u32,
        end_col: u32,
        end_row: u32,
    },
    BinOp {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    UnaryOp {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Function {
        name: String,
        args: Vec<Expr>,
    },
    Error(FormulaError),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Mod,
    Concat,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp {
    Neg,
    Percent,
}

impl Expr {
    pub fn references(&self) -> Vec<(u32, u32)> {
        self.try_references(MAX_EXPANDED_REFERENCES)
            .unwrap_or_default()
    }

    pub fn try_references(&self, max_references: usize) -> Result<Vec<(u32, u32)>, FormulaError> {
        let mut refs = Vec::new();
        self.for_each_qualified_reference(max_references, &mut |reference| {
            refs.push((reference.row, reference.col));
        })?;
        Ok(refs)
    }

    pub fn for_each_qualified_reference<F>(
        &self,
        max_references: usize,
        visitor: &mut F,
    ) -> Result<(), FormulaError>
    where
        F: FnMut(QualifiedCellRef),
    {
        self.visit_qualified_references(max_references, &mut 0, visitor)
    }

    fn visit_qualified_references<F>(
        &self,
        max_references: usize,
        visited: &mut usize,
        visitor: &mut F,
    ) -> Result<(), FormulaError>
    where
        F: FnMut(QualifiedCellRef),
    {
        match self {
            Expr::CellRef {
                sheet, col, row, ..
            } => {
                *visited = visited.saturating_add(1);
                if *visited > max_references {
                    return Err(reference_budget_error(max_references));
                }
                visitor(QualifiedCellRef {
                    sheet: sheet.clone(),
                    row: *row,
                    col: *col,
                });
            }
            Expr::RangeRef {
                sheet,
                start_col,
                start_row,
                end_col,
                end_row,
            } => {
                let count = u64::from(end_row - start_row + 1)
                    .saturating_mul(u64::from(end_col - start_col + 1));
                let remaining = max_references.saturating_sub(*visited) as u64;
                if count > remaining {
                    return Err(reference_budget_error(max_references));
                }
                *visited += count as usize;
                for row in *start_row..=*end_row {
                    for col in *start_col..=*end_col {
                        visitor(QualifiedCellRef {
                            sheet: sheet.clone(),
                            row,
                            col,
                        });
                    }
                }
            }
            Expr::BinOp { left, right, .. } => {
                left.visit_qualified_references(max_references, visited, visitor)?;
                right.visit_qualified_references(max_references, visited, visitor)?;
            }
            Expr::UnaryOp { operand, .. } => {
                operand.visit_qualified_references(max_references, visited, visitor)?;
            }
            Expr::Function { args, .. } => {
                for arg in args {
                    arg.visit_qualified_references(max_references, visited, visitor)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn reference_budget_error(max_references: usize) -> FormulaError {
    FormulaError::RefError(format!(
        "Range exceeds safe reference limit of {max_references} cells"
    ))
}

pub fn parse_cell_ref(s: &str) -> Option<(Option<String>, u32, u32, bool, bool)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let (sheet, rest) = if let Some(pos) = s.rfind('!') {
        let raw_sheet = &s[..pos];
        let sheet = if raw_sheet.starts_with('\'') && raw_sheet.ends_with('\'') {
            if raw_sheet.len() < 2 {
                return None;
            }
            raw_sheet[1..raw_sheet.len() - 1].replace("''", "'")
        } else {
            raw_sheet.to_string()
        };
        if sheet.is_empty() {
            return None;
        }
        (Some(sheet), &s[pos + 1..])
    } else {
        (None, s)
    };

    let abs_col = rest.starts_with('$');
    let rest = rest.trim_start_matches('$');

    let mut col_part = String::new();
    let mut row_part = String::new();

    for ch in rest.chars() {
        if ch.is_ascii_alphabetic() {
            col_part.push(ch.to_ascii_uppercase());
        } else if ch.is_ascii_digit() {
            row_part.push(ch);
        } else if ch == '$' {
            // absolute row marker
        } else {
            return None;
        }
    }

    if col_part.is_empty() || row_part.is_empty() {
        return None;
    }

    let col = col_label_to_index(&col_part)?;
    let row: u32 = row_part.parse().ok()?;
    if row == 0 || row > MAX_ROWS || col >= MAX_COLS {
        return None;
    }

    let abs_row = rest.contains('$') && !row_part.is_empty();

    Some((sheet, col, row - 1, abs_col, abs_row))
}

fn col_label_to_index(label: &str) -> Option<u32> {
    let label = label.to_uppercase();
    if label.is_empty() || !label.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let mut col: u32 = 0;
    for ch in label.chars() {
        let val = (ch as u32) - 64;
        col = col * 26 + val;
    }
    Some(col - 1)
}

pub fn parse_range_ref(s: &str) -> Option<(Option<String>, u32, u32, u32, u32)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return None;
    }

    let (sheet1, col1, row1, _, _) = parse_cell_ref(parts[0])?;
    let (sheet2, col2, row2, _, _) = parse_cell_ref(parts[1])?;

    if sheet1.is_some() && sheet2.is_some() && sheet1 != sheet2 {
        return None;
    }

    Some((
        sheet1.or(sheet2),
        col1.min(col2),
        row1.min(row2),
        col1.max(col2),
        row1.max(row2),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cell_ref() {
        let (_, col, row, _, _) = parse_cell_ref("A1").unwrap();
        assert_eq!(col, 0);
        assert_eq!(row, 0);

        let (_, col, row, _, _) = parse_cell_ref("Z10").unwrap();
        assert_eq!(col, 25);
        assert_eq!(row, 9);

        let (_, col, row, _, _) = parse_cell_ref("AA1").unwrap();
        assert_eq!(col, 26);
        assert_eq!(row, 0);
    }

    #[test]
    fn test_parse_range_ref() {
        let (_, sc, sr, ec, er) = parse_range_ref("A1:B5").unwrap();
        assert_eq!(sc, 0);
        assert_eq!(sr, 0);
        assert_eq!(ec, 1);
        assert_eq!(er, 4);
    }

    #[test]
    fn test_parse_quoted_and_bounded_refs() {
        let (sheet, col, row, abs_col, abs_row) =
            parse_cell_ref("'Sam''s Data'!$XFD$1000000").unwrap();
        assert_eq!(sheet.as_deref(), Some("Sam's Data"));
        assert_eq!((col, row), (16_383, 999_999));
        assert!(abs_col && abs_row);
        assert!(parse_cell_ref("XFE1").is_none());
        assert!(parse_cell_ref("A1000001").is_none());
        assert!(parse_range_ref("One!A1:Two!A2").is_none());
    }

    #[test]
    fn test_references() {
        let expr = Expr::CellRef {
            sheet: None,
            col: 0,
            row: 0,
            abs_col: false,
            abs_row: false,
        };
        assert_eq!(expr.references(), vec![(0, 0)]);
    }

    #[test]
    fn huge_reference_expansion_returns_explicit_budget_error() {
        let expr = crate::parser::Parser::parse_formula("A1:XFD1000000").unwrap();
        assert!(matches!(
            expr.try_references(MAX_EXPANDED_REFERENCES),
            Err(FormulaError::RefError(_))
        ));
    }
}
