use sheets_core::format::CellFormat;
use sheets_core::workbook::Workbook;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: generate_compat <output.xlsx>")?;
    let mut workbook = Workbook::new();
    let sheet = workbook.sheet_mut(0).ok_or("missing first sheet")?;
    sheet.set_cell_value(0, 0, "Value".into());
    sheet.set_cell_value(0, 1, "Double".into());
    sheet.set_cell_value(1, 0, "21".into());
    sheet.set_cell_value(1, 1, "=A2*2".into());
    sheet.set_format(0, 0, CellFormat::new().bold(true).bg_color("#DDEEFF"));
    std::fs::write(path, sheets_xlsx::export_workbook(&workbook)?)?;
    Ok(())
}
