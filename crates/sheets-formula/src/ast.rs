use crate::error::FormulaError;

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
        match self {
            Expr::CellRef { col, row, .. } => vec![(*row, *col)],
            Expr::RangeRef {
                start_col,
                start_row,
                end_col,
                end_row,
                ..
            } => {
                let mut refs = Vec::new();
                for r in *start_row..=*end_row {
                    for c in *start_col..=*end_col {
                        refs.push((r, c));
                    }
                }
                refs
            }
            Expr::BinOp { left, right, .. } => {
                let mut refs = left.references();
                refs.extend(right.references());
                refs
            }
            Expr::UnaryOp { operand, .. } => operand.references(),
            Expr::Function { args, .. } => {
                let mut refs = Vec::new();
                for arg in args {
                    refs.extend(arg.references());
                }
                refs
            }
            _ => Vec::new(),
        }
    }
}

pub fn parse_cell_ref(s: &str) -> Option<(Option<String>, u32, u32, bool, bool)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    let (sheet, rest) = if let Some(pos) = s.find('!') {
        (Some(s[..pos].to_string()), &s[pos + 1..])
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
    if row == 0 {
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
}
