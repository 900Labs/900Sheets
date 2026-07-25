use crate::document::{XlsxDocument, XlsxSheetFeatures};
use crate::error::XlsxError;
use roxmltree::{Document, Node};
use sheets_chart::{ChartAnchor, ChartObject, ChartObjectSeries, ChartType, LegendPosition};
use sheets_core::cell::CellValue;
use sheets_core::format::CellFormat;
use sheets_core::workbook::Workbook;
use sheets_tables::{Table, TableColumn, TableStyleInfo, TotalsRowFunction};
use sheets_validation::{
    ConditionOperator, ConditionType, ConditionalFormat, DataValidation, ValidationErrorStyle,
    ValidationOperator, ValidationRule, ValidationType,
};
use std::collections::HashMap;
use std::io::Read;

const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;
const MAX_XML_ENTRY_SIZE: u64 = 25 * 1024 * 1024;
const MAX_CELLS: usize = 10_000_000;
const MAX_ROWS: u32 = 1_000_000;
const MAX_COLS: u32 = 16_384;
const MAX_FEATURE_RECORDS: usize = 100_000;

pub fn import_workbook(data: &[u8]) -> Result<Workbook, XlsxError> {
    Ok(import_document(data)?.workbook)
}

pub fn import_document(data: &[u8]) -> Result<XlsxDocument, XlsxError> {
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
        return Ok(XlsxDocument::new(workbook));
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
    let mut total_features = 0usize;
    let mut sheet_features = Vec::with_capacity(sheets.len());
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
        let mut features = parse_sheet_features(&xml, &styles)?;
        features.tables = parse_sheet_tables(&mut archive, sheet_file, &xml)?;
        features.charts = parse_sheet_charts(&mut archive, sheet_file, &xml)?;
        total_features = total_features
            .checked_add(features.validations.len())
            .and_then(|count| count.checked_add(features.conditional_formats.len()))
            .and_then(|count| count.checked_add(features.tables.len()))
            .and_then(|count| count.checked_add(features.charts.len()))
            .ok_or_else(|| XlsxError::InvalidFormat("Too many worksheet feature records".into()))?;
        if total_features > MAX_FEATURE_RECORDS {
            return Err(XlsxError::InvalidFormat(format!(
                "Workbook contains {total_features} validation, conditional-format, table, and chart records; the limit is {MAX_FEATURE_RECORDS}"
            )));
        }
        sheet_features.push(features);
    }

    Ok(XlsxDocument {
        workbook,
        sheet_features,
    })
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
    read_shared_strings_with_limit(archive, MAX_XML_ENTRY_SIZE)
}

fn read_shared_strings_with_limit<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    max_entry_size: u64,
) -> Result<Vec<String>, XlsxError> {
    let xml = match read_zip_file_with_limit(archive, "xl/sharedStrings.xml", max_entry_size) {
        Ok(content) => content,
        Err(XlsxError::Zip(zip::result::ZipError::FileNotFound)) => return Ok(Vec::new()),
        Err(error) => return Err(error),
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

/// Path of a worksheet's relationship part: `xl/worksheets/sheet1.xml` becomes
/// `xl/worksheets/_rels/sheet1.xml.rels`.
fn worksheet_rels_path(sheet_file: &str) -> String {
    match sheet_file.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{sheet_file}.rels"),
    }
}

/// Parent directory of a worksheet file: `xl/worksheets/sheet1.xml` -> `xl/worksheets`.
fn sheet_directory(sheet_file: &str) -> &str {
    sheet_file.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// Resolve a relationship target that may be package-relative (starting with
/// `/`), part-relative, or use `..` segments, to a normalized archive path.
fn normalize_zip_path(dir: &str, target: &str) -> String {
    if let Some(stripped) = target.strip_prefix('/') {
        return stripped.to_string();
    }
    let mut parts: Vec<&str> = Vec::new();
    if !dir.is_empty() {
        parts.extend(dir.split('/'));
    }
    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// Read a worksheet's relationship part and return the relationships of a
/// given type as a map of relationship id to archive path. The `type_suffix`
/// is matched against the end of the OOXML relationship `Type` attribute, for
/// example `/table`, `/drawing`, or `/chart`. Missing relationship parts are
/// treated as an empty set.
fn read_relationship_targets<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    owner_file: &str,
    type_suffix: &str,
) -> Result<HashMap<String, String>, XlsxError> {
    let rels_path = worksheet_rels_path(owner_file);
    let xml = match read_zip_file(archive, &rels_path) {
        Ok(content) => content,
        Err(XlsxError::Zip(zip::result::ZipError::FileNotFound)) => return Ok(HashMap::new()),
        Err(error) => return Err(error),
    };
    let doc = Document::parse(&xml)?;
    let owner_dir = sheet_directory(owner_file);
    let mut targets = HashMap::new();
    for node in doc
        .descendants()
        .filter(|node| node.has_tag_name("Relationship"))
    {
        let relationship_type = node.attribute("Type").unwrap_or("");
        if !relationship_type.ends_with(type_suffix) {
            continue;
        }
        if let (Some(id), Some(target)) = (node.attribute("Id"), node.attribute("Target")) {
            targets.insert(id.to_string(), normalize_zip_path(owner_dir, target));
        }
    }
    Ok(targets)
}

/// Parse the `<tablePart r:id="..."/>` references embedded in a worksheet and
/// load each referenced table part, returning the table definitions for the
/// sheet. Malformed or out-of-bounds tables are skipped rather than failing
/// the whole import, matching how unsupported validation types are handled.
fn parse_sheet_tables<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    sheet_file: &str,
    worksheet_xml: &str,
) -> Result<Vec<Table>, XlsxError> {
    let table_targets = read_relationship_targets(archive, sheet_file, "/table")?;
    if table_targets.is_empty() {
        return Ok(Vec::new());
    }

    let referenced_ids: Vec<String> = Document::parse(worksheet_xml)?
        .descendants()
        .filter(|node| node.has_tag_name("tablePart"))
        .filter_map(|node| {
            node.attributes()
                .find(|attribute| attribute.name() == "id")
                .map(|attribute| attribute.value().to_string())
        })
        .collect();
    if referenced_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut tables = Vec::new();
    for relationship_id in referenced_ids {
        let Some(target) = table_targets.get(&relationship_id) else {
            continue;
        };
        let xml = read_zip_file(archive, target)?;
        if let Some(table) = parse_table_part(&xml)? {
            tables.push(table);
        }
    }
    Ok(tables)
}

/// Parse a single `xl/tables/tableN.xml` part. Returns `None` for a part that
/// lacks a name or a parseable range so the importer can skip it.
fn parse_table_part(xml: &str) -> Result<Option<Table>, XlsxError> {
    let doc = Document::parse(xml)?;
    let Some(root) = doc.descendants().find(|node| node.has_tag_name("table")) else {
        return Ok(None);
    };

    let name = root.attribute("name").unwrap_or("").to_string();
    let display_name = root
        .attribute("displayName")
        .filter(|value| !value.is_empty())
        .unwrap_or(&name)
        .to_string();
    let Some(range) = parse_range_ref(root.attribute("ref").unwrap_or("")) else {
        return Ok(None);
    };

    let header_row_count = root
        .attribute("headerRowCount")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1);
    let totals_row_shown = match root.attribute("totalsRowShown") {
        Some(value) => bool_attribute(Some(value), false),
        None => root
            .attribute("totalsRowCount")
            .and_then(|value| value.parse::<u32>().ok())
            .map(|count| count != 0)
            .unwrap_or(false),
    };
    let auto_filter_range = root
        .children()
        .find(|child| child.has_tag_name("autoFilter"))
        .and_then(|filter| filter.attribute("ref"))
        .and_then(parse_range_ref);

    let mut columns = Vec::new();
    for column in doc
        .descendants()
        .filter(|node| node.has_tag_name("tableColumn"))
    {
        columns.push(TableColumn {
            id: column
                .attribute("id")
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(0),
            name: column.attribute("name").unwrap_or("").to_string(),
            totals_row_function: column
                .attribute("totalsRowFunction")
                .and_then(TotalsRowFunction::from_xml),
            totals_row_label: column.attribute("totalsRowLabel").map(str::to_string),
        });
    }

    let style = doc
        .descendants()
        .find(|node| node.has_tag_name("tableStyleInfo"))
        .map(|node| TableStyleInfo {
            name: node.attribute("name").map(str::to_string),
            show_first_column: bool_attribute(node.attribute("showFirstColumn"), false),
            show_last_column: bool_attribute(node.attribute("showLastColumn"), false),
            show_row_stripes: bool_attribute(node.attribute("showRowStripes"), true),
            show_column_stripes: bool_attribute(node.attribute("showColumnStripes"), false),
        })
        .unwrap_or_default();

    let table = Table {
        name,
        display_name,
        range,
        header_row_count,
        totals_row_shown,
        columns,
        style,
        auto_filter_range,
    };
    if table.validate(MAX_ROWS, MAX_COLS).is_ok() {
        Ok(Some(table))
    } else {
        Ok(None)
    }
}

/// Parse the `<drawing r:id="..."/>` references embedded in a worksheet,
/// follow each to its drawing part, and load the charts anchored there.
/// Charts of an unsupported family, or charts whose references cannot be
/// resolved, are skipped rather than failing the whole import, matching how
/// unsupported table and validation records are handled.
fn parse_sheet_charts<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    sheet_file: &str,
    worksheet_xml: &str,
) -> Result<Vec<ChartObject>, XlsxError> {
    let drawing_targets = read_relationship_targets(archive, sheet_file, "/drawing")?;
    if drawing_targets.is_empty() {
        return Ok(Vec::new());
    }

    let referenced_ids: Vec<String> = Document::parse(worksheet_xml)?
        .descendants()
        .filter(|node| node.has_tag_name("drawing"))
        .filter_map(|node| {
            node.attributes()
                .find(|attribute| attribute.name() == "id")
                .map(|attribute| attribute.value().to_string())
        })
        .collect();
    if referenced_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut charts = Vec::new();
    for relationship_id in referenced_ids {
        let Some(drawing_path) = drawing_targets.get(&relationship_id) else {
            continue;
        };
        let drawing_xml = read_zip_file(archive, drawing_path)?;
        let anchored_charts = parse_drawing_anchors(&drawing_xml)?;
        if anchored_charts.is_empty() {
            continue;
        }
        let chart_targets = read_relationship_targets(archive, drawing_path, "/chart")?;
        for (anchor, chart_rid) in anchored_charts {
            let Some(chart_path) = chart_targets.get(&chart_rid) else {
                continue;
            };
            let chart_xml = read_zip_file(archive, chart_path)?;
            if let Some(mut chart) = parse_chart_space(&chart_xml)? {
                chart.anchor = anchor;
                charts.push(chart);
            }
        }
    }
    Ok(charts)
}

/// Extract each `(anchor, chart relationship id)` pair from a drawing part.
/// Only `twoCellAnchor`, `oneCellAnchor`, and `absoluteAnchor` frames that
/// reference a chart are considered. The anchor kind is preserved so a
/// one-cell or absolute frame is not collapsed into a degenerate two-cell
/// frame on re-export.
fn parse_drawing_anchors(drawing_xml: &str) -> Result<Vec<(ChartAnchor, String)>, XlsxError> {
    use sheets_chart::ChartAnchorKind;
    let doc = Document::parse(drawing_xml)?;
    let mut result = Vec::new();
    for anchor_node in doc.descendants().filter(|node| {
        node.has_tag_name("twoCellAnchor")
            || node.has_tag_name("oneCellAnchor")
            || node.has_tag_name("absoluteAnchor")
    }) {
        let Some(chart_rid) = anchor_node
            .descendants()
            .find(|node| node.has_tag_name("chart"))
            .and_then(|node| {
                node.attributes()
                    .find(|attribute| attribute.name() == "id")
                    .map(|attribute| attribute.value().to_string())
            })
        else {
            continue;
        };

        let kind = if anchor_node.has_tag_name("absoluteAnchor") {
            ChartAnchorKind::Absolute
        } else if anchor_node.has_tag_name("oneCellAnchor") {
            ChartAnchorKind::OneCell
        } else {
            ChartAnchorKind::TwoCell
        };

        let anchor = match kind {
            ChartAnchorKind::Absolute => {
                let pos = parse_anchor_pos(anchor_node).unwrap_or((0, 0));
                let ext = parse_anchor_ext(anchor_node).unwrap_or((0, 0));
                ChartAnchor::absolute(pos.0, pos.1, ext.0, ext.1)
            }
            ChartAnchorKind::OneCell => {
                let from = parse_anchor_marker(anchor_node, "from").unwrap_or((0, 0));
                let ext = parse_anchor_ext(anchor_node).unwrap_or((0, 0));
                ChartAnchor::one_cell(from.0, from.1, ext.0, ext.1)
            }
            ChartAnchorKind::TwoCell => {
                let from = parse_anchor_marker(anchor_node, "from").unwrap_or((0, 0));
                let to = parse_anchor_marker(anchor_node, "to").unwrap_or(from);
                ChartAnchor::new(from.0, from.1, to.0, to.1)
            }
        };
        result.push((anchor, chart_rid));
    }
    Ok(result)
}

/// Read a `from` or `to` anchor marker as `(row, col)`. Returns `None` when the
/// marker or its numeric cells are absent.
fn parse_anchor_marker(anchor_node: Node<'_, '_>, marker_name: &str) -> Option<(u32, u32)> {
    let marker = anchor_node
        .children()
        .find(|child| child.has_tag_name(marker_name))?;
    let col = marker
        .children()
        .find(|child| child.has_tag_name("col"))
        .and_then(|node| node.text())
        .and_then(|text| text.trim().parse::<u32>().ok())?;
    let row = marker
        .children()
        .find(|child| child.has_tag_name("row"))
        .and_then(|node| node.text())
        .and_then(|text| text.trim().parse::<u32>().ok())?;
    Some((row, col))
}

/// Read the EMU extent `(cx, cy)` of an anchor's `<ext>` child. Used by
/// `oneCellAnchor` and `absoluteAnchor` frames.
fn parse_anchor_ext(anchor_node: Node<'_, '_>) -> Option<(u32, u32)> {
    let ext = anchor_node
        .children()
        .find(|child| child.has_tag_name("ext"))?;
    let cx = ext
        .attribute("cx")
        .and_then(|value| value.trim().parse::<u32>().ok())?;
    let cy = ext
        .attribute("cy")
        .and_then(|value| value.trim().parse::<u32>().ok())?;
    Some((cx, cy))
}

/// Read the EMU position `(x, y)` of an `absoluteAnchor`'s `<pos>` child.
fn parse_anchor_pos(anchor_node: Node<'_, '_>) -> Option<(u32, u32)> {
    let pos = anchor_node
        .children()
        .find(|child| child.has_tag_name("pos"))?;
    let x = pos
        .attribute("x")
        .and_then(|value| value.trim().parse::<u32>().ok())?;
    let y = pos
        .attribute("y")
        .and_then(|value| value.trim().parse::<u32>().ok())?;
    Some((x, y))
}

/// Parse a `chartN.xml` part into a `ChartObject` without an anchor. Returns
/// `None` for a chart whose family 900Sheets does not preserve (for example
/// `scatterChart`) or one with no plottable series.
fn parse_chart_space(xml: &str) -> Result<Option<ChartObject>, XlsxError> {
    let doc = Document::parse(xml)?;
    let Some(chart) = doc.descendants().find(|node| node.has_tag_name("chart")) else {
        return Ok(None);
    };

    let title = parse_chart_title(chart);
    let legend_position = parse_legend_position(chart);
    let Some(plot_area) = chart.children().find(|node| node.has_tag_name("plotArea")) else {
        return Ok(None);
    };
    let Some((chart_type, type_node)) = detect_chart_type(plot_area) else {
        return Ok(None);
    };

    let mut series = Vec::new();
    for ser in type_node
        .descendants()
        .filter(|node| node.has_tag_name("ser"))
    {
        series.push(parse_series(ser));
    }
    if series.is_empty() {
        return Ok(None);
    }

    let chart_object = ChartObject {
        title,
        chart_type,
        anchor: ChartAnchor::new(0, 0, 0, 0),
        series,
        legend_position,
    };
    if chart_object.validate(MAX_ROWS, MAX_COLS).is_ok() {
        Ok(Some(chart_object))
    } else {
        Ok(None)
    }
}

/// Detect the OOXML chart family within a plot area and map it to the
/// `ChartType` 900Sheets preserves, returning the chart-type element so its
/// series can be read. Bar direction distinguishes `Bar` (horizontal) from
/// `Column` (vertical).
fn detect_chart_type<'a>(plot_area: Node<'a, 'a>) -> Option<(ChartType, Node<'a, 'a>)> {
    for node in plot_area.children() {
        if node.has_tag_name("barChart") {
            let horizontal = node
                .children()
                .find(|child| child.has_tag_name("barDir"))
                .and_then(|child| child.attribute("val"))
                == Some("bar");
            let chart_type = if horizontal {
                ChartType::Bar
            } else {
                ChartType::Column
            };
            return Some((chart_type, node));
        }
        if node.has_tag_name("lineChart") {
            return Some((ChartType::Line, node));
        }
        if node.has_tag_name("pieChart") {
            return Some((ChartType::Pie, node));
        }
        if node.has_tag_name("areaChart") {
            return Some((ChartType::Area, node));
        }
        if node.has_tag_name("doughnutChart") {
            return Some((ChartType::Doughnut, node));
        }
    }
    None
}

/// Extract the literal title text of a chart, joining rich-text runs. Returns
/// `None` when the title is only a dynamic reference or is absent.
fn parse_chart_title(chart: Node<'_, '_>) -> Option<String> {
    let title = chart.children().find(|node| node.has_tag_name("title"))?;
    let text: String = title
        .descendants()
        .filter(|node| node.has_tag_name("t"))
        .filter_map(|node| node.text())
        .collect::<Vec<_>>()
        .join("");
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Map a chart legend position to the persisted enum. A present legend with an
/// unrecognized position defaults to `Right`, matching Excel's default.
fn parse_legend_position(chart: Node<'_, '_>) -> LegendPosition {
    let Some(legend) = chart.children().find(|node| node.has_tag_name("legend")) else {
        return LegendPosition::None;
    };
    match legend
        .descendants()
        .find(|node| node.has_tag_name("legendPos"))
        .and_then(|node| node.attribute("val"))
    {
        Some("b") => LegendPosition::Bottom,
        Some("t") => LegendPosition::Top,
        Some("l") => LegendPosition::Left,
        _ => LegendPosition::Right,
    }
}

/// Read a single persisted chart series: title, category, and value worksheet
/// range references. Each reference is the OOXML `<c:f>` child of the series'
/// `tx`, `cat`, or `val` element.
fn parse_series(ser: Node<'_, '_>) -> ChartObjectSeries {
    ChartObjectSeries {
        name_ref: ser
            .children()
            .find(|node| node.has_tag_name("tx"))
            .and_then(|tx| first_child_formula(tx)),
        category_ref: ser
            .children()
            .find(|node| node.has_tag_name("cat"))
            .and_then(|cat| first_child_formula(cat)),
        value_ref: ser
            .children()
            .find(|node| node.has_tag_name("val"))
            .and_then(|val| first_child_formula(val)),
    }
}

/// First `<c:f>` formula text within a node, trimmed.
fn first_child_formula(node: Node<'_, '_>) -> Option<String> {
    node.descendants()
        .find(|descendant| descendant.has_tag_name("f"))
        .and_then(|formula| formula.text())
        .map(|text| text.trim().to_string())
}

#[derive(Default)]
struct XlsxStyles {
    num_fmts: HashMap<usize, String>,
    fonts: Vec<CellFormat>,
    fills: Vec<String>,
    borders: Vec<CellFormat>,
    cell_xfs: Vec<CellXf>,
    differential_formats: Vec<CellFormat>,
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
    read_styles_with_limit(archive, MAX_XML_ENTRY_SIZE)
}

fn read_styles_with_limit<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    max_entry_size: u64,
) -> Result<XlsxStyles, XlsxError> {
    let xml = match read_zip_file_with_limit(archive, "xl/styles.xml", max_entry_size) {
        Ok(content) => content,
        Err(XlsxError::Zip(zip::result::ZipError::FileNotFound)) => {
            return Ok(XlsxStyles::default())
        }
        Err(error) => return Err(error),
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
                    fmt.bold = Some(bool_attribute(child.attribute("val"), true));
                } else if child.has_tag_name("i") {
                    fmt.italic = Some(bool_attribute(child.attribute("val"), true));
                } else if child.has_tag_name("u") {
                    fmt.underline = Some(bool_attribute(child.attribute("val"), true));
                } else if child.has_tag_name("strike") {
                    fmt.strikethrough = Some(bool_attribute(child.attribute("val"), true));
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

    if let Some(dxfs) = doc.descendants().find(|node| node.has_tag_name("dxfs")) {
        for dxf in dxfs.children().filter(|node| node.has_tag_name("dxf")) {
            styles
                .differential_formats
                .push(parse_differential_format(dxf));
        }
    }

    Ok(styles)
}

fn parse_differential_format(node: roxmltree::Node<'_, '_>) -> CellFormat {
    let mut format = CellFormat::default();
    if let Some(font) = node.children().find(|child| child.has_tag_name("font")) {
        for child in font.children().filter(|child| child.is_element()) {
            match child.tag_name().name() {
                "b" => format.bold = Some(bool_attribute(child.attribute("val"), true)),
                "i" => format.italic = Some(bool_attribute(child.attribute("val"), true)),
                "u" => format.underline = Some(bool_attribute(child.attribute("val"), true)),
                "strike" => {
                    format.strikethrough = Some(bool_attribute(child.attribute("val"), true))
                }
                "sz" => format.font_size = child.attribute("val").and_then(|v| v.parse().ok()),
                "name" => format.font_name = child.attribute("val").map(str::to_string),
                "color" => format.font_color = parse_rgb_color(child.attribute("rgb")),
                _ => {}
            }
        }
    }
    if let Some(fill) = node.children().find(|child| child.has_tag_name("fill")) {
        format.bg_color = fill
            .descendants()
            .find(|child| child.has_tag_name("fgColor"))
            .and_then(|child| parse_rgb_color(child.attribute("rgb")));
    }
    if let Some(border) = node.children().find(|child| child.has_tag_name("border")) {
        for child in border.children().filter(|child| child.is_element()) {
            let parsed = parse_border(child);
            match child.tag_name().name() {
                "top" => format.border_top = parsed,
                "bottom" => format.border_bottom = parsed,
                "left" => format.border_left = parsed,
                "right" => format.border_right = parsed,
                _ => {}
            }
        }
    }
    if let Some(num_fmt) = node.children().find(|child| child.has_tag_name("numFmt")) {
        format.number_format = num_fmt.attribute("formatCode").map(str::to_string);
    }
    format
}

fn parse_sheet_features(xml: &str, styles: &XlsxStyles) -> Result<XlsxSheetFeatures, XlsxError> {
    let doc = Document::parse(xml)?;
    let mut features = XlsxSheetFeatures::default();

    for node in doc
        .descendants()
        .filter(|node| node.has_tag_name("dataValidation"))
    {
        let validation_type = match node.attribute("type").unwrap_or("none") {
            "whole" => ValidationType::WholeNumber,
            "decimal" => ValidationType::Decimal,
            "list" => ValidationType::List,
            "date" => ValidationType::Date,
            "time" => ValidationType::Time,
            "textLength" => ValidationType::TextLength,
            "custom" => ValidationType::Custom,
            _ => continue,
        };
        let formula1 = child_text(node, "formula1");
        let formula2 = child_text(node, "formula2");
        let source = if validation_type == ValidationType::List {
            formula1.as_deref().map(parse_list_source)
        } else {
            None
        };
        let validation = DataValidation {
            validation_type,
            operator: parse_validation_operator(node.attribute("operator")),
            formula1: if source.is_some() { None } else { formula1 },
            formula2,
            source,
            allow_blank: bool_attribute(node.attribute("allowBlank"), false),
            // OOXML showDropDown=true hides the list arrow.
            show_dropdown: !bool_attribute(node.attribute("showDropDown"), false),
            error_style: parse_validation_error_style(node.attribute("errorStyle")),
            error_title: node.attribute("errorTitle").map(str::to_string),
            error_message: node.attribute("error").map(str::to_string),
            prompt_title: node.attribute("promptTitle").map(str::to_string),
            prompt_message: node.attribute("prompt").map(str::to_string),
        };
        for range in parse_sqref(node.attribute("sqref").unwrap_or(""))? {
            if features.validations.len() >= MAX_FEATURE_RECORDS {
                return Err(XlsxError::InvalidFormat(format!(
                    "Worksheet contains more than {MAX_FEATURE_RECORDS} validation ranges"
                )));
            }
            features.validations.push(ValidationRule {
                range,
                validation: validation.clone(),
            });
        }
    }

    let mut rule_counter = 0usize;
    for group in doc
        .descendants()
        .filter(|node| node.has_tag_name("conditionalFormatting"))
    {
        let ranges = parse_sqref(group.attribute("sqref").unwrap_or(""))?;
        for node in group.children().filter(|node| node.has_tag_name("cfRule")) {
            let condition_type = match node.attribute("type") {
                Some("cellIs") => ConditionType::CellValue,
                Some("expression") => ConditionType::Formula,
                Some("containsText") => ConditionType::TextContains,
                Some("notContainsText") => ConditionType::TextNotContains,
                Some("beginsWith") => ConditionType::TextBeginsWith,
                Some("endsWith") => ConditionType::TextEndsWith,
                Some("containsBlanks") => ConditionType::Blanks,
                Some("notContainsBlanks") => ConditionType::NoBlanks,
                Some("duplicateValues") => ConditionType::Duplicate,
                _ => continue,
            };
            let values: Vec<String> = node
                .children()
                .filter(|child| child.has_tag_name("formula"))
                .filter_map(|child| child.text().map(str::to_string))
                .collect();
            let dxf_id = node
                .attribute("dxfId")
                .and_then(|value| value.parse::<usize>().ok());
            let format = dxf_id
                .and_then(|index| styles.differential_formats.get(index))
                .cloned()
                .unwrap_or_default();
            for range in &ranges {
                if features.conditional_formats.len() >= MAX_FEATURE_RECORDS {
                    return Err(XlsxError::InvalidFormat(format!(
                        "Worksheet contains more than {MAX_FEATURE_RECORDS} conditional-format ranges"
                    )));
                }
                rule_counter += 1;
                features.conditional_formats.push(ConditionalFormat {
                    id: format!("xlsx-cf-{rule_counter}"),
                    condition_type: condition_type.clone(),
                    range: *range,
                    operator: node
                        .attribute("operator")
                        .and_then(parse_condition_operator),
                    value1: if matches!(
                        condition_type,
                        ConditionType::TextContains
                            | ConditionType::TextNotContains
                            | ConditionType::TextBeginsWith
                            | ConditionType::TextEndsWith
                    ) {
                        node.attribute("text").map(str::to_string)
                    } else {
                        values.first().map(|value| {
                            if condition_type == ConditionType::Formula {
                                format!("={value}")
                            } else {
                                value.clone()
                            }
                        })
                    },
                    value2: values.get(1).cloned(),
                    format: format.clone(),
                    priority: node
                        .attribute("priority")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(rule_counter as u32),
                    stop_if_true: bool_attribute(node.attribute("stopIfTrue"), false),
                    ..Default::default()
                });
            }
        }
    }

    Ok(features)
}

fn child_text(node: roxmltree::Node<'_, '_>, name: &str) -> Option<String> {
    node.children()
        .find(|child| child.has_tag_name(name))
        .and_then(|child| child.text())
        .map(str::to_string)
}

fn parse_list_source(formula: &str) -> String {
    if formula.starts_with('"') && formula.ends_with('"') && formula.len() >= 2 {
        formula[1..formula.len() - 1].replace("\"\"", "\"")
    } else if formula.starts_with('=') {
        formula.to_string()
    } else {
        format!("={formula}")
    }
}

fn parse_validation_operator(value: Option<&str>) -> ValidationOperator {
    match value {
        Some("notBetween") => ValidationOperator::NotBetween,
        Some("equal") => ValidationOperator::Equal,
        Some("notEqual") => ValidationOperator::NotEqual,
        Some("greaterThan") => ValidationOperator::GreaterThan,
        Some("lessThan") => ValidationOperator::LessThan,
        Some("greaterThanOrEqual") => ValidationOperator::GreaterThanOrEqual,
        Some("lessThanOrEqual") => ValidationOperator::LessThanOrEqual,
        _ => ValidationOperator::Between,
    }
}

fn parse_condition_operator(value: &str) -> Option<ConditionOperator> {
    match value {
        "between" => Some(ConditionOperator::Between),
        "notBetween" => Some(ConditionOperator::NotBetween),
        "equal" => Some(ConditionOperator::Equal),
        "notEqual" => Some(ConditionOperator::NotEqual),
        "greaterThan" => Some(ConditionOperator::GreaterThan),
        "lessThan" => Some(ConditionOperator::LessThan),
        "greaterThanOrEqual" => Some(ConditionOperator::GreaterThanOrEqual),
        "lessThanOrEqual" => Some(ConditionOperator::LessThanOrEqual),
        _ => None,
    }
}

fn parse_validation_error_style(value: Option<&str>) -> ValidationErrorStyle {
    match value {
        Some("warning") => ValidationErrorStyle::Warning,
        Some("information") => ValidationErrorStyle::Information,
        _ => ValidationErrorStyle::Stop,
    }
}

fn bool_attribute(value: Option<&str>, default: bool) -> bool {
    value
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(default)
}

fn parse_sqref(value: &str) -> Result<Vec<(u32, u32, u32, u32)>, XlsxError> {
    let mut ranges = Vec::new();
    for reference in value.split_whitespace() {
        if let Some(range) = parse_range_ref(reference) {
            if ranges.len() >= MAX_FEATURE_RECORDS {
                return Err(XlsxError::InvalidFormat(format!(
                    "Worksheet feature range list exceeds the limit of {MAX_FEATURE_RECORDS}"
                )));
            }
            ranges.push(range);
        }
    }
    Ok(ranges)
}

fn parse_range_ref(value: &str) -> Option<(u32, u32, u32, u32)> {
    let value = value.replace('$', "");
    let mut parts = value.split(':');
    let start = parse_cell_ref(parts.next()?)?;
    let end = match parts.next() {
        Some(end) => parse_cell_ref(end)?,
        None => start,
    };
    if parts.next().is_some() || start.0 > end.0 || start.1 > end.1 {
        return None;
    }
    Some((start.0, start.1, end.0, end.1))
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
                        let index_text = value_node.and_then(|n| n.text()).ok_or_else(|| {
                            XlsxError::InvalidFormat(format!(
                                "Shared-string cell {ref_attr} contains no index"
                            ))
                        })?;
                        let idx: usize = index_text.parse().map_err(|_| {
                            XlsxError::InvalidFormat(format!(
                                "Shared-string cell {ref_attr} has invalid index {index_text}"
                            ))
                        })?;
                        let value = shared_strings.get(idx).ok_or_else(|| {
                            XlsxError::InvalidFormat(format!(
                                "Shared-string cell {ref_attr} refers to missing index {idx}"
                            ))
                        })?;
                        CellValue::text(value)
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
        col = col.checked_mul(26)?.checked_add(val)?;
    }
    col.checked_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

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
        assert_eq!(col_label_to_index(&"Z".repeat(32)), None);
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

    fn archive_with(path: &str, contents: &[u8]) -> zip::ZipArchive<std::io::Cursor<Vec<u8>>> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file(path, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(contents).unwrap();
        zip::ZipArchive::new(std::io::Cursor::new(zip.finish().unwrap().into_inner())).unwrap()
    }

    #[test]
    fn malformed_and_oversized_shared_strings_are_errors() {
        let mut malformed = archive_with("xl/sharedStrings.xml", b"<sst><si>");
        assert!(matches!(
            read_shared_strings(&mut malformed),
            Err(XlsxError::Xml(_))
        ));

        let mut oversized = archive_with("xl/sharedStrings.xml", b"123456");
        assert!(matches!(
            read_shared_strings_with_limit(&mut oversized, 5),
            Err(XlsxError::FileTooLarge(6, 5))
        ));
    }

    #[test]
    fn malformed_and_oversized_styles_are_errors() {
        let mut malformed = archive_with("xl/styles.xml", b"<styleSheet><fonts>");
        assert!(matches!(
            read_styles(&mut malformed),
            Err(XlsxError::Xml(_))
        ));

        let mut oversized = archive_with("xl/styles.xml", b"123456");
        assert!(matches!(
            read_styles_with_limit(&mut oversized, 5),
            Err(XlsxError::FileTooLarge(6, 5))
        ));
    }

    #[test]
    fn invalid_and_missing_shared_string_indexes_are_errors() {
        let invalid = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="s"><v>not-a-number</v></c></row></sheetData></worksheet>"#;
        assert!(matches!(
            parse_sheet_xml(invalid, &["only".into()]),
            Err(XlsxError::InvalidFormat(message)) if message.contains("invalid index")
        ));

        let missing = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="s"><v>4</v></c></row></sheetData></worksheet>"#;
        assert!(matches!(
            parse_sheet_xml(missing, &["only".into()]),
            Err(XlsxError::InvalidFormat(message)) if message.contains("missing index")
        ));
    }

    #[test]
    fn direct_font_boolean_false_values_remain_false() {
        let xml = r#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font><b val="false"/><i val="0"/><u val="false"/><strike val="0"/></font></fonts></styleSheet>"#;
        let mut archive = archive_with("xl/styles.xml", xml.as_bytes());
        let styles = read_styles(&mut archive).unwrap();
        let format = &styles.fonts[0];
        assert_eq!(format.bold, Some(false));
        assert_eq!(format.italic, Some(false));
        assert_eq!(format.underline, Some(false));
        assert_eq!(format.strikethrough, Some(false));
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
        zip.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData><row r=\"1\"><c r=\"A1\" t=\"s\"><v>0</v></c></row></sheetData><dataValidations count=\"1\"><dataValidation type=\"list\" sqref=\"A1\"><formula1>&quot;Hello,World&quot;</formula1></dataValidation></dataValidations></worksheet>").unwrap();
        zip.start_file("xl/worksheets/sheet2.xml", opts).unwrap();
        zip.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData><row r=\"1\"><c r=\"A1\" t=\"n\"><v>99</v></c></row></sheetData></worksheet>").unwrap();
        let data = zip.finish().unwrap().into_inner();

        let document = import_document(&data).unwrap();
        let workbook = &document.workbook;
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
        assert_eq!(document.sheet_features[0].validations.len(), 1);
        assert!(document.sheet_features[1].validations.is_empty());
        assert_eq!(
            document.sheet_features[0].validations[0]
                .validation
                .source
                .as_deref(),
            Some("Hello,World")
        );
    }

    #[test]
    fn feature_range_lists_are_bounded_before_unbounded_collection() {
        let sqref = std::iter::repeat_n("A1", MAX_FEATURE_RECORDS + 1)
            .collect::<Vec<_>>()
            .join(" ");
        let xml = format!(
            "<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData/><dataValidations count=\"1\"><dataValidation type=\"list\" sqref=\"{sqref}\"><formula1>&quot;Yes,No&quot;</formula1></dataValidation></dataValidations></worksheet>"
        );
        assert!(matches!(
            parse_sheet_features(&xml, &XlsxStyles::default()),
            Err(XlsxError::InvalidFormat(message)) if message.contains("range list exceeds")
        ));
    }

    #[test]
    fn multiple_sqref_ranges_become_separate_sheet_scoped_rules() {
        let xml = r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<sheetData/>
<dataValidations count="1"><dataValidation type="list" allowBlank="1" showDropDown="0" sqref="$A$1:$A$3 C2"><formula1>"Yes,No"</formula1></dataValidation></dataValidations>
<conditionalFormatting sqref="D2:D4 F1"><cfRule type="cellIs" operator="greaterThan" priority="1"><formula>10</formula></cfRule></conditionalFormatting>
</worksheet>"#;
        let features = parse_sheet_features(xml, &XlsxStyles::default()).unwrap();
        assert_eq!(features.validations.len(), 2);
        assert_eq!(features.validations[0].range, (0, 0, 2, 0));
        assert_eq!(features.validations[1].range, (1, 2, 1, 2));
        assert_eq!(features.conditional_formats.len(), 2);
        assert_eq!(features.conditional_formats[0].range, (1, 3, 3, 3));
        assert_eq!(features.conditional_formats[1].range, (0, 5, 0, 5));
    }

    #[test]
    fn differential_format_boolean_false_values_remain_false() {
        let xml = r#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dxfs count="1"><dxf><font><b val="false"/><i val="0"/><u val="true"/><strike val="1"/></font></dxf></dxfs></styleSheet>"#;
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        archive.start_file("xl/styles.xml", options).unwrap();
        archive.write_all(xml.as_bytes()).unwrap();
        let data = archive.finish().unwrap().into_inner();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data)).unwrap();
        let styles = read_styles(&mut archive).unwrap();
        let format = &styles.differential_formats[0];
        assert_eq!(format.bold, Some(false));
        assert_eq!(format.italic, Some(false));
        assert_eq!(format.underline, Some(true));
        assert_eq!(format.strikethrough, Some(true));
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

    #[test]
    fn worksheet_relationship_resolves_excel_table_parts() {
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

        // Worksheet with a <tableParts> reference and the relationships namespace.
        zip.start_file("xl/worksheets/sheet1.xml", opts).unwrap();
        zip.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">
<sheetData><row r=\"1\"><c r=\"A1\" t=\"s\"><v>0</v></c></row></sheetData>
<tableParts count=\"1\"><tablePart r:id=\"rId1\"/></tableParts>
</worksheet>").unwrap();

        // Worksheet relationship part pointing at a table via the ../tables
        // relative target that Excel emits.
        zip.start_file("xl/worksheets/_rels/sheet1.xml.rels", opts)
            .unwrap();
        zip.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/table\" Target=\"../tables/table1.xml\"/>
</Relationships>").unwrap();

        zip.start_file("xl/tables/table1.xml", opts).unwrap();
        zip.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>
<table xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" id=\"1\" name=\"Inventory\" displayName=\"Inventory\" ref=\"A1:C4\" headerRowCount=\"1\" totalsRowCount=\"1\">
<autoFilter ref=\"A1:C4\"/>
<tableColumns count=\"3\">
<tableColumn id=\"1\" name=\"SKU\"/>
<tableColumn id=\"2\" name=\"Name\"/>
<tableColumn id=\"3\" name=\"Qty\" totalsRowFunction=\"sum\"/>
</tableColumns>
<tableStyleInfo name=\"TableStyleMedium2\" showFirstColumn=\"0\" showLastColumn=\"0\" showRowStripes=\"1\" showColumnStripes=\"0\"/>
</table>").unwrap();

        let data = zip.finish().unwrap().into_inner();
        let document = import_document(&data).unwrap();

        assert_eq!(document.sheet_features[0].tables.len(), 1);
        let table = &document.sheet_features[0].tables[0];
        assert_eq!(table.name, "Inventory");
        assert_eq!(table.range, (0, 0, 3, 2));
        assert!(table.totals_row_shown);
        assert_eq!(table.columns.len(), 3);
        assert_eq!(table.columns[0].name, "SKU");
        assert_eq!(
            table.columns[2].totals_row_function,
            Some(sheets_tables::TotalsRowFunction::Sum)
        );
        assert_eq!(table.auto_filter_range, Some((0, 0, 3, 2)));
        assert!(table.style.show_row_stripes);
    }

    #[test]
    fn out_of_bounds_table_parts_are_skipped_not_fatal() {
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
        zip.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">
<sheetData/>
<tableParts count=\"1\"><tablePart r:id=\"rId1\"/></tableParts>
</worksheet>").unwrap();
        zip.start_file("xl/worksheets/_rels/sheet1.xml.rels", opts)
            .unwrap();
        zip.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/table\" Target=\"../tables/table1.xml\"/>
</Relationships>").unwrap();
        // Range extends past the column limit; importer must skip, not error.
        zip.start_file("xl/tables/table1.xml", opts).unwrap();
        zip.write_all(b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>
<table xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" id=\"1\" name=\"Wide\" displayName=\"Wide\" ref=\"A1:ZZZ1\">
<tableColumns count=\"1\"><tableColumn id=\"1\" name=\"X\"/></tableColumns>
</table>").unwrap();

        let data = zip.finish().unwrap().into_inner();
        let document = import_document(&data).unwrap();
        assert!(document.sheet_features[0].tables.is_empty());
    }
}
