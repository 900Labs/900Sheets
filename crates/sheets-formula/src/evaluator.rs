use crate::ast::*;
use crate::error::FormulaError;
use crate::functions::FunctionRegistry;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    String(String),
    Boolean(bool),
    Error(FormulaError),
    Empty,
    Array(Vec<Value>),
}

impl Value {
    pub fn as_number(&self) -> Result<f64, FormulaError> {
        match self {
            Value::Number(n) if n.is_finite() => Ok(*n),
            Value::Number(_) => Err(FormulaError::NumError("non-finite number".into())),
            Value::Boolean(true) => Ok(1.0),
            Value::Boolean(false) => Ok(0.0),
            Value::String(s) => {
                if s.is_empty() {
                    Ok(0.0)
                } else {
                    s.parse::<f64>().map_err(|_| FormulaError::TypeMismatch {
                        expected: "number".into(),
                        actual: format!("string \"{}\"", s),
                    })
                }
            }
            Value::Empty => Ok(0.0),
            Value::Error(e) => Err(e.clone()),
            Value::Array(arr) => {
                if let Some(first) = arr.first() {
                    first.as_number()
                } else {
                    Ok(0.0)
                }
            }
        }
    }

    pub fn as_string(&self) -> Result<String, FormulaError> {
        match self {
            Value::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    Ok(format!("{}", *n as i64))
                } else {
                    Ok(format!("{}", n))
                }
            }
            Value::String(s) => Ok(s.clone()),
            Value::Boolean(true) => Ok("TRUE".into()),
            Value::Boolean(false) => Ok("FALSE".into()),
            Value::Empty => Ok(String::new()),
            Value::Error(e) => Err(e.clone()),
            Value::Array(arr) => {
                if let Some(first) = arr.first() {
                    first.as_string()
                } else {
                    Ok(String::new())
                }
            }
        }
    }

    pub fn as_boolean(&self) -> Result<bool, FormulaError> {
        match self {
            Value::Number(n) => Ok(*n != 0.0),
            Value::Boolean(b) => Ok(*b),
            Value::String(s) => {
                let upper = s.to_uppercase();
                if upper == "TRUE" {
                    Ok(true)
                } else if upper == "FALSE" {
                    Ok(false)
                } else {
                    Err(FormulaError::TypeMismatch {
                        expected: "boolean".into(),
                        actual: format!("string \"{}\"", s),
                    })
                }
            }
            Value::Empty => Ok(false),
            Value::Error(e) => Err(e.clone()),
            Value::Array(arr) => {
                if let Some(first) = arr.first() {
                    first.as_boolean()
                } else {
                    Ok(false)
                }
            }
        }
    }

    pub fn is_error(&self) -> bool {
        match self {
            Value::Error(_) => true,
            Value::Array(arr) => arr.iter().any(|v| v.is_error()),
            _ => false,
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Value::Empty => true,
            Value::Array(arr) => arr.iter().all(|v| v.is_empty()),
            _ => false,
        }
    }

    pub fn to_display(&self) -> String {
        match self {
            Value::Number(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            Value::String(s) => s.clone(),
            Value::Boolean(b) => {
                if *b {
                    "TRUE".into()
                } else {
                    "FALSE".into()
                }
            }
            Value::Error(e) => e.to_string(),
            Value::Empty => String::new(),
            Value::Array(arr) => {
                if let Some(first) = arr.first() {
                    first.to_display()
                } else {
                    String::new()
                }
            }
        }
    }

    pub fn flatten_numbers(&self) -> Vec<f64> {
        match self {
            Value::Array(arr) => {
                let mut nums = Vec::new();
                for v in arr {
                    nums.extend(v.flatten_numbers());
                }
                nums
            }
            Value::Number(n) => vec![*n],
            Value::Boolean(b) => vec![if *b { 1.0 } else { 0.0 }],
            Value::String(s) => {
                if let Ok(n) = s.parse::<f64>() {
                    vec![n]
                } else {
                    vec![]
                }
            }
            _ => vec![],
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_display())
    }
}

pub trait CellProvider {
    fn get_cell(&self, row: u32, col: u32) -> Value;
    fn get_raw(&self, row: u32, col: u32) -> String;

    fn get_cell_on_sheet(&self, sheet: &str, row: u32, col: u32) -> Value {
        let _ = (row, col);
        Value::Error(FormulaError::RefError(format!(
            "Sheet '{sheet}' was not found"
        )))
    }

    fn get_raw_on_sheet(&self, sheet: &str, row: u32, col: u32) -> String {
        self.get_cell_on_sheet(sheet, row, col).to_display()
    }
}

pub struct SimpleProvider {
    cells: std::collections::HashMap<(u32, u32), Value>,
    sheets: std::collections::HashMap<String, std::collections::HashMap<(u32, u32), Value>>,
}

impl Default for SimpleProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SimpleProvider {
    pub fn new() -> Self {
        Self {
            cells: std::collections::HashMap::new(),
            sheets: std::collections::HashMap::new(),
        }
    }

    pub fn set(&mut self, row: u32, col: u32, value: Value) {
        self.cells.insert((row, col), value);
    }

    pub fn set_number(&mut self, row: u32, col: u32, n: f64) {
        self.cells.insert((row, col), Value::Number(n));
    }

    pub fn set_string(&mut self, row: u32, col: u32, s: &str) {
        self.cells.insert((row, col), Value::String(s.into()));
    }

    pub fn set_on_sheet(&mut self, sheet: &str, row: u32, col: u32, value: Value) {
        self.sheets
            .entry(sheet.to_ascii_lowercase())
            .or_default()
            .insert((row, col), value);
    }
}

impl CellProvider for SimpleProvider {
    fn get_cell(&self, row: u32, col: u32) -> Value {
        self.cells.get(&(row, col)).cloned().unwrap_or(Value::Empty)
    }

    fn get_raw(&self, row: u32, col: u32) -> String {
        self.cells
            .get(&(row, col))
            .map(|v| v.to_display())
            .unwrap_or_default()
    }

    fn get_cell_on_sheet(&self, sheet: &str, row: u32, col: u32) -> Value {
        let Some(cells) = self.sheets.get(&sheet.to_ascii_lowercase()) else {
            return Value::Error(FormulaError::RefError(format!(
                "Sheet '{sheet}' was not found"
            )));
        };
        cells.get(&(row, col)).cloned().unwrap_or(Value::Empty)
    }
}

pub struct Evaluator {
    registry: FunctionRegistry,
}

impl Evaluator {
    pub fn new() -> Self {
        Self {
            registry: FunctionRegistry::new(),
        }
    }

    pub fn evaluate(&self, expr: &Expr, provider: &dyn CellProvider) -> Value {
        self.eval_expr(expr, provider, &mut HashSet::new())
    }

    pub fn evaluate_with_visited(
        &self,
        expr: &Expr,
        provider: &dyn CellProvider,
        visited: &mut HashSet<(u32, u32)>,
    ) -> Value {
        self.eval_expr(expr, provider, visited)
    }

    fn eval_expr(
        &self,
        expr: &Expr,
        provider: &dyn CellProvider,
        visited: &mut HashSet<(u32, u32)>,
    ) -> Value {
        match expr {
            Expr::Number(n) => Value::Number(*n),
            Expr::String(s) => Value::String(s.clone()),
            Expr::Boolean(b) => Value::Boolean(*b),
            Expr::Error(e) => Value::Error(e.clone()),
            Expr::CellRef {
                sheet, col, row, ..
            } => {
                let key = (*row, *col);
                if visited.contains(&key) {
                    return Value::Error(FormulaError::CircularReference);
                }
                if let Some(sheet) = sheet {
                    provider.get_cell_on_sheet(sheet, *row, *col)
                } else {
                    provider.get_cell(*row, *col)
                }
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
                if count > MAX_EXPANDED_REFERENCES as u64 {
                    return Value::Error(FormulaError::RefError(format!(
                        "Range exceeds safe evaluation limit of {} cells",
                        MAX_EXPANDED_REFERENCES
                    )));
                }
                let mut values = Vec::with_capacity(count as usize);
                for r in *start_row..=*end_row {
                    for c in *start_col..=*end_col {
                        values.push(if let Some(sheet) = sheet {
                            provider.get_cell_on_sheet(sheet, r, c)
                        } else {
                            provider.get_cell(r, c)
                        });
                    }
                }
                Value::Array(values)
            }
            Expr::UnaryOp { op, operand } => {
                let val = self.eval_expr(operand, provider, visited);
                if val.is_error() {
                    return val;
                }
                match op {
                    UnaryOp::Neg => match val.as_number() {
                        Ok(n) => Value::Number(-n),
                        Err(e) => Value::Error(e),
                    },
                    UnaryOp::Percent => match val.as_number() {
                        Ok(n) => Value::Number(n / 100.0),
                        Err(e) => Value::Error(e),
                    },
                }
            }
            Expr::BinOp { op, left, right } => {
                let lv = self.eval_expr(left, provider, visited);
                let rv = self.eval_expr(right, provider, visited);
                if lv.is_error() {
                    return lv;
                }
                if rv.is_error() {
                    return rv;
                }
                self.eval_binop(*op, lv, rv)
            }
            Expr::Function { name, args } => {
                let upper = name.to_uppercase();

                // Lazy evaluation for IF: only evaluate the taken branch
                if upper == "IF" {
                    if args.is_empty() {
                        return Value::Error(FormulaError::InvalidArguments(
                            "IF requires at least 1 argument".into(),
                        ));
                    }
                    let cond = self.eval_expr(&args[0], provider, visited);
                    if cond.is_error() {
                        return cond;
                    }
                    return match cond.as_boolean() {
                        Ok(true) => {
                            if args.len() > 1 {
                                self.eval_expr(&args[1], provider, visited)
                            } else {
                                Value::Boolean(true)
                            }
                        }
                        Ok(false) => {
                            if args.len() > 2 {
                                self.eval_expr(&args[2], provider, visited)
                            } else {
                                Value::Boolean(false)
                            }
                        }
                        Err(e) => Value::Error(e),
                    };
                }

                // Lazy evaluation for IFERROR: only evaluate fallback if needed
                if upper == "IFERROR" {
                    if args.len() < 2 {
                        return Value::Error(FormulaError::InvalidArguments(
                            "IFERROR requires 2 arguments".into(),
                        ));
                    }
                    let val = self.eval_expr(&args[0], provider, visited);
                    if val.is_error() {
                        return self.eval_expr(&args[1], provider, visited);
                    } else {
                        return val;
                    }
                }

                // Lazy evaluation for IFNA: only evaluate fallback if N/A
                if upper == "IFNA" {
                    if args.len() < 2 {
                        return Value::Error(FormulaError::InvalidArguments(
                            "IFNA requires 2 arguments".into(),
                        ));
                    }
                    let val = self.eval_expr(&args[0], provider, visited);
                    if let Value::Error(FormulaError::NotAvailable(_)) = &val {
                        self.eval_expr(&args[1], provider, visited)
                    } else {
                        val
                    }
                } else {
                    // Eager evaluation for all other functions
                    let arg_values: Vec<Value> = args
                        .iter()
                        .map(|a| self.eval_expr(a, provider, visited))
                        .collect();
                    for v in &arg_values {
                        if v.is_error() {
                            return v.clone();
                        }
                    }
                    self.registry.call(
                        name,
                        &arg_values,
                        provider,
                        visited,
                        &args.iter().collect::<Vec<_>>(),
                    )
                }
            }
        }
    }

    fn eval_binop(&self, op: BinOp, lv: Value, rv: Value) -> Value {
        match op {
            BinOp::Add => arithmetic(lv, rv, |a, b| a + b),
            BinOp::Sub => arithmetic(lv, rv, |a, b| a - b),
            BinOp::Mul => arithmetic(lv, rv, |a, b| a * b),
            BinOp::Div => {
                let a = match lv.as_number() {
                    Ok(n) => n,
                    Err(e) => return Value::Error(e),
                };
                let b = match rv.as_number() {
                    Ok(n) => n,
                    Err(e) => return Value::Error(e),
                };
                if b == 0.0 {
                    Value::Error(FormulaError::DivisionByZero)
                } else {
                    checked_number(a / b, "division")
                }
            }
            BinOp::Pow => arithmetic(lv, rv, |a, b| a.powf(b)),
            BinOp::Mod => {
                let a = match lv.as_number() {
                    Ok(n) => n,
                    Err(e) => return Value::Error(e),
                };
                let b = match rv.as_number() {
                    Ok(n) => n,
                    Err(e) => return Value::Error(e),
                };
                if b == 0.0 {
                    Value::Error(FormulaError::DivisionByZero)
                } else {
                    checked_number(a % b, "modulo")
                }
            }
            BinOp::Concat => {
                let ls = match lv.as_string() {
                    Ok(s) => s,
                    Err(e) => return Value::Error(e),
                };
                let rs = match rv.as_string() {
                    Ok(s) => s,
                    Err(e) => return Value::Error(e),
                };
                Value::String(format!("{}{}", ls, rs))
            }
            BinOp::Eq => Value::Boolean(compare_eq(&lv, &rv)),
            BinOp::NotEq => Value::Boolean(!compare_eq(&lv, &rv)),
            BinOp::Lt => compare(lv, rv, |a, b| a < b),
            BinOp::LtEq => compare(lv, rv, |a, b| a <= b),
            BinOp::Gt => compare(lv, rv, |a, b| a > b),
            BinOp::GtEq => compare(lv, rv, |a, b| a >= b),
        }
    }
}

fn arithmetic(lv: Value, rv: Value, f: fn(f64, f64) -> f64) -> Value {
    let a = match lv.as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let b = match rv.as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    checked_number(f(a, b), "arithmetic operation")
}

fn checked_number(value: f64, context: &str) -> Value {
    if value.is_finite() {
        Value::Number(value)
    } else {
        Value::Error(FormulaError::NumError(format!(
            "{} produced a non-finite number",
            context
        )))
    }
}

fn compare(lv: Value, rv: Value, f: fn(f64, f64) -> bool) -> Value {
    // Try numeric comparison first
    if let (Ok(a), Ok(b)) = (lv.as_number(), rv.as_number()) {
        return Value::Boolean(f(a, b));
    }
    // Fall back to string comparison
    match (lv.as_string(), rv.as_string()) {
        (Ok(a), Ok(b)) => Value::Boolean(f_str(&a, &b, f)),
        (Err(e), _) => Value::Error(e),
        (_, Err(e)) => Value::Error(e),
    }
}

fn f_str(a: &str, b: &str, f: fn(f64, f64) -> bool) -> bool {
    let cmp = a.cmp(b);
    match cmp {
        std::cmp::Ordering::Less => f(-1.0, 0.0),
        std::cmp::Ordering::Equal => f(0.0, 0.0),
        std::cmp::Ordering::Greater => f(1.0, 0.0),
    }
}

fn compare_eq(lv: &Value, rv: &Value) -> bool {
    match (lv, rv) {
        (Value::Number(a), Value::Number(b)) => a == b,
        (Value::String(a), Value::String(b)) => a.eq_ignore_ascii_case(b),
        (Value::Boolean(a), Value::Boolean(b)) => a == b,
        (Value::Empty, Value::Empty) => true,
        (Value::Number(a), Value::Boolean(b)) => *a == if *b { 1.0 } else { 0.0 },
        (Value::Boolean(a), Value::Number(b)) => *b == if *a { 1.0 } else { 0.0 },
        (Value::Array(a), Value::Array(b)) => a == b,
        (Value::Array(a), _) => a.first().map(|v| compare_eq(v, rv)).unwrap_or(false),
        (_, Value::Array(b)) => b.first().map(|v| compare_eq(lv, v)).unwrap_or(false),
        _ => false,
    }
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    #[test]
    fn test_eval_number() {
        let evaluator = Evaluator::new();
        let provider = SimpleProvider::new();
        let expr = Parser::parse_formula("42").unwrap();
        let result = evaluator.evaluate(&expr, &provider);
        assert_eq!(result, Value::Number(42.0));
    }

    #[test]
    fn test_eval_add() {
        let evaluator = Evaluator::new();
        let provider = SimpleProvider::new();
        let expr = Parser::parse_formula("1+2").unwrap();
        let result = evaluator.evaluate(&expr, &provider);
        assert_eq!(result, Value::Number(3.0));
    }

    #[test]
    fn test_eval_precedence() {
        let evaluator = Evaluator::new();
        let provider = SimpleProvider::new();
        let expr = Parser::parse_formula("1+2*3").unwrap();
        let result = evaluator.evaluate(&expr, &provider);
        assert_eq!(result, Value::Number(7.0));
    }

    #[test]
    fn test_eval_div_zero() {
        let evaluator = Evaluator::new();
        let provider = SimpleProvider::new();
        let expr = Parser::parse_formula("1/0").unwrap();
        let result = evaluator.evaluate(&expr, &provider);
        assert_eq!(result, Value::Error(FormulaError::DivisionByZero));
    }

    #[test]
    fn test_eval_concat() {
        let evaluator = Evaluator::new();
        let provider = SimpleProvider::new();
        let expr = Parser::parse_formula("\"Hello\"&\" \"&\"World\"").unwrap();
        let result = evaluator.evaluate(&expr, &provider);
        assert_eq!(result, Value::String("Hello World".into()));
    }

    #[test]
    fn test_eval_comparison() {
        let evaluator = Evaluator::new();
        let provider = SimpleProvider::new();
        let expr = Parser::parse_formula("1<2").unwrap();
        let result = evaluator.evaluate(&expr, &provider);
        assert_eq!(result, Value::Boolean(true));
    }

    #[test]
    fn test_eval_cell_ref() {
        let evaluator = Evaluator::new();
        let mut provider = SimpleProvider::new();
        provider.set_number(0, 0, 10.0);
        provider.set_number(0, 1, 20.0);
        let expr = Parser::parse_formula("A1+B1").unwrap();
        let result = evaluator.evaluate(&expr, &provider);
        assert_eq!(result, Value::Number(30.0));
    }

    #[test]
    fn test_eval_qualified_cell_and_range_refs() {
        let evaluator = Evaluator::new();
        let mut provider = SimpleProvider::new();
        provider.set_on_sheet("Annual Budget", 0, 0, Value::Number(10.0));
        provider.set_on_sheet("Annual Budget", 1, 0, Value::Number(20.0));
        let expr = Parser::parse_formula("SUM('Annual Budget'!A1:A2)").unwrap();
        assert_eq!(evaluator.evaluate(&expr, &provider), Value::Number(30.0));
    }

    #[test]
    fn test_missing_qualified_sheet_is_ref_error() {
        let evaluator = Evaluator::new();
        let provider = SimpleProvider::new();
        let expr = Parser::parse_formula("Missing!A1").unwrap();
        assert!(matches!(
            evaluator.evaluate(&expr, &provider),
            Value::Error(FormulaError::RefError(_))
        ));
    }

    #[test]
    fn test_huge_range_is_rejected_before_materialization() {
        let evaluator = Evaluator::new();
        let provider = SimpleProvider::new();
        let expr = Parser::parse_formula("SUM(A1:XFD1000000)").unwrap();
        assert!(matches!(
            evaluator.evaluate(&expr, &provider),
            Value::Error(FormulaError::RefError(_))
        ));
    }

    #[test]
    fn test_eval_unary_neg() {
        let evaluator = Evaluator::new();
        let provider = SimpleProvider::new();
        let expr = Parser::parse_formula("-5").unwrap();
        let result = evaluator.evaluate(&expr, &provider);
        assert_eq!(result, Value::Number(-5.0));
    }

    #[test]
    fn test_eval_function_sum() {
        let evaluator = Evaluator::new();
        let mut provider = SimpleProvider::new();
        provider.set_number(0, 0, 1.0);
        provider.set_number(0, 1, 2.0);
        provider.set_number(0, 2, 3.0);
        let expr = Parser::parse_formula("SUM(A1:C1)").unwrap();
        let result = evaluator.evaluate(&expr, &provider);
        assert_eq!(result, Value::Number(6.0));
    }

    #[test]
    fn test_eval_function_if() {
        let evaluator = Evaluator::new();
        let provider = SimpleProvider::new();
        let expr = Parser::parse_formula("IF(1>0,\"yes\",\"no\")").unwrap();
        let result = evaluator.evaluate(&expr, &provider);
        assert_eq!(result, Value::String("yes".into()));
    }

    #[test]
    fn test_eval_boolean() {
        let evaluator = Evaluator::new();
        let provider = SimpleProvider::new();
        let expr = Parser::parse_formula("TRUE").unwrap();
        let result = evaluator.evaluate(&expr, &provider);
        assert_eq!(result, Value::Boolean(true));
    }

    #[test]
    fn test_value_as_number() {
        assert_eq!(Value::Number(42.0).as_number().unwrap(), 42.0);
        assert_eq!(Value::Boolean(true).as_number().unwrap(), 1.0);
        assert_eq!(Value::Empty.as_number().unwrap(), 0.0);
        assert!(Value::String("abc".into()).as_number().is_err());
    }

    #[test]
    fn test_value_as_string() {
        assert_eq!(Value::Number(42.0).as_string().unwrap(), "42");
        assert_eq!(Value::Boolean(true).as_string().unwrap(), "TRUE");
        assert_eq!(Value::String("hello".into()).as_string().unwrap(), "hello");
    }
}
