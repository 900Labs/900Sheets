use crate::error::XlsxError;
use roxmltree::Document;
use sheets_core::cell::CellValue;
use sheets_core::format::CellFormat;
use sheets_core::workbook::Workbook;
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
    let sheet_names = read_workbook_xml(&mut archive)?;
    let sheet_files = read_workbook_rels(&mut archive)?;
    let styles = read_styles(&mut archive)?;

    let mut workbook = Workbook::new();
    if sheet_names.is_empty() {
        return Ok(workbook);
    }

    workbook.rename_sheet(0, &sheet_names[0]);

    for (_i, name) in sheet_names.iter().enumerate().skip(1) {
        workbook.add_sheet(name);
    }

    let mut total_cells = 0usize;
    for (i, sheet_file) in sheet_files.iter().enumerate() {
        if i >= workbook.sheet_count() {
            break;
        }
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
) -> Result<Vec<String>, XlsxError> {
    let xml = read_zip_file(archive, "xl/workbook.xml")?;
    let doc = Document::parse(&xml)?;

    let mut names = Vec::new();
    for node in doc.descendants() {
        if node.has_tag_name("sheet") {
            if let Some(name) = node.attribute("name") {
                names.push(name.to_string());
            }
        }
    }
    Ok(names)
}

fn read_workbook_rels<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<Vec<String>, XlsxError> {
    let xml = read_zip_file(archive, "xl/_rels/workbook.xml.rels")?;
    let doc = Document::parse(&xml)?;

    let mut targets = Vec::new();
    for node in doc.descendants() {
        if node.has_tag_name("Relationship") {
            if let Some(target) = node.attribute("Target") {
                if target.contains("worksheets/") {
                    let full_path = if target.starts_with('/') {
                        target.to_string()
                    } else {
                        format!("xl/{}", target)
                    };
                    targets.push(full_path);
                }
            }
        }
    }
    Ok(targets)
}

type CellList = Vec<((u32, u32), CellValue)>;

#[derive(Default)]
struct XlsxStyles {
    num_fmts: Vec<String>,
    fonts: Vec<CellFormat>,
    fills: Vec<String>,
    cell_xfs: Vec<(usize, usize, usize)>,
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

    for node in doc.descendants() {
        if node.has_tag_name("numFmt") {
            if let Some(code) = node.attribute("formatCode") {
                styles.num_fmts.push(code.to_string());
            }
        }
    }

    for node in doc.descendants() {
        if node.has_tag_name("font") {
            let mut fmt = CellFormat::default();
            for child in node.descendants() {
                if child.has_tag_name("b") {
                    fmt.bold = Some(true);
                } else if child.has_tag_name("i") {
                    fmt.italic = Some(true);
                } else if child.has_tag_name("u") {
                    fmt.underline = Some(true);
                } else if child.has_tag_name("sz") {
                    if let Some(val) = child.attribute("val") {
                        fmt.font_size = val.parse().ok();
                    }
                } else if child.has_tag_name("color") {
                    if let Some(rgb) = child.attribute("rgb") {
                        if rgb.len() == 8 {
                            fmt.font_color = Some(format!("#{}", &rgb[2..]));
                        }
                    }
                } else if child.has_tag_name("name") {
                    if let Some(val) = child.attribute("val") {
                        fmt.font_name = Some(val.to_string());
                    }
                }
            }
            styles.fonts.push(fmt);
        }
    }

    for node in doc.descendants() {
        if node.has_tag_name("fill") {
            let mut bg = None;
            for child in node.descendants() {
                if child.has_tag_name("fgColor") {
                    if let Some(rgb) = child.attribute("rgb") {
                        if rgb.len() == 8 {
                            bg = Some(format!("#{}", &rgb[2..]));
                        }
                    }
                }
            }
            styles.fills.push(bg.unwrap_or_default());
        }
    }

    for node in doc.descendants() {
        if node.has_tag_name("xf") {
            let font_id = node
                .attribute("fontId")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
            let fill_id = node
                .attribute("fillId")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
            let num_fmt_id = node
                .attribute("numFmtId")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
            styles.cell_xfs.push((font_id, fill_id, num_fmt_id));
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
            let (font_id, fill_id, num_fmt_id) = styles.cell_xfs[style_idx];
            let mut fmt = CellFormat::default();
            if font_id < styles.fonts.len() {
                fmt = fmt.merge(&styles.fonts[font_id]);
            }
            if fill_id < styles.fills.len() && !styles.fills[fill_id].is_empty() {
                fmt.bg_color = Some(styles.fills[fill_id].clone());
            }
            if num_fmt_id >= 164 {
                let custom_idx = num_fmt_id - 164;
                if custom_idx < styles.num_fmts.len() {
                    fmt.number_format = Some(styles.num_fmts[custom_idx].clone());
                }
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

            let cell_value = match type_attr {
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
                        let f_node = node.descendants().find(|n| n.has_tag_name("f"));
                        if let Some(_fn) = f_node {
                            let formula_text: String = _fn.text().unwrap_or("").to_string();
                            if !formula_text.is_empty() {
                                CellValue::formula(format!("={}", formula_text))
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
