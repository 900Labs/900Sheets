use serde::{Deserialize, Serialize};
use sheets_core::sheet::Sheet;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChartType {
    Bar,
    Line,
    Pie,
    Scatter,
    Area,
    Column,
    Doughnut,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartSeries {
    pub name: String,
    pub x_column: u32,
    pub y_column: u32,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartConfig {
    pub title: String,
    pub chart_type: ChartType,
    pub series: Vec<ChartSeries>,
    pub header_row: u32,
    pub data_start_row: u32,
    pub data_end_row: u32,
    pub x_axis_label: Option<String>,
    pub y_axis_label: Option<String>,
    pub legend_position: LegendPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegendPosition {
    None,
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartData {
    pub series_name: String,
    pub points: Vec<(String, f64)>,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartResult {
    pub title: String,
    pub chart_type: ChartType,
    pub data: Vec<ChartData>,
    pub x_axis_label: String,
    pub y_axis_label: String,
    pub svg: String,
}

#[derive(Debug, Error, Serialize)]
pub enum ChartError {
    #[error("No series defined")]
    NoSeries,
    #[error("No data points found")]
    NoData,
    #[error("Invalid data range")]
    InvalidRange,
    #[error("Series {0} has no numeric data")]
    NoNumericData(String),
}

fn extract_cell_number(sheet: &Sheet, row: u32, col: u32) -> Option<f64> {
    sheet.cell(row, col).and_then(|c| c.as_number())
}

fn extract_cell_text(sheet: &Sheet, row: u32, col: u32) -> String {
    sheet.cell_value(row, col).unwrap_or_default()
}

fn default_colors() -> Vec<&'static str> {
    vec![
        "#4285F4", "#EA4335", "#FBBC04", "#34A853", "#FF6D01", "#46BDC6", "#7B1FA2", "#E91E63",
        "#00BCD4", "#8BC34A",
    ]
}

pub fn extract_chart_data(
    sheet: &Sheet,
    config: &ChartConfig,
) -> Result<Vec<ChartData>, ChartError> {
    if config.series.is_empty() {
        return Err(ChartError::NoSeries);
    }

    let colors = default_colors();
    let mut all_data = Vec::new();

    for (si, s) in config.series.iter().enumerate() {
        let mut points = Vec::new();
        let mut has_numeric = false;

        for row in config.data_start_row..=config.data_end_row {
            let x_val = extract_cell_text(sheet, row, s.x_column);
            if let Some(y_val) = extract_cell_number(sheet, row, s.y_column) {
                points.push((x_val, y_val));
                has_numeric = true;
            }
        }

        if !has_numeric {
            return Err(ChartError::NoNumericData(s.name.clone()));
        }

        if points.is_empty() {
            return Err(ChartError::NoData);
        }

        let color = s
            .color
            .clone()
            .unwrap_or_else(|| colors[si % colors.len()].to_string());

        all_data.push(ChartData {
            series_name: s.name.clone(),
            points,
            color,
        });
    }

    Ok(all_data)
}

pub fn build_chart(sheet: &Sheet, config: &ChartConfig) -> Result<ChartResult, ChartError> {
    let data = extract_chart_data(sheet, config)?;

    let x_label = config
        .x_axis_label
        .clone()
        .unwrap_or_else(|| "X".to_string());
    let y_label = config
        .y_axis_label
        .clone()
        .unwrap_or_else(|| "Y".to_string());

    let svg = match config.chart_type {
        ChartType::Bar | ChartType::Column => generate_bar_svg(
            &config.title,
            &data,
            &x_label,
            &y_label,
            &config.legend_position,
            config.chart_type == ChartType::Bar,
        ),
        ChartType::Line => generate_line_svg(
            &config.title,
            &data,
            &x_label,
            &y_label,
            &config.legend_position,
        ),
        ChartType::Area => generate_area_svg(
            &config.title,
            &data,
            &x_label,
            &y_label,
            &config.legend_position,
        ),
        ChartType::Pie | ChartType::Doughnut => generate_pie_svg(
            &config.title,
            &data,
            &config.legend_position,
            config.chart_type == ChartType::Doughnut,
        ),
        ChartType::Scatter => generate_scatter_svg(
            &config.title,
            &data,
            &x_label,
            &y_label,
            &config.legend_position,
        ),
    };

    Ok(ChartResult {
        title: config.title.clone(),
        chart_type: config.chart_type.clone(),
        data,
        x_axis_label: x_label,
        y_axis_label: y_label,
        svg,
    })
}

const SVG_W: f64 = 800.0;
const SVG_H: f64 = 450.0;
const PAD: f64 = 50.0;
const LEGEND_W: f64 = 150.0;

fn escape_svg_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn generate_bar_svg(
    title: &str,
    data: &[ChartData],
    x_label: &str,
    y_label: &str,
    legend: &LegendPosition,
    _horizontal: bool,
) -> String {
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
        SVG_W, SVG_H, SVG_W, SVG_H
    );

    svg.push_str(&format!(
        "<text x=\"{}\" y=\"25\" text-anchor=\"middle\" font-size=\"18\" font-weight=\"bold\">{}</text>\n",
        SVG_W / 2.0,
        escape_svg_text(title)
    ));

    let all_values: Vec<f64> = data
        .iter()
        .flat_map(|d| d.points.iter().map(|p| p.1))
        .collect();
    let max_val = all_values.iter().cloned().fold(0.0f64, f64::max);
    let max_val = if max_val == 0.0 { 1.0 } else { max_val * 1.1 };

    let categories: Vec<String> = if !data.is_empty() {
        data[0].points.iter().map(|p| p.0.clone()).collect()
    } else {
        vec![]
    };

    let chart_w = SVG_W - PAD * 2.0 - LEGEND_W;
    let chart_h = SVG_H - PAD * 2.0 - 20.0;
    let chart_x = PAD;
    let chart_y = PAD + 10.0;

    let n_cats = categories.len();
    let n_series = data.len();
    let group_w = chart_w / n_cats as f64;
    let bar_w = (group_w * 0.8 / n_series as f64).max(2.0);

    svg.push_str(&format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#333\"/>\n",
        chart_x,
        chart_y + chart_h,
        chart_x + chart_w,
        chart_y + chart_h
    ));
    svg.push_str(&format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#333\"/>\n",
        chart_x,
        chart_y,
        chart_x,
        chart_y + chart_h
    ));

    for (ci, cat) in categories.iter().enumerate() {
        let group_center = chart_x + ci as f64 * group_w + group_w / 2.0;
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"11\">{}</text>\n",
            group_center,
            chart_y + chart_h + 15.0,
            escape_svg_text(cat)
        ));

        for (si, series) in data.iter().enumerate() {
            if ci >= series.points.len() {
                continue;
            }
            let val = series.points[ci].1;
            let bar_h = (val / max_val * chart_h).max(1.0);
            let bar_x = chart_x + ci as f64 * group_w + si as f64 * bar_w + group_w * 0.1;
            let bar_y = chart_y + chart_h - bar_h;

            svg.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" rx=\"2\"/>\n",
                bar_x,
                bar_y,
                bar_w * 0.9,
                bar_h,
                series.color
            ));
        }
    }

    // Y axis labels
    for i in 0..=5 {
        let y = chart_y + chart_h - (i as f64 / 5.0) * chart_h;
        let val = (i as f64 / 5.0) * max_val;
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" font-size=\"10\">{:.0}</text>\n",
            chart_x - 5.0,
            y + 3.0,
            val
        ));
    }

    // Axis labels
    svg.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"12\">{}</text>\n",
        chart_x + chart_w / 2.0,
        SVG_H - 5.0,
        escape_svg_text(x_label)
    ));
    svg.push_str(&format!(
        "<text x=\"15\" y=\"{}\" text-anchor=\"middle\" font-size=\"12\" transform=\"rotate(-90 15 {})\" >{}</text>\n",
        chart_y + chart_h / 2.0,
        chart_y + chart_h / 2.0,
        escape_svg_text(y_label)
    ));

    // Legend
    if *legend != LegendPosition::None {
        let legend_x = SVG_W - LEGEND_W + 10.0;
        let legend_y = chart_y;
        for (si, series) in data.iter().enumerate() {
            let ly = legend_y + si as f64 * 20.0;
            svg.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"12\" height=\"12\" fill=\"{}\"/>\n",
                legend_x, ly, series.color
            ));
            svg.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-size=\"11\">{}</text>\n",
                legend_x + 16.0,
                ly + 10.0,
                escape_svg_text(&series.series_name)
            ));
        }
    }

    svg.push_str("</svg>");
    svg
}

fn generate_line_svg(
    title: &str,
    data: &[ChartData],
    x_label: &str,
    y_label: &str,
    legend: &LegendPosition,
) -> String {
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
        SVG_W, SVG_H, SVG_W, SVG_H
    );

    svg.push_str(&format!(
        "<text x=\"{}\" y=\"25\" text-anchor=\"middle\" font-size=\"18\" font-weight=\"bold\">{}</text>\n",
        SVG_W / 2.0,
        escape_svg_text(title)
    ));

    let all_values: Vec<f64> = data
        .iter()
        .flat_map(|d| d.points.iter().map(|p| p.1))
        .collect();
    let max_val = all_values.iter().cloned().fold(0.0f64, f64::max);
    let max_val = if max_val == 0.0 { 1.0 } else { max_val * 1.1 };

    let n_points = if !data.is_empty() {
        data[0].points.len()
    } else {
        0
    };

    let chart_w = SVG_W - PAD * 2.0 - LEGEND_W;
    let chart_h = SVG_H - PAD * 2.0 - 20.0;
    let chart_x = PAD;
    let chart_y = PAD + 10.0;

    // Axes
    svg.push_str(&format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#333\"/>\n",
        chart_x,
        chart_y + chart_h,
        chart_x + chart_w,
        chart_y + chart_h
    ));
    svg.push_str(&format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#333\"/>\n",
        chart_x,
        chart_y,
        chart_x,
        chart_y + chart_h
    ));

    // Grid lines
    for i in 0..=5 {
        let y = chart_y + chart_h - (i as f64 / 5.0) * chart_h;
        let val = (i as f64 / 5.0) * max_val;
        svg.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#eee\"/>\n",
            chart_x,
            y,
            chart_x + chart_w,
            y
        ));
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" font-size=\"10\">{:.0}</text>\n",
            chart_x - 5.0,
            y + 3.0,
            val
        ));
    }

    // X labels
    if n_points > 0 {
        let categories: Vec<String> = data[0].points.iter().map(|p| p.0.clone()).collect();
        let step = (n_points / 10).max(1);
        for (i, cat) in categories.iter().enumerate() {
            if i % step != 0 && i != n_points - 1 {
                continue;
            }
            let x = chart_x + (i as f64 / (n_points - 1).max(1) as f64) * chart_w;
            svg.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"10\">{}</text>\n",
                x,
                chart_y + chart_h + 15.0,
                escape_svg_text(cat)
            ));
        }
    }

    // Lines
    for series in data {
        if series.points.is_empty() {
            continue;
        }
        let mut path = String::new();
        for (i, (_, val)) in series.points.iter().enumerate() {
            let x = chart_x + (i as f64 / (n_points - 1).max(1) as f64) * chart_w;
            let y = chart_y + chart_h - (val / max_val) * chart_h;
            if i == 0 {
                path.push_str(&format!("M {} {}", x, y));
            } else {
                path.push_str(&format!(" L {} {}", x, y));
            }
        }
        svg.push_str(&format!(
            "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2\"/>\n",
            path, series.color
        ));

        // Points
        for (i, (_, val)) in series.points.iter().enumerate() {
            let x = chart_x + (i as f64 / (n_points - 1).max(1) as f64) * chart_w;
            let y = chart_y + chart_h - (val / max_val) * chart_h;
            svg.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"3\" fill=\"{}\"/>\n",
                x, y, series.color
            ));
        }
    }

    // Axis labels
    svg.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"12\">{}</text>\n",
        chart_x + chart_w / 2.0,
        SVG_H - 5.0,
        escape_svg_text(x_label)
    ));
    svg.push_str(&format!(
        "<text x=\"15\" y=\"{}\" text-anchor=\"middle\" font-size=\"12\" transform=\"rotate(-90 15 {})\" >{}</text>\n",
        chart_y + chart_h / 2.0,
        chart_y + chart_h / 2.0,
        escape_svg_text(y_label)
    ));

    // Legend
    if *legend != LegendPosition::None {
        let legend_x = SVG_W - LEGEND_W + 10.0;
        let legend_y = chart_y;
        for (si, series) in data.iter().enumerate() {
            let ly = legend_y + si as f64 * 20.0;
            svg.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"12\" height=\"12\" fill=\"{}\"/>\n",
                legend_x, ly, series.color
            ));
            svg.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-size=\"11\">{}</text>\n",
                legend_x + 16.0,
                ly + 10.0,
                escape_svg_text(&series.series_name)
            ));
        }
    }

    svg.push_str("</svg>");
    svg
}

fn generate_area_svg(
    title: &str,
    data: &[ChartData],
    x_label: &str,
    y_label: &str,
    legend: &LegendPosition,
) -> String {
    let mut svg = generate_line_svg(title, data, x_label, y_label, legend);

    let all_values: Vec<f64> = data
        .iter()
        .flat_map(|d| d.points.iter().map(|p| p.1))
        .collect();
    let max_val = all_values.iter().cloned().fold(0.0f64, f64::max);
    let max_val = if max_val == 0.0 { 1.0 } else { max_val * 1.1 };

    let n_points = if !data.is_empty() {
        data[0].points.len()
    } else {
        0
    };
    let chart_w = SVG_W - PAD * 2.0 - LEGEND_W;
    let chart_h = SVG_H - PAD * 2.0 - 20.0;
    let chart_x = PAD;
    let chart_y = PAD + 10.0;

    let insert_pos = svg.rfind("</svg>").unwrap_or(svg.len());
    let mut area_svg = String::new();

    for series in data {
        if series.points.is_empty() {
            continue;
        }
        let mut path = String::new();
        let x0 = chart_x;
        let y0 = chart_y + chart_h;
        path.push_str(&format!("M {} {}", x0, y0));

        for (i, (_, val)) in series.points.iter().enumerate() {
            let x = chart_x + (i as f64 / (n_points - 1).max(1) as f64) * chart_w;
            let y = chart_y + chart_h - (val / max_val) * chart_h;
            path.push_str(&format!(" L {} {}", x, y));
        }
        let x_end = chart_x + chart_w;
        path.push_str(&format!(" L {} {} Z", x_end, y0));

        let color = &series.color;
        area_svg.push_str(&format!(
            "<path d=\"{}\" fill=\"{}\" fill-opacity=\"0.3\" stroke=\"none\"/>\n",
            path, color
        ));
    }

    svg.insert_str(insert_pos, &area_svg);
    svg
}

fn generate_pie_svg(
    title: &str,
    data: &[ChartData],
    legend: &LegendPosition,
    doughnut: bool,
) -> String {
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
        SVG_W, SVG_H, SVG_W, SVG_H
    );

    svg.push_str(&format!(
        "<text x=\"{}\" y=\"25\" text-anchor=\"middle\" font-size=\"18\" font-weight=\"bold\">{}</text>\n",
        SVG_W / 2.0,
        escape_svg_text(title)
    ));

    // For pie charts, use first series
    if data.is_empty() || data[0].points.is_empty() {
        svg.push_str("</svg>");
        return svg;
    }

    let points = &data[0].points;
    let total: f64 = points.iter().map(|p| p.1).sum();
    if total == 0.0 {
        svg.push_str("</svg>");
        return svg;
    }

    let cx = SVG_W / 2.0 - 50.0;
    let cy = SVG_H / 2.0 + 10.0;
    let r = 150.0;
    let inner_r = if doughnut { r * 0.5 } else { 0.0 };

    let mut start_angle = -90.0_f64;

    for (i, (_label, val)) in points.iter().enumerate() {
        let fraction = val / total;
        let sweep_angle = fraction * 360.0;
        let end_angle = start_angle + sweep_angle;

        let start_rad = start_angle.to_radians();
        let end_rad = end_angle.to_radians();

        let x1 = cx + r * start_rad.cos();
        let y1 = cy + r * start_rad.sin();
        let x2 = cx + r * end_rad.cos();
        let y2 = cy + r * end_rad.sin();

        let large_arc = if sweep_angle > 180.0 { 1 } else { 0 };

        let colors = default_colors();
        let color = colors[i % colors.len()];

        if inner_r > 0.0 {
            let ix1 = cx + inner_r * start_rad.cos();
            let iy1 = cy + inner_r * start_rad.sin();
            let ix2 = cx + inner_r * end_rad.cos();
            let iy2 = cy + inner_r * end_rad.sin();
            svg.push_str(&format!(
                "<path d=\"M {} {} A {} {} 0 {} 1 {} {} L {} {} A {} {} 0 {} 0 {} {} Z\" fill=\"{}\" stroke=\"#fff\" stroke-width=\"1\"/>\n",
                x1, y1, r, r, large_arc, x2, y2,
                ix2, iy2, inner_r, inner_r, large_arc, ix1, iy1,
                color
            ));
        } else {
            svg.push_str(&format!(
                "<path d=\"M {} {} A {} {} 0 {} 1 {} {} L {} {} Z\" fill=\"{}\" stroke=\"#fff\" stroke-width=\"1\"/>\n",
                x1, y1, r, r, large_arc, x2, y2, cx, cy, color
            ));
        }

        // Percentage label
        let mid_angle = (start_angle + end_angle) / 2.0;
        let mid_rad = mid_angle.to_radians();
        let label_r = r * 0.65;
        let lx = cx + label_r * mid_rad.cos();
        let ly = cy + label_r * mid_rad.sin();
        if fraction > 0.05 {
            svg.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"11\" fill=\"#fff\" font-weight=\"bold\">{:.0}%</text>\n",
                lx, ly + 4.0, fraction * 100.0
            ));
        }

        start_angle = end_angle;
    }

    // Legend
    if *legend != LegendPosition::None {
        let legend_x = SVG_W - LEGEND_W + 10.0;
        let legend_y = PAD + 20.0;
        for (i, (label, val)) in points.iter().enumerate() {
            let ly = legend_y + i as f64 * 20.0;
            let colors = default_colors();
            let color = colors[i % colors.len()];
            svg.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"12\" height=\"12\" fill=\"{}\"/>\n",
                legend_x, ly, color
            ));
            svg.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-size=\"11\">{} ({:.0})</text>\n",
                legend_x + 16.0,
                ly + 10.0,
                escape_svg_text(label),
                val
            ));
        }
    }

    svg.push_str("</svg>");
    svg
}

fn generate_scatter_svg(
    title: &str,
    data: &[ChartData],
    x_label: &str,
    y_label: &str,
    legend: &LegendPosition,
) -> String {
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
        SVG_W, SVG_H, SVG_W, SVG_H
    );

    svg.push_str(&format!(
        "<text x=\"{}\" y=\"25\" text-anchor=\"middle\" font-size=\"18\" font-weight=\"bold\">{}</text>\n",
        SVG_W / 2.0,
        escape_svg_text(title)
    ));

    let all_values: Vec<f64> = data
        .iter()
        .flat_map(|d| d.points.iter().map(|p| p.1))
        .collect();
    let max_val = all_values.iter().cloned().fold(0.0f64, f64::max);
    let max_val = if max_val == 0.0 { 1.0 } else { max_val * 1.1 };

    let n_points = if !data.is_empty() {
        data[0].points.len()
    } else {
        0
    };

    let chart_w = SVG_W - PAD * 2.0 - LEGEND_W;
    let chart_h = SVG_H - PAD * 2.0 - 20.0;
    let chart_x = PAD;
    let chart_y = PAD + 10.0;

    // Axes
    svg.push_str(&format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#333\"/>\n",
        chart_x,
        chart_y + chart_h,
        chart_x + chart_w,
        chart_y + chart_h
    ));
    svg.push_str(&format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#333\"/>\n",
        chart_x,
        chart_y,
        chart_x,
        chart_y + chart_h
    ));

    // Grid lines
    for i in 0..=5 {
        let y = chart_y + chart_h - (i as f64 / 5.0) * chart_h;
        svg.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#eee\"/>\n",
            chart_x,
            y,
            chart_x + chart_w,
            y
        ));
        let val = (i as f64 / 5.0) * max_val;
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"end\" font-size=\"10\">{:.0}</text>\n",
            chart_x - 5.0,
            y + 3.0,
            val
        ));
    }

    // Scatter points
    for series in data {
        for (i, (_, val)) in series.points.iter().enumerate() {
            let x = chart_x + (i as f64 / (n_points - 1).max(1) as f64) * chart_w;
            let y = chart_y + chart_h - (val / max_val) * chart_h;
            svg.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"4\" fill=\"{}\" fill-opacity=\"0.7\"/>\n",
                x, y, series.color
            ));
        }
    }

    // Axis labels
    svg.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"12\">{}</text>\n",
        chart_x + chart_w / 2.0,
        SVG_H - 5.0,
        escape_svg_text(x_label)
    ));
    svg.push_str(&format!(
        "<text x=\"15\" y=\"{}\" text-anchor=\"middle\" font-size=\"12\" transform=\"rotate(-90 15 {})\" >{}</text>\n",
        chart_y + chart_h / 2.0,
        chart_y + chart_h / 2.0,
        escape_svg_text(y_label)
    ));

    // Legend
    if *legend != LegendPosition::None {
        let legend_x = SVG_W - LEGEND_W + 10.0;
        let legend_y = chart_y;
        for (si, series) in data.iter().enumerate() {
            let ly = legend_y + si as f64 * 20.0;
            svg.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"5\" fill=\"{}\"/>\n",
                legend_x + 6.0,
                ly + 6.0,
                series.color
            ));
            svg.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-size=\"11\">{}</text>\n",
                legend_x + 16.0,
                ly + 10.0,
                escape_svg_text(&series.series_name)
            ));
        }
    }

    svg.push_str("</svg>");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use sheets_core::sheet::Sheet;

    fn make_test_sheet() -> Sheet {
        let mut sheet = Sheet::new("Data");
        sheet.set_cell_value(0, 0, "Month".into());
        sheet.set_cell_value(0, 1, "Sales".into());
        sheet.set_cell_value(0, 2, "Costs".into());

        sheet.set_cell_value(1, 0, "Jan".into());
        sheet.set_cell_value(1, 1, "100".into());
        sheet.set_cell_value(1, 2, "60".into());

        sheet.set_cell_value(2, 0, "Feb".into());
        sheet.set_cell_value(2, 1, "150".into());
        sheet.set_cell_value(2, 2, "80".into());

        sheet.set_cell_value(3, 0, "Mar".into());
        sheet.set_cell_value(3, 1, "200".into());
        sheet.set_cell_value(3, 2, "90".into());

        sheet
    }

    #[test]
    fn test_extract_chart_data() {
        let sheet = make_test_sheet();
        let config = ChartConfig {
            title: "Sales vs Costs".into(),
            chart_type: ChartType::Bar,
            series: vec![
                ChartSeries {
                    name: "Sales".into(),
                    x_column: 0,
                    y_column: 1,
                    color: None,
                },
                ChartSeries {
                    name: "Costs".into(),
                    x_column: 0,
                    y_column: 2,
                    color: None,
                },
            ],
            header_row: 0,
            data_start_row: 1,
            data_end_row: 3,
            x_axis_label: Some("Month".into()),
            y_axis_label: Some("Amount".into()),
            legend_position: LegendPosition::Right,
        };

        let data = extract_chart_data(&sheet, &config).unwrap();
        assert_eq!(data.len(), 2);
        assert_eq!(data[0].points.len(), 3);
        assert_eq!(data[0].points[0], ("Jan".to_string(), 100.0));
    }

    #[test]
    fn test_build_bar_chart() {
        let sheet = make_test_sheet();
        let config = ChartConfig {
            title: "Sales".into(),
            chart_type: ChartType::Bar,
            series: vec![ChartSeries {
                name: "Sales".into(),
                x_column: 0,
                y_column: 1,
                color: None,
            }],
            header_row: 0,
            data_start_row: 1,
            data_end_row: 3,
            x_axis_label: None,
            y_axis_label: None,
            legend_position: LegendPosition::Right,
        };

        let result = build_chart(&sheet, &config).unwrap();
        assert!(result.svg.contains("<svg"));
        assert!(result.svg.contains("</svg>"));
    }

    #[test]
    fn test_build_line_chart() {
        let sheet = make_test_sheet();
        let config = ChartConfig {
            title: "Trend".into(),
            chart_type: ChartType::Line,
            series: vec![ChartSeries {
                name: "Sales".into(),
                x_column: 0,
                y_column: 1,
                color: None,
            }],
            header_row: 0,
            data_start_row: 1,
            data_end_row: 3,
            x_axis_label: None,
            y_axis_label: None,
            legend_position: LegendPosition::None,
        };

        let result = build_chart(&sheet, &config).unwrap();
        assert!(result.svg.contains("<path"));
    }

    #[test]
    fn test_build_pie_chart() {
        let sheet = make_test_sheet();
        let config = ChartConfig {
            title: "Sales Distribution".into(),
            chart_type: ChartType::Pie,
            series: vec![ChartSeries {
                name: "Sales".into(),
                x_column: 0,
                y_column: 1,
                color: None,
            }],
            header_row: 0,
            data_start_row: 1,
            data_end_row: 3,
            x_axis_label: None,
            y_axis_label: None,
            legend_position: LegendPosition::Right,
        };

        let result = build_chart(&sheet, &config).unwrap();
        assert!(result.svg.contains("<path"));
    }

    #[test]
    fn test_build_scatter_chart() {
        let sheet = make_test_sheet();
        let config = ChartConfig {
            title: "Scatter".into(),
            chart_type: ChartType::Scatter,
            series: vec![ChartSeries {
                name: "Sales".into(),
                x_column: 0,
                y_column: 1,
                color: None,
            }],
            header_row: 0,
            data_start_row: 1,
            data_end_row: 3,
            x_axis_label: None,
            y_axis_label: None,
            legend_position: LegendPosition::None,
        };

        let result = build_chart(&sheet, &config).unwrap();
        assert!(result.svg.contains("circle"));
    }

    #[test]
    fn test_build_area_chart() {
        let sheet = make_test_sheet();
        let config = ChartConfig {
            title: "Area".into(),
            chart_type: ChartType::Area,
            series: vec![ChartSeries {
                name: "Sales".into(),
                x_column: 0,
                y_column: 1,
                color: None,
            }],
            header_row: 0,
            data_start_row: 1,
            data_end_row: 3,
            x_axis_label: None,
            y_axis_label: None,
            legend_position: LegendPosition::None,
        };

        let result = build_chart(&sheet, &config).unwrap();
        assert!(result.svg.contains("fill-opacity"));
    }

    #[test]
    fn test_build_doughnut_chart() {
        let sheet = make_test_sheet();
        let config = ChartConfig {
            title: "Doughnut".into(),
            chart_type: ChartType::Doughnut,
            series: vec![ChartSeries {
                name: "Sales".into(),
                x_column: 0,
                y_column: 1,
                color: None,
            }],
            header_row: 0,
            data_start_row: 1,
            data_end_row: 3,
            x_axis_label: None,
            y_axis_label: None,
            legend_position: LegendPosition::Right,
        };

        let result = build_chart(&sheet, &config).unwrap();
        assert!(result.svg.contains("<path"));
    }

    #[test]
    fn test_no_series_error() {
        let sheet = make_test_sheet();
        let config = ChartConfig {
            title: "Empty".into(),
            chart_type: ChartType::Bar,
            series: vec![],
            header_row: 0,
            data_start_row: 1,
            data_end_row: 3,
            x_axis_label: None,
            y_axis_label: None,
            legend_position: LegendPosition::None,
        };

        let result = build_chart(&sheet, &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_no_numeric_data_error() {
        let mut sheet = Sheet::new("Data");
        sheet.set_cell_value(0, 0, "Label".into());
        sheet.set_cell_value(0, 1, "Value".into());
        sheet.set_cell_value(1, 0, "A".into());
        sheet.set_cell_value(1, 1, "not_a_number".into());

        let config = ChartConfig {
            title: "Test".into(),
            chart_type: ChartType::Bar,
            series: vec![ChartSeries {
                name: "Value".into(),
                x_column: 0,
                y_column: 1,
                color: None,
            }],
            header_row: 0,
            data_start_row: 1,
            data_end_row: 1,
            x_axis_label: None,
            y_axis_label: None,
            legend_position: LegendPosition::None,
        };

        let result = build_chart(&sheet, &config);
        assert!(result.is_err());
    }
}
