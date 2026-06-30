use crate::cell::{CellType, CellValue};
use crate::sheet::Sheet;

pub fn sort_sheet(
    sheet: &Sheet,
    sort_col: u32,
    start_row: u32,
    end_row: u32,
    ascending: bool,
) -> Vec<((u32, u32), CellValue)> {
    let mut rows: Vec<Vec<((u32, u32), CellValue)>> = Vec::new();

    for row in start_row..=end_row {
        let mut row_data = Vec::new();
        let mut has_data = false;
        for col in 0..sheet.max_cols() {
            if let Some(cell) = sheet.cell(row, col) {
                if !cell.is_empty() {
                    row_data.push(((row, col), cell.clone()));
                    has_data = true;
                }
            }
        }
        if has_data {
            rows.push(row_data);
        }
    }

    rows.sort_by(|a, b| {
        let a_val = a
            .iter()
            .find(|((_, c), _)| *c == sort_col)
            .map(|(_, v)| v)
            .cloned()
            .unwrap_or(CellValue::empty());
        let b_val = b
            .iter()
            .find(|((_, c), _)| *c == sort_col)
            .map(|(_, v)| v)
            .cloned()
            .unwrap_or(CellValue::empty());
        let cmp = compare_cells(&a_val, &b_val);
        if ascending {
            cmp
        } else {
            cmp.reverse()
        }
    });

    let mut result = Vec::new();
    for (new_row_idx, row_data) in rows.iter().enumerate() {
        let new_row = start_row + new_row_idx as u32;
        for ((_, col), cell) in row_data {
            result.push(((new_row, *col), cell.clone()));
        }
    }
    result
}

pub fn apply_sort(sheet: &mut Sheet, sort_col: u32, start_row: u32, end_row: u32, ascending: bool) {
    let sorted = sort_sheet(sheet, sort_col, start_row, end_row, ascending);

    for row in start_row..=end_row {
        for col in 0..sheet.max_cols() {
            sheet.clear_cell(row, col);
        }
    }

    for ((row, col), cell) in sorted {
        sheet.set_cell(row, col, cell);
    }
}

fn compare_cells(a: &CellValue, b: &CellValue) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    if a.is_empty() && b.is_empty() {
        return Ordering::Equal;
    }
    if a.is_empty() {
        return Ordering::Less;
    }
    if b.is_empty() {
        return Ordering::Greater;
    }

    if a.cell_type == CellType::Number && b.cell_type == CellType::Number {
        let a_num = a.as_number().unwrap_or(0.0);
        let b_num = b.as_number().unwrap_or(0.0);
        return a_num.partial_cmp(&b_num).unwrap_or(Ordering::Equal);
    }

    if a.cell_type == CellType::Number {
        return Ordering::Less;
    }
    if b.cell_type == CellType::Number {
        return Ordering::Greater;
    }

    if a.cell_type == CellType::Boolean && b.cell_type == CellType::Boolean {
        let a_val = a.raw.eq_ignore_ascii_case("true");
        let b_val = b.raw.eq_ignore_ascii_case("true");
        return a_val.cmp(&b_val);
    }

    a.raw.cmp(&b.raw)
}

pub struct FilterCriteria {
    pub col: u32,
    pub condition: FilterCondition,
}

pub enum FilterCondition {
    Equals(String),
    Contains(String),
    StartsWith(String),
    EndsWith(String),
    NotEquals(String),
    NotContains(String),
    GreaterThan(f64),
    LessThan(f64),
    GreaterThanOrEqual(f64),
    LessThanOrEqual(f64),
    IsEmpty,
    IsNotEmpty,
}

impl FilterCondition {
    fn matches(&self, cell: &CellValue) -> bool {
        match self {
            FilterCondition::Equals(s) => cell.raw.eq_ignore_ascii_case(s),
            FilterCondition::Contains(s) => cell.raw.to_lowercase().contains(&s.to_lowercase()),
            FilterCondition::StartsWith(s) => {
                cell.raw.to_lowercase().starts_with(&s.to_lowercase())
            }
            FilterCondition::EndsWith(s) => cell.raw.to_lowercase().ends_with(&s.to_lowercase()),
            FilterCondition::NotEquals(s) => !cell.raw.eq_ignore_ascii_case(s),
            FilterCondition::NotContains(s) => !cell.raw.to_lowercase().contains(&s.to_lowercase()),
            FilterCondition::GreaterThan(n) => cell.as_number().is_some_and(|v| v > *n),
            FilterCondition::LessThan(n) => cell.as_number().is_some_and(|v| v < *n),
            FilterCondition::GreaterThanOrEqual(n) => cell.as_number().is_some_and(|v| v >= *n),
            FilterCondition::LessThanOrEqual(n) => cell.as_number().is_some_and(|v| v <= *n),
            FilterCondition::IsEmpty => cell.is_empty(),
            FilterCondition::IsNotEmpty => !cell.is_empty(),
        }
    }
}

pub fn filter_sheet(
    sheet: &Sheet,
    start_row: u32,
    end_row: u32,
    criteria: &[FilterCriteria],
) -> Vec<u32> {
    let mut matching_rows = Vec::new();

    for row in start_row..=end_row {
        let mut matches = true;
        for criterion in criteria {
            let cell = sheet
                .cell(row, criterion.col)
                .cloned()
                .unwrap_or(CellValue::empty());
            if !criterion.condition.matches(&cell) {
                matches = false;
                break;
            }
        }
        if matches {
            matching_rows.push(row);
        }
    }

    matching_rows
}

pub fn apply_filter(
    sheet: &Sheet,
    start_row: u32,
    end_row: u32,
    criteria: &[FilterCriteria],
) -> Vec<((u32, u32), CellValue)> {
    let matching_rows = filter_sheet(sheet, start_row, end_row, criteria);
    let mut result = Vec::new();

    for (new_row_idx, &src_row) in matching_rows.iter().enumerate() {
        let new_row = start_row + new_row_idx as u32;
        for col in 0..sheet.max_cols() {
            if let Some(cell) = sheet.cell(src_row, col) {
                if !cell.is_empty() {
                    result.push(((new_row, col), cell.clone()));
                }
            }
        }
    }

    result
}

pub struct SearchResult {
    pub row: u32,
    pub col: u32,
    pub matched_text: String,
}

pub fn find_in_sheet(sheet: &Sheet, query: &str, match_case: bool) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let query_lower = if !match_case {
        query.to_lowercase()
    } else {
        query.to_string()
    };

    for ((row, col), cell) in sheet.iter_cells() {
        let text = if match_case {
            cell.raw.clone()
        } else {
            cell.raw.to_lowercase()
        };

        if text.contains(&query_lower) {
            results.push(SearchResult {
                row,
                col,
                matched_text: cell.raw.clone(),
            });
        }
    }

    results.sort_by_key(|r| (r.row, r.col));
    results
}

pub fn find_first_in_sheet(sheet: &Sheet, query: &str, match_case: bool) -> Option<SearchResult> {
    find_in_sheet(sheet, query, match_case).into_iter().next()
}

pub fn replace_in_sheet(sheet: &mut Sheet, find: &str, replace: &str, match_case: bool) -> usize {
    let results = find_in_sheet(sheet, find, match_case);
    let count = results.len();

    for result in &results {
        let cell = sheet
            .cell(result.row, result.col)
            .cloned()
            .unwrap_or(CellValue::empty());
        let new_raw = if match_case {
            cell.raw.replacen(find, replace, 1)
        } else {
            case_insensitive_replace_n(&cell.raw, find, replace, 1)
        };

        if new_raw != cell.raw {
            let new_cell = match cell.cell_type {
                CellType::Text => CellValue::text(new_raw),
                CellType::Number => {
                    if let Ok(n) = new_raw.parse::<f64>() {
                        CellValue::number(n)
                    } else {
                        CellValue::text(new_raw)
                    }
                }
                CellType::Formula => CellValue::formula(new_raw),
                _ => CellValue::text(new_raw),
            };
            sheet.set_cell(result.row, result.col, new_cell);
        }
    }

    count
}

pub fn replace_all_in_sheet(
    sheet: &mut Sheet,
    find: &str,
    replace: &str,
    match_case: bool,
) -> usize {
    let results = find_in_sheet(sheet, find, match_case);
    let count = results.len();

    for result in &results {
        let cell = sheet
            .cell(result.row, result.col)
            .cloned()
            .unwrap_or(CellValue::empty());
        let new_raw = if match_case {
            cell.raw.replace(find, replace)
        } else {
            case_insensitive_replace(&cell.raw, find, replace)
        };

        if new_raw != cell.raw {
            let new_cell = match cell.cell_type {
                CellType::Text => CellValue::text(new_raw),
                CellType::Number => {
                    if let Ok(n) = new_raw.parse::<f64>() {
                        CellValue::number(n)
                    } else {
                        CellValue::text(new_raw)
                    }
                }
                CellType::Formula => CellValue::formula(new_raw),
                _ => CellValue::text(new_raw),
            };
            sheet.set_cell(result.row, result.col, new_cell);
        }
    }

    count
}

fn case_insensitive_replace(text: &str, find: &str, replace: &str) -> String {
    case_insensitive_replace_n(text, find, replace, usize::MAX)
}

fn case_insensitive_replace_n(text: &str, find: &str, replace: &str, max: usize) -> String {
    let text_lower = text.to_lowercase();
    let find_lower = find.to_lowercase();
    let mut result = String::new();
    let mut last_end = 0;

    for (count, (idx, _)) in text_lower.match_indices(&find_lower).enumerate() {
        if count >= max {
            break;
        }
        result.push_str(&text[last_end..idx]);
        result.push_str(replace);
        last_end = idx + find.len();
    }

    result.push_str(&text[last_end..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sheet() -> Sheet {
        let mut sheet = Sheet::new("Test");
        sheet.set_cell_value(0, 0, "Name".into());
        sheet.set_cell_value(0, 1, "Age".into());
        sheet.set_cell_value(1, 0, "Charlie".into());
        sheet.set_cell_value(1, 1, "30".into());
        sheet.set_cell_value(2, 0, "Alice".into());
        sheet.set_cell_value(2, 1, "25".into());
        sheet.set_cell_value(3, 0, "Bob".into());
        sheet.set_cell_value(3, 1, "35".into());
        sheet
    }

    #[test]
    fn test_sort_ascending() {
        let sheet = make_sheet();
        let sorted = sort_sheet(&sheet, 0, 1, 3, true);
        let names: Vec<&str> = sorted
            .iter()
            .filter(|((_, c), _)| *c == 0)
            .map(|(_, v)| v.raw.as_str())
            .collect();
        assert_eq!(names, vec!["Alice", "Bob", "Charlie"]);
    }

    #[test]
    fn test_sort_descending() {
        let sheet = make_sheet();
        let sorted = sort_sheet(&sheet, 0, 1, 3, false);
        let names: Vec<&str> = sorted
            .iter()
            .filter(|((_, c), _)| *c == 0)
            .map(|(_, v)| v.raw.as_str())
            .collect();
        assert_eq!(names, vec!["Charlie", "Bob", "Alice"]);
    }

    #[test]
    fn test_sort_by_number() {
        let sheet = make_sheet();
        let sorted = sort_sheet(&sheet, 1, 1, 3, true);
        let ages: Vec<&str> = sorted
            .iter()
            .filter(|((_, c), _)| *c == 1)
            .map(|(_, v)| v.raw.as_str())
            .collect();
        assert_eq!(ages, vec!["25", "30", "35"]);
    }

    #[test]
    fn test_apply_sort() {
        let mut sheet = make_sheet();
        apply_sort(&mut sheet, 0, 1, 3, true);
        assert_eq!(sheet.cell_value(1, 0), Some("Alice".into()));
        assert_eq!(sheet.cell_value(2, 0), Some("Bob".into()));
        assert_eq!(sheet.cell_value(3, 0), Some("Charlie".into()));
    }

    #[test]
    fn test_filter_contains() {
        let sheet = make_sheet();
        let criteria = vec![FilterCriteria {
            col: 0,
            condition: FilterCondition::Contains("a".into()),
        }];
        let rows = filter_sheet(&sheet, 1, 3, &criteria);
        assert_eq!(rows.len(), 2);
        assert!(rows.contains(&1));
        assert!(rows.contains(&2));
    }

    #[test]
    fn test_filter_equals() {
        let sheet = make_sheet();
        let criteria = vec![FilterCriteria {
            col: 0,
            condition: FilterCondition::Equals("Alice".into()),
        }];
        let rows = filter_sheet(&sheet, 1, 3, &criteria);
        assert_eq!(rows, vec![2]);
    }

    #[test]
    fn test_filter_greater_than() {
        let sheet = make_sheet();
        let criteria = vec![FilterCriteria {
            col: 1,
            condition: FilterCondition::GreaterThan(28.0),
        }];
        let rows = filter_sheet(&sheet, 1, 3, &criteria);
        assert_eq!(rows.len(), 2);
        assert!(rows.contains(&1));
        assert!(rows.contains(&3));
    }

    #[test]
    fn test_filter_multiple_criteria() {
        let sheet = make_sheet();
        let criteria = vec![
            FilterCriteria {
                col: 0,
                condition: FilterCondition::Contains("a".into()),
            },
            FilterCriteria {
                col: 1,
                condition: FilterCondition::GreaterThan(28.0),
            },
        ];
        let rows = filter_sheet(&sheet, 1, 3, &criteria);
        assert_eq!(rows, vec![1]);
    }

    #[test]
    fn test_apply_filter() {
        let sheet = make_sheet();
        let criteria = vec![FilterCriteria {
            col: 1,
            condition: FilterCondition::GreaterThan(28.0),
        }];
        let result = apply_filter(&sheet, 1, 3, &criteria);
        let names: Vec<&str> = result
            .iter()
            .filter(|((_, c), _)| *c == 0)
            .map(|(_, v)| v.raw.as_str())
            .collect();
        assert_eq!(names, vec!["Charlie", "Bob"]);
    }

    #[test]
    fn test_find_simple() {
        let sheet = make_sheet();
        let results = find_in_sheet(&sheet, "Alice", false);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].row, 2);
        assert_eq!(results[0].col, 0);
    }

    #[test]
    fn test_find_case_insensitive() {
        let sheet = make_sheet();
        let results = find_in_sheet(&sheet, "alice", false);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].matched_text, "Alice");
    }

    #[test]
    fn test_find_case_sensitive() {
        let sheet = make_sheet();
        let results = find_in_sheet(&sheet, "alice", true);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_find_partial() {
        let sheet = make_sheet();
        let results = find_in_sheet(&sheet, "ar", false);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].matched_text, "Charlie");
    }

    #[test]
    fn test_find_first() {
        let sheet = make_sheet();
        let result = find_first_in_sheet(&sheet, "Bob", false);
        assert!(result.is_some());
        assert_eq!(result.unwrap().row, 3);
    }

    #[test]
    fn test_replace() {
        let mut sheet = make_sheet();
        let count = replace_in_sheet(&mut sheet, "Alice", "Alicia", false);
        assert_eq!(count, 1);
        assert_eq!(sheet.cell_value(2, 0), Some("Alicia".into()));
    }

    #[test]
    fn test_replace_case_insensitive() {
        let mut sheet = make_sheet();
        let count = replace_in_sheet(&mut sheet, "alice", "Alicia", false);
        assert_eq!(count, 1);
        assert_eq!(sheet.cell_value(2, 0), Some("Alicia".into()));
    }

    #[test]
    fn test_replace_all() {
        let mut sheet = Sheet::new("Test");
        sheet.set_cell_value(0, 0, "foo bar".into());
        sheet.set_cell_value(1, 0, "foo baz".into());
        let count = replace_in_sheet(&mut sheet, "foo", "qux", false);
        assert_eq!(count, 2);
        assert_eq!(sheet.cell_value(0, 0), Some("qux bar".into()));
        assert_eq!(sheet.cell_value(1, 0), Some("qux baz".into()));
    }

    #[test]
    fn test_case_insensitive_replace() {
        assert_eq!(
            case_insensitive_replace("Hello World", "world", "Universe"),
            "Hello Universe"
        );
        assert_eq!(case_insensitive_replace("ABC abc AbC", "abc", "X"), "X X X");
    }

    #[test]
    fn test_filter_is_empty() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(4, 0, "".into());
        let criteria = vec![FilterCriteria {
            col: 0,
            condition: FilterCondition::IsNotEmpty,
        }];
        let rows = filter_sheet(&sheet, 1, 4, &criteria);
        assert_eq!(rows.len(), 3);
    }
}
