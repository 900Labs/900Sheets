use crate::ast::*;
use crate::error::FormulaError;
use crate::tokenizer::{Token, Tokenizer};

pub const MAX_FORMULA_BYTES: usize = 32_768;
pub const MAX_FORMULA_TOKENS: usize = 1_024;
pub const MAX_FORMULA_DEPTH: usize = 128;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<Expr, FormulaError> {
        if self.tokens.len() > MAX_FORMULA_TOKENS {
            return Err(FormulaError::ParseError(format!(
                "Formula exceeds safe token limit of {MAX_FORMULA_TOKENS}"
            )));
        }
        let expr = self.parse_expr(0, 0)?;
        if self.pos < self.tokens.len() {
            return Err(FormulaError::ParseError(format!(
                "Unexpected token at position {}",
                self.pos
            )));
        }
        // Left-associative operators can build a deep tree without deeply
        // nesting the parser. Bound that tree before recursive evaluation,
        // reference collection, cloning, or formatting can consume it.
        let mut pending = vec![(&expr, 0)];
        while let Some((node, depth)) = pending.pop() {
            check_depth(depth)?;
            match node {
                Expr::BinOp { left, right, .. } => {
                    pending.push((left, depth + 1));
                    pending.push((right, depth + 1));
                }
                Expr::UnaryOp { operand, .. } => pending.push((operand, depth + 1)),
                Expr::Function { args, .. } => {
                    pending.extend(args.iter().map(|arg| (arg, depth + 1)));
                }
                _ => {}
            }
        }
        Ok(expr)
    }

    pub fn parse_formula(input: &str) -> Result<Expr, FormulaError> {
        if input.len() > MAX_FORMULA_BYTES {
            return Err(FormulaError::ParseError(format!(
                "Formula exceeds safe size limit of {MAX_FORMULA_BYTES} bytes"
            )));
        }
        let trimmed = input.trim();
        let expr_str = trimmed.strip_prefix('=').unwrap_or(trimmed);
        let mut tokenizer = Tokenizer::new(expr_str);
        let tokens = tokenizer.tokenize()?;
        let mut parser = Parser::new(tokens);
        parser.parse()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.peek().cloned();
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    fn expect(&mut self, expected: &Token) -> Result<(), FormulaError> {
        if self.peek() == Some(expected) {
            self.advance();
            Ok(())
        } else {
            Err(FormulaError::ParseError(format!(
                "Expected {:?}, got {:?}",
                expected,
                self.peek()
            )))
        }
    }

    fn parse_expr(&mut self, min_prec: u8, depth: usize) -> Result<Expr, FormulaError> {
        check_depth(depth)?;
        let mut left = self.parse_unary(depth)?;

        loop {
            let op_token = match self.peek() {
                Some(t) if t.is_operator() => t.clone(),
                _ => break,
            };

            let prec = op_token.precedence();
            if prec < min_prec {
                break;
            }

            self.advance();

            let next_min = if op_token.is_right_associative() {
                prec
            } else {
                prec + 1
            };
            let right = self.parse_expr(next_min, depth + 1)?;

            let op = match op_token {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                Token::Asterisk => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Caret => BinOp::Pow,
                Token::Ampersand | Token::Concat => BinOp::Concat,
                Token::Eq => BinOp::Eq,
                Token::NotEq => BinOp::NotEq,
                Token::Lt => BinOp::Lt,
                Token::LtEq => BinOp::LtEq,
                Token::Gt => BinOp::Gt,
                Token::GtEq => BinOp::GtEq,
                _ => {
                    return Err(FormulaError::ParseError(format!(
                        "Unexpected operator token: {:?}",
                        op_token
                    )))
                }
            };

            left = Expr::BinOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_unary(&mut self, depth: usize) -> Result<Expr, FormulaError> {
        check_depth(depth)?;
        if self.peek() == Some(&Token::Minus) {
            self.advance();
            let operand = self.parse_unary(depth + 1)?;
            return Ok(Expr::UnaryOp {
                op: UnaryOp::Neg,
                operand: Box::new(operand),
            });
        }
        if self.peek() == Some(&Token::Plus) {
            self.advance();
            return self.parse_unary(depth + 1);
        }
        let mut expr = self.parse_primary(depth)?;
        while self.peek() == Some(&Token::Percent) {
            self.advance();
            expr = Expr::UnaryOp {
                op: UnaryOp::Percent,
                operand: Box::new(expr),
            };
        }
        Ok(expr)
    }

    fn parse_primary(&mut self, depth: usize) -> Result<Expr, FormulaError> {
        let token = self
            .advance()
            .ok_or(FormulaError::ParseError("Unexpected end of input".into()))?;

        match token {
            Token::Number(n) => Ok(Expr::Number(n)),
            Token::String(s) => Ok(Expr::String(s)),
            Token::Boolean(b) => Ok(Expr::Boolean(b)),
            Token::Error(error) => Ok(Expr::Error(error)),
            Token::CellRef(s) => {
                let (sheet, col, row, abs_col, abs_row) = parse_cell_ref(&s)
                    .ok_or(FormulaError::ParseError(format!("Invalid cell ref: {}", s)))?;
                Ok(Expr::CellRef {
                    sheet,
                    col,
                    row,
                    abs_col,
                    abs_row,
                })
            }
            Token::RangeRef(s) => {
                let (sheet, sc, sr, ec, er) = parse_range_ref(&s)
                    .ok_or(FormulaError::ParseError(format!("Invalid range: {}", s)))?;
                Ok(Expr::RangeRef {
                    sheet,
                    start_col: sc,
                    start_row: sr,
                    end_col: ec,
                    end_row: er,
                })
            }
            Token::Function(name) => {
                self.expect(&Token::LParen)?;
                let mut args = Vec::new();
                if self.peek() != Some(&Token::RParen) {
                    args.push(self.parse_expr(0, depth + 1)?);
                    while self.peek() == Some(&Token::Comma) {
                        self.advance();
                        args.push(self.parse_expr(0, depth + 1)?);
                    }
                }
                self.expect(&Token::RParen)?;
                Ok(Expr::Function { name, args })
            }
            Token::LParen => {
                let expr = self.parse_expr(0, depth + 1)?;
                self.expect(&Token::RParen)?;
                Ok(expr)
            }
            _ => Err(FormulaError::ParseError(format!(
                "Unexpected token: {:?}",
                token
            ))),
        }
    }
}

fn check_depth(depth: usize) -> Result<(), FormulaError> {
    if depth > MAX_FORMULA_DEPTH {
        Err(FormulaError::ParseError(format!(
            "Formula exceeds safe nesting limit of {MAX_FORMULA_DEPTH}"
        )))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formula_size_and_token_limits_are_enforced_before_parsing() {
        assert!(Parser::parse_formula(&" ".repeat(MAX_FORMULA_BYTES + 1))
            .unwrap_err()
            .to_string()
            .contains("safe size limit"));
        let tokens = vec![Token::Plus; MAX_FORMULA_TOKENS + 1];
        assert!(Parser::new(tokens)
            .parse()
            .unwrap_err()
            .to_string()
            .contains("safe token limit"));
    }

    #[test]
    fn deeply_nested_grammar_and_expression_trees_are_rejected() {
        let depth = MAX_FORMULA_DEPTH + 1;
        let formulas = [
            format!("{}1{}", "(".repeat(depth), ")".repeat(depth)),
            format!("{}1", "-".repeat(depth)),
            format!("{}1", "1^".repeat(depth)),
            format!("1{}", "+1".repeat(depth)),
            format!("1{}", "%".repeat(depth)),
        ];
        for formula in formulas {
            assert!(Parser::parse_formula(&formula)
                .unwrap_err()
                .to_string()
                .contains("safe nesting limit"));
        }
    }

    #[test]
    fn normal_nested_formulas_and_wide_functions_remain_supported() {
        let formula = format!("{}1{}", "(".repeat(64), ")".repeat(64));
        assert_eq!(Parser::parse_formula(&formula).unwrap(), Expr::Number(1.0));
        let formula = format!("SUM({})", vec!["1"; 400].join(","));
        assert!(Parser::parse_formula(&formula).is_ok());
    }

    #[test]
    fn percentages_work_as_postfix_values_in_arithmetic() {
        let evaluator = crate::evaluator::Evaluator::new();
        let provider = crate::evaluator::SimpleProvider::new();
        for (formula, expected) in [
            ("10%", 0.1),
            ("100*10%", 10.0),
            ("10%+1", 1.1),
            ("(5+5)%", 0.1),
            ("-10%", -0.1),
            ("10%^2", 0.01),
            ("SUM(10%,20%)", 0.3),
            ("MOD(10,3)", 1.0),
        ] {
            let expression = Parser::parse_formula(formula).unwrap();
            let result = evaluator
                .evaluate(&expression, &provider)
                .as_number()
                .unwrap();
            assert!((result - expected).abs() < 1e-12, "{formula}: {result}");
        }
        assert!(Parser::parse_formula("10%3").is_err());
    }

    #[test]
    fn test_parse_number() {
        let expr = Parser::parse_formula("42").unwrap();
        assert_eq!(expr, Expr::Number(42.0));
    }

    #[test]
    fn test_parse_string() {
        let expr = Parser::parse_formula("\"hello\"").unwrap();
        assert_eq!(expr, Expr::String("hello".into()));
    }

    #[test]
    fn test_parse_boolean() {
        let expr = Parser::parse_formula("TRUE").unwrap();
        assert_eq!(expr, Expr::Boolean(true));
    }

    #[test]
    fn test_parse_cell_ref() {
        let expr = Parser::parse_formula("A1").unwrap();
        match expr {
            Expr::CellRef { col, row, .. } => {
                assert_eq!(col, 0);
                assert_eq!(row, 0);
            }
            _ => panic!("Expected CellRef"),
        }
    }

    #[test]
    fn test_parse_binop() {
        let expr = Parser::parse_formula("1+2").unwrap();
        match expr {
            Expr::BinOp {
                op: BinOp::Add,
                left,
                right,
            } => {
                assert_eq!(*left, Expr::Number(1.0));
                assert_eq!(*right, Expr::Number(2.0));
            }
            _ => panic!("Expected BinOp"),
        }
    }

    #[test]
    fn test_parse_precedence() {
        let expr = Parser::parse_formula("1+2*3").unwrap();
        match expr {
            Expr::BinOp {
                op: BinOp::Add,
                left,
                right,
            } => {
                assert_eq!(*left, Expr::Number(1.0));
                match *right {
                    Expr::BinOp { op: BinOp::Mul, .. } => {}
                    _ => panic!("Expected Mul on right"),
                }
            }
            _ => panic!("Expected Add"),
        }
    }

    #[test]
    fn test_parse_function() {
        let expr = Parser::parse_formula("SUM(1,2,3)").unwrap();
        match expr {
            Expr::Function { name, args } => {
                assert_eq!(name, "SUM");
                assert_eq!(args.len(), 3);
            }
            _ => panic!("Expected Function"),
        }
    }

    #[test]
    fn test_parse_nested_function() {
        let expr = Parser::parse_formula("SUM(A1:A10, AVG(B1:B10))").unwrap();
        match expr {
            Expr::Function { name, args } => {
                assert_eq!(name, "SUM");
                assert_eq!(args.len(), 2);
                match &args[1] {
                    Expr::Function { name, .. } => assert_eq!(*name, "AVG"),
                    _ => panic!("Expected nested function"),
                }
            }
            _ => panic!("Expected Function"),
        }
    }

    #[test]
    fn test_parse_unary_neg() {
        let expr = Parser::parse_formula("-5").unwrap();
        match expr {
            Expr::UnaryOp {
                op: UnaryOp::Neg,
                operand,
            } => {
                assert_eq!(*operand, Expr::Number(5.0));
            }
            _ => panic!("Expected UnaryOp"),
        }
    }

    #[test]
    fn test_parse_comparison() {
        let expr = Parser::parse_formula("1<2").unwrap();
        match expr {
            Expr::BinOp { op: BinOp::Lt, .. } => {}
            _ => panic!("Expected Lt"),
        }
    }

    #[test]
    fn test_parse_concat() {
        let expr = Parser::parse_formula("\"a\"&\"b\"").unwrap();
        match expr {
            Expr::BinOp {
                op: BinOp::Concat, ..
            } => {}
            _ => panic!("Expected Concat"),
        }
    }

    #[test]
    fn test_parse_parens() {
        let expr = Parser::parse_formula("(1+2)*3").unwrap();
        match expr {
            Expr::BinOp {
                op: BinOp::Mul,
                left,
                right,
            } => {
                assert_eq!(*right, Expr::Number(3.0));
                match *left {
                    Expr::BinOp { op: BinOp::Add, .. } => {}
                    _ => panic!("Expected Add in parens"),
                }
            }
            _ => panic!("Expected Mul"),
        }
    }

    #[test]
    fn test_parse_with_equals_sign() {
        let expr = Parser::parse_formula("=1+2").unwrap();
        match expr {
            Expr::BinOp { op: BinOp::Add, .. } => {}
            _ => panic!("Expected Add"),
        }
    }
}
