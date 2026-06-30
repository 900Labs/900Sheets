use crate::error::FormulaError;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    String(String),
    Boolean(bool),
    CellRef(String),
    RangeRef(String),
    Function(String),
    Plus,
    Minus,
    Asterisk,
    Slash,
    Percent,
    Caret,
    LParen,
    RParen,
    Comma,
    Colon,
    Ampersand,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Concat,
}

impl Token {
    pub fn precedence(&self) -> u8 {
        match self {
            Token::Caret => 7,
            Token::Asterisk | Token::Slash | Token::Percent => 6,
            Token::Plus | Token::Minus => 5,
            Token::Ampersand | Token::Concat => 4,
            Token::Eq | Token::NotEq | Token::Lt | Token::LtEq | Token::Gt | Token::GtEq => 3,
            _ => 0,
        }
    }

    pub fn is_operator(&self) -> bool {
        matches!(
            self,
            Token::Plus
                | Token::Minus
                | Token::Asterisk
                | Token::Slash
                | Token::Percent
                | Token::Caret
                | Token::Ampersand
                | Token::Concat
                | Token::Eq
                | Token::NotEq
                | Token::Lt
                | Token::LtEq
                | Token::Gt
                | Token::GtEq
        )
    }

    pub fn is_right_associative(&self) -> bool {
        matches!(self, Token::Caret)
    }
}

pub struct Tokenizer {
    input: Vec<char>,
    pos: usize,
}

impl Tokenizer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, FormulaError> {
        let mut tokens = Vec::new();
        while self.pos < self.input.len() {
            self.skip_whitespace();
            if self.pos >= self.input.len() {
                break;
            }
            let token = self.next_token()?;
            tokens.push(token);
        }
        Ok(tokens)
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() && self.input[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.input.get(self.pos + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek();
        self.pos += 1;
        ch
    }

    fn next_token(&mut self) -> Result<Token, FormulaError> {
        let ch = self
            .peek()
            .ok_or(FormulaError::ParseError("Unexpected end of input".into()))?;

        if ch.is_ascii_digit() || (ch == '.' && self.peek_at(1).is_some_and(|c| c.is_ascii_digit()))
        {
            return self.read_number();
        }

        if ch == '"' {
            return self.read_string();
        }

        if ch == '#' {
            return self.read_error_literal();
        }

        if ch.is_ascii_alphabetic() || ch == '_' || ch == '$' {
            return self.read_identifier();
        }

        let token = match ch {
            '+' => {
                self.advance();
                Token::Plus
            }
            '-' => {
                self.advance();
                Token::Minus
            }
            '*' => {
                self.advance();
                Token::Asterisk
            }
            '/' => {
                self.advance();
                Token::Slash
            }
            '%' => {
                self.advance();
                Token::Percent
            }
            '^' => {
                self.advance();
                Token::Caret
            }
            '(' => {
                self.advance();
                Token::LParen
            }
            ')' => {
                self.advance();
                Token::RParen
            }
            ',' => {
                self.advance();
                Token::Comma
            }
            ':' => {
                self.advance();
                Token::Colon
            }
            '&' => {
                self.advance();
                Token::Ampersand
            }
            '=' => {
                self.advance();
                Token::Eq
            }
            '<' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Token::LtEq
                } else if self.peek() == Some('>') {
                    self.advance();
                    Token::NotEq
                } else {
                    Token::Lt
                }
            }
            '>' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Token::GtEq
                } else {
                    Token::Gt
                }
            }
            _ => {
                return Err(FormulaError::ParseError(format!(
                    "Unexpected character: '{}'",
                    ch
                )))
            }
        };
        Ok(token)
    }

    fn read_number(&mut self) -> Result<Token, FormulaError> {
        let start = self.pos;
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos < self.input.len() && self.input[self.pos] == '.' {
            self.pos += 1;
            while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        if self.pos < self.input.len()
            && (self.input[self.pos] == 'e' || self.input[self.pos] == 'E')
        {
            self.pos += 1;
            if self.pos < self.input.len()
                && (self.input[self.pos] == '+' || self.input[self.pos] == '-')
            {
                self.pos += 1;
            }
            while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        let s: String = self.input[start..self.pos].iter().collect();
        let n = s
            .parse::<f64>()
            .map_err(|e| FormulaError::ParseError(e.to_string()))?;
        Ok(Token::Number(n))
    }

    fn read_string(&mut self) -> Result<Token, FormulaError> {
        self.advance();
        let mut s = String::new();
        loop {
            match self.advance() {
                None => return Err(FormulaError::ParseError("Unterminated string".into())),
                Some('"') => {
                    if self.peek() == Some('"') {
                        s.push('"');
                        self.advance();
                    } else {
                        break;
                    }
                }
                Some(ch) => s.push(ch),
            }
        }
        Ok(Token::String(s))
    }

    fn read_error_literal(&mut self) -> Result<Token, FormulaError> {
        let start = self.pos;
        while self.pos < self.input.len()
            && self.input[self.pos].is_ascii_alphanumeric()
            && self.input[self.pos] != '!'
            && self.input[self.pos] != '?'
        {
            self.pos += 1;
        }
        if self.pos < self.input.len()
            && (self.input[self.pos] == '!' || self.input[self.pos] == '?')
        {
            self.pos += 1;
        }
        let s: String = self.input[start..self.pos].iter().collect();
        Ok(Token::String(s))
    }

    fn read_identifier(&mut self) -> Result<Token, FormulaError> {
        let start = self.pos;
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' || ch == '.' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let s: String = self.input[start..self.pos].iter().collect();

        if self.peek() == Some('(') {
            return Ok(Token::Function(s.to_uppercase()));
        }

        let upper = s.to_uppercase();
        if upper == "TRUE" {
            return Ok(Token::Boolean(true));
        }
        if upper == "FALSE" {
            return Ok(Token::Boolean(false));
        }

        if self.peek() == Some(':') {
            let saved = self.pos;
            self.advance();
            let s2_start = self.pos;
            while self.pos < self.input.len() {
                let ch = self.input[self.pos];
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            if self.pos > s2_start {
                let s2: String = self.input[s2_start..self.pos].iter().collect();
                return Ok(Token::RangeRef(format!("{}:{}", s, s2)));
            }
            self.pos = saved;
        }

        Ok(Token::CellRef(s))
    }
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, FormulaError> {
    let mut tokenizer = Tokenizer::new(input);
    tokenizer.tokenize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_number() {
        let tokens = tokenize("42").unwrap();
        assert_eq!(tokens, vec![Token::Number(42.0)]);
    }

    #[test]
    fn test_tokenize_decimal() {
        let tokens = tokenize("3.14").unwrap();
        assert_eq!(tokens, vec![Token::Number(314.0 / 100.0)]);
    }

    #[test]
    fn test_tokenize_string() {
        let tokens = tokenize("\"hello\"").unwrap();
        assert_eq!(tokens, vec![Token::String("hello".into())]);
    }

    #[test]
    fn test_tokenize_escaped_string() {
        let tokens = tokenize("\"say \"\"hi\"\"\"").unwrap();
        assert_eq!(tokens, vec![Token::String("say \"hi\"".into())]);
    }

    #[test]
    fn test_tokenize_boolean() {
        let tokens = tokenize("TRUE").unwrap();
        assert_eq!(tokens, vec![Token::Boolean(true)]);
        let tokens = tokenize("FALSE").unwrap();
        assert_eq!(tokens, vec![Token::Boolean(false)]);
    }

    #[test]
    fn test_tokenize_cell_ref() {
        let tokens = tokenize("A1").unwrap();
        assert_eq!(tokens, vec![Token::CellRef("A1".into())]);
    }

    #[test]
    fn test_tokenize_range_ref() {
        let tokens = tokenize("A1:B5").unwrap();
        assert_eq!(tokens, vec![Token::RangeRef("A1:B5".into())]);
    }

    #[test]
    fn test_tokenize_function() {
        let tokens = tokenize("SUM(A1:B5)").unwrap();
        assert_eq!(tokens[0], Token::Function("SUM".into()));
        assert_eq!(tokens[1], Token::LParen);
    }

    #[test]
    fn test_tokenize_operators() {
        let tokens = tokenize("1+2-3*4/5^6").unwrap();
        assert_eq!(tokens.len(), 11);
        assert_eq!(tokens[1], Token::Plus);
        assert_eq!(tokens[3], Token::Minus);
        assert_eq!(tokens[5], Token::Asterisk);
        assert_eq!(tokens[7], Token::Slash);
        assert_eq!(tokens[9], Token::Caret);
    }

    #[test]
    fn test_tokenize_comparison() {
        let tokens = tokenize("1=2<>3<4<=5>6>=7").unwrap();
        assert_eq!(tokens[1], Token::Eq);
        assert_eq!(tokens[3], Token::NotEq);
        assert_eq!(tokens[5], Token::Lt);
        assert_eq!(tokens[7], Token::LtEq);
        assert_eq!(tokens[9], Token::Gt);
        assert_eq!(tokens[11], Token::GtEq);
    }

    #[test]
    fn test_tokenize_concat() {
        let tokens = tokenize("\"a\"&\"b\"").unwrap();
        assert_eq!(tokens[1], Token::Ampersand);
    }

    #[test]
    fn test_tokenize_complex() {
        let tokens = tokenize("SUM(A1:A10) + B1 * 2").unwrap();
        assert_eq!(tokens.len(), 8);
    }
}
