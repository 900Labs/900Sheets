use crate::error::XlsxError;
use sheets_core::cell::{CellType, CellValue};
use sheets_core::format::CellFormat;
use sheets_core::workbook::Workbook;
use std::io::Write;

pub fn export_workbook(workbook: &Workbook) -> Result<Vec<u8>, XlsxError> {
    let buf: Vec<u8> = Vec::new();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(buf));
    let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();

    let style_table = build_style_table(workbook);
    let has_styles = !style_table.formats.is_empty();

    zip.start_file("[Content_Types].xml", opts)?;
    zip.write_all(generate_content_types_xml(workbook, has_styles).as_bytes())?;

    zip.start_file("_rels/.rels", opts)?;
    zip.write_all(ROOT_RELS_XML.as_bytes())?;

    zip.start_file("xl/workbook.xml", opts)?;
    zip.write_all(generate_workbook_xml(workbook).as_bytes())?;

    zip.start_file("xl/_rels/workbook.xml.rels", opts)?;
    zip.write_all(generate_workbook_rels_xml(workbook).as_bytes())?;

    let mut shared_strings = Vec::new();
    let mut shared_string_index: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for sheet_idx in 0..workbook.sheet_count() {
        if let Some(sheet) = workbook.sheet(sheet_idx) {
            for (_, cell) in sheet.iter_cells() {
                if cell.cell_type == CellType::Text {
                    shared_string_index
                        .entry(cell.raw.clone())
                        .or_insert_with(|| {
                            let idx = shared_strings.len();
                            shared_strings.push(cell.raw.clone());
                            idx
                        });
                }
            }
        }
    }

    zip.start_file("xl/sharedStrings.xml", opts)?;
    zip.write_all(generate_shared_strings_xml(&shared_strings).as_bytes())?;

    if has_styles {
        zip.start_file("xl/styles.xml", opts)?;
        zip.write_all(generate_styles_xml(&style_table).as_bytes())?;
    }

    for sheet_idx in 0..workbook.sheet_count() {
        let path = format!("xl/worksheets/sheet{}.xml", sheet_idx + 1);
        zip.start_file(&path, opts)?;
        let xml = generate_sheet_xml(workbook, sheet_idx, &shared_string_index, &style_table);
        zip.write_all(xml.as_bytes())?;
    }

    let result = zip.finish()?;
    Ok(result.into_inner())
}

fn generate_workbook_xml(workbook: &Workbook) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\n<sheets>\n",
    );
    for (i, sheet) in workbook.sheets().iter().enumerate() {
        xml.push_str(&format!(
            "<sheet name=\"{}\" sheetId=\"{}\" r:id=\"rId{}\"/>\n",
            escape_xml(sheet.name()),
            i + 1,
            i + 1
        ));
    }
    xml.push_str("</sheets>\n</workbook>");
    xml
}

fn generate_workbook_rels_xml(workbook: &Workbook) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n",
    );
    for i in 0..workbook.sheet_count() {
        xml.push_str(&format!(
            "<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet{}.xml\"/>\n",
            i + 1,
            i + 1
        ));
    }
    xml.push_str("</Relationships>");
    xml
}

fn generate_shared_strings_xml(strings: &[String]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<sst xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\n",
    );
    for s in strings {
        xml.push_str(&format!("<si><t>{}</t></si>\n", escape_xml(s)));
    }
    xml.push_str("</sst>");
    xml
}

fn generate_sheet_xml(
    workbook: &Workbook,
    sheet_idx: usize,
    shared_string_index: &std::collections::HashMap<String, usize>,
    style_table: &StyleTable,
) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\n<sheetData>\n",
    );

    let sheet = match workbook.sheet(sheet_idx) {
        Some(s) => s,
        None => {
            xml.push_str("</sheetData>\n</worksheet>");
            return xml;
        }
    };

    let mut rows: std::collections::BTreeMap<u32, Vec<(u32, &CellValue)>> =
        std::collections::BTreeMap::new();

    for ((row, col), cell) in sheet.iter_cells() {
        rows.entry(row).or_default().push((col, cell));
    }

    for (row, cells) in &rows {
        xml.push_str(&format!("<row r=\"{}\">\n", row + 1));
        let mut sorted_cells = cells.clone();
        sorted_cells.sort_by_key(|(col, _)| *col);
        for (col, cell) in &sorted_cells {
            let ref_str = format!("{}{}", col_to_label(*col), row + 1);
            let style_idx = sheet
                .get_format(*row, *col)
                .and_then(|fmt| style_table.get_index(fmt));
            let s_attr = match style_idx {
                Some(idx) => format!(" s=\"{}\"", idx),
                None => String::new(),
            };
            match cell.cell_type {
                CellType::Number => {
                    xml.push_str(&format!(
                        "<c r=\"{}\"{} t=\"n\"><v>{}</v></c>\n",
                        ref_str,
                        s_attr,
                        escape_xml(&cell.raw)
                    ));
                }
                CellType::Text => {
                    if let Some(&idx) = shared_string_index.get(&cell.raw) {
                        xml.push_str(&format!(
                            "<c r=\"{}\"{} t=\"s\"><v>{}</v></c>\n",
                            ref_str, s_attr, idx
                        ));
                    } else {
                        xml.push_str(&format!(
                            "<c r=\"{}\"{} t=\"inlineStr\"><is><t>{}</t></is></c>\n",
                            ref_str,
                            s_attr,
                            escape_xml(&cell.raw)
                        ));
                    }
                }
                CellType::Boolean => {
                    let val = if cell.raw.eq_ignore_ascii_case("true") {
                        "1"
                    } else {
                        "0"
                    };
                    xml.push_str(&format!(
                        "<c r=\"{}\"{} t=\"b\"><v>{}</v></c>\n",
                        ref_str, s_attr, val
                    ));
                }
                CellType::Formula => {
                    let formula = cell.raw.strip_prefix('=').unwrap_or(&cell.raw);
                    xml.push_str(&format!(
                        "<c r=\"{}\"{}><f>{}</f></c>\n",
                        ref_str,
                        s_attr,
                        escape_xml(formula)
                    ));
                }
                CellType::Error => {
                    xml.push_str(&format!(
                        "<c r=\"{}\"{} t=\"e\"><v>{}</v></c>\n",
                        ref_str,
                        s_attr,
                        escape_xml(&cell.raw)
                    ));
                }
                CellType::Empty => {}
            }
        }
        xml.push_str("</row>\n");
    }

    xml.push_str("</sheetData>\n</worksheet>");
    xml
}

fn col_to_label(col: u32) -> String {
    let mut label = String::new();
    let mut c = col;
    loop {
        label.insert(0, char::from_u32(65 + (c % 26)).unwrap_or('A'));
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    label
}

struct StyleTable {
    formats: Vec<CellFormat>,
    format_indices: std::collections::HashMap<String, usize>,
}

impl StyleTable {
    fn get_index(&self, fmt: &CellFormat) -> Option<usize> {
        if fmt.is_empty() {
            return None;
        }
        let key = serde_json::to_string(fmt).ok()?;
        self.format_indices.get(&key).copied()
    }
}

fn build_style_table(workbook: &Workbook) -> StyleTable {
    let mut formats: Vec<CellFormat> = Vec::new();
    let mut format_indices: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for sheet_idx in 0..workbook.sheet_count() {
        if let Some(sheet) = workbook.sheet(sheet_idx) {
            for ((row, col), _) in sheet.iter_cells() {
                if let Some(fmt) = sheet.get_format(row, col) {
                    if fmt.is_empty() {
                        continue;
                    }
                    let key = serde_json::to_string(fmt).unwrap_or_default();
                    if let std::collections::hash_map::Entry::Vacant(e) = format_indices.entry(key)
                    {
                        e.insert(formats.len());
                        formats.push(fmt.clone());
                    }
                }
            }
        }
    }

    StyleTable {
        formats,
        format_indices,
    }
}

fn generate_styles_xml(table: &StyleTable) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<styleSheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">\n",
    );

    let mut num_fmts: Vec<String> = Vec::new();
    let mut num_fmt_indices: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for fmt in &table.formats {
        if let Some(nf) = &fmt.number_format {
            if !num_fmt_indices.contains_key(nf) {
                num_fmt_indices.insert(nf.clone(), 164 + num_fmts.len());
                num_fmts.push(nf.clone());
            }
        }
    }
    if !num_fmts.is_empty() {
        xml.push_str(&format!("<numFmts count=\"{}\">\n", num_fmts.len()));
        for (i, nf) in num_fmts.iter().enumerate() {
            xml.push_str(&format!(
                "<numFmt numFmtId=\"{}\" formatCode=\"{}\"/>\n",
                164 + i,
                escape_xml(nf)
            ));
        }
        xml.push_str("</numFmts>\n");
    }

    xml.push_str(
        "<fonts count=\"1\">\n<font><sz val=\"11\"/><name val=\"Calibri\"/></font>\n</fonts>\n",
    );
    xml.push_str(
        "<fills count=\"1\">\n<fill><patternFill patternType=\"none\"/></fill>\n</fills>\n",
    );
    xml.push_str("<borders count=\"1\"><border/></borders>\n");

    xml.push_str(&format!(
        "<cellXfs count=\"{}\">\n",
        table.formats.len() + 1
    ));
    xml.push_str("<xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>\n");
    for fmt in &table.formats {
        let num_fmt_id = fmt
            .number_format
            .as_ref()
            .and_then(|nf| num_fmt_indices.get(nf).copied())
            .unwrap_or(0);
        let bold_attr = if fmt.bold == Some(true) {
            " b=\"1\""
        } else {
            ""
        };
        let italic_attr = if fmt.italic == Some(true) {
            " i=\"1\""
        } else {
            ""
        };
        xml.push_str(&format!(
            "<xf numFmtId=\"{}\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"{}{} applyFont=\"1\"/>\n",
            num_fmt_id, bold_attr, italic_attr
        ));
    }
    xml.push_str("</cellXfs>\n</styleSheet>");
    xml
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn generate_content_types_xml(workbook: &Workbook, has_styles: bool) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\n<Default Extension=\"xml\" ContentType=\"application/xml\"/>\n<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\n<Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>\n<Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml\"/>\n",
    );
    if has_styles {
        xml.push_str("<Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>\n");
    }
    for i in 0..workbook.sheet_count() {
        xml.push_str(&format!(
            "<Override PartName=\"/xl/worksheets/sheet{}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>\n",
            i + 1
        ));
    }
    xml.push_str("</Types>");
    xml
}

const ROOT_RELS_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/>
</Relationships>";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_empty_workbook() {
        let wb = Workbook::new();
        let data = export_workbook(&wb).unwrap();
        assert!(!data.is_empty());
    }

    #[test]
    fn test_export_with_data() {
        let mut wb = Workbook::new();
        let sheet = wb.sheet_mut(0).unwrap();
        sheet.set_cell_value(0, 0, "Hello".into());
        sheet.set_cell_value(0, 1, "42".into());
        sheet.set_cell_value(1, 0, "3.14".into());

        let data = export_workbook(&wb).unwrap();
        assert!(!data.is_empty());
    }

    #[test]
    fn test_export_import_roundtrip() {
        let mut wb = Workbook::new();
        let sheet = wb.sheet_mut(0).unwrap();
        sheet.set_cell_value(0, 0, "Hello".into());
        sheet.set_cell_value(0, 1, "42".into());
        sheet.set_cell_value(1, 0, "World".into());
        sheet.set_cell_value(1, 1, "3.14".into());

        let data = export_workbook(&wb).unwrap();
        let wb2 = crate::import_workbook(&data).unwrap();

        assert_eq!(wb2.sheet_count(), 1);
        assert_eq!(wb2.sheets()[0].cell_value(0, 0), Some("Hello".into()));
        assert_eq!(wb2.sheets()[0].cell_value(0, 1), Some("42".into()));
        assert_eq!(wb2.sheets()[0].cell_value(1, 0), Some("World".into()));
        assert_eq!(wb2.sheets()[0].cell_value(1, 1), Some("3.14".into()));
    }

    #[test]
    fn test_export_multiple_sheets() {
        let mut wb = Workbook::new();
        wb.rename_sheet(0, "First");
        wb.add_sheet("Second");
        wb.sheet_mut(0).unwrap().set_cell_value(0, 0, "A1".into());
        wb.sheet_mut(1).unwrap().set_cell_value(0, 0, "B1".into());

        let data = export_workbook(&wb).unwrap();
        let wb2 = crate::import_workbook(&data).unwrap();

        assert_eq!(wb2.sheet_count(), 2);
        assert_eq!(wb2.sheets()[0].name(), "First");
        assert_eq!(wb2.sheets()[1].name(), "Second");
        assert_eq!(wb2.sheets()[0].cell_value(0, 0), Some("A1".into()));
        assert_eq!(wb2.sheets()[1].cell_value(0, 0), Some("B1".into()));
    }

    #[test]
    fn test_col_to_label() {
        assert_eq!(col_to_label(0), "A");
        assert_eq!(col_to_label(25), "Z");
        assert_eq!(col_to_label(26), "AA");
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("a<b>c"), "a&lt;b&gt;c");
        assert_eq!(escape_xml("a&b"), "a&amp;b");
    }
}
