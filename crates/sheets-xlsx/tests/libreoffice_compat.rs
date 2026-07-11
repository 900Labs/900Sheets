use sheets_fixtures::community_budget_compatibility_fixture;
use sheets_xlsx::{XlsxDocument, XlsxSheetFeatures};
use std::process::Command;

#[test]
fn libreoffice_can_open_and_resave_exported_workbook() {
    if Command::new("soffice").arg("--version").output().is_err() {
        assert!(
            std::env::var_os("CI").is_none(),
            "soffice is required for the external XLSX compatibility check in CI"
        );
        eprintln!("soffice is not installed; external XLSX compatibility check skipped");
        return;
    }

    let root = std::env::temp_dir().join(format!("900sheets-lo-{}", std::process::id()));
    let output_dir = root.join("output");
    let profile_dir = root.join("profile");
    std::fs::create_dir_all(&output_dir).unwrap();
    std::fs::create_dir_all(&profile_dir).unwrap();
    let source = root.join("compat.xlsx");

    let fixture = community_budget_compatibility_fixture();
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
    std::fs::write(&source, sheets_xlsx::export_document(&document).unwrap()).unwrap();

    let profile_url = format!("file://{}", profile_dir.display());
    let result = Command::new("soffice")
        .arg(format!("-env:UserInstallation={profile_url}"))
        .args(["--headless", "--convert-to", "xlsx", "--outdir"])
        .arg(&output_dir)
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "LibreOffice failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let converted = std::fs::read(output_dir.join("compat.xlsx")).unwrap();
    let reopened = sheets_xlsx::import_document(&converted).unwrap();
    assert_eq!(
        reopened.workbook.sheet(0).unwrap().cell_value(1, 0),
        Some("Venue".into())
    );
    assert_eq!(
        reopened.workbook.sheet(0).unwrap().cell_value(4, 3),
        Some("=SUM(D2:D4)".into())
    );
    assert_eq!(
        reopened
            .workbook
            .sheet(0)
            .unwrap()
            .get_format(0, 0)
            .unwrap()
            .bold,
        Some(true)
    );
    assert_eq!(reopened.sheet_features[0].validations.len(), 1);
    assert_eq!(reopened.sheet_features[0].conditional_formats.len(), 1);

    let _ = std::fs::remove_dir_all(root);
}
