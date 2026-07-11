use crate::error::XlsxError;
use sheets_core::cell::CellType;
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
    zip.write_all(generate_workbook_rels_xml(workbook, has_styles).as_bytes())?;

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

fn generate_workbook_rels_xml(workbook: &Workbook, has_styles: bool) -> String {
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
    let mut next_id = workbook.sheet_count() + 1;
    xml.push_str(&format!(
        "<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings\" Target=\"sharedStrings.xml\"/>\n",
        next_id
    ));
    next_id += 1;
    if has_styles {
        xml.push_str(&format!(
            "<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>\n",
            next_id
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

    let mut positions = std::collections::BTreeSet::new();

    for ((row, col), _) in sheet.iter_cells() {
        positions.insert((row, col));
    }
    for ((row, col), _) in sheet.iter_formats() {
        positions.insert((row, col));
    }

    let mut rows: std::collections::BTreeMap<u32, Vec<u32>> = std::collections::BTreeMap::new();
    for (row, col) in positions {
        rows.entry(row).or_default().push(col);
    }

    for (row, cols) in &rows {
        xml.push_str(&format!("<row r=\"{}\">\n", row + 1));
        for col in cols {
            let ref_str = format!("{}{}", col_to_label(*col), row + 1);
            let style_idx = sheet
                .get_format(*row, *col)
                .and_then(|fmt| style_table.get_index(fmt));
            let s_attr = match style_idx {
                Some(idx) => format!(" s=\"{}\"", idx),
                None => String::new(),
            };
            let Some(cell) = sheet.cell(*row, *col) else {
                if !s_attr.is_empty() {
                    xml.push_str(&format!("<c r=\"{}\"{}/>\n", ref_str, s_attr));
                }
                continue;
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
        self.format_indices.get(&key).map(|index| index + 1)
    }
}

fn build_style_table(workbook: &Workbook) -> StyleTable {
    let mut formats: Vec<CellFormat> = Vec::new();
    let mut format_indices: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for sheet_idx in 0..workbook.sheet_count() {
        if let Some(sheet) = workbook.sheet(sheet_idx) {
            for (_, fmt) in sheet.iter_formats() {
                if fmt.is_empty() {
                    continue;
                }
                let key = serde_json::to_string(fmt).unwrap_or_default();
                if let std::collections::hash_map::Entry::Vacant(entry) = format_indices.entry(key)
                {
                    entry.insert(formats.len());
                    formats.push(fmt.clone());
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

    xml.push_str(&format!("<fonts count=\"{}\">\n", table.formats.len() + 1));
    xml.push_str("<font><sz val=\"11\"/><name val=\"Calibri\"/></font>\n");
    for fmt in &table.formats {
        xml.push_str("<font>");
        if fmt.bold == Some(true) {
            xml.push_str("<b/>");
        }
        if fmt.italic == Some(true) {
            xml.push_str("<i/>");
        }
        if fmt.underline == Some(true) {
            xml.push_str("<u/>");
        }
        if fmt.strikethrough == Some(true) {
            xml.push_str("<strike/>");
        }
        xml.push_str(&format!(
            "<sz val=\"{}\"/><name val=\"{}\"/>",
            fmt.font_size.unwrap_or(11.0),
            escape_xml(fmt.font_name.as_deref().unwrap_or("Calibri"))
        ));
        if let Some(color) = &fmt.font_color {
            xml.push_str(&format!("<color rgb=\"{}\"/>", xlsx_color(color)));
        }
        xml.push_str("</font>\n");
    }
    xml.push_str("</fonts>\n");

    xml.push_str(&format!("<fills count=\"{}\">\n", table.formats.len() + 2));
    xml.push_str("<fill><patternFill patternType=\"none\"/></fill>\n");
    xml.push_str("<fill><patternFill patternType=\"gray125\"/></fill>\n");
    for fmt in &table.formats {
        if let Some(color) = &fmt.bg_color {
            xml.push_str(&format!(
                "<fill><patternFill patternType=\"solid\"><fgColor rgb=\"{}\"/><bgColor indexed=\"64\"/></patternFill></fill>\n",
                xlsx_color(color)
            ));
        } else {
            xml.push_str("<fill><patternFill patternType=\"none\"/></fill>\n");
        }
    }
    xml.push_str("</fills>\n");

    xml.push_str(&format!(
        "<borders count=\"{}\"><border/>\n",
        table.formats.len() + 1
    ));
    for fmt in &table.formats {
        xml.push_str("<border>");
        write_border(&mut xml, "left", fmt.border_left.as_ref());
        write_border(&mut xml, "right", fmt.border_right.as_ref());
        write_border(&mut xml, "top", fmt.border_top.as_ref());
        write_border(&mut xml, "bottom", fmt.border_bottom.as_ref());
        xml.push_str("<diagonal/></border>\n");
    }
    xml.push_str("</borders>\n");

    xml.push_str(&format!(
        "<cellXfs count=\"{}\">\n",
        table.formats.len() + 1
    ));
    xml.push_str("<xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/>\n");
    for (index, fmt) in table.formats.iter().enumerate() {
        let num_fmt_id = fmt
            .number_format
            .as_ref()
            .and_then(|nf| num_fmt_indices.get(nf).copied())
            .unwrap_or(0);
        xml.push_str(&format!(
            "<xf numFmtId=\"{}\" fontId=\"{}\" fillId=\"{}\" borderId=\"{}\" xfId=\"0\" applyFont=\"1\" applyFill=\"1\" applyBorder=\"1\"{}>",
            num_fmt_id,
            index + 1,
            index + 2,
            index + 1,
            if num_fmt_id == 0 { "" } else { " applyNumberFormat=\"1\"" }
        ));
        if fmt.h_align.is_some() || fmt.v_align.is_some() || fmt.wrap_text.is_some() {
            xml.push_str("<alignment");
            if let Some(horizontal) = fmt.h_align {
                let value = match horizontal {
                    sheets_core::format::HorizontalAlignment::Left => "left",
                    sheets_core::format::HorizontalAlignment::Center => "center",
                    sheets_core::format::HorizontalAlignment::Right => "right",
                    sheets_core::format::HorizontalAlignment::General => "general",
                };
                xml.push_str(&format!(" horizontal=\"{value}\""));
            }
            if let Some(vertical) = fmt.v_align {
                let value = match vertical {
                    sheets_core::format::VerticalAlignment::Top => "top",
                    sheets_core::format::VerticalAlignment::Middle => "center",
                    sheets_core::format::VerticalAlignment::Bottom => "bottom",
                };
                xml.push_str(&format!(" vertical=\"{value}\""));
            }
            if let Some(wrap) = fmt.wrap_text {
                xml.push_str(if wrap {
                    " wrapText=\"1\""
                } else {
                    " wrapText=\"0\""
                });
            }
            xml.push_str("/>");
        }
        xml.push_str("</xf>\n");
    }
    xml.push_str("</cellXfs>\n</styleSheet>");
    xml
}

fn xlsx_color(color: &str) -> String {
    let hex = color.trim_start_matches('#');
    if hex.len() == 8 {
        hex.to_ascii_uppercase()
    } else if hex.len() == 6 {
        format!("FF{}", hex.to_ascii_uppercase())
    } else {
        "FF000000".to_string()
    }
}

fn write_border(xml: &mut String, side: &str, border: Option<&sheets_core::format::Border>) {
    use sheets_core::format::BorderStyle;
    let Some(border) = border else {
        xml.push_str(&format!("<{side}/>"));
        return;
    };
    let style = match border.style {
        BorderStyle::None => None,
        BorderStyle::Thin => Some("thin"),
        BorderStyle::Medium => Some("medium"),
        BorderStyle::Thick => Some("thick"),
        BorderStyle::Dotted => Some("dotted"),
        BorderStyle::Dashed => Some("dashed"),
        BorderStyle::Double => Some("double"),
    };
    let Some(style) = style else {
        xml.push_str(&format!("<{side}/>"));
        return;
    };
    xml.push_str(&format!("<{side} style=\"{style}\">"));
    if let Some(color) = &border.color {
        xml.push_str(&format!("<color rgb=\"{}\"/>", xlsx_color(color)));
    }
    xml.push_str(&format!("</{side}>"));
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
        wb.rename_sheet(0, "First").unwrap();
        wb.add_sheet("Second").unwrap();
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
    fn test_cross_sheet_formula_roundtrip_preserves_quoted_absolute_reference() {
        let mut workbook = Workbook::new();
        workbook.rename_sheet(0, "Annual Budget").unwrap();
        let report = workbook.add_sheet("Report").unwrap();
        workbook.sheet_mut(report).unwrap().set_cell_value(
            0,
            0,
            "=SUM('Annual Budget'!$A$1:$A$2)".into(),
        );

        let restored = crate::import_workbook(&export_workbook(&workbook).unwrap()).unwrap();
        assert_eq!(
            restored.sheet(1).unwrap().cell_value(0, 0),
            Some("=SUM('Annual Budget'!$A$1:$A$2)".into())
        );
    }

    #[test]
    fn test_export_import_preserves_complete_styles_and_formatted_blank_cells() {
        use sheets_core::format::{Border, BorderStyle, HorizontalAlignment, VerticalAlignment};

        let mut wb = Workbook::new();
        let sheet = wb.sheet_mut(0).unwrap();
        sheet.set_cell_value(0, 0, "Styled".into());
        let format = CellFormat {
            bold: Some(true),
            italic: Some(true),
            underline: Some(true),
            strikethrough: Some(true),
            font_size: Some(14.0),
            font_name: Some("Arial".into()),
            font_color: Some("#112233".into()),
            bg_color: Some("#AABBCC".into()),
            h_align: Some(HorizontalAlignment::Center),
            v_align: Some(VerticalAlignment::Middle),
            wrap_text: Some(true),
            number_format: Some("0.000".into()),
            border_top: Some(Border {
                style: BorderStyle::Thin,
                color: Some("#FF0000".into()),
            }),
            border_bottom: None,
            border_left: None,
            border_right: None,
        };
        sheet.set_format(0, 0, format.clone());
        sheet.set_format(4, 4, format.clone());

        let data = export_workbook(&wb).unwrap();
        let imported = crate::import_workbook(&data).unwrap();
        assert_eq!(imported.sheet(0).unwrap().get_format(0, 0), Some(&format));
        assert_eq!(imported.sheet(0).unwrap().get_format(4, 4), Some(&format));
        assert!(imported.sheet(0).unwrap().cell(4, 4).is_none());
    }

    #[test]
    fn test_export_package_declares_shared_strings_and_styles_relationships() {
        use std::io::Read;

        let mut workbook = Workbook::new();
        workbook
            .sheet_mut(0)
            .unwrap()
            .set_cell_value(0, 0, "text".into());
        workbook
            .sheet_mut(0)
            .unwrap()
            .set_format(0, 0, CellFormat::new().bold(true));
        let data = export_workbook(&workbook).unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data)).unwrap();
        let mut relationships = String::new();
        archive
            .by_name("xl/_rels/workbook.xml.rels")
            .unwrap()
            .read_to_string(&mut relationships)
            .unwrap();

        assert!(relationships.contains("relationships/sharedStrings"));
        assert!(relationships.contains("Target=\"sharedStrings.xml\""));
        assert!(relationships.contains("relationships/styles"));
        assert!(relationships.contains("Target=\"styles.xml\""));
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
