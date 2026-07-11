use crate::document::{XlsxDocument, XlsxSheetFeatures};
use crate::error::XlsxError;
use sheets_core::cell::CellType;
use sheets_core::format::CellFormat;
use sheets_core::workbook::Workbook;
use sheets_validation::{
    ConditionOperator, ConditionType, ConditionalFormat, ValidationErrorStyle, ValidationOperator,
    ValidationRule, ValidationType,
};
use std::io::Write;

const MAX_FEATURE_RECORDS: usize = 100_000;

pub fn export_workbook(workbook: &Workbook) -> Result<Vec<u8>, XlsxError> {
    export_document(&XlsxDocument::new(workbook.clone()))
}

pub fn export_document(document: &XlsxDocument) -> Result<Vec<u8>, XlsxError> {
    validate_document(document)?;
    let workbook = &document.workbook;
    let buf: Vec<u8> = Vec::new();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(buf));
    let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();

    let style_table = build_style_table(workbook, &document.sheet_features);
    let has_styles =
        !style_table.formats.is_empty() || !style_table.differential_formats.is_empty();

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
        let features = &document.sheet_features[sheet_idx];
        let xml = generate_sheet_xml(
            workbook,
            sheet_idx,
            &shared_string_index,
            &style_table,
            features,
        );
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
    features: &XlsxSheetFeatures,
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

    xml.push_str("</sheetData>\n");
    write_conditional_formats(&mut xml, &features.conditional_formats, style_table);
    write_data_validations(&mut xml, &features.validations);
    xml.push_str("</worksheet>");
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
    differential_formats: Vec<CellFormat>,
    differential_format_indices: std::collections::HashMap<String, usize>,
}

impl StyleTable {
    fn get_index(&self, fmt: &CellFormat) -> Option<usize> {
        if fmt.is_empty() {
            return None;
        }
        let key = serde_json::to_string(fmt).ok()?;
        self.format_indices.get(&key).map(|index| index + 1)
    }

    fn get_differential_index(&self, fmt: &CellFormat) -> Option<usize> {
        let key = serde_json::to_string(fmt).ok()?;
        self.differential_format_indices.get(&key).copied()
    }
}

fn build_style_table(workbook: &Workbook, sheet_features: &[XlsxSheetFeatures]) -> StyleTable {
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

    let mut differential_formats = Vec::new();
    let mut differential_format_indices = std::collections::HashMap::new();
    for format in sheet_features
        .iter()
        .flat_map(|features| features.conditional_formats.iter())
        .map(|rule| &rule.format)
        .filter(|format| !format.is_empty())
    {
        let key = serde_json::to_string(format).unwrap_or_default();
        if let std::collections::hash_map::Entry::Vacant(entry) =
            differential_format_indices.entry(key)
        {
            entry.insert(differential_formats.len());
            differential_formats.push(format.clone());
        }
    }

    StyleTable {
        formats,
        format_indices,
        differential_formats,
        differential_format_indices,
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
    xml.push_str("</cellXfs>\n");
    if !table.differential_formats.is_empty() {
        xml.push_str(&format!(
            "<dxfs count=\"{}\">\n",
            table.differential_formats.len()
        ));
        for format in &table.differential_formats {
            write_differential_format(&mut xml, format);
        }
        xml.push_str("</dxfs>\n");
    }
    xml.push_str("</styleSheet>");
    xml
}

fn write_data_validations(xml: &mut String, rules: &[ValidationRule]) {
    if rules.is_empty() {
        return;
    }
    xml.push_str(&format!("<dataValidations count=\"{}\">\n", rules.len()));
    for rule in rules {
        let validation = &rule.validation;
        xml.push_str(&format!(
            "<dataValidation type=\"{}\" allowBlank=\"{}\" showDropDown=\"{}\" errorStyle=\"{}\" sqref=\"{}\"",
            validation_type_name(&validation.validation_type),
            bool_xml(validation.allow_blank),
            // OOXML showDropDown has inverted semantics: true hides the arrow.
            bool_xml(!validation.show_dropdown),
            validation_error_style_name(&validation.error_style),
            range_to_sqref(rule.range),
        ));
        if !matches!(
            validation.validation_type,
            ValidationType::List | ValidationType::Custom
        ) {
            xml.push_str(&format!(
                " operator=\"{}\"",
                validation_operator_name(&validation.operator)
            ));
        }
        write_optional_attribute(xml, "errorTitle", validation.error_title.as_deref());
        write_optional_attribute(xml, "error", validation.error_message.as_deref());
        write_optional_attribute(xml, "promptTitle", validation.prompt_title.as_deref());
        write_optional_attribute(xml, "prompt", validation.prompt_message.as_deref());
        xml.push_str(">\n");

        let formula1 = match validation.validation_type {
            ValidationType::List => validation.source.as_deref().map(list_source_formula),
            _ => validation.formula1.clone(),
        };
        if let Some(formula) = formula1.as_deref() {
            xml.push_str(&format!("<formula1>{}</formula1>\n", escape_xml(formula)));
        }
        if let Some(formula) = validation.formula2.as_deref() {
            xml.push_str(&format!("<formula2>{}</formula2>\n", escape_xml(formula)));
        }
        xml.push_str("</dataValidation>\n");
    }
    xml.push_str("</dataValidations>\n");
}

fn write_conditional_formats(xml: &mut String, rules: &[ConditionalFormat], styles: &StyleTable) {
    let priorities = normalized_conditional_priorities(rules);
    for (rule, priority) in rules.iter().zip(priorities) {
        let (rule_type, operator) = match rule.condition_type {
            ConditionType::CellValue => (
                "cellIs",
                rule.operator.as_ref().map(condition_operator_name),
            ),
            ConditionType::Formula => ("expression", None),
            ConditionType::TextContains => ("containsText", None),
            ConditionType::TextNotContains => ("notContainsText", None),
            ConditionType::TextBeginsWith => ("beginsWith", None),
            ConditionType::TextEndsWith => ("endsWith", None),
            ConditionType::Blanks => ("containsBlanks", None),
            ConditionType::NoBlanks => ("notContainsBlanks", None),
            ConditionType::Duplicate => ("duplicateValues", None),
            _ => unreachable!("export validation rejects unsupported conditional-format types"),
        };
        xml.push_str(&format!(
            "<conditionalFormatting sqref=\"{}\"><cfRule type=\"{}\" priority=\"{}\"",
            range_to_sqref(rule.range),
            rule_type,
            priority,
        ));
        if let Some(operator) = operator {
            xml.push_str(&format!(" operator=\"{operator}\""));
        }
        if let Some(dxf_id) = styles.get_differential_index(&rule.format) {
            xml.push_str(&format!(" dxfId=\"{dxf_id}\""));
        }
        if rule.stop_if_true {
            xml.push_str(" stopIfTrue=\"1\"");
        }
        if matches!(
            rule.condition_type,
            ConditionType::TextContains
                | ConditionType::TextNotContains
                | ConditionType::TextBeginsWith
                | ConditionType::TextEndsWith
        ) {
            write_optional_attribute(xml, "text", rule.value1.as_deref());
        }
        xml.push('>');
        let generated_formula = generated_conditional_formula(rule);
        let first_formula = generated_formula.as_deref().or(rule.value1.as_deref());
        if let Some(value) = first_formula {
            xml.push_str(&format!(
                "<formula>{}</formula>",
                escape_xml(value.trim_start_matches('='))
            ));
        }
        if let Some(value) = rule.value2.as_deref() {
            xml.push_str(&format!(
                "<formula>{}</formula>",
                escape_xml(value.trim_start_matches('='))
            ));
        }
        xml.push_str("</cfRule></conditionalFormatting>\n");
    }
}

fn normalized_conditional_priorities(rules: &[ConditionalFormat]) -> Vec<u32> {
    let mut used = std::collections::HashSet::new();
    let mut next_available = 1u32;
    rules
        .iter()
        .map(|rule| {
            if rule.priority > 0 && used.insert(rule.priority) {
                return rule.priority;
            }
            while used.contains(&next_available) {
                next_available = next_available.saturating_add(1);
            }
            let priority = next_available;
            used.insert(priority);
            next_available = next_available.saturating_add(1);
            priority
        })
        .collect()
}

fn write_differential_format(xml: &mut String, format: &CellFormat) {
    xml.push_str("<dxf>");
    if format.bold.is_some()
        || format.italic.is_some()
        || format.underline.is_some()
        || format.strikethrough.is_some()
        || format.font_size.is_some()
        || format.font_name.is_some()
        || format.font_color.is_some()
    {
        xml.push_str("<font>");
        if let Some(value) = format.bold {
            xml.push_str(if value { "<b/>" } else { "<b val=\"0\"/>" });
        }
        if let Some(value) = format.italic {
            xml.push_str(if value { "<i/>" } else { "<i val=\"0\"/>" });
        }
        if let Some(value) = format.underline {
            xml.push_str(if value { "<u/>" } else { "<u val=\"0\"/>" });
        }
        if let Some(value) = format.strikethrough {
            xml.push_str(if value {
                "<strike/>"
            } else {
                "<strike val=\"0\"/>"
            });
        }
        if let Some(size) = format.font_size {
            xml.push_str(&format!("<sz val=\"{size}\"/>"));
        }
        if let Some(name) = format.font_name.as_deref() {
            xml.push_str(&format!("<name val=\"{}\"/>", escape_xml(name)));
        }
        if let Some(color) = format.font_color.as_deref() {
            xml.push_str(&format!("<color rgb=\"{}\"/>", xlsx_color(color)));
        }
        xml.push_str("</font>");
    }
    if let Some(color) = format.bg_color.as_deref() {
        xml.push_str(&format!(
            "<fill><patternFill patternType=\"solid\"><fgColor rgb=\"{}\"/><bgColor indexed=\"64\"/></patternFill></fill>",
            xlsx_color(color)
        ));
    }
    if format.border_top.is_some()
        || format.border_bottom.is_some()
        || format.border_left.is_some()
        || format.border_right.is_some()
    {
        xml.push_str("<border>");
        write_border(xml, "left", format.border_left.as_ref());
        write_border(xml, "right", format.border_right.as_ref());
        write_border(xml, "top", format.border_top.as_ref());
        write_border(xml, "bottom", format.border_bottom.as_ref());
        xml.push_str("<diagonal/></border>");
    }
    if let Some(number_format) = format.number_format.as_deref() {
        xml.push_str(&format!(
            "<numFmt numFmtId=\"0\" formatCode=\"{}\"/>",
            escape_xml(number_format)
        ));
    }
    xml.push_str("</dxf>\n");
}

fn validate_document(document: &XlsxDocument) -> Result<(), XlsxError> {
    let sheet_count = document.workbook.sheet_count();
    if document.sheet_features.len() != sheet_count {
        return Err(XlsxError::InvalidFormat(format!(
            "XLSX feature mapping has {} sheet entries for a workbook with {sheet_count} sheets",
            document.sheet_features.len()
        )));
    }
    let mut total = 0usize;
    for (sheet_index, features) in document.sheet_features.iter().enumerate() {
        total = total
            .checked_add(features.validations.len())
            .and_then(|count| count.checked_add(features.conditional_formats.len()))
            .ok_or_else(|| XlsxError::InvalidFormat("Too many worksheet feature records".into()))?;
        if total > MAX_FEATURE_RECORDS {
            return Err(XlsxError::InvalidFormat(format!(
                "XLSX document contains more than {MAX_FEATURE_RECORDS} worksheet feature records"
            )));
        }
        for range in features
            .validations
            .iter()
            .map(|rule| rule.range)
            .chain(features.conditional_formats.iter().map(|rule| rule.range))
        {
            if !valid_range(range) {
                return Err(XlsxError::InvalidFormat(format!(
                    "Sheet {} contains an invalid worksheet feature range",
                    sheet_index + 1
                )));
            }
        }
        for rule in &features.conditional_formats {
            if !matches!(
                rule.condition_type,
                ConditionType::CellValue
                    | ConditionType::Formula
                    | ConditionType::TextContains
                    | ConditionType::TextNotContains
                    | ConditionType::TextBeginsWith
                    | ConditionType::TextEndsWith
                    | ConditionType::Blanks
                    | ConditionType::NoBlanks
                    | ConditionType::Duplicate
            ) {
                return Err(XlsxError::InvalidFormat(format!(
                    "Sheet {} contains a conditional-format type that XLSX export does not support",
                    sheet_index + 1
                )));
            }
            if rule.condition_type == ConditionType::CellValue && rule.operator.is_none() {
                return Err(XlsxError::InvalidFormat(format!(
                    "Sheet {} contains a cell-value conditional format without an operator",
                    sheet_index + 1
                )));
            }
        }
    }
    Ok(())
}

fn generated_conditional_formula(rule: &ConditionalFormat) -> Option<String> {
    let text = rule.value1.as_deref().unwrap_or("").replace('"', "\"\"");
    let (row, col, _, _) = rule.range;
    let cell = format!("{}{}", col_to_label(col), row + 1);
    match rule.condition_type {
        ConditionType::TextContains => Some(format!("NOT(ISERROR(SEARCH(\"{text}\",{cell})))")),
        ConditionType::TextNotContains => Some(format!("ISERROR(SEARCH(\"{text}\",{cell}))")),
        ConditionType::TextBeginsWith => Some(format!("LEFT({cell},LEN(\"{text}\"))=\"{text}\"")),
        ConditionType::TextEndsWith => Some(format!("RIGHT({cell},LEN(\"{text}\"))=\"{text}\"")),
        ConditionType::Blanks => Some(format!("LEN(TRIM({cell}))=0")),
        ConditionType::NoBlanks => Some(format!("LEN(TRIM({cell}))>0")),
        _ => None,
    }
}

fn validation_type_name(value: &ValidationType) -> &'static str {
    match value {
        ValidationType::WholeNumber => "whole",
        ValidationType::Decimal => "decimal",
        ValidationType::List => "list",
        ValidationType::Date => "date",
        ValidationType::Time => "time",
        ValidationType::TextLength => "textLength",
        ValidationType::Custom => "custom",
    }
}

fn validation_operator_name(value: &ValidationOperator) -> &'static str {
    match value {
        ValidationOperator::Between => "between",
        ValidationOperator::NotBetween => "notBetween",
        ValidationOperator::Equal => "equal",
        ValidationOperator::NotEqual => "notEqual",
        ValidationOperator::GreaterThan => "greaterThan",
        ValidationOperator::LessThan => "lessThan",
        ValidationOperator::GreaterThanOrEqual => "greaterThanOrEqual",
        ValidationOperator::LessThanOrEqual => "lessThanOrEqual",
    }
}

fn condition_operator_name(value: &ConditionOperator) -> &'static str {
    match value {
        ConditionOperator::Between => "between",
        ConditionOperator::NotBetween => "notBetween",
        ConditionOperator::Equal => "equal",
        ConditionOperator::NotEqual => "notEqual",
        ConditionOperator::GreaterThan => "greaterThan",
        ConditionOperator::LessThan => "lessThan",
        ConditionOperator::GreaterThanOrEqual => "greaterThanOrEqual",
        ConditionOperator::LessThanOrEqual => "lessThanOrEqual",
    }
}

fn validation_error_style_name(value: &ValidationErrorStyle) -> &'static str {
    match value {
        ValidationErrorStyle::Stop => "stop",
        ValidationErrorStyle::Warning => "warning",
        ValidationErrorStyle::Information => "information",
    }
}

fn list_source_formula(source: &str) -> String {
    if let Some(formula) = source.strip_prefix('=') {
        formula.to_string()
    } else {
        format!("\"{}\"", source.replace('"', "\"\""))
    }
}

fn write_optional_attribute(xml: &mut String, name: &str, value: Option<&str>) {
    if let Some(value) = value {
        xml.push_str(&format!(" {name}=\"{}\"", escape_xml(value)));
    }
}

fn bool_xml(value: bool) -> &'static str {
    if value {
        "1"
    } else {
        "0"
    }
}

fn valid_range((start_row, start_col, end_row, end_col): (u32, u32, u32, u32)) -> bool {
    start_row <= end_row && start_col <= end_col && end_row < 1_000_000 && end_col < 16_384
}

fn range_to_sqref((start_row, start_col, end_row, end_col): (u32, u32, u32, u32)) -> String {
    let start = format!("{}{}", col_to_label(start_col), start_row + 1);
    let end = format!("{}{}", col_to_label(end_col), end_row + 1);
    if start == end {
        start
    } else {
        format!("{start}:{end}")
    }
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
    fn generated_compatibility_document_roundtrips_supported_features() {
        let fixture = sheets_fixtures::community_budget_compatibility_fixture();
        let document = XlsxDocument {
            workbook: fixture.workbook,
            sheet_features: fixture
                .validations
                .into_iter()
                .zip(fixture.conditional_formats)
                .map(|(validations, conditional_formats)| XlsxSheetFeatures {
                    validations,
                    conditional_formats,
                })
                .collect(),
        };

        let exported = export_document(&document).unwrap();
        let restored = crate::import_document(&exported).unwrap();
        assert_eq!(restored.workbook.sheet_count(), 2);
        assert_eq!(restored.sheet_features[0].validations.len(), 1);
        assert_eq!(
            restored.sheet_features[0].validations[0]
                .validation
                .source
                .as_deref(),
            Some("Venue,Supplies,Transport")
        );
        assert_eq!(restored.sheet_features[1].validations.len(), 1);
        let conditional = &restored.sheet_features[0].conditional_formats[0];
        assert_eq!(conditional.condition_type, ConditionType::CellValue);
        assert_eq!(conditional.operator, Some(ConditionOperator::LessThan));
        assert_eq!(conditional.value1.as_deref(), Some("0"));
        assert_eq!(conditional.format.font_color.as_deref(), Some("#9C0006"));
        assert_eq!(conditional.format.bg_color.as_deref(), Some("#FFC7CE"));
    }

    #[test]
    fn validation_fields_and_xml_escaping_roundtrip_exactly() {
        use sheets_validation::{DataValidation, ValidationErrorStyle};

        let mut validation =
            DataValidation::whole_number(ValidationOperator::GreaterThanOrEqual, "2", "99");
        validation.allow_blank = false;
        validation.show_dropdown = false;
        validation.error_style = ValidationErrorStyle::Warning;
        validation.error_title = Some("Check <range> & retry".into());
        validation.error_message = Some("Use 2 through 99, not \"none\".".into());
        validation.prompt_title = Some("Invented values".into());
        validation.prompt_message = Some("No personal data & no names".into());
        let document = XlsxDocument {
            workbook: Workbook::new(),
            sheet_features: vec![XlsxSheetFeatures {
                validations: vec![ValidationRule {
                    range: (1, 1, 4, 2),
                    validation: validation.clone(),
                }],
                conditional_formats: Vec::new(),
            }],
        };

        let restored = crate::import_document(&export_document(&document).unwrap()).unwrap();
        assert_eq!(
            restored.sheet_features[0].validations[0].validation,
            validation
        );
        assert_eq!(
            restored.sheet_features[0].validations[0].range,
            (1, 1, 4, 2)
        );
    }

    #[test]
    fn list_validation_literals_references_and_names_roundtrip_without_ambiguity() {
        use sheets_validation::DataValidation;

        let sources = ["Yes,No", "Ready", "=$A$1", "=$A$1:$A$3", "=AllowedValues"];
        let validations = sources
            .iter()
            .enumerate()
            .map(|(row, source)| ValidationRule {
                range: (row as u32, 0, row as u32, 0),
                validation: DataValidation::list(source),
            })
            .collect();
        let document = XlsxDocument {
            workbook: Workbook::new(),
            sheet_features: vec![XlsxSheetFeatures {
                validations,
                conditional_formats: Vec::new(),
            }],
        };

        let restored = crate::import_document(&export_document(&document).unwrap()).unwrap();
        let restored_sources: Vec<&str> = restored.sheet_features[0]
            .validations
            .iter()
            .map(|rule| rule.validation.source.as_deref().unwrap())
            .collect();
        assert_eq!(restored_sources, sources);
    }

    #[test]
    fn conditional_format_priorities_stop_flags_and_false_dxf_values_roundtrip() {
        let false_format = CellFormat {
            bold: Some(false),
            italic: Some(false),
            underline: Some(false),
            strikethrough: Some(false),
            ..Default::default()
        };
        let rules = vec![
            ConditionalFormat {
                id: "later".into(),
                condition_type: ConditionType::Duplicate,
                range: (0, 0, 2, 0),
                format: false_format.clone(),
                priority: 9,
                stop_if_true: true,
                ..Default::default()
            },
            ConditionalFormat {
                id: "earlier".into(),
                condition_type: ConditionType::Blanks,
                range: (0, 1, 2, 1),
                priority: 2,
                ..Default::default()
            },
        ];
        let document = XlsxDocument {
            workbook: Workbook::new(),
            sheet_features: vec![XlsxSheetFeatures {
                validations: Vec::new(),
                conditional_formats: rules,
            }],
        };

        let restored = crate::import_document(&export_document(&document).unwrap()).unwrap();
        let restored = &restored.sheet_features[0].conditional_formats;
        assert_eq!(restored[0].priority, 9);
        assert_eq!(restored[1].priority, 2);
        assert!(restored[0].stop_if_true);
        assert_eq!(restored[0].format.bold, Some(false));
        assert_eq!(restored[0].format.italic, Some(false));
        assert_eq!(restored[0].format.underline, Some(false));
        assert_eq!(restored[0].format.strikethrough, Some(false));
    }

    #[test]
    fn duplicate_or_missing_conditional_priorities_are_normalized_uniquely() {
        let rules = vec![
            ConditionalFormat {
                priority: 4,
                ..Default::default()
            },
            ConditionalFormat {
                priority: 4,
                ..Default::default()
            },
            ConditionalFormat::default(),
        ];
        assert_eq!(normalized_conditional_priorities(&rules), vec![4, 1, 2]);
    }

    #[test]
    fn user_visible_conditional_format_types_roundtrip_without_silent_loss() {
        let types = [
            ConditionType::TextContains,
            ConditionType::TextNotContains,
            ConditionType::TextBeginsWith,
            ConditionType::TextEndsWith,
            ConditionType::Blanks,
            ConditionType::NoBlanks,
            ConditionType::Duplicate,
        ];
        let rules = types
            .iter()
            .enumerate()
            .map(|(index, condition_type)| ConditionalFormat {
                id: format!("rule-{index}"),
                condition_type: condition_type.clone(),
                range: (index as u32, 0, index as u32, 2),
                value1: matches!(
                    condition_type,
                    ConditionType::TextContains
                        | ConditionType::TextNotContains
                        | ConditionType::TextBeginsWith
                        | ConditionType::TextEndsWith
                )
                .then(|| "community \"check\"".into()),
                format: CellFormat::new().bold(true),
                ..Default::default()
            })
            .collect();
        let document = XlsxDocument {
            workbook: Workbook::new(),
            sheet_features: vec![XlsxSheetFeatures {
                validations: Vec::new(),
                conditional_formats: rules,
            }],
        };

        let restored = crate::import_document(&export_document(&document).unwrap()).unwrap();
        let restored_rules = &restored.sheet_features[0].conditional_formats;
        assert_eq!(restored_rules.len(), types.len());
        for (rule, expected) in restored_rules.iter().zip(types) {
            assert_eq!(rule.condition_type, expected);
            assert_eq!(rule.format.bold, Some(true));
        }
        assert_eq!(
            restored_rules[0].value1.as_deref(),
            Some("community \"check\"")
        );
    }

    #[test]
    fn export_rejects_feature_mapping_mismatches_invalid_ranges_and_unsupported_rules() {
        let missing_mapping = XlsxDocument {
            workbook: Workbook::new(),
            sheet_features: Vec::new(),
        };
        assert!(matches!(
            export_document(&missing_mapping),
            Err(XlsxError::InvalidFormat(message)) if message.contains("0 sheet entries")
        ));

        let invalid_range = XlsxDocument {
            workbook: Workbook::new(),
            sheet_features: vec![XlsxSheetFeatures {
                validations: vec![ValidationRule {
                    range: (4, 0, 2, 0),
                    validation: Default::default(),
                }],
                conditional_formats: Vec::new(),
            }],
        };
        assert!(matches!(
            export_document(&invalid_range),
            Err(XlsxError::InvalidFormat(message)) if message.contains("invalid worksheet feature range")
        ));

        let unsupported = XlsxDocument {
            workbook: Workbook::new(),
            sheet_features: vec![XlsxSheetFeatures {
                validations: Vec::new(),
                conditional_formats: vec![ConditionalFormat {
                    condition_type: ConditionType::ColorScale,
                    ..Default::default()
                }],
            }],
        };
        assert!(matches!(
            export_document(&unsupported),
            Err(XlsxError::InvalidFormat(message)) if message.contains("does not support")
        ));
    }

    #[test]
    fn structural_edits_preserve_complex_formula_rewrites_through_xlsx() {
        use sheets_core::sheet::StructureEdit;

        let mut workbook = Workbook::new();
        workbook.rename_sheet(0, "Annual Budget").unwrap();
        workbook.add_sheet("Summary").unwrap();
        workbook.sheet_mut(0).unwrap().set_cell_value(
            0,
            0,
            "=IF(A2>0,SUM($B$2:C4),\"A2 is text\")".into(),
        );
        workbook.sheet_mut(1).unwrap().set_cell_value(
            0,
            0,
            "=SUM('Annual Budget'!$B$2:C4)+LOG10(A1)".into(),
        );

        assert!(workbook.apply_sheet_structure_edit(0, StructureEdit::InsertRow(1)));
        assert!(workbook.apply_sheet_structure_edit(0, StructureEdit::DeleteColumn(1)));
        let restored = crate::import_workbook(&export_workbook(&workbook).unwrap()).unwrap();

        assert_eq!(
            restored.sheet(0).unwrap().cell_value(0, 0),
            Some("=IF(A3>0,SUM($B$3:B5),\"A2 is text\")".into())
        );
        assert_eq!(
            restored.sheet(1).unwrap().cell_value(0, 0),
            Some("=SUM('Annual Budget'!$B$3:B5)+LOG10(A1)".into())
        );
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
