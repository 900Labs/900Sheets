//! Sanitized, deterministic test data.
//!
//! Every name and value in this crate is invented. Fixtures must not include
//! customer files, personal paths, credentials, or copied production data.

use sheets_core::format::CellFormat;
use sheets_core::workbook::Workbook;
use sheets_validation::{
    ConditionOperator, ConditionalFormat, DataValidation, ValidationOperator, ValidationRule,
};

pub struct CompatibilityFixture {
    pub workbook: Workbook,
    pub validations: Vec<Vec<ValidationRule>>,
    pub conditional_formats: Vec<Vec<ConditionalFormat>>,
}

/// A generated workbook for deterministic OOXML and manual application checks.
///
/// This fixture is Excel-compatible test input. It is not represented as a file
/// produced or approved by Microsoft Excel.
pub fn community_budget_compatibility_fixture() -> CompatibilityFixture {
    let mut workbook = Workbook::new();
    workbook.rename_sheet(0, "Community Budget").unwrap();
    workbook.add_sheet("Assumptions").unwrap();

    let budget = workbook.sheet_mut(0).unwrap();
    for (column, heading) in ["Category", "Planned", "Actual", "Difference"]
        .into_iter()
        .enumerate()
    {
        budget.set_cell_value(0, column as u32, heading.into());
        budget.set_format(
            0,
            column as u32,
            CellFormat::new().bold(true).bg_color("#DDEBF7"),
        );
    }
    for (row, (category, planned, actual)) in [
        ("Venue", "1200", "1180"),
        ("Supplies", "480", "455"),
        ("Transport", "325", "340"),
    ]
    .into_iter()
    .enumerate()
    {
        let row = row as u32 + 1;
        budget.set_cell_value(row, 0, category.into());
        budget.set_cell_value(row, 1, planned.into());
        budget.set_cell_value(row, 2, actual.into());
        budget.set_cell_value(row, 3, format!("=B{}-C{}", row + 1, row + 1));
    }
    budget.set_cell_value(4, 0, "Total".into());
    budget.set_cell_value(4, 1, "=SUM(B2:B4)".into());
    budget.set_cell_value(4, 2, "=SUM(C2:C4)".into());
    budget.set_cell_value(4, 3, "=SUM(D2:D4)".into());

    let assumptions = workbook.sheet_mut(1).unwrap();
    assumptions.set_cell_value(0, 0, "Setting".into());
    assumptions.set_cell_value(0, 1, "Value".into());
    assumptions.set_cell_value(1, 0, "Contingency rate".into());
    assumptions.set_cell_value(1, 1, "0.08".into());
    assumptions.set_cell_value(2, 0, "Projected contingency".into());
    assumptions.set_cell_value(2, 1, "='Community Budget'!$B$5*B2".into());
    assumptions.set_format(1, 1, CellFormat::new().number_format("0%"));

    let validation = DataValidation {
        error_title: Some("Choose a listed category".into()),
        error_message: Some("Use Venue, Supplies, or Transport.".into()),
        ..DataValidation::list("Venue,Supplies,Transport")
    };
    let validations = vec![
        vec![ValidationRule {
            range: (1, 0, 3, 0),
            validation,
        }],
        vec![ValidationRule {
            range: (1, 1, 1, 1),
            validation: DataValidation::decimal(ValidationOperator::Between, "0", "0.25"),
        }],
    ];
    let conditional_formats = vec![
        vec![ConditionalFormat::cell_value(
            ConditionOperator::LessThan,
            "0",
            CellFormat::new().font_color("#9C0006").bg_color("#FFC7CE"),
            (1, 3, 3, 3),
        )],
        Vec::new(),
    ];

    CompatibilityFixture {
        workbook,
        validations,
        conditional_formats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_fixture_is_invented_and_deterministic() {
        let fixture = community_budget_compatibility_fixture();
        assert_eq!(fixture.workbook.sheet_count(), 2);
        assert_eq!(
            fixture.workbook.sheet(0).unwrap().cell_value(1, 0),
            Some("Venue".into())
        );
        assert_eq!(fixture.validations.iter().map(Vec::len).sum::<usize>(), 2);
        assert_eq!(
            fixture
                .conditional_formats
                .iter()
                .map(Vec::len)
                .sum::<usize>(),
            1
        );
    }
}
