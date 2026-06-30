use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CellFormat {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strikethrough: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h_align: Option<HorizontalAlignment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub v_align: Option<VerticalAlignment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap_text: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_top: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_bottom: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_left: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_right: Option<Border>,
}

impl CellFormat {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bold(mut self, bold: bool) -> Self {
        self.bold = Some(bold);
        self
    }

    pub fn italic(mut self, italic: bool) -> Self {
        self.italic = Some(italic);
        self
    }

    pub fn font_size(mut self, size: f64) -> Self {
        self.font_size = Some(size);
        self
    }

    pub fn font_color(mut self, color: &str) -> Self {
        self.font_color = Some(color.into());
        self
    }

    pub fn bg_color(mut self, color: &str) -> Self {
        self.bg_color = Some(color.into());
        self
    }

    pub fn h_align(mut self, align: HorizontalAlignment) -> Self {
        self.h_align = Some(align);
        self
    }

    pub fn number_format(mut self, format: &str) -> Self {
        self.number_format = Some(format.into());
        self
    }

    pub fn merge(&self, other: &CellFormat) -> CellFormat {
        CellFormat {
            bold: other.bold.or(self.bold),
            italic: other.italic.or(self.italic),
            underline: other.underline.or(self.underline),
            strikethrough: other.strikethrough.or(self.strikethrough),
            font_size: other.font_size.or(self.font_size),
            font_name: other.font_name.clone().or(self.font_name.clone()),
            font_color: other.font_color.clone().or(self.font_color.clone()),
            bg_color: other.bg_color.clone().or(self.bg_color.clone()),
            h_align: other.h_align.or(self.h_align),
            v_align: other.v_align.or(self.v_align),
            wrap_text: other.wrap_text.or(self.wrap_text),
            number_format: other.number_format.clone().or(self.number_format.clone()),
            border_top: other.border_top.clone().or(self.border_top.clone()),
            border_bottom: other.border_bottom.clone().or(self.border_bottom.clone()),
            border_left: other.border_left.clone().or(self.border_left.clone()),
            border_right: other.border_right.clone().or(self.border_right.clone()),
        }
    }

    pub fn is_empty(&self) -> bool {
        self == &CellFormat::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HorizontalAlignment {
    Left,
    Center,
    Right,
    General,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerticalAlignment {
    Top,
    Middle,
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Border {
    pub style: BorderStyle,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BorderStyle {
    #[default]
    None,
    Thin,
    Medium,
    Thick,
    Dotted,
    Dashed,
    Double,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_format() {
        let fmt = CellFormat::default();
        assert!(fmt.is_empty());
    }

    #[test]
    fn test_builder() {
        let fmt = CellFormat::new()
            .bold(true)
            .italic(true)
            .font_size(14.0)
            .font_color("#FF0000")
            .bg_color("#F0F0F0")
            .h_align(HorizontalAlignment::Center)
            .number_format("$#,##0.00");
        assert_eq!(fmt.bold, Some(true));
        assert_eq!(fmt.italic, Some(true));
        assert_eq!(fmt.font_size, Some(14.0));
        assert_eq!(fmt.font_color, Some("#FF0000".into()));
        assert_eq!(fmt.bg_color, Some("#F0F0F0".into()));
        assert_eq!(fmt.h_align, Some(HorizontalAlignment::Center));
        assert_eq!(fmt.number_format, Some("$#,##0.00".into()));
        assert!(!fmt.is_empty());
    }

    #[test]
    fn test_merge() {
        let base = CellFormat::new().bold(true).font_size(12.0);
        let override_fmt = CellFormat::new().italic(true);
        let merged = base.merge(&override_fmt);
        assert_eq!(merged.bold, Some(true));
        assert_eq!(merged.italic, Some(true));
        assert_eq!(merged.font_size, Some(12.0));
    }

    #[test]
    fn test_merge_override() {
        let base = CellFormat::new().bold(true).font_size(12.0);
        let override_fmt = CellFormat::new().bold(false).font_size(14.0);
        let merged = base.merge(&override_fmt);
        assert_eq!(merged.bold, Some(false));
        assert_eq!(merged.font_size, Some(14.0));
    }

    #[test]
    fn test_serde_roundtrip() {
        let fmt = CellFormat::new().bold(true).font_color("#FF0000");
        let json = serde_json::to_string(&fmt).unwrap();
        let deserialized: CellFormat = serde_json::from_str(&json).unwrap();
        assert_eq!(fmt, deserialized);
    }
}
