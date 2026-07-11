use sheets_core::format::CellFormat;
use sheets_core::workbook::Workbook;
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

    let mut workbook = Workbook::new();
    let sheet = workbook.sheet_mut(0).unwrap();
    sheet.set_cell_value(0, 0, "Value".into());
    sheet.set_cell_value(0, 1, "Double".into());
    sheet.set_cell_value(1, 0, "21".into());
    sheet.set_cell_value(1, 1, "=A2*2".into());
    sheet.set_format(0, 0, CellFormat::new().bold(true).bg_color("#DDEEFF"));
    std::fs::write(&source, sheets_xlsx::export_workbook(&workbook).unwrap()).unwrap();

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
    let reopened = sheets_xlsx::import_workbook(&converted).unwrap();
    assert_eq!(
        reopened.sheet(0).unwrap().cell_value(1, 0),
        Some("21".into())
    );
    assert_eq!(
        reopened.sheet(0).unwrap().cell_value(1, 1),
        Some("=A2*2".into())
    );
    assert_eq!(
        reopened.sheet(0).unwrap().get_format(0, 0).unwrap().bold,
        Some(true)
    );

    let _ = std::fs::remove_dir_all(root);
}
