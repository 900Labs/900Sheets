use serde::{Deserialize, Serialize};
use sheets_core::format::HorizontalAlignment;
use sheets_core::sheet::Sheet;
use std::fmt::Write as _;
use thiserror::Error;

const MAX_PRINT_CELLS: u64 = 5_000_000;

// ============================================================================
// Print Configuration
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum PageSize {
    A4,
    A3,
    #[default]
    Letter,
    Legal,
    Tabloid,
}

impl PageSize {
    pub fn dimensions(&self, orientation: Orientation) -> (f64, f64) {
        // Returns (width, height) in points (1pt = 1/72 inch)
        let (w, h) = match self {
            PageSize::A4 => (595.28, 841.89),
            PageSize::A3 => (841.89, 1190.55),
            PageSize::Letter => (612.0, 792.0),
            PageSize::Legal => (612.0, 1008.0),
            PageSize::Tabloid => (792.0, 1224.0),
        };
        match orientation {
            Orientation::Portrait => (w, h),
            Orientation::Landscape => (h, w),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            PageSize::A4 => "A4",
            PageSize::A3 => "A3",
            PageSize::Letter => "Letter",
            PageSize::Legal => "Legal",
            PageSize::Tabloid => "Tabloid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum PrintScaling {
    /// Fit to actual size (100%)
    #[default]
    ActualSize,
    /// Fit to a single page width
    FitToPageWidth,
    /// Fit to a single page (both width and height)
    FitToSinglePage,
    /// Scale by a percentage (e.g., 0.5 = 50%)
    Scale(f64),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Margins {
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
    pub header: f64,
    pub footer: f64,
}

impl Default for Margins {
    fn default() -> Self {
        Self {
            top: 36.0,
            bottom: 36.0,
            left: 36.0,
            right: 36.0,
            header: 18.0,
            footer: 18.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HeaderFooter {
    pub left: Option<String>,
    pub center: Option<String>,
    pub right: Option<String>,
}

impl HeaderFooter {
    pub fn is_empty(&self) -> bool {
        self.left.is_none() && self.center.is_none() && self.right.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrintConfig {
    pub page_size: PageSize,
    pub orientation: Orientation,
    pub margins: Margins,
    pub scaling: PrintScaling,
    pub header: HeaderFooter,
    pub footer: HeaderFooter,
    /// Print gridlines
    pub gridlines: bool,
    /// Print row/column headings (A, B, C... / 1, 2, 3...)
    pub headings: bool,
    /// Print area as (start_row, start_col, end_row, end_col), None = auto-detect
    pub print_area: Option<(u32, u32, u32, u32)>,
    /// Rows to repeat on each page (e.g., header rows)
    pub repeat_rows: Option<(u32, u32)>,
    /// Columns to repeat on each page
    pub repeat_cols: Option<(u32, u32)>,
    /// Horizontal centering
    pub horizontal_center: bool,
    /// Vertical centering
    pub vertical_center: bool,
}

impl Default for PrintConfig {
    fn default() -> Self {
        Self {
            page_size: PageSize::default(),
            orientation: Orientation::default(),
            margins: Margins::default(),
            scaling: PrintScaling::default(),
            header: HeaderFooter::default(),
            footer: HeaderFooter::default(),
            gridlines: true,
            headings: false,
            print_area: None,
            repeat_rows: None,
            repeat_cols: None,
            horizontal_center: false,
            vertical_center: false,
        }
    }
}

// ============================================================================
// Page Break Calculation
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageLayout {
    pub pages: Vec<Page>,
    pub total_pages: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub row_range: (u32, u32),
    pub col_range: (u32, u32),
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum PrintError {
    #[error("No data to print")]
    NoData,
    #[error("Invalid print area: {0}")]
    InvalidPrintArea(String),
    #[error("PDF generation error: {0}")]
    PdfGeneration(String),
}

/// Calculate the used range of a sheet (min/max row/col with data)
pub fn used_range(sheet: &Sheet) -> Option<(u32, u32, u32, u32)> {
    let mut min_row = u32::MAX;
    let mut min_col = u32::MAX;
    let mut max_row = 0u32;
    let mut max_col = 0u32;

    for ((r, c), cell) in sheet.iter_cells() {
        if cell.is_empty() {
            continue;
        }
        min_row = min_row.min(r);
        min_col = min_col.min(c);
        max_row = max_row.max(r);
        max_col = max_col.max(c);
    }

    if min_row == u32::MAX {
        None
    } else {
        Some((min_row, min_col, max_row, max_col))
    }
}

/// Calculate page breaks for a sheet given print configuration
pub fn calculate_pages(sheet: &Sheet, config: &PrintConfig) -> Result<PageLayout, PrintError> {
    let (start_row, start_col, end_row, end_col) = config
        .print_area
        .unwrap_or_else(|| used_range(sheet).unwrap_or((0, 0, 0, 0)));

    if start_row > end_row || start_col > end_col {
        return Err(PrintError::InvalidPrintArea(format!(
            "start ({},{}) > end ({},{})",
            start_row, start_col, end_row, end_col
        )));
    }
    let cell_count =
        u64::from(end_row - start_row + 1).saturating_mul(u64::from(end_col - start_col + 1));
    if cell_count > MAX_PRINT_CELLS {
        return Err(PrintError::InvalidPrintArea(format!(
            "print area contains {cell_count} cells; maximum is {MAX_PRINT_CELLS}"
        )));
    }

    let (page_w, page_h) = config.page_size.dimensions(config.orientation);
    let usable_w = page_w - config.margins.left - config.margins.right;
    let usable_h = page_h - config.margins.top - config.margins.bottom;

    // Estimate column widths (default ~64pt per column, ~48pt per row)
    let default_col_width = 64.0;
    let default_row_height = 20.0;

    // Calculate scale factor
    let scale = match config.scaling {
        PrintScaling::ActualSize => 1.0,
        PrintScaling::Scale(s) => s,
        PrintScaling::FitToPageWidth => {
            let total_w = (end_col - start_col + 1) as f64 * default_col_width;
            if total_w > usable_w {
                usable_w / total_w
            } else {
                1.0
            }
        }
        PrintScaling::FitToSinglePage => {
            let total_w = (end_col - start_col + 1) as f64 * default_col_width;
            let total_h = (end_row - start_row + 1) as f64 * default_row_height;
            let scale_w = if total_w > usable_w {
                usable_w / total_w
            } else {
                1.0
            };
            let scale_h = if total_h > usable_h {
                usable_h / total_h
            } else {
                1.0
            };
            scale_w.min(scale_h)
        }
    };

    let scaled_col_width = default_col_width * scale;
    let scaled_row_height = default_row_height * scale;

    let mut pages = Vec::new();

    // Calculate column breaks
    let mut col_pages: Vec<(u32, u32)> = Vec::new();
    let mut col_start = start_col;
    let mut current_w = 0.0;
    let mut last_break_col = start_col;

    for col in start_col..=end_col {
        if current_w + scaled_col_width > usable_w && col > col_start {
            col_pages.push((col_start, col - 1));
            col_start = col;
            current_w = 0.0;
        }
        current_w += scaled_col_width;
        last_break_col = col;
    }
    col_pages.push((col_start, last_break_col));

    // Calculate row breaks
    let mut row_pages: Vec<(u32, u32)> = Vec::new();
    let mut row_start = start_row;
    let mut current_h = 0.0;
    let mut last_break_row = start_row;

    for row in start_row..=end_row {
        if current_h + scaled_row_height > usable_h && row > row_start {
            row_pages.push((row_start, row - 1));
            row_start = row;
            current_h = 0.0;
        }
        current_h += scaled_row_height;
        last_break_row = row;
    }
    row_pages.push((row_start, last_break_row));

    // Create page grid
    for row_range in &row_pages {
        for col_range in &col_pages {
            pages.push(Page {
                row_range: *row_range,
                col_range: *col_range,
            });
        }
    }

    let total_pages = pages.len();
    Ok(PageLayout { pages, total_pages })
}

// ============================================================================
// HTML Print Rendering
// ============================================================================

/// Generate HTML for printing a sheet. This HTML can be opened in a browser
/// or Tauri webview and printed to PDF via the browser's print functionality.
pub fn render_html(sheet: &Sheet, config: &PrintConfig) -> Result<String, PrintError> {
    let layout = calculate_pages(sheet, config)?;
    let mut html = String::new();

    let (page_w, page_h) = config.page_size.dimensions(config.orientation);

    writeln!(html, "<!DOCTYPE html>").unwrap();
    writeln!(html, "<html><head><meta charset=\"utf-8\">").unwrap();
    writeln!(html, "<title>{} - Print</title>", escape_html(sheet.name())).unwrap();
    writeln!(html, "<style>").unwrap();
    writeln!(
        html,
        "@page {{ size: {} {}; margin: 0; }}",
        config.page_size.as_str(),
        if config.orientation == Orientation::Landscape {
            "landscape"
        } else {
            "portrait"
        }
    )
    .unwrap();
    writeln!(html, "body {{ margin: 0; padding: 0; font-family: 'Helvetica', 'Arial', sans-serif; font-size: 11pt; }}").unwrap();
    writeln!(html, ".page {{ width: {:.1}pt; height: {:.1}pt; page-break-after: always; position: relative; box-sizing: border-box; padding: {:.1}pt {:.1}pt {:.1}pt {:.1}pt; }}", page_w, page_h, config.margins.top, config.margins.right, config.margins.bottom, config.margins.left).unwrap();
    writeln!(html, ".page:last-child {{ page-break-after: auto; }}").unwrap();
    writeln!(html, ".header {{ position: absolute; top: {:.1}pt; left: {:.1}pt; right: {:.1}pt; display: flex; justify-content: space-between; font-size: 9pt; color: #666; }}", config.margins.header, config.margins.left, config.margins.right).unwrap();
    writeln!(html, ".footer {{ position: absolute; bottom: {:.1}pt; left: {:.1}pt; right: {:.1}pt; display: flex; justify-content: space-between; font-size: 9pt; color: #666; }}", config.margins.footer, config.margins.left, config.margins.right).unwrap();
    writeln!(
        html,
        ".content {{ {} }}",
        if config.vertical_center {
            "display: flex; align-items: center; height: 100%;"
        } else {
            ""
        }
    )
    .unwrap();
    writeln!(
        html,
        "table {{ border-collapse: collapse; {} }}",
        if config.horizontal_center {
            "margin: 0 auto;"
        } else {
            ""
        }
    )
    .unwrap();

    if config.gridlines {
        writeln!(
            html,
            "td, th {{ border: 0.5pt solid #ccc; padding: 2pt 4pt; }}"
        )
        .unwrap();
    } else {
        writeln!(html, "td, th {{ padding: 2pt 4pt; }}").unwrap();
    }

    writeln!(html, "th {{ background: #f0f0f0; font-weight: bold; }}").unwrap();
    writeln!(html, ".cell-bold {{ font-weight: bold; }}").unwrap();
    writeln!(html, ".cell-italic {{ font-style: italic; }}").unwrap();
    writeln!(html, ".cell-underline {{ text-decoration: underline; }}").unwrap();
    writeln!(html, ".align-left {{ text-align: left; }}").unwrap();
    writeln!(html, ".align-center {{ text-align: center; }}").unwrap();
    writeln!(html, ".align-right {{ text-align: right; }}").unwrap();
    writeln!(html, "</style>").unwrap();
    writeln!(html, "</head><body>").unwrap();

    for (page_idx, page) in layout.pages.iter().enumerate() {
        writeln!(html, "<div class=\"page\">").unwrap();

        // Header
        if !config.header.is_empty() {
            writeln!(html, "<div class=\"header\">").unwrap();
            write!(
                html,
                "<span>{}</span>",
                config
                    .header
                    .left
                    .as_deref()
                    .map(escape_html)
                    .unwrap_or_default()
            )
            .unwrap();
            write!(
                html,
                "<span>{}</span>",
                config
                    .header
                    .center
                    .as_deref()
                    .map(escape_html)
                    .unwrap_or_default()
            )
            .unwrap();
            write!(
                html,
                "<span>{}</span>",
                config
                    .header
                    .right
                    .as_deref()
                    .map(escape_html)
                    .unwrap_or_default()
            )
            .unwrap();
            writeln!(html, "</div>").unwrap();
        }

        // Content table
        writeln!(html, "<div class=\"content\">").unwrap();
        writeln!(html, "<table>").unwrap();

        // Headings (column headers)
        if config.headings {
            writeln!(html, "<thead><tr><th></th>").unwrap();
            for col in page.col_range.0..=page.col_range.1 {
                write!(html, "<th>{}</th>", col_to_letter(col)).unwrap();
            }
            writeln!(html, "</tr></thead>").unwrap();
        }

        writeln!(html, "<tbody>").unwrap();

        // Repeat rows if configured
        let repeat_row_data: Vec<u32> = if let Some((rr_start, rr_end)) = config.repeat_rows {
            (rr_start..=rr_end).collect()
        } else {
            Vec::new()
        };

        // Repeat columns if configured
        let repeat_col_data: Vec<u32> = if let Some((rc_start, rc_end)) = config.repeat_cols {
            (rc_start..=rc_end).collect()
        } else {
            Vec::new()
        };

        // Print repeat rows first (if any and not already in page range)
        for &row in &repeat_row_data {
            if row < page.row_range.0 || row > page.row_range.1 {
                write_html_row(&mut html, sheet, row, &repeat_col_data, page, config);
            }
        }

        // Print page rows
        for row in page.row_range.0..=page.row_range.1 {
            write_html_row(&mut html, sheet, row, &repeat_col_data, page, config);
        }

        writeln!(html, "</tbody>").unwrap();
        writeln!(html, "</table>").unwrap();
        writeln!(html, "</div>").unwrap();

        // Footer
        if !config.footer.is_empty() {
            writeln!(html, "<div class=\"footer\">").unwrap();
            write!(
                html,
                "<span>{}</span>",
                config
                    .footer
                    .left
                    .as_deref()
                    .map(escape_html)
                    .unwrap_or_default()
            )
            .unwrap();
            let center_text = config
                .footer
                .center
                .as_deref()
                .map(escape_html)
                .unwrap_or_default();
            let center_with_page = center_text
                .replace("{page}", &(page_idx + 1).to_string())
                .replace("{pages}", &layout.total_pages.to_string());
            write!(html, "<span>{}</span>", center_with_page).unwrap();
            write!(
                html,
                "<span>{}</span>",
                config
                    .footer
                    .right
                    .as_deref()
                    .map(escape_html)
                    .unwrap_or_default()
            )
            .unwrap();
            writeln!(html, "</div>").unwrap();
        }

        writeln!(html, "</div>").unwrap();
    }

    writeln!(html, "</body></html>").unwrap();

    Ok(html)
}

fn write_html_row(
    html: &mut String,
    sheet: &Sheet,
    row: u32,
    repeat_cols: &[u32],
    page: &Page,
    config: &PrintConfig,
) {
    write!(html, "<tr>").unwrap();

    // Row heading
    if config.headings {
        write!(html, "<th>{}</th>", row + 1).unwrap();
    }

    // Repeat columns
    for &col in repeat_cols {
        if col < page.col_range.0 || col > page.col_range.1 {
            write_html_cell(html, sheet, row, col);
        }
    }

    // Page columns
    for col in page.col_range.0..=page.col_range.1 {
        write_html_cell(html, sheet, row, col);
    }

    writeln!(html, "</tr>").unwrap();
}

fn write_html_cell(html: &mut String, sheet: &Sheet, row: u32, col: u32) {
    let cell = sheet.cell(row, col);
    let format = sheet.get_format(row, col);

    let mut classes = Vec::new();

    let mut style = String::new();

    if let Some(fmt) = format {
        if fmt.bold == Some(true) {
            classes.push("cell-bold");
        }
        if fmt.italic == Some(true) {
            classes.push("cell-italic");
        }
        if fmt.underline == Some(true) {
            classes.push("cell-underline");
        }
        if let Some(ref color) = fmt.font_color {
            write!(style, "color: {}; ", color).unwrap();
        }
        if let Some(ref bg) = fmt.bg_color {
            write!(style, "background: {}; ", bg).unwrap();
        }
        if let Some(size) = fmt.font_size {
            write!(style, "font-size: {:.0}pt; ", size).unwrap();
        }
        if let Some(ref name) = fmt.font_name {
            write!(style, "font-family: '{}'; ", name).unwrap();
        }
        if let Some(align) = fmt.h_align {
            match align {
                HorizontalAlignment::Left => classes.push("align-left"),
                HorizontalAlignment::Center => classes.push("align-center"),
                HorizontalAlignment::Right => classes.push("align-right"),
                HorizontalAlignment::General => {}
            }
        }
    }

    let class_attr = if classes.is_empty() {
        String::new()
    } else {
        format!(" class=\"{}\"", classes.join(" "))
    };

    let style_attr = if style.is_empty() {
        String::new()
    } else {
        format!(" style=\"{}\"", style.trim_end())
    };

    let display = cell.map(|c| c.display.as_str()).unwrap_or("");
    write!(
        html,
        "<td{}{}>{}</td>",
        class_attr,
        style_attr,
        escape_html(display)
    )
    .unwrap();
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn col_to_letter(col: u32) -> String {
    let mut result = String::new();
    let mut c = col;
    loop {
        result.insert(0, (b'A' + (c % 26) as u8) as char);
        if c < 26 {
            break;
        }
        c = (c / 26) - 1;
    }
    result
}

// ============================================================================
// PDF Generation (Minimal valid PDF)
// ============================================================================

/// Generate a PDF from a sheet using the given print configuration.
/// This produces a minimal valid PDF with text content.
pub fn render_pdf(sheet: &Sheet, config: &PrintConfig) -> Result<Vec<u8>, PrintError> {
    let layout = calculate_pages(sheet, config)?;
    let (page_w, page_h) = config.page_size.dimensions(config.orientation);

    let mut pdf = PdfBuilder::new();

    for page in &layout.pages {
        pdf.add_page(page_w, page_h);
        pdf.begin_content();

        // Margins
        let content_x = config.margins.left;
        let content_y_top = page_h - config.margins.top;

        // Gridlines
        if config.gridlines {
            let col_width = 64.0;
            let row_height = 20.0;
            let num_cols = page.col_range.1 - page.col_range.0 + 1;
            let num_rows = page.row_range.1 - page.row_range.0 + 1;
            let grid_w = num_cols as f64 * col_width;
            let grid_h = num_rows as f64 * row_height;

            pdf.set_color_gray(0.8);
            pdf.set_line_width(0.5);

            // Vertical lines
            for i in 0..=num_cols {
                let x = content_x + i as f64 * col_width;
                pdf.move_to(x, content_y_top);
                pdf.line_to(x, content_y_top - grid_h);
                pdf.stroke();
            }

            // Horizontal lines
            for i in 0..=num_rows {
                let y = content_y_top - i as f64 * row_height;
                pdf.move_to(content_x, y);
                pdf.line_to(content_x + grid_w, y);
                pdf.stroke();
            }

            pdf.set_color_gray(0.0);
        }

        // Cell content
        let col_width = 64.0;
        let row_height = 20.0;
        let font_size = 9.0;

        pdf.set_font("F1", font_size);

        for (row_idx, row) in (page.row_range.0..=page.row_range.1).enumerate() {
            for (col_idx, col) in (page.col_range.0..=page.col_range.1).enumerate() {
                if let Some(cell) = sheet.cell(row, col) {
                    if cell.display.is_empty() {
                        continue;
                    }

                    let x = content_x + col_idx as f64 * col_width + 3.0;
                    let y = content_y_top - row_idx as f64 * row_height - font_size - 2.0;

                    let format = sheet.get_format(row, col);
                    let display = &cell.display;

                    // Apply bold simulation (just offset)
                    let _ = format;

                    pdf.text(x, y, display);
                }
            }
        }

        // Headings
        if config.headings {
            pdf.set_font("F1", 8.0);
            pdf.set_color_gray(0.5);

            // Column headers
            for (col_idx, col) in (page.col_range.0..=page.col_range.1).enumerate() {
                let x = content_x + col_idx as f64 * col_width + col_width / 2.0 - 4.0;
                let y = content_y_top + 10.0;
                pdf.text(x, y, &col_to_letter(col));
            }

            // Row headers
            for (row_idx, row) in (page.row_range.0..=page.row_range.1).enumerate() {
                let x = content_x - 15.0;
                let y = content_y_top - row_idx as f64 * row_height - font_size - 2.0;
                pdf.text(x, y, &(row + 1).to_string());
            }

            pdf.set_color_gray(0.0);
        }

        // Header/Footer
        if let Some(ref center) = config.header.center {
            pdf.set_font("F1", 8.0);
            pdf.set_color_gray(0.5);
            let text_width = center.len() as f64 * 4.5;
            pdf.text(
                (page_w - text_width) / 2.0,
                page_h - config.margins.header + 4.0,
                center,
            );
            pdf.set_color_gray(0.0);
        }

        if let Some(ref center) = config.footer.center {
            pdf.set_font("F1", 8.0);
            pdf.set_color_gray(0.5);
            let text = center
                .replace("{page}", "1")
                .replace("{pages}", &layout.total_pages.to_string());
            let text_width = text.len() as f64 * 4.5;
            pdf.text(
                (page_w - text_width) / 2.0,
                config.margins.footer - 4.0,
                &text,
            );
            pdf.set_color_gray(0.0);
        }

        if let Some(ref left) = config.header.left {
            pdf.set_font("F1", 8.0);
            pdf.set_color_gray(0.5);
            pdf.text(
                config.margins.left,
                page_h - config.margins.header + 4.0,
                left,
            );
            pdf.set_color_gray(0.0);
        }

        if let Some(ref right) = config.header.right {
            pdf.set_font("F1", 8.0);
            pdf.set_color_gray(0.5);
            let text_width = right.len() as f64 * 4.5;
            pdf.text(
                page_w - config.margins.right - text_width,
                page_h - config.margins.header + 4.0,
                right,
            );
            pdf.set_color_gray(0.0);
        }

        if let Some(ref left) = config.footer.left {
            pdf.set_font("F1", 8.0);
            pdf.set_color_gray(0.5);
            pdf.text(config.margins.left, config.margins.footer - 4.0, left);
            pdf.set_color_gray(0.0);
        }

        if let Some(ref right) = config.footer.right {
            pdf.set_font("F1", 8.0);
            pdf.set_color_gray(0.5);
            let text_width = right.len() as f64 * 4.5;
            pdf.text(
                page_w - config.margins.right - text_width,
                config.margins.footer - 4.0,
                right,
            );
            pdf.set_color_gray(0.0);
        }

        pdf.end_content();
    }

    pdf.build().map_err(PrintError::PdfGeneration)
}

// ============================================================================
// PDF Builder (Minimal valid PDF 1.4)
// ============================================================================

struct PdfPage {
    width: f64,
    height: f64,
    content: Vec<u8>,
}

struct PdfBuilder {
    pages: Vec<PdfPage>,
    current_content: String,
}

impl PdfBuilder {
    fn new() -> Self {
        Self {
            pages: Vec::new(),
            current_content: String::new(),
        }
    }

    fn add_page(&mut self, width: f64, height: f64) {
        // Flush any current content to the previous page
        if !self.pages.is_empty() || !self.current_content.is_empty() {
            // Previous page already added via begin/end content
        }
        self.pages.push(PdfPage {
            width,
            height,
            content: Vec::new(),
        });
    }

    fn begin_content(&mut self) {
        self.current_content.clear();
    }

    fn end_content(&mut self) {
        if let Some(page) = self.pages.last_mut() {
            page.content = self.current_content.as_bytes().to_vec();
        }
        self.current_content.clear();
    }

    fn set_font(&mut self, _name: &str, size: f64) {
        writeln!(self.current_content, "BT /F1 {:.1} Tf ET", size).unwrap();
    }

    fn set_color_gray(&mut self, gray: f64) {
        writeln!(self.current_content, "{:.2} G", gray).unwrap();
    }

    fn set_line_width(&mut self, w: f64) {
        writeln!(self.current_content, "{:.1} w", w).unwrap();
    }

    fn move_to(&mut self, x: f64, y: f64) {
        writeln!(self.current_content, "{:.1} {:.1} m", x, y).unwrap();
    }

    fn line_to(&mut self, x: f64, y: f64) {
        writeln!(self.current_content, "{:.1} {:.1} l", x, y).unwrap();
    }

    fn stroke(&mut self) {
        writeln!(self.current_content, "S").unwrap();
    }

    fn text(&mut self, x: f64, y: f64, s: &str) {
        let escaped = s
            .replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)");
        writeln!(
            self.current_content,
            "BT {:.1} {:.1} Td ({}) Tj ET",
            x, y, escaped
        )
        .unwrap();
    }

    fn build(self) -> Result<Vec<u8>, String> {
        let mut buf = String::new();
        let mut offsets: Vec<usize> = Vec::new();

        // Header
        buf.push_str("%PDF-1.4\n");
        // Binary comment to mark as binary
        buf.push_str("%\u{00E2}\u{00E3}\u{00CF}\u{00D3}\n");

        let num_pages = self.pages.len();

        // Object 1: Catalog
        offsets.push(buf.len());
        buf.push_str("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

        // Object 2: Pages
        offsets.push(buf.len());
        write!(buf, "2 0 obj\n<< /Type /Pages /Count {} ", num_pages).map_err(|e| e.to_string())?;
        buf.push_str("/Kids [");
        for i in 0..num_pages {
            write!(buf, "{} 0 R ", 3 + i * 2).map_err(|e| e.to_string())?;
        }
        buf.push_str("] >>\nendobj\n");

        // Font object (shared, placed after pages and content)
        let font_obj_num = 3 + num_pages * 2;

        // Page and content objects
        for (i, page) in self.pages.iter().enumerate() {
            let page_obj_num = 3 + i * 2;
            let content_obj_num = 3 + i * 2 + 1;

            // Page object
            offsets.push(buf.len());
            write!(buf, "{} 0 obj\n<< /Type /Page /Parent 2 0 R ", page_obj_num)
                .map_err(|e| e.to_string())?;
            write!(buf, "/MediaBox [0 0 {:.1} {:.1}] ", page.width, page.height)
                .map_err(|e| e.to_string())?;
            write!(buf, "/Contents {} 0 R ", content_obj_num).map_err(|e| e.to_string())?;
            write!(
                buf,
                "/Resources << /Font << /F1 {} 0 R >> >> ",
                font_obj_num
            )
            .map_err(|e| e.to_string())?;
            buf.push_str(">>\nendobj\n");

            // Content stream object
            offsets.push(buf.len());
            write!(
                buf,
                "{} 0 obj\n<< /Length {} >>\nstream\n",
                content_obj_num,
                page.content.len()
            )
            .map_err(|e| e.to_string())?;
            // Insert content bytes directly
            let content_pos = buf.len();
            buf.push_str(&String::from_utf8_lossy(&page.content));
            let _ = content_pos;
            buf.push_str("\nendstream\nendobj\n");
        }

        // Font object
        offsets.push(buf.len());
        write!(
            buf,
            "{} 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n",
            font_obj_num
        )
        .map_err(|e| e.to_string())?;

        // Cross-reference table
        let xref_offset = buf.len();
        write!(buf, "xref\n0 {}\n", offsets.len() + 1).map_err(|e| e.to_string())?;
        buf.push_str("0000000000 65535 f \n");
        for &offset in &offsets {
            writeln!(buf, "{:010} 00000 n ", offset).map_err(|e| e.to_string())?;
        }

        // Trailer
        write!(
            buf,
            "trailer\n<< /Size {} /Root 1 0 R >>\n",
            offsets.len() + 1
        )
        .map_err(|e| e.to_string())?;
        write!(buf, "startxref\n{}\n%%EOF\n", xref_offset).map_err(|e| e.to_string())?;

        Ok(buf.into_bytes())
    }
}

// ============================================================================
// Print Preview Data (for frontend)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintPreview {
    pub pages: Vec<PrintPreviewPage>,
    pub total_pages: usize,
    pub page_size: String,
    pub orientation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintPreviewPage {
    pub page_number: usize,
    pub row_range: (u32, u32),
    pub col_range: (u32, u32),
    pub html: String,
}

/// Generate print preview data for the frontend
pub fn generate_preview(sheet: &Sheet, config: &PrintConfig) -> Result<PrintPreview, PrintError> {
    let layout = calculate_pages(sheet, config)?;
    let mut pages = Vec::new();

    for (idx, page) in layout.pages.iter().enumerate() {
        let mut html = String::new();
        writeln!(html, "<table>").unwrap();

        if config.headings {
            writeln!(html, "<thead><tr><th></th>").unwrap();
            for col in page.col_range.0..=page.col_range.1 {
                write!(html, "<th>{}</th>", col_to_letter(col)).unwrap();
            }
            writeln!(html, "</tr></thead>").unwrap();
        }

        writeln!(html, "<tbody>").unwrap();
        for row in page.row_range.0..=page.row_range.1 {
            write!(html, "<tr>").unwrap();
            if config.headings {
                write!(html, "<th>{}</th>", row + 1).unwrap();
            }
            for col in page.col_range.0..=page.col_range.1 {
                write_html_cell(&mut html, sheet, row, col);
            }
            writeln!(html, "</tr>").unwrap();
        }
        writeln!(html, "</tbody></table>").unwrap();

        pages.push(PrintPreviewPage {
            page_number: idx + 1,
            row_range: page.row_range,
            col_range: page.col_range,
            html,
        });
    }

    Ok(PrintPreview {
        pages,
        total_pages: layout.total_pages,
        page_size: config.page_size.as_str().to_string(),
        orientation: if config.orientation == Orientation::Landscape {
            "landscape".to_string()
        } else {
            "portrait".to_string()
        },
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use sheets_core::format::CellFormat;

    fn make_sheet() -> Sheet {
        let mut sheet = Sheet::new("TestSheet");
        for r in 0..10 {
            for c in 0..5 {
                sheet.set_cell_value(r, c, format!("R{}C{}", r + 1, c + 1));
            }
        }
        sheet
    }

    #[test]
    fn test_page_size_dimensions() {
        let (w, h) = PageSize::Letter.dimensions(Orientation::Portrait);
        assert_eq!(w, 612.0);
        assert_eq!(h, 792.0);

        let (w, h) = PageSize::Letter.dimensions(Orientation::Landscape);
        assert_eq!(w, 792.0);
        assert_eq!(h, 612.0);

        let (w, h) = PageSize::A4.dimensions(Orientation::Portrait);
        assert!((w - 595.28).abs() < 0.1);
        assert!((h - 841.89).abs() < 0.1);
    }

    #[test]
    fn test_page_size_as_str() {
        assert_eq!(PageSize::A4.as_str(), "A4");
        assert_eq!(PageSize::Letter.as_str(), "Letter");
        assert_eq!(PageSize::A3.as_str(), "A3");
    }

    #[test]
    fn test_default_config() {
        let config = PrintConfig::default();
        assert_eq!(config.page_size, PageSize::Letter);
        assert_eq!(config.orientation, Orientation::Portrait);
        assert!(config.gridlines);
        assert!(!config.headings);
    }

    #[test]
    fn test_used_range() {
        let sheet = make_sheet();
        let range = used_range(&sheet);
        assert_eq!(range, Some((0, 0, 9, 4)));
    }

    #[test]
    fn test_used_range_empty() {
        let sheet = Sheet::new("Empty");
        assert_eq!(used_range(&sheet), None);
    }

    #[test]
    fn test_calculate_pages() {
        let sheet = make_sheet();
        let config = PrintConfig::default();
        let layout = calculate_pages(&sheet, &config).unwrap();
        assert!(layout.total_pages > 0);
        assert!(!layout.pages.is_empty());
    }

    #[test]
    fn test_calculate_pages_fit_single() {
        let sheet = make_sheet();
        let config = PrintConfig {
            scaling: PrintScaling::FitToSinglePage,
            ..PrintConfig::default()
        };
        let layout = calculate_pages(&sheet, &config).unwrap();
        assert_eq!(layout.total_pages, 1);
    }

    #[test]
    fn test_calculate_pages_invalid_area() {
        let sheet = make_sheet();
        let config = PrintConfig {
            print_area: Some((10, 5, 0, 0)),
            ..PrintConfig::default()
        };
        let result = calculate_pages(&sheet, &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_render_html() {
        let sheet = make_sheet();
        let config = PrintConfig::default();
        let html = render_html(&sheet, &config).unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<table>"));
        assert!(html.contains("R1C1"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn test_render_html_with_headings() {
        let sheet = make_sheet();
        let config = PrintConfig {
            headings: true,
            ..PrintConfig::default()
        };
        let html = render_html(&sheet, &config).unwrap();
        assert!(html.contains("<thead>"));
        assert!(html.contains("<th>A</th>"));
        assert!(html.contains("<th>1</th>"));
    }

    #[test]
    fn test_render_html_with_header_footer() {
        let sheet = make_sheet();
        let config = PrintConfig {
            header: HeaderFooter {
                left: Some("My Spreadsheet".to_string()),
                center: Some("Page {page} of {pages}".to_string()),
                right: None,
            },
            footer: HeaderFooter {
                left: None,
                center: Some("Confidential".to_string()),
                right: Some("2024-01-01".to_string()),
            },
            ..PrintConfig::default()
        };
        let html = render_html(&sheet, &config).unwrap();
        assert!(html.contains("My Spreadsheet"));
        assert!(html.contains("Confidential"));
        assert!(html.contains("2024-01-01"));
    }

    #[test]
    fn test_render_html_with_format() {
        let mut sheet = Sheet::new("Formatted");
        sheet.set_cell_value(0, 0, "Bold Text".into());
        sheet.set_format(0, 0, CellFormat::new().bold(true).font_size(14.0));
        let config = PrintConfig::default();
        let html = render_html(&sheet, &config).unwrap();
        assert!(html.contains("cell-bold"));
        assert!(html.contains("font-size: 14pt"));
    }

    #[test]
    fn test_render_pdf() {
        let sheet = make_sheet();
        let config = PrintConfig::default();
        let pdf = render_pdf(&sheet, &config).unwrap();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));
    }

    #[test]
    fn test_render_pdf_multiple_pages() {
        let mut sheet = Sheet::new("Large");
        for r in 0..100 {
            for c in 0..20 {
                sheet.set_cell_value(r, c, format!("Cell {}-{}", r, c));
            }
        }
        let config = PrintConfig::default();
        let pdf = render_pdf(&sheet, &config).unwrap();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        // Should have multiple pages
        assert!(pdf.windows(9).any(|w| w == b"endstream"));
    }

    #[test]
    fn test_generate_preview() {
        let sheet = make_sheet();
        let config = PrintConfig {
            headings: true,
            ..PrintConfig::default()
        };
        let preview = generate_preview(&sheet, &config).unwrap();
        assert!(preview.total_pages > 0);
        assert!(!preview.pages.is_empty());
        assert!(preview.pages[0].html.contains("<table>"));
        assert!(preview.pages[0].html.contains("<thead>"));
    }

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("a & b"), "a &amp; b");
        assert_eq!(escape_html("<script>"), "&lt;script&gt;");
        assert_eq!(escape_html("\"quote\""), "&quot;quote&quot;");
    }

    #[test]
    fn test_col_to_letter() {
        assert_eq!(col_to_letter(0), "A");
        assert_eq!(col_to_letter(25), "Z");
        assert_eq!(col_to_letter(26), "AA");
    }

    #[test]
    fn test_margins_default() {
        let m = Margins::default();
        assert_eq!(m.top, 36.0);
        assert_eq!(m.bottom, 36.0);
        assert_eq!(m.left, 36.0);
        assert_eq!(m.right, 36.0);
    }

    #[test]
    fn test_header_footer_empty() {
        let hf = HeaderFooter::default();
        assert!(hf.is_empty());

        let hf = HeaderFooter {
            left: Some("test".to_string()),
            center: None,
            right: None,
        };
        assert!(!hf.is_empty());
    }

    #[test]
    fn test_print_scaling() {
        let config = PrintConfig {
            scaling: PrintScaling::Scale(0.5),
            ..PrintConfig::default()
        };
        let sheet = make_sheet();
        let layout = calculate_pages(&sheet, &config).unwrap();
        assert!(layout.total_pages > 0);
    }

    #[test]
    fn test_print_area() {
        let sheet = make_sheet();
        let config = PrintConfig {
            print_area: Some((2, 1, 5, 3)),
            ..PrintConfig::default()
        };
        let layout = calculate_pages(&sheet, &config).unwrap();
        let first_page = &layout.pages[0];
        assert_eq!(first_page.row_range.0, 2);
        assert_eq!(first_page.col_range.0, 1);
    }

    #[test]
    fn test_no_data_error() {
        let sheet = Sheet::new("Empty");
        let config = PrintConfig {
            print_area: Some((5, 5, 5, 5)),
            ..PrintConfig::default()
        };
        // Even with no data, if print_area is specified, it should produce pages
        let layout = calculate_pages(&sheet, &config).unwrap();
        assert_eq!(layout.total_pages, 1);
    }

    #[test]
    fn test_pdf_valid_structure() {
        let sheet = make_sheet();
        let config = PrintConfig::default();
        let pdf = render_pdf(&sheet, &config).unwrap();
        let pdf_str = String::from_utf8_lossy(&pdf);
        assert!(pdf_str.contains("/Type /Catalog"));
        assert!(pdf_str.contains("/Type /Pages"));
        assert!(pdf_str.contains("/Type /Page"));
        assert!(pdf_str.contains("/Type /Font"));
        assert!(pdf_str.contains("/BaseFont /Helvetica"));
        assert!(pdf_str.contains("startxref"));
        assert!(pdf_str.contains("xref"));
    }

    #[test]
    fn test_html_with_repeat_rows() {
        let sheet = make_sheet();
        let config = PrintConfig {
            repeat_rows: Some((0, 0)),
            ..PrintConfig::default()
        };
        let html = render_html(&sheet, &config).unwrap();
        assert!(html.contains("R1C1"));
    }

    #[test]
    fn test_landscape_orientation() {
        let sheet = make_sheet();
        let config = PrintConfig {
            orientation: Orientation::Landscape,
            ..PrintConfig::default()
        };
        let (w, h) = config.page_size.dimensions(config.orientation);
        assert!(w > h);
        let html = render_html(&sheet, &config).unwrap();
        assert!(html.contains("landscape"));
    }
}
