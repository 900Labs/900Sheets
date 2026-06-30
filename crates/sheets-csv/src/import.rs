use crate::error::CsvError;
use sheets_core::sheet::Sheet;

const MAX_CSV_SIZE: usize = 100 * 1024 * 1024;

pub fn import_csv(data: &str, delimiter: char) -> Result<Sheet, CsvError> {
    import_csv_with_name(data, delimiter, "Sheet1")
}

pub fn import_csv_with_name(data: &str, delimiter: char, name: &str) -> Result<Sheet, CsvError> {
    if data.len() > MAX_CSV_SIZE {
        return Err(CsvError::FileTooLarge(data.len(), MAX_CSV_SIZE));
    }
    let mut sheet = Sheet::new(name);

    let rows = parse_csv_data(data, delimiter);
    for (row, fields) in rows.iter().enumerate() {
        for (col, field) in fields.iter().enumerate() {
            if !field.is_empty() {
                sheet.set_cell_value(row as u32, col as u32, field.clone());
            }
        }
    }

    Ok(sheet)
}

fn parse_csv_data(data: &str, delimiter: char) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut current_row = Vec::new();
    let mut current_field = String::new();
    let mut in_quotes = false;
    let mut chars = data.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    current_field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                current_field.push(ch);
            }
        } else if ch == '"' {
            in_quotes = true;
        } else if ch == delimiter {
            current_row.push(std::mem::take(&mut current_field));
        } else if ch == '\n' {
            current_row.push(std::mem::take(&mut current_field));
            rows.push(std::mem::take(&mut current_row));
        } else if ch == '\r' {
            // Skip \r — handle \r\n and bare \r
            if chars.peek() != Some(&'\n') {
                current_row.push(std::mem::take(&mut current_field));
                rows.push(std::mem::take(&mut current_row));
            }
        } else {
            current_field.push(ch);
        }
    }

    // Flush last field/row if there's remaining data
    if !current_field.is_empty() || !current_row.is_empty() {
        current_row.push(current_field);
        rows.push(current_row);
    }

    rows
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
}
