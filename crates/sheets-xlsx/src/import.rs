use crate::error::XlsxError;
use roxmltree::Document;
use sheets_core::cell::CellValue;
use sheets_core::format::CellFormat;
use sheets_core::workbook::Workbook;
use std::collections::HashMap;
use std::io::Read;

const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;
const MAX_XML_ENTRY_SIZE: u64 = 25 * 1024 * 1024;
const MAX_CELLS: usize = 10_000_000;
const MAX_ROWS: u32 = 1_000_000;
const MAX_COLS: u32 = 16_384;

pub fn import_workbook(data: &[u8]) -> Result<Workbook, XlsxError> {
    if data.len() as u64 > MAX_FILE_SIZE {
        return Err(XlsxError::FileTooLarge(data.len() as u64, MAX_FILE_SIZE));
    }

    let cursor = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor)?;

    let shared_strings = read_shared_strings(&mut archive)?;
    let sheets = read_workbook_xml(&mut archive)?;
    let sheet_files = read_workbook_rels(&mut archive)?;
    let styles = read_styles(&mut archive)?;

    let mut workbook = Workbook::new();
    if sheets.is_empty() {
        return Ok(workbook);
    }

    workbook
        .rename_sheet(0, &sheets[0].0)
        .map_err(|error| XlsxError::InvalidFormat(error.to_string()))?;

    for (name, _) in sheets.iter().skip(1) {
        workbook
            .add_sheet(name)
            .map_err(|error| XlsxError::InvalidFormat(error.to_string()))?;
    }

    let mut total_cells = 0usize;
    for (i, (_, relationship_id)) in sheets.iter().enumerate() {
        let sheet_file = sheet_files.get(relationship_id).ok_or_else(|| {
            XlsxError::InvalidFormat(format!(
                "Workbook sheet relationship {relationship_id} was not found"
            ))
        })?;
        let xml = read_zip_file(&mut archive, sheet_file)?;
        let cells = parse_sheet_xml(&xml, &shared_strings)?;
        total_cells += cells.len();
        if total_cells > MAX_CELLS {
            return Err(XlsxError::TooManyCells(total_cells, MAX_CELLS));
        }
        if let Some(sheet) = workbook.sheet_mut(i) {
            for ((row, col), value) in cells {
                sheet.set_cell(row, col, value);
            }
            apply_styles(sheet, &xml, &styles);
        }
    }

    Ok(workbook)
}

fn read_zip_file<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> Result<String, XlsxError> {
    read_zip_file_with_limit(archive, name, MAX_XML_ENTRY_SIZE)
}

fn read_zip_file_with_limit<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    max_entry_size: u64,
) -> Result<String, XlsxError> {
    let file = archive.by_name(name)?;
    if file.size() > max_entry_size {
        return Err(XlsxError::FileTooLarge(file.size(), max_entry_size));
    }
    let mut content = String::new();
    let mut limited = file.take(max_entry_size + 1);
    limited.read_to_string(&mut content)?;
    if content.len() as u64 > max_entry_size {
        return Err(XlsxError::FileTooLarge(
            content.len() as u64,
            max_entry_size,
        ));
    }
    Ok(content)
}

fn read_shared_strings<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<Vec<String>, XlsxError> {
    let xml = match read_zip_file(archive, "xl/sharedStrings.xml") {
        Ok(content) => content,
        Err(_) => return Ok(Vec::new()),
    };

    let doc = Document::parse(&xml)?;
    let mut strings = Vec::new();

    for node in doc.descendants() {
        if node.has_tag_name("si") {
            let text: String = node
                .descendants()
                .filter(|n| n.has_tag_name("t"))
                .filter_map(|n| n.text())
                .collect::<Vec<_>>()
                .join("");
            strings.push(text);
        }
    }

    Ok(strings)
}

fn read_workbook_xml<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<Vec<(String, String)>, XlsxError> {
    let xml = read_zip_file(archive, "xl/workbook.xml")?;
    let doc = Document::parse(&xml)?;

    let mut sheets = Vec::new();
    for node in doc.descendants() {
        if node.has_tag_name("sheet") {
            if let (Some(name), Some(relationship_id)) = (
                node.attribute("name"),
                node.attributes()
                    .find(|attribute| attribute.name() == "id")
                    .map(|attribute| attribute.value()),
            ) {
                sheets.push((name.to_string(), relationship_id.to_string()));
            }
        }
    }
    Ok(sheets)
}

fn read_workbook_rels<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<HashMap<String, String>, XlsxError> {
    let xml = read_zip_file(archive, "xl/_rels/workbook.xml.rels")?;
    let doc = Document::parse(&xml)?;

    let mut targets = HashMap::new();
    for node in doc.descendants() {
        if node.has_tag_name("Relationship") {
            if let (Some(id), Some(target)) = (node.attribute("Id"), node.attribute("Target")) {
                if target.contains("worksheets/") {
                    let full_path = if target.starts_with('/') {
                        target.trim_start_matches('/').to_string()
                    } else {
                        format!("xl/{}", target)
                    };
                    targets.insert(id.to_string(), full_path);
                }
            }
        }
    }
    Ok(targets)
}

type CellList = Vec<((u32, u32), CellValue)>;

#[derive(Default)]
struct XlsxStyles {
    num_fmts: HashMap<usize, String>,
    fonts: Vec<CellFormat>,
    fills: Vec<String>,
    borders: Vec<CellFormat>,
    cell_xfs: Vec<CellXf>,
}

#[derive(Default)]
struct CellXf {
    font_id: usize,
    fill_id: usize,
    border_id: usize,
    num_fmt_id: usize,
    alignment: CellFormat,
}

fn usize_attribute(node: roxmltree::Node<'_, '_>, name: &str) -> usize {
    node.attribute(name)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
}

fn parse_rgb_color(rgb: Option<&str>) -> Option<String> {
    let rgb = rgb?;
    match rgb.len() {
        8 => Some(format!("#{}", &rgb[2..])),
        6 => Some(format!("#{rgb}")),
        _ => None,
    }
}

fn parse_border(node: roxmltree::Node<'_, '_>) -> Option<sheets_core::format::Border> {
    use sheets_core::format::{Border, BorderStyle};
    let style = match node.attribute("style") {
        Some("thin") => BorderStyle::Thin,
        Some("medium") => BorderStyle::Medium,
        Some("thick") => BorderStyle::Thick,
        Some("dotted") => BorderStyle::Dotted,
        Some("dashed") => BorderStyle::Dashed,
        Some("double") => BorderStyle::Double,
        _ => return None,
    };
    let color = node
        .children()
        .find(|child| child.has_tag_name("color"))
        .and_then(|child| parse_rgb_color(child.attribute("rgb")));
    Some(Border { style, color })
}

fn builtin_number_format(id: usize) -> Option<&'static str> {
    match id {
        1 => Some("0"),
        2 => Some("0.00"),
        3 => Some("#,##0"),
        4 => Some("#,##0.00"),
        9 => Some("0%"),
        10 => Some("0.00%"),
        11 => Some("0.00E+00"),
        14 => Some("m/d/yy"),
        15 => Some("d-mmm-yy"),
        16 => Some("d-mmm"),
        17 => Some("mmm-yy"),
        18 => Some("h:mm AM/PM"),
        19 => Some("h:mm:ss AM/PM"),
        20 => Some("h:mm"),
        21 => Some("h:mm:ss"),
        22 => Some("m/d/yy h:mm"),
        37 => Some("#,##0;(#,##0)"),
        38 => Some("#,##0;[Red](#,##0)"),
        39 => Some("#,##0.00;(#,##0.00)"),
        40 => Some("#,##0.00;[Red](#,##0.00)"),
        49 => Some("@"),
        _ => None,
    }
}

fn read_styles<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<XlsxStyles, XlsxError> {
    let xml = match read_zip_file(archive, "xl/styles.xml") {
        Ok(content) => content,
        Err(_) => return Ok(XlsxStyles::default()),
    };
    let doc = Document::parse(&xml)?;

    let mut styles = XlsxStyles::default();

    for node in doc.descendants().filter(|node| node.has_tag_name("numFmt")) {
        if let (Some(id), Some(code)) = (
            node.attribute("numFmtId")
                .and_then(|value| value.parse::<usize>().ok()),
            node.attribute("formatCode"),
        ) {
            styles.num_fmts.insert(id, code.to_string());
        }
    }

    if let Some(fonts) = doc.descendants().find(|node| node.has_tag_name("fonts")) {
        for node in fonts.children().filter(|node| node.has_tag_name("font")) {
            let mut fmt = CellFormat::default();
            for child in node.descendants() {
                if child.has_tag_name("b") {
                    fmt.bold = Some(child.attribute("val") != Some("0"));
                } else if child.has_tag_name("i") {
                    fmt.italic = Some(child.attribute("val") != Some("0"));
                } else if child.has_tag_name("u") {
                    fmt.underline = Some(child.attribute("val") != Some("0"));
                } else if child.has_tag_name("strike") {
                    fmt.strikethrough = Some(child.attribute("val") != Some("0"));
                } else if child.has_tag_name("sz") {
                    fmt.font_size = child.attribute("val").and_then(|value| value.parse().ok());
                } else if child.has_tag_name("color") {
                    fmt.font_color = parse_rgb_color(child.attribute("rgb"));
                } else if child.has_tag_name("name") {
                    fmt.font_name = child.attribute("val").map(str::to_string);
                }
            }
            styles.fonts.push(fmt);
        }
    }

    if let Some(fills) = doc.descendants().find(|node| node.has_tag_name("fills")) {
        for node in fills.children().filter(|node| node.has_tag_name("fill")) {
            let bg = node
                .descendants()
                .find(|child| child.has_tag_name("fgColor"))
                .and_then(|child| parse_rgb_color(child.attribute("rgb")))
                .unwrap_or_default();
            styles.fills.push(bg);
        }
    }

    if let Some(borders) = doc.descendants().find(|node| node.has_tag_name("borders")) {
        for node in borders
            .children()
            .filter(|node| node.has_tag_name("border"))
        {
            let mut fmt = CellFormat::default();
            for child in node.children().filter(|node| node.is_element()) {
                let border = parse_border(child);
                match child.tag_name().name() {
                    "top" => fmt.border_top = border,
                    "bottom" => fmt.border_bottom = border,
                    "left" => fmt.border_left = border,
                    "right" => fmt.border_right = border,
                    _ => {}
                }
            }
            styles.borders.push(fmt);
        }
    }

    if let Some(cell_xfs) = doc.descendants().find(|node| node.has_tag_name("cellXfs")) {
        for node in cell_xfs.children().filter(|node| node.has_tag_name("xf")) {
            let mut xf = CellXf {
                font_id: usize_attribute(node, "fontId"),
                fill_id: usize_attribute(node, "fillId"),
                border_id: usize_attribute(node, "borderId"),
                num_fmt_id: usize_attribute(node, "numFmtId"),
                ..Default::default()
            };
            if let Some(alignment) = node
                .children()
                .find(|child| child.has_tag_name("alignment"))
            {
                xf.alignment.h_align = match alignment.attribute("horizontal") {
                    Some("left") => Some(sheets_core::format::HorizontalAlignment::Left),
                    Some("center") => Some(sheets_core::format::HorizontalAlignment::Center),
                    Some("right") => Some(sheets_core::format::HorizontalAlignment::Right),
                    Some("general") => Some(sheets_core::format::HorizontalAlignment::General),
                    _ => None,
                };
                xf.alignment.v_align = match alignment.attribute("vertical") {
                    Some("top") => Some(sheets_core::format::VerticalAlignment::Top),
                    Some("center") => Some(sheets_core::format::VerticalAlignment::Middle),
                    Some("bottom") => Some(sheets_core::format::VerticalAlignment::Bottom),
                    _ => None,
                };
                xf.alignment.wrap_text = alignment
                    .attribute("wrapText")
                    .map(|value| value == "1" || value.eq_ignore_ascii_case("true"));
            }
            styles.cell_xfs.push(xf);
        }
    }

    Ok(styles)
}

fn apply_styles(sheet: &mut sheets_core::sheet::Sheet, xml: &str, styles: &XlsxStyles) {
    let doc = match Document::parse(xml) {
        Ok(d) => d,
        Err(_) => return,
    };

    for node in doc.descendants() {
        if node.has_tag_name("c") {
            let ref_attr = node.attribute("r").unwrap_or("");
            let style_idx = match node.attribute("s").and_then(|v| v.parse::<usize>().ok()) {
                Some(idx) => idx,
                None => continue,
            };
            let (row, col) = match parse_cell_ref(ref_attr) {
                Some(rc) => rc,
                None => continue,
            };
            if style_idx >= styles.cell_xfs.len() {
                continue;
            }
            let xf = &styles.cell_xfs[style_idx];
            let mut fmt = CellFormat::default();
            if xf.font_id < styles.fonts.len() {
                fmt = fmt.merge(&styles.fonts[xf.font_id]);
            }
            if xf.fill_id < styles.fills.len() && !styles.fills[xf.fill_id].is_empty() {
                fmt.bg_color = Some(styles.fills[xf.fill_id].clone());
            }
            if xf.border_id < styles.borders.len() {
                fmt = fmt.merge(&styles.borders[xf.border_id]);
            }
            fmt = fmt.merge(&xf.alignment);
            if let Some(number_format) = styles
                .num_fmts
                .get(&xf.num_fmt_id)
                .cloned()
                .or_else(|| builtin_number_format(xf.num_fmt_id).map(str::to_string))
            {
                fmt.number_format = Some(number_format);
            }
            if !fmt.is_empty() {
                sheet.set_format(row, col, fmt);
            }
        }
    }
}

fn parse_sheet_xml(xml: &str, shared_strings: &[String]) -> Result<CellList, XlsxError> {
    let doc = Document::parse(xml)?;
    let mut cells = Vec::new();
    let mut shared_formulas: HashMap<String, (u32, u32, String)> = HashMap::new();

    for node in doc.descendants() {
        if node.has_tag_name("c") {
            let ref_attr = node.attribute("r").unwrap_or("");
            let type_attr = node.attribute("t").unwrap_or("n");

            let (row, col) = match parse_cell_ref(ref_attr) {
                Some(rc) => rc,
                None => continue,
            };

            let value_node = node.descendants().find(|n| n.has_tag_name("v"));
            let is_node = node.descendants().find(|n| n.has_tag_name("is"));
            let formula_node = node.descendants().find(|n| n.has_tag_name("f"));

            let cell_value = if let Some(formula_node) = formula_node {
                let formula_text = formula_node.text().unwrap_or("");
                if formula_node.attribute("t") == Some("shared") {
                    let shared_index = formula_node.attribute("si").ok_or_else(|| {
                        XlsxError::InvalidFormat(format!(
                            "Shared formula at {ref_attr} has no shared index"
                        ))
                    })?;
                    if !formula_text.is_empty() {
                        shared_formulas.insert(
                            shared_index.to_string(),
                            (row, col, formula_text.to_string()),
                        );
                        CellValue::formula(format!("={formula_text}"))
                    } else {
                        let (master_row, master_col, master_formula) =
                            shared_formulas.get(shared_index).ok_or_else(|| {
                                XlsxError::InvalidFormat(format!(
                                    "Shared formula follower at {ref_attr} has no preceding master"
                                ))
                            })?;
                        let translated = translate_shared_formula(
                            master_formula,
                            i64::from(row) - i64::from(*master_row),
                            i64::from(col) - i64::from(*master_col),
                        )?;
                        CellValue::formula(format!("={translated}"))
                    }
                } else if formula_text.is_empty() {
                    return Err(XlsxError::InvalidFormat(format!(
                        "Formula cell {ref_attr} contains no formula text"
                    )));
                } else {
                    CellValue::formula(format!("={formula_text}"))
                }
            } else {
                match type_attr {
                    "s" => {
                        let idx: usize = value_node
                            .and_then(|n| n.text())
                            .and_then(|t| t.parse().ok())
                            .unwrap_or(0);
                        if idx < shared_strings.len() {
                            CellValue::text(&shared_strings[idx])
                        } else {
                            CellValue::empty()
                        }
                    }
                    "str" | "inlineStr" => {
                        let text: String = if let Some(is) = is_node {
                            is.descendants()
                                .filter(|n| n.has_tag_name("t"))
                                .filter_map(|n| n.text())
                                .collect::<Vec<_>>()
                                .join("")
                        } else {
                            value_node.and_then(|n| n.text()).unwrap_or("").to_string()
                        };
                        CellValue::text(text)
                    }
                    "b" => {
                        let val = value_node.and_then(|n| n.text()).unwrap_or("0");
                        CellValue::boolean(val == "1" || val.eq_ignore_ascii_case("true"))
                    }
                    _ => {
                        if let Some(vn) = value_node {
                            if let Some(text) = vn.text() {
                                if let Ok(n) = text.parse::<f64>() {
                                    CellValue::number(n)
                                } else {
                                    CellValue::text(text)
                                }
                            } else {
                                CellValue::empty()
                            }
                        } else {
                            CellValue::empty()
                        }
                    }
                }
            };

            if !cell_value.is_empty() {
                cells.push(((row, col), cell_value));
            }
        }
    }

    Ok(cells)
}

fn translate_shared_formula(
    formula: &str,
    row_delta: i64,
    col_delta: i64,
) -> Result<String, XlsxError> {
    let bytes = formula.as_bytes();
    let mut output = String::with_capacity(formula.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            let start = index;
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'"' {
                    index += 1;
                    if index < bytes.len() && bytes[index] == b'"' {
                        index += 1;
                        continue;
                    }
                    break;
                }
                index += 1;
            }
            output.push_str(&formula[start..index]);
            continue;
        }
        let start = index;
        let absolute_col = bytes[index] == b'$';
        if absolute_col {
            index += 1;
        }
        let col_start = index;
        while index < bytes.len() && bytes[index].is_ascii_alphabetic() {
            index += 1;
        }
        let col_end = index;
        if col_end == col_start || col_end - col_start > 3 {
            output.push(bytes[start] as char);
            index = start + 1;
            continue;
        }
        let absolute_row = index < bytes.len() && bytes[index] == b'$';
        if absolute_row {
            index += 1;
        }
        let row_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        let boundary_before =
            start == 0 || (!bytes[start - 1].is_ascii_alphanumeric() && bytes[start - 1] != b'_');
        let boundary_after =
            index == bytes.len() || (!bytes[index].is_ascii_alphanumeric() && bytes[index] != b'_');
        if row_start == index
            || !boundary_before
            || !boundary_after
            || (index < bytes.len() && bytes[index] == b'(')
        {
            output.push(bytes[start] as char);
            index = start + 1;
            continue;
        }
        let Some(column) = col_label_to_index(&formula[col_start..col_end]) else {
            output.push_str(&formula[start..index]);
            continue;
        };
        let row_number = formula[row_start..index]
            .parse::<u32>()
            .map_err(|_| XlsxError::InvalidFormat("Invalid shared formula row".into()))?;
        if row_number == 0 {
            return Err(XlsxError::InvalidFormat(
                "Shared formula contains row zero".into(),
            ));
        }
        let translated_col = if absolute_col {
            i64::from(column)
        } else {
            i64::from(column) + col_delta
        };
        let translated_row = if absolute_row {
            i64::from(row_number - 1)
        } else {
            i64::from(row_number - 1) + row_delta
        };
        if translated_col < 0
            || translated_col >= i64::from(MAX_COLS)
            || translated_row < 0
            || translated_row >= i64::from(MAX_ROWS)
        {
            output.push_str("#REF!");
            continue;
        }
        if absolute_col {
            output.push('$');
        }
        output.push_str(&index_to_col_label(translated_col as u32));
        if absolute_row {
            output.push('$');
        }
        output.push_str(&(translated_row + 1).to_string());
    }
    Ok(output)
}

fn index_to_col_label(mut col: u32) -> String {
    let mut label = String::new();
    loop {
        label.insert(0, (b'A' + (col % 26) as u8) as char);
        if col < 26 {
            break;
        }
        col = col / 26 - 1;
    }
    label
}

fn parse_cell_ref(ref_str: &str) -> Option<(u32, u32)> {
    if ref_str.is_empty() {
        return None;
    }

    let mut col_part = String::new();
    let mut row_part = String::new();

    for ch in ref_str.chars() {
        if ch.is_ascii_alphabetic() {
            col_part.push(ch.to_ascii_uppercase());
        } else if ch.is_ascii_digit() {
            row_part.push(ch);
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

    let row = row - 1;
    if row >= MAX_ROWS || col >= MAX_COLS {
        return None;
    }

    Some((row, col))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cell_ref() {
        assert_eq!(parse_cell_ref("A1"), Some((0, 0)));
        assert_eq!(parse_cell_ref("B3"), Some((2, 1)));
        assert_eq!(parse_cell_ref("AA10"), Some((9, 26)));
        assert_eq!(parse_cell_ref(""), None);
        assert_eq!(parse_cell_ref("XFD1000000"), Some((999_999, 16_383)));
        assert_eq!(parse_cell_ref("XFE1"), None);
        assert_eq!(parse_cell_ref("A1000001"), None);
    }

    #[test]
    fn test_col_label_to_index() {
        assert_eq!(col_label_to_index("A"), Some(0));
        assert_eq!(col_label_to_index("Z"), Some(25));
        assert_eq!(col_label_to_index("AA"), Some(26));
    }

    #[test]
    fn test_import_empty_archive() {
        let result = import_workbook(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_import_minimal_xlsx() {
        let xlsx_data = create_minimal_xlsx();
        let workbook = import_workbook(&xlsx_data).unwrap();
        assert_eq!(workbook.sheet_count(), 1);
        assert_eq!(workbook.sheets()[0].name(), "Sheet1");
        assert_eq!(workbook.sheets()[0].cell_value(0, 0), Some("Hello".into()));
        assert_eq!(workbook.sheets()[0].cell_value(0, 1), Some("42".into()));
    }

    #[test]
    fn test_read_zip_file_enforces_decompressed_entry_limit() {
        use std::io::Write;

        let buf: Vec<u8> = Vec::new();
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(buf));
        let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
        zip.start_file("xl/workbook.xml", opts).unwrap();
        zip.write_all(b"abcdef").unwrap();
        let result = zip.finish().unwrap().into_inner();

        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(result)).unwrap();
        let result = read_zip_file_with_limit(&mut archive, "xl/workbook.xml", 5);
        assert!(matches!(result, Err(XlsxError::FileTooLarge(6, 5))));
    }

    #[test]
    fn test_parse_sheet_xml_skips_out_of_bounds_cells() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<sheetData>
<row r="1">
<c r="XFE1" t="n"><v>1</v></c>
<c r="A1000001" t="n"><v>2</v></c>
<c r="A1" t="n"><v>3</v></c>
</row>
</sheetData>
</worksheet>"#;
        let cells = parse_sheet_xml(xml, &[]).unwrap();
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].0, (0, 0));
        assert_eq!(cells[0].1.display, "3");
    }

    #[test]
    fn test_formula_with_cached_value_preserves_formula() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<sheetData><row r="1"><c r="A1"><f>1+1</f><v>2</v></c></row></sheetData>
</worksheet>"#;
        let cells = parse_sheet_xml(xml, &[]).unwrap();
        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].1.raw, "=1+1");
        assert!(cells[0].1.is_formula());
    }

    #[test]
    fn test_shared_formula_followers_are_expanded() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<sheetData>
  <row r="1"><c r="B1"><f t="shared" si="0" ref="B1:B3">A1*$D$1+LOG10(A1)+"A1"</f><v>2</v></c></row>
  <row r="2"><c r="B2"><f t="shared" si="0"/><v>4</v></c></row>
  <row r="3"><c r="B3"><f t="shared" si="0"/><v>6</v></c></row>
</sheetData>
</worksheet>"#;
        let cells = parse_sheet_xml(xml, &[]).unwrap();
        assert_eq!(cells[0].1.raw, "=A1*$D$1+LOG10(A1)+\"A1\"");
        assert_eq!(cells[1].1.raw, "=A2*$D$1+LOG10(A2)+\"A1\"");
        assert_eq!(cells[2].1.raw, "=A3*$D$1+LOG10(A3)+\"A1\"");
    }

    #[test]
    fn test_shared_formula_follower_without_master_fails() {
        let xml = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1"><f t="shared" si="4"/><v>1</v></c></row></sheetData></worksheet>"#;
        assert!(matches!(
            parse_sheet_xml(xml, &[]),
            Err(XlsxError::InvalidFormat(message)) if message.contains("no preceding master")
        ));
    }

    #[test]
    fn test_sheet_relationship_ids_control_worksheet_mapping() {
        use std::io::Write;

        let buf: Vec<u8> = Vec::new();
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(buf));
        let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
        zip.start_file("[Content_Types].xml", opts).unwrap();
        zip.write_all(BRACKET_CONTENT_TYPES).unwrap();
        zip.start_file("_rels/.rels", opts).unwrap();
        zip.write_all(RELS).unwrap();
        zip.start_file("xl/workbook.xml", opts).unwrap();
        zip.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets>
<sheet name=\"First\" sheetId=\"1\" r:id=\"rId2\"/><sheet name=\"Second\" sheetId=\"2\" r:id=\"rId1\"/>
</sheets></workbook>").unwrap();
        zip.start_file("xl/_rels/workbook.xml.rels", opts).unwrap();
        zip.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet2.xml\"/>
<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/>
</Relationships>").unwrap();
        zip.start_file("xl/sharedStrings.xml", opts).unwrap();
        zip.write_all(SHARED_STRINGS).unwrap();
        zip.start_file("xl/worksheets/sheet1.xml", opts).unwrap();
        zip.write_all(SHEET_XML).unwrap();
        zip.start_file("xl/worksheets/sheet2.xml", opts).unwrap();
        zip.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData><row r=\"1\"><c r=\"A1\" t=\"n\"><v>99</v></c></row></sheetData></worksheet>").unwrap();
        let data = zip.finish().unwrap().into_inner();

        let workbook = import_workbook(&data).unwrap();
        assert_eq!(workbook.sheet(0).unwrap().name(), "First");
        assert_eq!(
            workbook.sheet(0).unwrap().cell_value(0, 0),
            Some("Hello".into())
        );
        assert_eq!(workbook.sheet(1).unwrap().name(), "Second");
        assert_eq!(
            workbook.sheet(1).unwrap().cell_value(0, 0),
            Some("99".into())
        );
    }

    #[test]
    fn test_import_rejects_case_insensitive_duplicate_sheet_names() {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("xl/workbook.xml", options).unwrap();
        zip.write_all(br#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Data" sheetId="1" r:id="rId1"/><sheet name="data" sheetId="2" r:id="rId2"/></sheets></workbook>"#).unwrap();
        zip.start_file("xl/_rels/workbook.xml.rels", options)
            .unwrap();
        zip.write_all(br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/></Relationships>"#).unwrap();
        let data = zip.finish().unwrap().into_inner();

        assert!(matches!(
            import_workbook(&data),
            Err(XlsxError::InvalidFormat(message)) if message.contains("already exists")
        ));
    }

    fn create_minimal_xlsx() -> Vec<u8> {
        use std::io::Write;
        let buf: Vec<u8> = Vec::new();
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(buf));
        let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();

        zip.start_file("[Content_Types].xml", opts).unwrap();
        zip.write_all(BRACKET_CONTENT_TYPES).unwrap();

        zip.start_file("_rels/.rels", opts).unwrap();
        zip.write_all(RELS).unwrap();

        zip.start_file("xl/workbook.xml", opts).unwrap();
        zip.write_all(WORKBOOK_XML).unwrap();

        zip.start_file("xl/_rels/workbook.xml.rels", opts).unwrap();
        zip.write_all(WORKBOOK_RELS).unwrap();

        zip.start_file("xl/sharedStrings.xml", opts).unwrap();
        zip.write_all(SHARED_STRINGS).unwrap();

        zip.start_file("xl/worksheets/sheet1.xml", opts).unwrap();
        zip.write_all(SHEET_XML).unwrap();

        let result = zip.finish().unwrap();
        result.into_inner()
    }

    const BRACKET_CONTENT_TYPES: &[u8] = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">
<Default Extension=\"xml\" ContentType=\"application/xml\"/>
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>
<Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>
<Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>
<Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\"/>
</Types>";

    const RELS: &[u8] = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/>
</Relationships>";

    const WORKBOOK_XML: &[u8] = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">
<sheets>
<sheet name=\"Sheet1\" sheetId=\"1\" r:id=\"rId1\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"/>
</sheets>
</workbook>";

    const WORKBOOK_RELS: &[u8] = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/>
</Relationships>";

    const SHARED_STRINGS: &[u8] = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<sst xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">
<si><t>Hello</t></si>
</sst>";

    const SHEET_XML: &[u8] = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">
<sheetData>
<row r=\"1\">
<c r=\"A1\" t=\"s\"><v>0</v></c>
<c r=\"B1\" t=\"n\"><v>42</v></c>
</row>
</sheetData>
</worksheet>";
}
