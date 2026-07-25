use sheets_core::workbook::Workbook;
use sheets_tables::Table;
use sheets_validation::{ConditionalFormat, ValidationRule};

/// Workbook content plus the worksheet feature records that 900Sheets can map
/// to and from OOXML.
#[derive(Clone)]
pub struct XlsxDocument {
    pub workbook: Workbook,
    pub sheet_features: Vec<XlsxSheetFeatures>,
}

impl XlsxDocument {
    pub fn new(workbook: Workbook) -> Self {
        let sheet_features = vec![XlsxSheetFeatures::default(); workbook.sheet_count()];
        Self {
            workbook,
            sheet_features,
        }
    }

    pub fn features(&self, sheet_index: usize) -> Option<&XlsxSheetFeatures> {
        self.sheet_features.get(sheet_index)
    }
}

#[derive(Clone, Default, PartialEq)]
pub struct XlsxSheetFeatures {
    pub validations: Vec<ValidationRule>,
    pub conditional_formats: Vec<ConditionalFormat>,
    pub tables: Vec<Table>,
}
