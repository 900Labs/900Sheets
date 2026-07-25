use crate::document::{XlsxDocument, XlsxSheetFeatures};
use crate::error::XlsxError;
use sheets_chart::{ChartObject, ChartType, LegendPosition};
use sheets_core::cell::CellType;
use sheets_core::format::CellFormat;
use sheets_core::workbook::Workbook;
use sheets_tables::Table;
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
    let table_plan = plan_tables(document);
    let table_count = table_plan.len();
    let chart_plan = plan_charts(document, &table_plan);
    let chart_count = chart_plan.len();
    let drawing_count = drawing_part_count(&chart_plan);

    zip.start_file("[Content_Types].xml", opts)?;
    zip.write_all(
        generate_content_types_xml(
            workbook,
            has_styles,
            table_count,
            drawing_count,
            chart_count,
        )
        .as_bytes(),
    )?;

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
        let sheet_table_ids: Vec<String> = table_plan
            .iter()
            .filter(|assignment| assignment.sheet_idx == sheet_idx)
            .map(|assignment| assignment.relationship_id.clone())
            .collect();
        let sheet_drawing_id = chart_plan
            .iter()
            .find(|assignment| assignment.sheet_idx == sheet_idx)
            .map(|assignment| assignment.worksheet_relationship_id.clone());
        let xml = generate_sheet_xml(
            workbook,
            sheet_idx,
            &shared_string_index,
            &style_table,
            features,
            &sheet_table_ids,
            sheet_drawing_id.as_deref(),
        );
        zip.write_all(xml.as_bytes())?;
    }

    for sheet_idx in 0..workbook.sheet_count() {
        let sheet_table_assignments: Vec<&TablePartAssignment<'_>> = table_plan
            .iter()
            .filter(|assignment| assignment.sheet_idx == sheet_idx)
            .collect();
        let sheet_drawing = chart_plan
            .iter()
            .find(|assignment| assignment.sheet_idx == sheet_idx)
            .map(|assignment| {
                (
                    assignment.worksheet_relationship_id.as_str(),
                    assignment.drawing_part_number,
                )
            });
        if sheet_table_assignments.is_empty() && sheet_drawing.is_none() {
            continue;
        }
        let rels_path = format!("xl/worksheets/_rels/sheet{}.xml.rels", sheet_idx + 1);
        zip.start_file(&rels_path, opts)?;
        zip.write_all(
            generate_worksheet_rels_xml(&sheet_table_assignments, sheet_drawing).as_bytes(),
        )?;
    }

    for assignment in &table_plan {
        let path = format!("xl/tables/table{}.xml", assignment.part_number);
        zip.start_file(&path, opts)?;
        zip.write_all(generate_table_xml(assignment).as_bytes())?;
    }

    for sheet_idx in 0..workbook.sheet_count() {
        let sheet_chart_assignments: Vec<&ChartPartAssignment<'_>> = chart_plan
            .iter()
            .filter(|assignment| assignment.sheet_idx == sheet_idx)
            .collect();
        if sheet_chart_assignments.is_empty() {
            continue;
        }
        let drawing_part_number = sheet_chart_assignments[0].drawing_part_number;
        let drawing_path = format!("xl/drawings/drawing{}.xml", drawing_part_number);
        zip.start_file(&drawing_path, opts)?;
        zip.write_all(generate_drawing_xml(&sheet_chart_assignments).as_bytes())?;

        let drawing_rels_path =
            format!("xl/drawings/_rels/drawing{}.xml.rels", drawing_part_number);
        zip.start_file(&drawing_rels_path, opts)?;
        zip.write_all(generate_drawing_rels_xml(&sheet_chart_assignments).as_bytes())?;
    }

    for assignment in &chart_plan {
        let path = format!("xl/charts/chart{}.xml", assignment.chart_part_number);
        zip.start_file(&path, opts)?;
        zip.write_all(generate_chart_xml(assignment).as_bytes())?;
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
    table_relationship_ids: &[String],
    drawing_relationship_id: Option<&str>,
) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\n<sheetData>\n",
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
    write_drawing_ref(&mut xml, drawing_relationship_id);
    write_table_parts(&mut xml, table_relationship_ids);
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

fn bool_int(value: bool) -> &'static str {
    if value {
        "1"
    } else {
        "0"
    }
}

/// A table assigned a workbook-global part number during export.
struct TablePartAssignment<'a> {
    sheet_idx: usize,
    relationship_id: String,
    part_number: u32,
    table_id: u32,
    table: &'a Table,
}

/// Assign each table a workbook-global part number (`tableN.xml`) and a
/// per-worksheet relationship id. Part numbers and OOXML `id` values are
/// 1-based and unique across the workbook, matching Excel's output.
fn plan_tables(document: &XlsxDocument) -> Vec<TablePartAssignment<'_>> {
    let mut plan = Vec::new();
    let mut next_part = 1u32;
    for (sheet_idx, features) in document.sheet_features.iter().enumerate() {
        let mut next_rel_id = 1u32;
        for table in &features.tables {
            plan.push(TablePartAssignment {
                sheet_idx,
                relationship_id: format!("rId{next_rel_id}"),
                part_number: next_part,
                table_id: next_part,
                table,
            });
            next_part += 1;
            next_rel_id += 1;
        }
    }
    plan
}

fn write_table_parts(xml: &mut String, relationship_ids: &[String]) {
    if relationship_ids.is_empty() {
        return;
    }
    xml.push_str(&format!(
        "<tableParts count=\"{}\">",
        relationship_ids.len()
    ));
    for id in relationship_ids {
        xml.push_str(&format!("<tablePart r:id=\"{id}\"/>"));
    }
    xml.push_str("</tableParts>\n");
}

/// Emit the worksheet `<drawing r:id="..."/>` reference. OOXML schema places the
/// drawing element before `tableParts`, so this is written first.
fn write_drawing_ref(xml: &mut String, relationship_id: Option<&str>) {
    if let Some(id) = relationship_id {
        xml.push_str(&format!("<drawing r:id=\"{id}\"/>\n"));
    }
}

fn generate_worksheet_rels_xml(
    table_assignments: &[&TablePartAssignment<'_>],
    drawing: Option<(&str, u32)>,
) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n",
    );
    for assignment in table_assignments {
        xml.push_str(&format!(
            "<Relationship Id=\"{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/table\" Target=\"../tables/table{}.xml\"/>\n",
            assignment.relationship_id, assignment.part_number
        ));
    }
    if let Some((relationship_id, part_number)) = drawing {
        xml.push_str(&format!(
            "<Relationship Id=\"{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing\" Target=\"../drawings/drawing{}.xml\"/>\n",
            relationship_id, part_number
        ));
    }
    xml.push_str("</Relationships>");
    xml
}

fn generate_table_xml(assignment: &TablePartAssignment<'_>) -> String {
    let table = assignment.table;
    let (start_row, start_col, end_row, end_col) = table.range;
    let reference = format!(
        "{}{}:{}{}",
        col_to_label(start_col),
        start_row + 1,
        col_to_label(end_col),
        end_row + 1
    );
    let totals_row_count = if table.totals_row_shown { 1 } else { 0 };
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<table xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" id=\"{}\" name=\"{}\" displayName=\"{}\" ref=\"{}\" headerRowCount=\"{}\" totalsRowCount=\"{}\">\n",
        assignment.table_id,
        escape_xml(&table.name),
        escape_xml(&table.display_name),
        reference,
        table.header_row_count,
        totals_row_count
    );
    if let Some((af_start_row, af_start_col, af_end_row, af_end_col)) = table.auto_filter_range {
        let af_reference = format!(
            "{}{}:{}{}",
            col_to_label(af_start_col),
            af_start_row + 1,
            col_to_label(af_end_col),
            af_end_row + 1
        );
        xml.push_str(&format!("<autoFilter ref=\"{af_reference}\"/>\n"));
    }
    xml.push_str(&format!(
        "<tableColumns count=\"{}\">\n",
        table.columns.len()
    ));
    for column in &table.columns {
        let function = column
            .totals_row_function
            .map(|function: sheets_tables::TotalsRowFunction| {
                format!(" totalsRowFunction=\"{}\"", function.as_xml())
            })
            .unwrap_or_default();
        let label = column
            .totals_row_label
            .as_ref()
            .map(|label| format!(" totalsRowLabel=\"{}\"", escape_xml(label)))
            .unwrap_or_default();
        xml.push_str(&format!(
            "<tableColumn id=\"{}\" name=\"{}\"{}{}/>\n",
            column.id,
            escape_xml(&column.name),
            function,
            label
        ));
    }
    xml.push_str("</tableColumns>\n");
    let style = &table.style;
    xml.push_str(&format!(
        "<tableStyleInfo name=\"{}\" showFirstColumn=\"{}\" showLastColumn=\"{}\" showRowStripes=\"{}\" showColumnStripes=\"{}\"/>\n",
        escape_xml(style.name.as_deref().unwrap_or("")),
        bool_int(style.show_first_column),
        bool_int(style.show_last_column),
        bool_int(style.show_row_stripes),
        bool_int(style.show_column_stripes)
    ));
    xml.push_str("</table>");
    xml
}

/// A chart assigned part numbers and relationship ids during export. Charts on
/// a worksheet share a single drawing part; each chart gets its own chart part.
struct ChartPartAssignment<'a> {
    sheet_idx: usize,
    /// Relationship id of the drawing within the worksheet relationship part.
    worksheet_relationship_id: String,
    /// Workbook-global part number of the shared drawing part (`drawingN.xml`).
    drawing_part_number: u32,
    /// Workbook-global part number of this chart (`chartN.xml`).
    chart_part_number: u32,
    /// Relationship id of the chart within the drawing relationship part.
    drawing_relationship_id: String,
    chart: &'a ChartObject,
}

/// Assign part numbers and relationship ids to every chart in the document.
/// Worksheet relationship ids continue after the ones already consumed by
/// tables so the same relationship part stays collision-free. Each worksheet
/// with charts gets exactly one drawing part, shared by all its charts.
fn plan_charts<'a>(
    document: &'a XlsxDocument,
    table_plan: &[TablePartAssignment<'_>],
) -> Vec<ChartPartAssignment<'a>> {
    let mut rel_ids_used_by_sheet = std::collections::HashMap::new();
    for assignment in table_plan {
        *rel_ids_used_by_sheet
            .entry(assignment.sheet_idx)
            .or_insert(0u32) += 1;
    }

    let mut plan = Vec::new();
    let mut next_drawing_part = 1u32;
    let mut next_chart_part = 1u32;
    for (sheet_idx, features) in document.sheet_features.iter().enumerate() {
        if features.charts.is_empty() {
            continue;
        }
        // Each worksheet contributes exactly one drawing relationship, whose
        // id follows the ids already consumed by that sheet's tables.
        let rel_id = rel_ids_used_by_sheet.get(&sheet_idx).copied().unwrap_or(0) + 1;
        let worksheet_relationship_id = format!("rId{rel_id}");
        let drawing_part_number = next_drawing_part;
        next_drawing_part += 1;
        for chart in &features.charts {
            let chart_part_number = next_chart_part;
            next_chart_part += 1;
            plan.push(ChartPartAssignment {
                sheet_idx,
                worksheet_relationship_id: worksheet_relationship_id.clone(),
                drawing_part_number,
                chart_part_number,
                drawing_relationship_id: format!("rId{chart_part_number}"),
                chart,
            });
        }
    }
    plan
}

/// Number of distinct drawing parts the chart plan will emit: one per
/// worksheet that has at least one chart.
fn drawing_part_count(chart_plan: &[ChartPartAssignment<'_>]) -> usize {
    let mut seen = std::collections::HashSet::new();
    for assignment in chart_plan {
        seen.insert(assignment.drawing_part_number);
    }
    seen.len()
}

fn generate_drawing_rels_xml(assignments: &[&ChartPartAssignment<'_>]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n",
    );
    for assignment in assignments {
        xml.push_str(&format!(
            "<Relationship Id=\"{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart\" Target=\"../charts/chart{}.xml\"/>\n",
            assignment.drawing_relationship_id, assignment.chart_part_number
        ));
    }
    xml.push_str("</Relationships>");
    xml
}

fn generate_drawing_xml(assignments: &[&ChartPartAssignment<'_>]) -> String {
    use sheets_chart::ChartAnchorKind;
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<xdr:wsDr xmlns:xdr=\"http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\n",
    );
    for assignment in assignments {
        let anchor = &assignment.chart.anchor;
        let frame_id = assignment.chart_part_number;
        let rid = &assignment.drawing_relationship_id;
        let graphic_frame = format!(
            "<xdr:graphicFrame macro=\"\">\n<xdr:nvGraphicFramePr><xdr:cNvPr id=\"{frame_id}\" name=\"Chart {frame_id}\"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr>\n<xdr:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/></xdr:xfrm>\n<a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"><c:chart xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" r:id=\"{rid}\"/></a:graphicData></a:graphic>\n</xdr:graphicFrame>\n<xdr:clientData/>\n"
        );
        match anchor.kind {
            ChartAnchorKind::TwoCell => xml.push_str(&format!(
                "<xdr:twoCellAnchor editAs=\"oneCell\">\n<xdr:from><xdr:col>{from_col}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{from_row}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from>\n<xdr:to><xdr:col>{to_col}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{to_row}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>\n{graphic_frame}</xdr:twoCellAnchor>\n",
                from_col = anchor.from_col,
                from_row = anchor.from_row,
                to_col = anchor.to_col,
                to_row = anchor.to_row,
            )),
            ChartAnchorKind::OneCell => xml.push_str(&format!(
                "<xdr:oneCellAnchor>\n<xdr:from><xdr:col>{from_col}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{from_row}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from>\n<xdr:ext cx=\"{ext_cx}\" cy=\"{ext_cy}\"/>\n{graphic_frame}</xdr:oneCellAnchor>\n",
                from_col = anchor.from_col,
                from_row = anchor.from_row,
                ext_cx = anchor.ext_cx,
                ext_cy = anchor.ext_cy,
            )),
            ChartAnchorKind::Absolute => xml.push_str(&format!(
                "<xdr:absoluteAnchor>\n<xdr:pos x=\"{pos_x}\" y=\"{pos_y}\"/>\n<xdr:ext cx=\"{ext_cx}\" cy=\"{ext_cy}\"/>\n{graphic_frame}</xdr:absoluteAnchor>\n",
                pos_x = anchor.pos_x,
                pos_y = anchor.pos_y,
                ext_cx = anchor.ext_cx,
                ext_cy = anchor.ext_cy,
            )),
        }
    }
    xml.push_str("</xdr:wsDr>");
    xml
}

/// Stable axis identifiers shared between a chart-type element and its axis
/// declarations so Excel accepts the plot area as well-formed.
const CATEGORY_AXIS_ID: &str = "111111111";
const VALUE_AXIS_ID: &str = "222222222";

fn generate_chart_xml(assignment: &ChartPartAssignment<'_>) -> String {
    let chart = assignment.chart;
    let title_xml = match &chart.title {
        Some(title) => format!(
            "<c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-US\"/><a:t>{}</a:t></a:r></a:p></c:rich></c:tx><c:overlay val=\"0\"/></c:title>",
            escape_xml(title)
        ),
        None => String::new(),
    };
    let legend_xml = if chart.legend_position == LegendPosition::None {
        String::new()
    } else {
        format!(
            "<c:legend><c:legendPos val=\"{}\"/><c:overlay val=\"0\"/></c:legend>",
            legend_position_xml(&chart.legend_position)
        )
    };
    let plot_area_xml = build_plot_area_xml(chart);

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><c:chart>{title_xml}<c:plotArea>{plot_area_xml}</c:plotArea>{legend_xml}<c:plotVisOnly val=\"1\"/><c:dispBlanksAs val=\"gap\"/></c:chart></c:chartSpace>"
    )
}

/// Build the contents of `<c:plotArea>`: layout, the chart-type element with its
/// series, and (for cartesian families) the category and value axes that Excel
/// requires. Pie and doughnut charts omit the axes.
fn build_plot_area_xml(chart: &ChartObject) -> String {
    let chart_type_xml = build_chart_type_xml(chart);
    let axes_xml = match chart.chart_type {
        ChartType::Pie | ChartType::Doughnut => String::new(),
        _ => format!(
            "<c:catAx><c:axId val=\"{CATEGORY_AXIS_ID}\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"b\"/><c:crossAx val=\"{VALUE_AXIS_ID}\"/></c:catAx><c:valAx><c:axId val=\"{VALUE_AXIS_ID}\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"l\"/><c:crossAx val=\"{CATEGORY_AXIS_ID}\"/></c:valAx>"
        ),
    };
    format!("<c:layout/>{chart_type_xml}{axes_xml}")
}

fn build_chart_type_xml(chart: &ChartObject) -> String {
    let series_xml = build_chart_series_xml(chart);
    match chart.chart_type {
        ChartType::Bar => format!(
            "<c:barChart><c:barDir val=\"bar\"/><c:grouping val=\"clustered\"/><c:varyColors val=\"0\"/>{series_xml}<c:axId val=\"{CATEGORY_AXIS_ID}\"/><c:axId val=\"{VALUE_AXIS_ID}\"/></c:barChart>"
        ),
        ChartType::Column => format!(
            "<c:barChart><c:barDir val=\"col\"/><c:grouping val=\"clustered\"/><c:varyColors val=\"0\"/>{series_xml}<c:axId val=\"{CATEGORY_AXIS_ID}\"/><c:axId val=\"{VALUE_AXIS_ID}\"/></c:barChart>"
        ),
        ChartType::Line => format!(
            "<c:lineChart><c:grouping val=\"clustered\"/><c:marker val=\"1\"/><c:varyColors val=\"0\"/>{series_xml}<c:axId val=\"{CATEGORY_AXIS_ID}\"/><c:axId val=\"{VALUE_AXIS_ID}\"/></c:lineChart>"
        ),
        ChartType::Area => format!(
            "<c:areaChart><c:grouping val=\"standard\"/><c:varyColors val=\"0\"/>{series_xml}<c:axId val=\"{CATEGORY_AXIS_ID}\"/><c:axId val=\"{VALUE_AXIS_ID}\"/></c:areaChart>"
        ),
        ChartType::Pie => format!(
            "<c:pieChart><c:varyColors val=\"1\"/>{series_xml}</c:pieChart>"
        ),
        ChartType::Doughnut => format!(
            "<c:doughnutChart><c:varyColors val=\"1\"/>{series_xml}</c:doughnutChart>"
        ),
        ChartType::Scatter => String::new(),
    }
}

fn build_chart_series_xml(chart: &ChartObject) -> String {
    let mut xml = String::new();
    for (index, series) in chart.series.iter().enumerate() {
        xml.push_str(&format!(
            "<c:ser><c:idx val=\"{index}\"/><c:order val=\"{index}\"/>"
        ));
        if let Some(name_ref) = &series.name_ref {
            xml.push_str(&format!(
                "<c:tx><c:strRef><c:f>{}</c:f></c:strRef></c:tx>",
                escape_xml(name_ref)
            ));
        }
        if let Some(category_ref) = &series.category_ref {
            xml.push_str(&format!(
                "<c:cat><c:strRef><c:f>{}</c:f></c:strRef></c:cat>",
                escape_xml(category_ref)
            ));
        }
        if let Some(value_ref) = &series.value_ref {
            xml.push_str(&format!(
                "<c:val><c:numRef><c:f>{}</c:f></c:numRef></c:val>",
                escape_xml(value_ref)
            ));
        }
        xml.push_str("</c:ser>");
    }
    xml
}

fn legend_position_xml(position: &LegendPosition) -> &'static str {
    match position {
        LegendPosition::None => "r",
        LegendPosition::Top => "t",
        LegendPosition::Bottom => "b",
        LegendPosition::Left => "l",
        LegendPosition::Right => "r",
    }
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
            .and_then(|count| count.checked_add(features.tables.len()))
            .and_then(|count| count.checked_add(features.charts.len()))
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
        for table in &features.tables {
            if table.validate(1_000_000, 16_384).is_err() {
                return Err(XlsxError::InvalidFormat(format!(
                    "Sheet {} contains an invalid table definition",
                    sheet_index + 1
                )));
            }
        }
        for chart in &features.charts {
            if chart.validate(1_000_000, 16_384).is_err() {
                return Err(XlsxError::InvalidFormat(format!(
                    "Sheet {} contains a chart definition that XLSX export does not support",
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

fn generate_content_types_xml(
    workbook: &Workbook,
    has_styles: bool,
    table_count: usize,
    drawing_count: usize,
    chart_count: usize,
) -> String {
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
    for part_number in 1..=table_count {
        xml.push_str(&format!(
            "<Override PartName=\"/xl/tables/table{}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml\"/>\n",
            part_number
        ));
    }
    for part_number in 1..=drawing_count {
        xml.push_str(&format!(
            "<Override PartName=\"/xl/drawings/drawing{}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.drawing+xml\"/>\n",
            part_number
        ));
    }
    for part_number in 1..=chart_count {
        xml.push_str(&format!(
            "<Override PartName=\"/xl/charts/chart{}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.drawingml.chart+xml\"/>\n",
            part_number
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
                    tables: Vec::new(),
                    charts: Vec::new(),
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
                tables: Vec::new(),
                charts: Vec::new(),
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
                tables: Vec::new(),
                charts: Vec::new(),
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
                tables: Vec::new(),
                charts: Vec::new(),
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
                tables: Vec::new(),
                charts: Vec::new(),
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
                tables: Vec::new(),
                charts: Vec::new(),
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
                tables: Vec::new(),
                charts: Vec::new(),
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

    #[test]
    fn excel_tables_roundtrip_through_xlsx() {
        use sheets_tables::{Table, TableColumn, TableStyleInfo, TotalsRowFunction};

        let mut workbook = Workbook::new();
        workbook
            .sheet_mut(0)
            .unwrap()
            .set_cell_value(0, 0, "Region".into());
        let table = Table {
            name: "Sales".into(),
            display_name: "Sales".into(),
            range: (0, 0, 3, 2),
            header_row_count: 1,
            totals_row_shown: true,
            columns: vec![
                TableColumn {
                    id: 1,
                    name: "Region".into(),
                    totals_row_function: None,
                    totals_row_label: Some("Total".into()),
                },
                TableColumn {
                    id: 2,
                    name: "Item".into(),
                    totals_row_function: None,
                    totals_row_label: None,
                },
                TableColumn {
                    id: 3,
                    name: "Total".into(),
                    totals_row_function: Some(TotalsRowFunction::Sum),
                    totals_row_label: None,
                },
            ],
            style: TableStyleInfo {
                name: Some("TableStyleMedium9".into()),
                show_first_column: true,
                show_last_column: false,
                show_row_stripes: true,
                show_column_stripes: true,
            },
            auto_filter_range: Some((0, 0, 3, 2)),
        };
        let document = XlsxDocument {
            workbook,
            sheet_features: vec![XlsxSheetFeatures {
                validations: Vec::new(),
                conditional_formats: Vec::new(),
                tables: vec![table],
                charts: Vec::new(),
            }],
        };

        let exported = export_document(&document).unwrap();
        let restored = crate::import_document(&exported).unwrap();

        assert_eq!(restored.sheet_features[0].tables.len(), 1);
        let restored_table = &restored.sheet_features[0].tables[0];
        assert_eq!(restored_table.name, "Sales");
        assert_eq!(restored_table.display_name, "Sales");
        assert_eq!(restored_table.range, (0, 0, 3, 2));
        assert_eq!(restored_table.header_row_count, 1);
        assert!(restored_table.totals_row_shown);
        assert_eq!(restored_table.columns.len(), 3);
        assert_eq!(restored_table.columns[0].name, "Region");
        assert_eq!(
            restored_table.columns[0].totals_row_label.as_deref(),
            Some("Total")
        );
        assert_eq!(
            restored_table.columns[2].totals_row_function,
            Some(TotalsRowFunction::Sum)
        );
        assert_eq!(
            restored_table.style.name.as_deref(),
            Some("TableStyleMedium9")
        );
        assert!(restored_table.style.show_first_column);
        assert!(restored_table.style.show_column_stripes);
        assert_eq!(restored_table.auto_filter_range, Some((0, 0, 3, 2)));
    }

    #[test]
    fn tables_on_multiple_sheets_get_distinct_workbook_scoped_parts() {
        use sheets_tables::Table;

        let mut workbook = Workbook::new();
        workbook.add_sheet("Second").unwrap();
        let document = XlsxDocument {
            workbook,
            sheet_features: vec![
                XlsxSheetFeatures {
                    validations: Vec::new(),
                    conditional_formats: Vec::new(),
                    tables: vec![
                        Table::new("FirstA", (0, 0, 2, 1)),
                        Table::new("FirstB", (0, 3, 2, 4)),
                    ],
                    charts: Vec::new(),
                },
                XlsxSheetFeatures {
                    validations: Vec::new(),
                    conditional_formats: Vec::new(),
                    tables: vec![Table::new("SecondA", (0, 0, 1, 0))],
                    charts: Vec::new(),
                },
            ],
        };

        let restored = crate::import_document(&export_document(&document).unwrap()).unwrap();
        assert_eq!(restored.sheet_features[0].tables.len(), 2);
        assert_eq!(restored.sheet_features[1].tables.len(), 1);
        let names: Vec<&str> = restored
            .sheet_features
            .iter()
            .flat_map(|features| features.tables.iter())
            .map(|table| table.name.as_str())
            .collect();
        assert_eq!(names, vec!["FirstA", "FirstB", "SecondA"]);
    }

    #[test]
    fn exported_archive_includes_table_parts_and_content_types() {
        use sheets_tables::Table;
        use std::io::Read;

        let document = XlsxDocument {
            workbook: Workbook::new(),
            sheet_features: vec![XlsxSheetFeatures {
                validations: Vec::new(),
                conditional_formats: Vec::new(),
                tables: vec![Table::new("Sales", (0, 0, 2, 1))],
                charts: Vec::new(),
            }],
        };
        let data = export_document(&document).unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data)).unwrap();

        // Table part, worksheet relationship part, and content-types override
        // must all be present.
        assert!(archive.by_name("xl/tables/table1.xml").is_ok());
        assert!(archive
            .by_name("xl/worksheets/_rels/sheet1.xml.rels")
            .is_ok());

        let mut content = String::new();
        archive
            .by_name("[Content_Types].xml")
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert!(content.contains("/xl/tables/table1.xml"));

        let mut sheet = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .unwrap()
            .read_to_string(&mut sheet)
            .unwrap();
        assert!(sheet.contains("xmlns:r="));
        assert!(sheet.contains("<tableParts count=\"1\">"));
    }

    use sheets_chart::{ChartAnchor, ChartObject, ChartObjectSeries};

    fn column_chart() -> ChartObject {
        ChartObject {
            title: Some("Sales by Month".into()),
            chart_type: ChartType::Column,
            anchor: ChartAnchor::new(0, 4, 18, 12),
            series: vec![ChartObjectSeries {
                name_ref: Some("Sheet1!$B$1".into()),
                category_ref: Some("Sheet1!$A$2:$A$4".into()),
                value_ref: Some("Sheet1!$B$2:$B$4".into()),
            }],
            legend_position: LegendPosition::Right,
        }
    }

    #[test]
    fn exported_archive_includes_chart_drawing_and_content_types() {
        use std::io::Read;

        let document = XlsxDocument {
            workbook: Workbook::new(),
            sheet_features: vec![XlsxSheetFeatures {
                validations: Vec::new(),
                conditional_formats: Vec::new(),
                tables: Vec::new(),
                charts: vec![column_chart()],
            }],
        };
        let data = export_document(&document).unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data)).unwrap();

        // Drawing part, drawing rels, chart part, and worksheet rels that
        // carry the drawing relationship must all be present.
        assert!(archive.by_name("xl/drawings/drawing1.xml").is_ok());
        assert!(archive
            .by_name("xl/drawings/_rels/drawing1.xml.rels")
            .is_ok());
        assert!(archive.by_name("xl/charts/chart1.xml").is_ok());
        assert!(archive
            .by_name("xl/worksheets/_rels/sheet1.xml.rels")
            .is_ok());

        let mut content_types = String::new();
        archive
            .by_name("[Content_Types].xml")
            .unwrap()
            .read_to_string(&mut content_types)
            .unwrap();
        assert!(content_types.contains("/xl/drawings/drawing1.xml"));
        assert!(content_types.contains("/xl/charts/chart1.xml"));

        let mut worksheet = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .unwrap()
            .read_to_string(&mut worksheet)
            .unwrap();
        assert!(worksheet.contains("<drawing r:id=\"rId1\"/>"));
    }

    #[test]
    fn chart_round_trips_through_export_and_import() {
        let document = XlsxDocument {
            workbook: Workbook::new(),
            sheet_features: vec![XlsxSheetFeatures {
                validations: Vec::new(),
                conditional_formats: Vec::new(),
                tables: Vec::new(),
                charts: vec![column_chart()],
            }],
        };
        let bytes = export_document(&document).unwrap();
        let reimported = crate::import::import_document(&bytes).unwrap();

        assert_eq!(reimported.sheet_features[0].charts.len(), 1);
        let chart = &reimported.sheet_features[0].charts[0];
        assert_eq!(chart.chart_type, ChartType::Column);
        assert_eq!(chart.title.as_deref(), Some("Sales by Month"));
        assert_eq!(chart.anchor, ChartAnchor::new(0, 4, 18, 12));
        assert_eq!(chart.legend_position, LegendPosition::Right);
        assert_eq!(chart.series.len(), 1);
        assert_eq!(chart.series[0].name_ref.as_deref(), Some("Sheet1!$B$1"));
        assert_eq!(
            chart.series[0].category_ref.as_deref(),
            Some("Sheet1!$A$2:$A$4")
        );
        assert_eq!(
            chart.series[0].value_ref.as_deref(),
            Some("Sheet1!$B$2:$B$4")
        );
    }

    #[test]
    fn every_round_trip_chart_family_round_trips_its_type_and_anchor() {
        for (chart_type, bar_dir) in [
            (ChartType::Bar, "bar"),
            (ChartType::Column, "col"),
            (ChartType::Line, ""),
            (ChartType::Area, ""),
            (ChartType::Pie, ""),
            (ChartType::Doughnut, ""),
        ] {
            let document = XlsxDocument {
                workbook: Workbook::new(),
                sheet_features: vec![XlsxSheetFeatures {
                    validations: Vec::new(),
                    conditional_formats: Vec::new(),
                    tables: Vec::new(),
                    charts: vec![ChartObject {
                        title: None,
                        chart_type: chart_type.clone(),
                        anchor: ChartAnchor::new(2, 2, 12, 9),
                        series: vec![ChartObjectSeries {
                            name_ref: Some("Sheet1!$B$1".into()),
                            category_ref: Some("Sheet1!$A$2:$A$4".into()),
                            value_ref: Some("Sheet1!$B$2:$B$4".into()),
                        }],
                        legend_position: LegendPosition::Bottom,
                    }],
                }],
            };
            let bytes = export_document(&document).unwrap();
            // A barChart must carry the expected barDir; other families emit
            // no barDir element.
            let chart_xml = {
                use std::io::Read;
                let mut archive =
                    zip::ZipArchive::new(std::io::Cursor::new(bytes.clone())).unwrap();
                let mut text = String::new();
                archive
                    .by_name("xl/charts/chart1.xml")
                    .unwrap()
                    .read_to_string(&mut text)
                    .unwrap();
                text
            };
            if bar_dir.is_empty() {
                assert!(
                    !chart_xml.contains("barDir"),
                    "unexpected barDir for {chart_type:?}"
                );
            } else {
                assert!(
                    chart_xml.contains(&format!("barDir val=\"{bar_dir}\"")),
                    "missing barDir={bar_dir} for {chart_type:?}"
                );
            }

            let reimported = crate::import::import_document(&bytes).unwrap();
            assert_eq!(
                reimported.sheet_features[0].charts.len(),
                1,
                "{chart_type:?} did not round-trip"
            );
            assert_eq!(
                reimported.sheet_features[0].charts[0].chart_type, chart_type,
                "{chart_type:?} type changed on round trip"
            );
            assert_eq!(
                reimported.sheet_features[0].charts[0].anchor,
                ChartAnchor::new(2, 2, 12, 9),
                "{chart_type:?} anchor changed on round trip"
            );
        }
    }

    #[test]
    fn multiple_charts_on_one_sheet_share_a_single_drawing_part() {
        use std::io::Read;

        let document = XlsxDocument {
            workbook: Workbook::new(),
            sheet_features: vec![XlsxSheetFeatures {
                validations: Vec::new(),
                conditional_formats: Vec::new(),
                tables: Vec::new(),
                charts: vec![column_chart(), column_chart()],
            }],
        };
        let data = export_document(&document).unwrap();
        let reimported = {
            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data.clone())).unwrap();

            assert!(archive.by_name("xl/drawings/drawing1.xml").is_ok());
            assert!(
                archive.by_name("xl/drawings/drawing2.xml").is_err(),
                "two charts on one sheet must share one drawing part"
            );
            assert!(archive.by_name("xl/charts/chart1.xml").is_ok());
            assert!(archive.by_name("xl/charts/chart2.xml").is_ok());

            let mut drawing = String::new();
            archive
                .by_name("xl/drawings/drawing1.xml")
                .unwrap()
                .read_to_string(&mut drawing)
                .unwrap();
            assert_eq!(
                drawing.matches("twoCellAnchor editAs").count(),
                2,
                "both charts must be anchored in the shared drawing"
            );

            crate::import::import_document(&data).unwrap()
        };
        assert_eq!(reimported.sheet_features[0].charts.len(), 2);
    }

    #[test]
    fn tables_and_charts_on_one_sheet_do_not_collide_relationship_ids() {
        use std::io::Read;

        let document = XlsxDocument {
            workbook: Workbook::new(),
            sheet_features: vec![XlsxSheetFeatures {
                validations: Vec::new(),
                conditional_formats: Vec::new(),
                tables: vec![
                    sheets_tables::Table::new("Sales", (0, 0, 2, 1)),
                    sheets_tables::Table::new("Costs", (5, 0, 7, 1)),
                ],
                charts: vec![column_chart()],
            }],
        };
        let data = export_document(&document).unwrap();
        let reimported = {
            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data.clone())).unwrap();

            // Tables take rId1 and rId2; the drawing must take rId3.
            let mut rels = String::new();
            archive
                .by_name("xl/worksheets/_rels/sheet1.xml.rels")
                .unwrap()
                .read_to_string(&mut rels)
                .unwrap();
            assert!(rels.contains("Id=\"rId1\""));
            assert!(rels.contains("Id=\"rId2\""));
            assert!(rels.contains("Id=\"rId3\""));
            assert_eq!(rels.matches("relationships/drawing\"").count(), 1);

            let mut worksheet = String::new();
            archive
                .by_name("xl/worksheets/sheet1.xml")
                .unwrap()
                .read_to_string(&mut worksheet)
                .unwrap();
            assert!(worksheet.contains("<drawing r:id=\"rId3\"/>"));
            assert!(worksheet.contains("<tableParts count=\"2\">"));

            crate::import::import_document(&data).unwrap()
        };
        // The whole bundle still reimports cleanly.
        assert_eq!(reimported.sheet_features[0].tables.len(), 2);
        assert_eq!(reimported.sheet_features[0].charts.len(), 1);
    }

    #[test]
    fn unsupported_chart_type_is_rejected_by_export_validation() {
        let document = XlsxDocument {
            workbook: Workbook::new(),
            sheet_features: vec![XlsxSheetFeatures {
                validations: Vec::new(),
                conditional_formats: Vec::new(),
                tables: Vec::new(),
                charts: vec![ChartObject {
                    title: None,
                    chart_type: ChartType::Scatter,
                    anchor: ChartAnchor::new(0, 0, 8, 4),
                    series: vec![ChartObjectSeries {
                        name_ref: None,
                        category_ref: None,
                        value_ref: None,
                    }],
                    legend_position: LegendPosition::None,
                }],
            }],
        };
        assert!(export_document(&document).is_err());
    }

    #[test]
    fn one_cell_and_absolute_chart_anchors_round_trip_their_kind() {
        use sheets_chart::ChartAnchorKind;
        use std::io::Read;

        for anchor in [
            ChartAnchor::one_cell(3, 2, 1_828_800, 1_371_600),
            ChartAnchor::absolute(457_200, 274_320, 1_828_800, 1_371_600),
        ] {
            let document = XlsxDocument {
                workbook: Workbook::new(),
                sheet_features: vec![XlsxSheetFeatures {
                    validations: Vec::new(),
                    conditional_formats: Vec::new(),
                    tables: Vec::new(),
                    charts: vec![ChartObject {
                        title: None,
                        chart_type: ChartType::Column,
                        anchor,
                        series: vec![ChartObjectSeries {
                            name_ref: Some("Sheet1!$B$1".into()),
                            category_ref: Some("Sheet1!$A$2:$A$4".into()),
                            value_ref: Some("Sheet1!$B$2:$B$4".into()),
                        }],
                        legend_position: LegendPosition::None,
                    }],
                }],
            };
            let bytes = export_document(&document).unwrap();

            // The drawing must emit the matching anchor element, not a
            // collapsed twoCellAnchor.
            let mut drawing = String::new();
            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes.clone())).unwrap();
            archive
                .by_name("xl/drawings/drawing1.xml")
                .unwrap()
                .read_to_string(&mut drawing)
                .unwrap();
            let (expected_open, expected_close) = match anchor.kind {
                ChartAnchorKind::OneCell => ("oneCellAnchor", "oneCellAnchor"),
                ChartAnchorKind::Absolute => ("absoluteAnchor", "absoluteAnchor"),
                ChartAnchorKind::TwoCell => ("twoCellAnchor", "twoCellAnchor"),
            };
            assert!(
                drawing.contains(&format!("<xdr:{expected_open}>")),
                "expected <xdr:{expected_open}> in drawing for {:?}",
                anchor.kind
            );
            assert!(
                drawing.contains(&format!("</xdr:{expected_close}>")),
                "expected </xdr:{expected_close}> in drawing for {:?}",
                anchor.kind
            );
            if anchor.kind == ChartAnchorKind::OneCell {
                assert!(drawing.contains("<xdr:ext"));
                assert!(
                    !drawing.contains("<xdr:to>"),
                    "oneCellAnchor must not emit a <to> marker"
                );
            }

            let reimported = crate::import::import_document(&bytes).unwrap();
            assert_eq!(reimported.sheet_features[0].charts.len(), 1);
            assert_eq!(
                reimported.sheet_features[0].charts[0].anchor.kind, anchor.kind,
                "anchor kind changed on round trip"
            );
            assert_eq!(
                reimported.sheet_features[0].charts[0].anchor, anchor,
                "anchor fields changed on round trip"
            );
        }
    }
}
