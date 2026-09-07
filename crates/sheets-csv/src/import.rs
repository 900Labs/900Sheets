use crate::error::CsvError;
use sheets_core::sheet::Sheet;

const MAX_CSV_SIZE: usize = 100 * 1024 * 1024;
const MAX_CELLS: usize = 10_000_000;
const MAX_ROWS: usize = 1_000_000;
const MAX_COLS: usize = 16_384;

#[derive(Clone, Copy)]
struct ImportLimits {
    max_bytes: usize,
    max_cells: usize,
    max_rows: usize,
    max_cols: usize,
}

impl Default for ImportLimits {
    fn default() -> Self {
        Self {
            max_bytes: MAX_CSV_SIZE,
            max_cells: MAX_CELLS,
            max_rows: MAX_ROWS,
            max_cols: MAX_COLS,
        }
    }
}

#[derive(Default)]
struct ImportBudget {
    cells: usize,
}

pub fn import_csv(data: &str, delimiter: char) -> Result<Sheet, CsvError> {
    import_csv_with_name(data, delimiter, "Sheet1")
}

pub fn import_csv_with_name(data: &str, delimiter: char, name: &str) -> Result<Sheet, CsvError> {
    import_csv_with_name_and_limits(data, delimiter, name, ImportLimits::default())
}

fn import_csv_with_name_and_limits(
    data: &str,
    delimiter: char,
    name: &str,
    limits: ImportLimits,
) -> Result<Sheet, CsvError> {
    if data.len() > limits.max_bytes {
        return Err(CsvError::FileTooLarge(data.len(), limits.max_bytes));
    }
    let mut sheet = Sheet::new(name);

    parse_csv_data(data, delimiter, limits, |row, fields| {
        for (col, field) in fields.into_iter().enumerate() {
            if !field.is_empty() {
                sheet.set_cell_value(row as u32, col as u32, field);
            }
        }
    })?;

    Ok(sheet)
}

fn push_field(
    current_row: &mut Vec<String>,
    current_field: &mut String,
    budget: &mut ImportBudget,
    limits: ImportLimits,
) -> Result<(), CsvError> {
    let next_col_count = current_row.len() + 1;
    if next_col_count > limits.max_cols {
        return Err(CsvError::TooManyColumns(next_col_count, limits.max_cols));
    }
    budget.cells = budget.cells.saturating_add(1);
    if budget.cells > limits.max_cells {
        return Err(CsvError::TooManyCells(budget.cells, limits.max_cells));
    }
    current_row.push(std::mem::take(current_field));
    Ok(())
}

fn push_row(
    row_count: &mut usize,
    current_row: &mut Vec<String>,
    limits: ImportLimits,
    consume_row: &mut impl FnMut(usize, Vec<String>),
) -> Result<(), CsvError> {
    let next_row_count = *row_count + 1;
    if next_row_count > limits.max_rows {
        return Err(CsvError::TooManyRows(next_row_count, limits.max_rows));
    }
    consume_row(*row_count, std::mem::take(current_row));
    *row_count = next_row_count;
    Ok(())
}

fn parse_csv_data(
    data: &str,
    delimiter: char,
    limits: ImportLimits,
    mut consume_row: impl FnMut(usize, Vec<String>),
) -> Result<(), CsvError> {
    if matches!(delimiter, '"' | '\r' | '\n') {
        return Err(CsvError::InvalidFormat(
            "delimiter must not be a quote or line break".into(),
        ));
    }
    let mut row_count = 0;
    let mut current_row = Vec::new();
    let mut current_field = String::new();
    let mut budget = ImportBudget::default();
    let mut in_quotes = false;
    let mut closed_quote = false;
    let mut record_started = false;
    let mut chars = data
        .strip_prefix('\u{feff}')
        .unwrap_or(data)
        .chars()
        .peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    current_field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                    closed_quote = true;
                }
            } else {
                current_field.push(ch);
            }
        } else if ch == '"' {
            if closed_quote || !current_field.is_empty() {
                return Err(CsvError::InvalidFormat(format!(
                    "unexpected quote in row {}, column {}",
                    row_count + 1,
                    current_row.len() + 1
                )));
            }
            in_quotes = true;
            record_started = true;
        } else if ch == delimiter {
            push_field(&mut current_row, &mut current_field, &mut budget, limits)?;
            closed_quote = false;
            record_started = true;
        } else if ch == '\n' || ch == '\r' {
            if ch == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            push_field(&mut current_row, &mut current_field, &mut budget, limits)?;
            push_row(&mut row_count, &mut current_row, limits, &mut consume_row)?;
            closed_quote = false;
            record_started = false;
        } else {
            if closed_quote {
                return Err(CsvError::InvalidFormat(format!(
                    "unexpected character after closing quote in row {}, column {}",
                    row_count + 1,
                    current_row.len() + 1
                )));
            }
            current_field.push(ch);
            record_started = true;
        }
    }

    if in_quotes {
        return Err(CsvError::InvalidFormat(format!(
            "unterminated quoted field in row {}, column {}",
            row_count + 1,
            current_row.len() + 1
        )));
    }
    // An empty quoted field still consumes a coordinate and row budget.
    if record_started {
        push_field(&mut current_row, &mut current_field, &mut budget, limits)?;
        push_row(&mut row_count, &mut current_row, limits, &mut consume_row)?;
    }

    Ok(())
}

pub fn detect_delimiter(data: &str) -> char {
    let binding = data.lines().take(5).collect::<Vec<_>>().join("\n");
    let sample: &str = binding.as_str();
    let candidates = [',', '\t', ';', '|'];
    let mut best = ',';
    let mut best_score = 0usize;

    for &delim in &candidates {
        let mut counts = Vec::new();
        for line in sample.lines() {
            let count = line.chars().filter(|&c| c == delim).count();
            counts.push(count);
        }
        if counts.is_empty() {
            continue;
        }
        let first = counts[0];
        if first == 0 {
            continue;
        }
        let consistent = counts.iter().all(|&c| c == first);
        if consistent && first > best_score {
            best_score = first;
            best = delim;
        }
    }

    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_csv() {
        let data = "a,b,c\n1,2,3";
        let sheet = import_csv(data, ',').unwrap();
        assert_eq!(sheet.cell_value(0, 0), Some("a".into()));
        assert_eq!(sheet.cell_value(0, 2), Some("c".into()));
        assert_eq!(sheet.cell_value(1, 0), Some("1".into()));
        assert_eq!(sheet.cell_value(1, 2), Some("3".into()));
    }

    #[test]
    fn test_parse_quoted_csv() {
        let data = "\"hello,world\",b";
        let sheet = import_csv(data, ',').unwrap();
        assert_eq!(sheet.cell_value(0, 0), Some("hello,world".into()));
        assert_eq!(sheet.cell_value(0, 1), Some("b".into()));
    }

    #[test]
    fn test_parse_escaped_quotes() {
        let data = "\"say \"\"hi\"\"\",b";
        let sheet = import_csv(data, ',').unwrap();
        assert_eq!(sheet.cell_value(0, 0), Some("say \"hi\"".into()));
    }

    #[test]
    fn test_parse_tab_delimited() {
        let data = "a\tb\tc\n1\t2\t3";
        let sheet = import_csv(data, '\t').unwrap();
        assert_eq!(sheet.cell_value(0, 0), Some("a".into()));
        assert_eq!(sheet.cell_value(1, 2), Some("3".into()));
    }

    #[test]
    fn test_parse_semicolon_delimited() {
        let data = "a;b;c\n1;2;3";
        let sheet = import_csv(data, ';').unwrap();
        assert_eq!(sheet.cell_value(0, 0), Some("a".into()));
        assert_eq!(sheet.cell_value(1, 2), Some("3".into()));
    }

    #[test]
    fn test_detect_delimiter_comma() {
        let data = "a,b,c\n1,2,3\n4,5,6";
        assert_eq!(detect_delimiter(data), ',');
    }

    #[test]
    fn test_detect_delimiter_tab() {
        let data = "a\tb\tc\n1\t2\t3\n4\t5\t6";
        assert_eq!(detect_delimiter(data), '\t');
    }

    #[test]
    fn test_detect_delimiter_semicolon() {
        let data = "a;b;c\n1;2;3\n4;5;6";
        assert_eq!(detect_delimiter(data), ';');
    }

    #[test]
    fn test_empty_csv() {
        let sheet = import_csv("", ',').unwrap();
        assert_eq!(sheet.cell_count(), 0);
    }

    #[test]
    fn test_csv_with_empty_fields() {
        let data = "a,,c";
        let sheet = import_csv(data, ',').unwrap();
        assert_eq!(sheet.cell_value(0, 0), Some("a".into()));
        assert_eq!(sheet.cell_value(0, 1), None);
        assert_eq!(sheet.cell_value(0, 2), Some("c".into()));
    }

    #[test]
    fn test_parse_multiline_quoted_field() {
        let data = "a,b\n\"line1\nline2\",c\n";
        let sheet = import_csv(data, ',').unwrap();
        assert_eq!(sheet.cell_value(0, 0), Some("a".into()));
        assert_eq!(sheet.cell_value(0, 1), Some("b".into()));
        assert_eq!(sheet.cell_value(1, 0), Some("line1\nline2".into()));
        assert_eq!(sheet.cell_value(1, 1), Some("c".into()));
    }

    #[test]
    fn test_parse_crlf_line_endings() {
        let data = "a,b\r\nc,d\r\n";
        let sheet = import_csv(data, ',').unwrap();
        assert_eq!(sheet.cell_value(0, 0), Some("a".into()));
        assert_eq!(sheet.cell_value(0, 1), Some("b".into()));
        assert_eq!(sheet.cell_value(1, 0), Some("c".into()));
        assert_eq!(sheet.cell_value(1, 1), Some("d".into()));
    }

    #[test]
    fn test_import_rejects_too_many_rows() {
        let limits = ImportLimits {
            max_rows: 1,
            ..ImportLimits::default()
        };
        let result = import_csv_with_name_and_limits("a\nb", ',', "Sheet1", limits);
        assert!(matches!(result, Err(CsvError::TooManyRows(2, 1))));
    }

    #[test]
    fn test_import_rejects_too_many_columns() {
        let limits = ImportLimits {
            max_cols: 2,
            ..ImportLimits::default()
        };
        let result = import_csv_with_name_and_limits("a,b,c", ',', "Sheet1", limits);
        assert!(matches!(result, Err(CsvError::TooManyColumns(3, 2))));
    }

    #[test]
    fn test_import_rejects_too_many_cells() {
        let limits = ImportLimits {
            max_cells: 3,
            ..ImportLimits::default()
        };
        let result = import_csv_with_name_and_limits("a,b\nc,d", ',', "Sheet1", limits);
        assert!(matches!(result, Err(CsvError::TooManyCells(4, 3))));
    }

    #[test]
    fn test_import_rejects_oversized_csv_before_parse() {
        let limits = ImportLimits {
            max_bytes: 4,
            ..ImportLimits::default()
        };
        let result = import_csv_with_name_and_limits("a,b,c", ',', "Sheet1", limits);
        assert!(matches!(result, Err(CsvError::FileTooLarge(5, 4))));
    }

    #[test]
    fn malformed_quotes_are_rejected_instead_of_silently_changing_data() {
        for data in ["name,value\n\"unfinished,42", "a\"b,c", "\"a\"tail,b"] {
            assert!(matches!(
                import_csv(data, ','),
                Err(CsvError::InvalidFormat(_))
            ));
        }
    }

    #[test]
    fn empty_quoted_final_record_counts_toward_import_limits() {
        let limits = ImportLimits {
            max_rows: 1,
            ..ImportLimits::default()
        };
        assert!(matches!(
            import_csv_with_name_and_limits("value\n\"\"", ',', "Sheet1", limits),
            Err(CsvError::TooManyRows(2, 1))
        ));
        let limits = ImportLimits {
            max_cells: 0,
            ..ImportLimits::default()
        };
        assert!(matches!(
            import_csv_with_name_and_limits("\"\"", ',', "Sheet1", limits),
            Err(CsvError::TooManyCells(1, 0))
        ));
    }

    #[test]
    fn utf8_bom_does_not_become_part_of_the_first_header() {
        let sheet = import_csv("\u{feff}\"Name\",Value\rAda,42", ',').unwrap();
        assert_eq!(sheet.cell_value(0, 0), Some("Name".into()));
        assert_eq!(sheet.cell_value(1, 1), Some("42".into()));
    }

    #[test]
    fn quote_and_line_break_delimiters_are_rejected() {
        for delimiter in ['"', '\r', '\n'] {
            assert!(matches!(
                import_csv("a,b", delimiter),
                Err(CsvError::InvalidFormat(_))
            ));
        }
    }
}
