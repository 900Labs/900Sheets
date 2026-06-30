use chrono::Datelike;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum NumberFormat {
    #[default]
    General,
    Number {
        decimals: u8,
        thousands_sep: bool,
    },
    Currency {
        decimals: u8,
        symbol: String,
    },
    Percentage {
        decimals: u8,
    },
    Scientific {
        decimals: u8,
    },
    Fraction {
        denominator: u32,
    },
    Date,
    Time,
    DateTime,
    Duration,
    Text,
    Custom(String),
}

impl NumberFormat {
    pub fn from_pattern(pattern: &str) -> Self {
        let p = pattern.trim();

        if p == "General" || p == "general" || p.is_empty() {
            return NumberFormat::General;
        }

        if p == "0" || p == "#" {
            return NumberFormat::Number {
                decimals: 0,
                thousands_sep: false,
            };
        }

        if p.starts_with("$") || p.starts_with("€") || p.starts_with("£") || p.starts_with("¥")
        {
            let decimals = count_decimals(p);
            let symbol = p.chars().next().unwrap().to_string();
            return NumberFormat::Currency { decimals, symbol };
        }

        if p.ends_with('%') {
            let decimals = count_decimals(p);
            return NumberFormat::Percentage { decimals };
        }

        if p == "0.00E+00" || p.to_uppercase() == "SCIENTIFIC" {
            return NumberFormat::Scientific { decimals: 2 };
        }

        if p.contains("yyyy") || p.contains("dd") || p.contains("mm/dd") || p.contains("m/d") {
            if p.contains("HH") || p.contains("hh") || p.contains(":mm") {
                return NumberFormat::DateTime;
            }
            return NumberFormat::Date;
        }

        if p.contains("HH:mm") || p.contains("hh:mm") || p.contains(":ss") {
            return NumberFormat::Time;
        }

        if p == "@" || p.to_uppercase() == "TEXT" {
            return NumberFormat::Text;
        }

        if p.contains('?') || p.contains("/ ") {
            return NumberFormat::Fraction { denominator: 100 };
        }

        if p.contains(',') {
            let decimals = count_decimals(p);
            return NumberFormat::Number {
                decimals,
                thousands_sep: true,
            };
        }

        if p.contains('0') || p.contains('#') || p.contains('.') {
            let decimals = count_decimals(p);
            return NumberFormat::Number {
                decimals,
                thousands_sep: false,
            };
        }

        NumberFormat::Custom(p.into())
    }

    pub fn format(&self, value: f64) -> String {
        match self {
            NumberFormat::General => format_general(value),
            NumberFormat::Number {
                decimals,
                thousands_sep,
            } => format_number(value, *decimals, *thousands_sep),
            NumberFormat::Currency { decimals, symbol } => {
                format_currency(value, *decimals, symbol)
            }
            NumberFormat::Percentage { decimals } => format_percentage(value, *decimals),
            NumberFormat::Scientific { decimals } => format_scientific(value, *decimals),
            NumberFormat::Fraction { denominator } => format_fraction(value, *denominator),
            NumberFormat::Date => format_date(value),
            NumberFormat::Time => format_time(value),
            NumberFormat::DateTime => format_datetime(value),
            NumberFormat::Duration => format_duration(value),
            NumberFormat::Text => value.to_string(),
            NumberFormat::Custom(p) => format_custom(value, p),
        }
    }
}

fn count_decimals(pattern: &str) -> u8 {
    if let Some(pos) = pattern.find('.') {
        let after = &pattern[pos + 1..];
        let count = after
            .chars()
            .take_while(|c| *c == '0' || *c == '#' || *c == '?')
            .filter(|c| *c == '0' || *c == '?')
            .count();
        count as u8
    } else {
        0
    }
}

fn format_general(value: f64) -> String {
    if value == value.trunc() && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{}", value)
    }
}

fn format_number(value: f64, decimals: u8, thousands_sep: bool) -> String {
    let formatted = format!("{:.*}", decimals as usize, value);
    if thousands_sep {
        add_thousands_sep(&formatted)
    } else {
        formatted
    }
}

fn format_currency(value: f64, decimals: u8, symbol: &str) -> String {
    let formatted = format!("{:.*}", decimals as usize, value.abs());
    let with_sep = add_thousands_sep(&formatted);
    if value < 0.0 {
        format!("-{}{}", symbol, with_sep)
    } else {
        format!("{}{}", symbol, with_sep)
    }
}

fn format_percentage(value: f64, decimals: u8) -> String {
    format!("{:.*}%", decimals as usize, value * 100.0)
}

fn format_scientific(value: f64, decimals: u8) -> String {
    format!("{:.*e}", decimals as usize, value)
}

fn format_fraction(value: f64, denominator: u32) -> String {
    let whole = value.trunc() as i64;
    let frac = value.fract().abs();
    let numerator = (frac * denominator as f64).round() as u32;
    if numerator == 0 {
        return format!("{}", whole);
    }
    if whole != 0 {
        format!("{} {}/{}", whole, numerator, denominator)
    } else {
        format!("{}/{}", numerator, denominator)
    }
}

fn format_date(value: f64) -> String {
    let days = value as i64;
    let (year, month, day) = days_to_date(days);
    format!("{:04}-{:02}-{:02}", year, month, day)
}

fn format_time(value: f64) -> String {
    let frac = value.fract().abs();
    let total_seconds = (frac * 86400.0).round() as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

fn format_datetime(value: f64) -> String {
    format!("{} {}", format_date(value), format_time(value))
}

fn format_duration(value: f64) -> String {
    let total_seconds = value.abs() as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

fn format_custom(value: f64, pattern: &str) -> String {
    if pattern.is_empty() {
        return format_general(value);
    }

    let (positive_part, rest) = match pattern.find(';') {
        Some(pos) => (&pattern[..pos], &pattern[pos + 1..]),
        None => (pattern, ""),
    };
    let _negative_part = if !rest.is_empty() {
        let (neg, _) = match rest.find(';') {
            Some(pos) => (&rest[..pos], &rest[pos + 1..]),
            None => (rest, ""),
        };
        neg
    } else {
        ""
    };

    let is_negative = value < 0.0;
    let abs_value = value.abs();

    let section = if is_negative && !_negative_part.is_empty() {
        _negative_part
    } else {
        positive_part
    };

    let mut result = String::new();
    let mut chars = section.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '0' => {
                let mut digit_str = String::from('0');
                while chars.peek() == Some(&'0') {
                    digit_str.push('0');
                    chars.next();
                }
                let int_digits = digit_str.len();
                let int_val = abs_value.trunc() as u64;
                let int_str = int_val.to_string();
                if int_str.len() >= int_digits {
                    result.push_str(&int_str);
                } else {
                    result.push_str(&"0".repeat(int_digits - int_str.len()));
                    result.push_str(&int_str);
                }
            }
            '#' => {
                while chars.peek() == Some(&'#') {
                    chars.next();
                }
                let int_val = abs_value.trunc() as u64;
                result.push_str(&int_val.to_string());
            }
            '.' => {
                result.push('.');
                let mut dec_digits = 0;
                while chars.peek() == Some(&'0') || chars.peek() == Some(&'#') {
                    if chars.peek() == Some(&'0') {
                        dec_digits += 1;
                    }
                    chars.next();
                }
                if dec_digits > 0 {
                    let scaled = abs_value * 10f64.powi(dec_digits as i32);
                    let rounded = scaled.round() as u64;
                    let dec_str = format!("{:0width$}", rounded, width = dec_digits);
                    result.push_str(&dec_str);
                }
            }
            ',' => {
                if result.len() >= 3 {
                    result = add_thousands_sep(&result);
                }
            }
            '"' => {
                let mut literal = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '"' {
                        chars.next();
                        break;
                    }
                    literal.push(c);
                    chars.next();
                }
                result.push_str(&literal);
            }
            '\\' => {
                if let Some(&next) = chars.peek() {
                    result.push(next);
                    chars.next();
                }
            }
            _ => {
                result.push(ch);
            }
        }
    }

    if is_negative && !_negative_part.is_empty() {
        result
    } else if is_negative {
        format!("-{}", result)
    } else {
        result
    }
}

fn add_thousands_sep(s: &str) -> String {
    let (int_part, dec_part) = match s.find('.') {
        Some(pos) => (&s[..pos], Some(&s[pos..])),
        None => (s, None),
    };

    let is_negative = int_part.starts_with('-');
    let digits = if is_negative {
        &int_part[1..]
    } else {
        int_part
    };

    let mut result = String::new();
    for (i, ch) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.insert(0, ',');
        }
        result.insert(0, ch);
    }

    if is_negative {
        result.insert(0, '-');
    }

    if let Some(dec) = dec_part {
        result.push_str(dec);
    }

    result
}

fn days_to_date(days: i64) -> (i64, u32, u32) {
    let epoch = days + 25569;
    let total_secs = epoch * 86400;
    let dt = chrono::DateTime::from_timestamp(total_secs, 0)
        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
    (dt.year() as i64, dt.month(), dt.day())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_general() {
        assert_eq!(NumberFormat::General.format(42.0), "42");
        assert_eq!(NumberFormat::General.format(314.0 / 100.0), "3.14");
    }

    #[test]
    fn test_number() {
        let fmt = NumberFormat::Number {
            decimals: 2,
            thousands_sep: false,
        };
        assert_eq!(fmt.format(1234.567), "1234.57");
    }

    #[test]
    fn test_number_thousands() {
        let fmt = NumberFormat::Number {
            decimals: 0,
            thousands_sep: true,
        };
        assert_eq!(fmt.format(1234567.0), "1,234,567");
    }

    #[test]
    fn test_currency() {
        let fmt = NumberFormat::Currency {
            decimals: 2,
            symbol: "$".into(),
        };
        assert_eq!(fmt.format(1234.5), "$1,234.50");
        assert_eq!(fmt.format(-1234.5), "-$1,234.50");
    }

    #[test]
    fn test_percentage() {
        let fmt = NumberFormat::Percentage { decimals: 1 };
        assert_eq!(fmt.format(0.1234), "12.3%");
    }

    #[test]
    fn test_scientific() {
        let fmt = NumberFormat::Scientific { decimals: 2 };
        assert_eq!(fmt.format(1234.0), "1.23e3");
    }

    #[test]
    fn test_fraction() {
        let fmt = NumberFormat::Fraction { denominator: 4 };
        assert_eq!(fmt.format(1.25), "1 1/4");
        assert_eq!(fmt.format(0.5), "2/4");
    }

    #[test]
    fn test_from_pattern_general() {
        assert_eq!(NumberFormat::from_pattern("General"), NumberFormat::General);
    }

    #[test]
    fn test_from_pattern_currency() {
        match NumberFormat::from_pattern("$#,##0.00") {
            NumberFormat::Currency { decimals, symbol } => {
                assert_eq!(decimals, 2);
                assert_eq!(symbol, "$");
            }
            _ => panic!("Expected Currency"),
        }
    }

    #[test]
    fn test_from_pattern_percentage() {
        match NumberFormat::from_pattern("0.00%") {
            NumberFormat::Percentage { decimals } => assert_eq!(decimals, 2),
            _ => panic!("Expected Percentage"),
        }
    }

    #[test]
    fn test_from_pattern_number() {
        match NumberFormat::from_pattern("#,##0") {
            NumberFormat::Number {
                decimals,
                thousands_sep,
            } => {
                assert_eq!(decimals, 0);
                assert!(thousands_sep);
            }
            _ => panic!("Expected Number"),
        }
    }

    #[test]
    fn test_add_thousands_sep() {
        assert_eq!(add_thousands_sep("1234567"), "1,234,567");
        assert_eq!(add_thousands_sep("1234567.89"), "1,234,567.89");
        assert_eq!(add_thousands_sep("-1234"), "-1,234");
        assert_eq!(add_thousands_sep("100"), "100");
    }

    #[test]
    fn test_format_date() {
        let fmt = NumberFormat::Date;
        let result = fmt.format(45000.0);
        assert!(!result.is_empty());
    }
}
