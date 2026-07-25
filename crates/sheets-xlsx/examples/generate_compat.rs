use sheets_fixtures::community_budget_compatibility_fixture;
use sheets_xlsx::{XlsxDocument, XlsxSheetFeatures};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: generate_compat <output.xlsx>")?;
    let fixture = community_budget_compatibility_fixture();
    let sheet_features = fixture
        .validations
        .into_iter()
        .zip(fixture.conditional_formats)
        .map(|(validations, conditional_formats)| XlsxSheetFeatures {
            validations,
            conditional_formats,
            tables: Vec::new(),
        })
        .collect();
    let document = XlsxDocument {
        workbook: fixture.workbook,
        sheet_features,
    };
    std::fs::write(path, sheets_xlsx::export_document(&document)?)?;
    Ok(())
}
