use sheets_advanced::{
    CellComment, CellLockManager, GoalSeekConfig, GoalSeekResult, ProtectionAction, Scenario,
    SheetProtection,
};
use sheets_chart::{ChartConfig, ChartResult};
use sheets_core::format::CellFormat;
use sheets_core::number_format::NumberFormat;
use sheets_core::workbook::Workbook;
use sheets_formula::dependency::DependencyGraph;
use sheets_formula::evaluator::{CellProvider, Evaluator, Value};
use sheets_formula::parser::Parser;
use sheets_i18n::{Locale, NavigationDirection, TranslationKey, TranslationProvider};
use sheets_pivot::{PivotConfig, PivotResult};
use sheets_print::{PrintConfig, PrintPreview};
use sheets_validation::{ConditionalFormat, DataValidation, ValidationRule};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use tauri::State;

struct AppState {
    workbook: Mutex<Workbook>,
    dep_graphs: Mutex<Vec<DependencyGraph>>,
    protections: Mutex<Vec<SheetProtection>>,
    cell_locks: Mutex<Vec<CellLockManager>>,
}

#[derive(Default)]
struct SheetComments {
    comments: HashMap<(u32, u32, u32), CellComment>,
}

impl SheetComments {
    fn clear(&mut self) {
        self.comments.clear();
    }
}

struct TauriProvider<'a> {
    sheet: &'a sheets_core::sheet::Sheet,
    visited: &'a HashSet<(u32, u32)>,
}

impl<'a> CellProvider for TauriProvider<'a> {
    fn get_cell(&self, row: u32, col: u32) -> Value {
        if let Some(cell) = self.sheet.cell(row, col) {
            match cell.cell_type {
                sheets_core::cell::CellType::Number => {
                    Value::Number(cell.as_number().unwrap_or(0.0))
                }
                sheets_core::cell::CellType::Text => Value::String(cell.raw.clone()),
                sheets_core::cell::CellType::Boolean => {
                    Value::Boolean(cell.raw.eq_ignore_ascii_case("true"))
                }
                sheets_core::cell::CellType::Formula => {
                    let key = (row, col);
                    if self.visited.contains(&key) {
                        return Value::Error(
                            sheets_formula::error::FormulaError::CircularReference,
                        );
                    }
                    let evaluator = Evaluator::new();
                    if let Ok(expr) = Parser::parse_formula(&cell.raw) {
                        let mut new_visited = self.visited.clone();
                        new_visited.insert(key);
                        let provider = TauriProvider {
                            sheet: self.sheet,
                            visited: &new_visited,
                        };
                        evaluator.evaluate(&expr, &provider)
                    } else {
                        Value::Error(sheets_formula::error::FormulaError::ParseError(
                            "Invalid formula".into(),
                        ))
                    }
                }
                sheets_core::cell::CellType::Error => Value::Error(
                    sheets_formula::error::FormulaError::EvalError(cell.raw.clone()),
                ),
                sheets_core::cell::CellType::Empty => Value::Empty,
            }
        } else {
            Value::Empty
        }
    }

    fn get_raw(&self, row: u32, col: u32) -> String {
        self.sheet.cell_value(row, col).unwrap_or_default()
    }
}

#[derive(serde::Serialize, Clone)]
struct SheetInfo {
    id: u32,
    stable_id: u64,
    name: String,
}

#[derive(serde::Serialize)]
struct NativeOpenResult {
    sheets: Vec<SheetInfo>,
    metadata: serde_json::Value,
}

#[derive(serde::Serialize)]
struct CellData {
    row: u32,
    col: u32,
    value: String,
    display: String,
    cell_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<CellFormat>,
}

fn sheet_infos(workbook: &Workbook) -> Vec<SheetInfo> {
    workbook
        .sheets()
        .iter()
        .enumerate()
        .map(|(i, s)| SheetInfo {
            id: i as u32,
            stable_id: s.stable_id(),
            name: s.name().to_string(),
        })
        .collect()
}

fn clear_derived_workbook_state(state: &AppState) -> Result<(), String> {
    {
        let mut graphs = state.dep_graphs.lock().map_err(|e| e.to_string())?;
        graphs.clear();
    }
    {
        let mut protections = state.protections.lock().map_err(|e| e.to_string())?;
        protections.clear();
    }
    {
        let mut locks = state.cell_locks.lock().map_err(|e| e.to_string())?;
        locks.clear();
    }
    Ok(())
}

fn clear_comments(comments: &Mutex<SheetComments>) -> Result<(), String> {
    let mut comments = comments.lock().map_err(|e| e.to_string())?;
    comments.clear();
    Ok(())
}

fn delete_sheet_comments(comments: &mut SheetComments, deleted_sheet_id: u32) {
    comments.comments = comments
        .comments
        .drain()
        .filter_map(|((sheet_id, row, col), comment)| {
            if sheet_id == deleted_sheet_id {
                None
            } else {
                let next_sheet_id = if sheet_id > deleted_sheet_id {
                    sheet_id - 1
                } else {
                    sheet_id
                };
                Some(((next_sheet_id, row, col), comment))
            }
        })
        .collect();
}

fn clear_sheet_coordinate_state(
    state: &AppState,
    comments: &Mutex<SheetComments>,
    sheet_id: u32,
) -> Result<(), String> {
    if let Some(manager) = state
        .cell_locks
        .lock()
        .map_err(|error| error.to_string())?
        .get_mut(sheet_id as usize)
    {
        manager.clear();
    }
    comments
        .lock()
        .map_err(|error| error.to_string())?
        .comments
        .retain(|(candidate, _, _), _| *candidate != sheet_id);
    Ok(())
}

fn clear_full_workbook_state(
    state: &AppState,
    comments: &Mutex<SheetComments>,
) -> Result<(), String> {
    clear_derived_workbook_state(state)?;
    clear_comments(comments)?;
    Ok(())
}

fn checked_absolute_path(file_path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(file_path);
    if !path.is_absolute() {
        return Err("File path must be absolute".into());
    }
    if path.components().any(|c| c == Component::ParentDir) {
        return Err("Invalid file path".into());
    }
    Ok(path)
}

fn ensure_allowed_extension(path: &Path, allowed_extensions: &[&str]) -> Result<(), String> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if allowed_extensions
        .iter()
        .any(|allowed| extension == *allowed)
    {
        return Ok(());
    }
    Err(format!(
        "File extension must be one of: {}",
        allowed_extensions.join(", ")
    ))
}

fn write_atomic(path: &Path, data: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("file")
    ));
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&temporary).map_err(|error| error.to_string())?;
        file.write_all(data).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
    }
    std::fs::rename(&temporary, path).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        error.to_string()
    })
}

fn ensure_can_edit_cell(state: &AppState, sheet_id: u32, row: u32, col: u32) -> Result<(), String> {
    let protections = state.protections.lock().map_err(|e| e.to_string())?;
    if let Some(protection) = protections.get(sheet_id as usize) {
        if protection.protected {
            let locks = state.cell_locks.lock().map_err(|e| e.to_string())?;
            let cell_locked = locks
                .get(sheet_id as usize)
                .map(|m| m.is_locked(row, col))
                .unwrap_or(true);
            if !protection.can_edit_cell(cell_locked) {
                return Err(format!(
                    "Cell ({}, {}) on sheet {} is locked",
                    row, col, sheet_id
                ));
            }
        }
    }
    Ok(())
}

fn ensure_sheet_not_protected(state: &AppState, sheet_id: u32) -> Result<(), String> {
    let protections = state.protections.lock().map_err(|e| e.to_string())?;
    if let Some(protection) = protections.get(sheet_id as usize) {
        if protection.protected {
            return Err(format!("Sheet {} is protected", sheet_id));
        }
    }
    Ok(())
}

fn ensure_can_perform_protected_action(
    state: &AppState,
    sheet_id: u32,
    action: ProtectionAction,
) -> Result<(), String> {
    let protections = state.protections.lock().map_err(|e| e.to_string())?;
    if let Some(protection) = protections.get(sheet_id as usize) {
        if !protection.can_perform(action) {
            return Err(format!("Sheet {} is protected for {:?}", sheet_id, action));
        }
    }
    Ok(())
}

fn ensure_dependency_graphs(graphs: &mut Vec<DependencyGraph>, sheet_count: usize) {
    while graphs.len() < sheet_count {
        graphs.push(DependencyGraph::new());
    }
}

fn update_cell_dependencies(
    state: &AppState,
    sheet_id: u32,
    sheet_count: usize,
    row: u32,
    col: u32,
    value: &str,
) -> Result<(), String> {
    let mut graphs = state.dep_graphs.lock().map_err(|e| e.to_string())?;
    if value.starts_with('=') {
        ensure_dependency_graphs(&mut graphs, sheet_count);
        let graph = graphs
            .get_mut(sheet_id as usize)
            .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
        if Parser::parse_formula(value).is_err() {
            graph.clear_cell(row, col);
            return Ok(());
        }
        graph.set_formula(row, col, value)?;
    } else if let Some(graph) = graphs.get_mut(sheet_id as usize) {
        graph.clear_cell(row, col);
    }
    Ok(())
}

fn set_cell_value_in_workbook(
    state: &AppState,
    sheet_id: u32,
    row: u32,
    col: u32,
    value: String,
) -> Result<(), String> {
    let mut wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet_count = wb.sheet_count();
    if sheet_id as usize >= sheet_count {
        return Err(format!("Sheet {} not found", sheet_id));
    }
    update_cell_dependencies(state, sheet_id, sheet_count, row, col, &value)?;
    let sheet = wb
        .sheet_mut(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    sheet.set_cell_value(row, col, value);
    Ok(())
}

fn clear_cell_in_workbook(
    state: &AppState,
    sheet_id: u32,
    row: u32,
    col: u32,
) -> Result<(), String> {
    let mut wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet_mut(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    sheet.clear_value(row, col);
    let mut graphs = state.dep_graphs.lock().map_err(|e| e.to_string())?;
    if let Some(graph) = graphs.get_mut(sheet_id as usize) {
        graph.clear_cell(row, col);
    }
    Ok(())
}

#[derive(serde::Deserialize)]
struct CellChange {
    row: u32,
    col: u32,
    value: String,
}

#[derive(serde::Deserialize)]
struct FormatChange {
    row: u32,
    col: u32,
    format: CellFormat,
}

fn dependency_graph_for_sheet(
    sheet: &sheets_core::sheet::Sheet,
) -> Result<DependencyGraph, String> {
    let mut graph = DependencyGraph::new();
    for ((row, col), cell) in sheet.iter_cells() {
        if cell.is_formula() {
            Parser::parse_formula(&cell.raw).map_err(|error| error.to_string())?;
            graph.set_formula(row, col, &cell.raw)?;
        }
    }
    Ok(graph)
}

fn dependency_graphs_for_workbook(workbook: &Workbook) -> Result<Vec<DependencyGraph>, String> {
    workbook
        .sheets()
        .iter()
        .map(dependency_graph_for_sheet)
        .collect()
}

fn replace_loaded_workbook(
    state: &AppState,
    comments: &Mutex<SheetComments>,
    imported: Workbook,
) -> Result<Vec<SheetInfo>, String> {
    let graphs = dependency_graphs_for_workbook(&imported)?;
    let infos = sheet_infos(&imported);
    *state.workbook.lock().map_err(|error| error.to_string())? = imported;
    *state.dep_graphs.lock().map_err(|error| error.to_string())? = graphs;
    state
        .protections
        .lock()
        .map_err(|error| error.to_string())?
        .clear();
    state
        .cell_locks
        .lock()
        .map_err(|error| error.to_string())?
        .clear();
    comments.lock().map_err(|error| error.to_string())?.clear();
    Ok(infos)
}

#[tauri::command]
fn batch_set_cells(
    sheet_id: u32,
    changes: Vec<CellChange>,
    state: State<AppState>,
) -> Result<(), String> {
    for change in &changes {
        ensure_can_edit_cell(state.inner(), sheet_id, change.row, change.col)?;
    }
    let mut workbook = state.workbook.lock().map_err(|error| error.to_string())?;
    let mut next = workbook
        .sheet(sheet_id as usize)
        .cloned()
        .ok_or_else(|| format!("Sheet {sheet_id} not found"))?;
    for change in changes {
        if change.value.is_empty() {
            next.clear_value(change.row, change.col);
        } else {
            next.set_cell_value(change.row, change.col, change.value);
        }
    }
    let graph = dependency_graph_for_sheet(&next)?;
    *workbook
        .sheet_mut(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {sheet_id} not found"))? = next;
    let sheet_count = workbook.sheet_count();
    drop(workbook);
    let mut graphs = state.dep_graphs.lock().map_err(|error| error.to_string())?;
    ensure_dependency_graphs(&mut graphs, sheet_count);
    graphs[sheet_id as usize] = graph;
    Ok(())
}

#[tauri::command]
fn batch_set_formats(
    sheet_id: u32,
    changes: Vec<FormatChange>,
    state: State<AppState>,
) -> Result<(), String> {
    ensure_can_perform_protected_action(state.inner(), sheet_id, ProtectionAction::FormatCells)?;
    let mut workbook = state.workbook.lock().map_err(|error| error.to_string())?;
    let mut next = workbook
        .sheet(sheet_id as usize)
        .cloned()
        .ok_or_else(|| format!("Sheet {sheet_id} not found"))?;
    for change in changes {
        next.set_format(change.row, change.col, change.format);
    }
    *workbook
        .sheet_mut(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {sheet_id} not found"))? = next;
    Ok(())
}

#[tauri::command]
fn replace_sheet_snapshot(
    sheet_id: u32,
    cells: Vec<CellChange>,
    formats: Vec<FormatChange>,
    state: State<AppState>,
) -> Result<(), String> {
    ensure_sheet_not_protected(state.inner(), sheet_id)?;
    let mut next = {
        let workbook = state.workbook.lock().map_err(|error| error.to_string())?;
        let current = workbook
            .sheet(sheet_id as usize)
            .ok_or_else(|| format!("Sheet {sheet_id} not found"))?;
        sheets_core::sheet::Sheet::with_stable_id(current.stable_id(), current.name())
    };
    for cell in cells {
        next.set_cell_value(cell.row, cell.col, cell.value);
    }
    for format in formats {
        next.set_format(format.row, format.col, format.format);
    }
    let graph = dependency_graph_for_sheet(&next)?;
    let mut workbook = state.workbook.lock().map_err(|error| error.to_string())?;
    *workbook
        .sheet_mut(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {sheet_id} not found"))? = next;
    let sheet_count = workbook.sheet_count();
    drop(workbook);
    let mut graphs = state.dep_graphs.lock().map_err(|error| error.to_string())?;
    ensure_dependency_graphs(&mut graphs, sheet_count);
    graphs[sheet_id as usize] = graph;
    Ok(())
}

#[tauri::command]
fn edit_sheet_structure(
    sheet_id: u32,
    operation: String,
    index: u32,
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<(), String> {
    use sheets_core::sheet::StructureEdit;
    let edit = match operation.as_str() {
        "insert_row" => StructureEdit::InsertRow(index),
        "delete_row" => StructureEdit::DeleteRow(index),
        "insert_column" => StructureEdit::InsertColumn(index),
        "delete_column" => StructureEdit::DeleteColumn(index),
        _ => return Err(format!("Unknown structure operation: {operation}")),
    };
    let action = match edit {
        StructureEdit::InsertRow(_) => ProtectionAction::InsertRows,
        StructureEdit::DeleteRow(_) => ProtectionAction::DeleteRows,
        StructureEdit::InsertColumn(_) => ProtectionAction::InsertCols,
        StructureEdit::DeleteColumn(_) => ProtectionAction::DeleteCols,
    };
    ensure_can_perform_protected_action(state.inner(), sheet_id, action)?;
    let mut workbook = state.workbook.lock().map_err(|error| error.to_string())?;
    let mut next = workbook
        .sheet(sheet_id as usize)
        .cloned()
        .ok_or_else(|| format!("Sheet {sheet_id} not found"))?;
    next.apply_structure_edit(edit);
    let graph = dependency_graph_for_sheet(&next)?;
    *workbook
        .sheet_mut(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {sheet_id} not found"))? = next;
    let sheet_count = workbook.sheet_count();
    drop(workbook);
    let mut graphs = state.dep_graphs.lock().map_err(|error| error.to_string())?;
    ensure_dependency_graphs(&mut graphs, sheet_count);
    graphs[sheet_id as usize] = graph;
    drop(graphs);
    clear_sheet_coordinate_state(state.inner(), comments.inner(), sheet_id)?;
    Ok(())
}

#[derive(serde::Serialize)]
struct SearchResultData {
    row: u32,
    col: u32,
    matched_text: String,
}

#[tauri::command]
fn new_workbook(
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<Vec<SheetInfo>, String> {
    let mut wb = state.workbook.lock().map_err(|e| e.to_string())?;
    *wb = Workbook::new();
    clear_full_workbook_state(state.inner(), comments.inner())?;

    Ok(sheet_infos(&wb))
}

#[tauri::command]
fn get_sheets(state: State<AppState>) -> Result<Vec<SheetInfo>, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    Ok(sheet_infos(&wb))
}

#[tauri::command]
fn set_active_sheet(sheet_id: u32, state: State<AppState>) -> Result<(), String> {
    let mut workbook = state.workbook.lock().map_err(|error| error.to_string())?;
    if sheet_id as usize >= workbook.sheet_count() {
        return Err(format!("Sheet {sheet_id} not found"));
    }
    workbook.set_active_sheet(sheet_id as usize);
    Ok(())
}

#[tauri::command]
fn add_sheet(name: String, state: State<AppState>) -> Result<Vec<SheetInfo>, String> {
    let mut wb = state.workbook.lock().map_err(|e| e.to_string())?;
    wb.add_sheet(&name);
    Ok(sheet_infos(&wb))
}

#[tauri::command]
fn delete_sheet(
    sheet_id: u32,
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<Vec<SheetInfo>, String> {
    ensure_sheet_not_protected(state.inner(), sheet_id)?;
    let mut wb = state.workbook.lock().map_err(|e| e.to_string())?;
    if sheet_id as usize >= wb.sheet_count() {
        return Err(format!("Sheet {} not found", sheet_id));
    }
    if wb.sheet_count() <= 1 {
        return Err("Cannot delete the last sheet".into());
    }
    wb.delete_sheet(sheet_id as usize);
    let mut graphs = state.dep_graphs.lock().map_err(|e| e.to_string())?;
    if (sheet_id as usize) < graphs.len() {
        graphs.remove(sheet_id as usize);
    }
    let mut protections = state.protections.lock().map_err(|e| e.to_string())?;
    if (sheet_id as usize) < protections.len() {
        protections.remove(sheet_id as usize);
    }
    let mut locks = state.cell_locks.lock().map_err(|e| e.to_string())?;
    if (sheet_id as usize) < locks.len() {
        locks.remove(sheet_id as usize);
    }
    drop(locks);
    let mut comment_guard = comments.lock().map_err(|error| error.to_string())?;
    delete_sheet_comments(&mut comment_guard, sheet_id);
    Ok(sheet_infos(&wb))
}

#[tauri::command]
fn rename_sheet(
    sheet_id: u32,
    name: String,
    state: State<AppState>,
) -> Result<Vec<SheetInfo>, String> {
    let mut wb = state.workbook.lock().map_err(|e| e.to_string())?;
    if sheet_id as usize >= wb.sheet_count() {
        return Err(format!("Sheet {} not found", sheet_id));
    }
    if name.trim().is_empty() {
        return Err("Sheet name cannot be empty".into());
    }
    wb.rename_sheet(sheet_id as usize, &name);
    Ok(sheet_infos(&wb))
}

#[tauri::command]
fn get_cell(sheet_id: u32, row: u32, col: u32, state: State<AppState>) -> Result<String, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    Ok(sheet.cell_value(row, col).unwrap_or_default())
}

#[tauri::command]
fn set_cell(
    sheet_id: u32,
    row: u32,
    col: u32,
    value: String,
    state: State<AppState>,
) -> Result<(), String> {
    ensure_can_edit_cell(state.inner(), sheet_id, row, col)?;
    set_cell_value_in_workbook(state.inner(), sheet_id, row, col, value)
}

#[tauri::command]
fn clear_cell(sheet_id: u32, row: u32, col: u32, state: State<AppState>) -> Result<(), String> {
    ensure_can_edit_cell(state.inner(), sheet_id, row, col)?;
    clear_cell_in_workbook(state.inner(), sheet_id, row, col)
}

#[tauri::command]
fn get_sheet_data(sheet_id: u32, state: State<AppState>) -> Result<Vec<CellData>, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    let evaluator = Evaluator::new();
    Ok(sheet
        .iter_cells()
        .map(|((row, col), cell)| {
            let display = if cell.cell_type == sheets_core::cell::CellType::Formula {
                let visited = HashSet::new();
                let provider = TauriProvider {
                    sheet,
                    visited: &visited,
                };
                let expr_result = Parser::parse_formula(&cell.raw);
                match expr_result {
                    Ok(expr) => {
                        let mut v = visited.clone();
                        match evaluator.evaluate_with_visited(&expr, &provider, &mut v) {
                            Value::Number(n) => {
                                if let Some(fmt) = sheet.get_format(row, col) {
                                    if let Some(nf_str) = &fmt.number_format {
                                        NumberFormat::from_pattern(nf_str).format(n)
                                    } else {
                                        n.to_string()
                                    }
                                } else {
                                    n.to_string()
                                }
                            }
                            v => v.to_display(),
                        }
                    }
                    Err(_) => "#ERROR!".to_string(),
                }
            } else if cell.cell_type == sheets_core::cell::CellType::Number {
                if let Some(fmt) = sheet.get_format(row, col) {
                    if let Some(nf_str) = &fmt.number_format {
                        NumberFormat::from_pattern(nf_str).format(cell.as_number().unwrap_or(0.0))
                    } else {
                        cell.display.clone()
                    }
                } else {
                    cell.display.clone()
                }
            } else {
                cell.display.clone()
            };
            CellData {
                row,
                col,
                value: cell.raw.clone(),
                display,
                cell_type: format!("{:?}", cell.cell_type).to_lowercase(),
                format: sheet.get_format(row, col).cloned(),
            }
        })
        .collect())
}

#[tauri::command]
fn evaluate_formula(
    sheet_id: u32,
    formula: String,
    state: State<AppState>,
) -> Result<String, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    let expr = Parser::parse_formula(&formula).map_err(|e| format!("Parse error: {}", e))?;
    let evaluator = Evaluator::new();
    let visited = HashSet::new();
    let provider = TauriProvider {
        sheet,
        visited: &visited,
    };
    let result = evaluator.evaluate(&expr, &provider);
    Ok(match result {
        Value::Number(n) => format!("{}", n),
        Value::String(s) => s,
        Value::Boolean(b) => {
            if b {
                "TRUE".into()
            } else {
                "FALSE".into()
            }
        }
        Value::Error(e) => format!("{}", e),
        Value::Empty => String::new(),
        Value::Array(arr) => {
            if let Some(first) = arr.first() {
                first.to_display()
            } else {
                String::new()
            }
        }
    })
}

#[tauri::command]
fn import_xlsx(
    data: Vec<u8>,
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<Vec<SheetInfo>, String> {
    let imported = sheets_xlsx::import_workbook(&data).map_err(|e| e.to_string())?;
    replace_loaded_workbook(state.inner(), comments.inner(), imported)
}

#[tauri::command]
fn export_xlsx(state: State<AppState>) -> Result<Vec<u8>, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    sheets_xlsx::export_workbook(&wb).map_err(|e| e.to_string())
}

#[tauri::command]
fn import_xlsx_file(
    file_path: String,
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<Vec<SheetInfo>, String> {
    let path = checked_absolute_path(&file_path)?;
    ensure_allowed_extension(&path, &["xlsx"])?;
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    import_xlsx(data, state, comments)
}

#[tauri::command]
fn export_xlsx_file(file_path: String, state: State<AppState>) -> Result<(), String> {
    let path = checked_absolute_path(&file_path)?;
    ensure_allowed_extension(&path, &["xlsx"])?;
    let data = export_xlsx(state)?;
    std::fs::write(path, data).map_err(|e| e.to_string())
}

#[tauri::command]
fn import_csv_data(
    data: String,
    delimiter: String,
    sheet_id: Option<u32>,
    state: State<AppState>,
) -> Result<Vec<SheetInfo>, String> {
    let delim = delimiter.chars().next().unwrap_or(',');
    if let Some(id) = sheet_id {
        ensure_sheet_not_protected(state.inner(), id)?;
    }
    let sheet = sheets_csv::import_csv(&data, delim).map_err(|e| e.to_string())?;
    if let Some(id) = sheet_id {
        let mut workbook = state.workbook.lock().map_err(|e| e.to_string())?;
        let mut next = workbook
            .sheet(id as usize)
            .cloned()
            .ok_or_else(|| format!("Sheet {} not found", id))?;
        for ((row, col), cell) in sheet.iter_cells() {
            next.set_cell(row, col, cell.clone());
        }
        let graph = dependency_graph_for_sheet(&next)?;
        *workbook
            .sheet_mut(id as usize)
            .ok_or_else(|| format!("Sheet {} not found", id))? = next;
        let count = workbook.sheet_count();
        let infos = sheet_infos(&workbook);
        drop(workbook);
        let mut graphs = state.dep_graphs.lock().map_err(|e| e.to_string())?;
        ensure_dependency_graphs(&mut graphs, count);
        graphs[id as usize] = graph;
        Ok(infos)
    } else {
        let mut wb = state.workbook.lock().map_err(|e| e.to_string())?;
        let sheet_name = format!("Sheet{}", wb.sheet_count() + 1);
        let idx = wb.add_sheet(&sheet_name);
        for ((row, col), cell) in sheet.iter_cells() {
            if let Some(s) = wb.sheet_mut(idx) {
                s.set_cell(row, col, cell.clone());
            }
        }
        let graph = dependency_graph_for_sheet(
            wb.sheet(idx)
                .ok_or_else(|| format!("Sheet {} not found", idx))?,
        )?;
        let count = wb.sheet_count();
        let infos = sheet_infos(&wb);
        drop(wb);
        let mut graphs = state.dep_graphs.lock().map_err(|e| e.to_string())?;
        ensure_dependency_graphs(&mut graphs, count);
        graphs[idx] = graph;
        Ok(infos)
    }
}

#[tauri::command]
fn export_csv(sheet_id: u32, delimiter: String, state: State<AppState>) -> Result<String, String> {
    let delim = delimiter.chars().next().unwrap_or(',');
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    sheets_csv::export_sheet_csv(sheet, delim).map_err(|e| e.to_string())
}

#[tauri::command]
fn import_csv_file(
    file_path: String,
    delimiter: String,
    sheet_id: Option<u32>,
    state: State<AppState>,
) -> Result<Vec<SheetInfo>, String> {
    let path = checked_absolute_path(&file_path)?;
    ensure_allowed_extension(&path, &["csv", "tsv", "txt"])?;
    let data = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    import_csv_data(data, delimiter, sheet_id, state)
}

#[tauri::command]
fn export_csv_file(
    sheet_id: u32,
    delimiter: String,
    file_path: String,
    state: State<AppState>,
) -> Result<(), String> {
    let path = checked_absolute_path(&file_path)?;
    ensure_allowed_extension(&path, &["csv", "tsv", "txt"])?;
    let data = export_csv(sheet_id, delimiter, state)?;
    std::fs::write(path, data).map_err(|e| e.to_string())
}

#[tauri::command]
fn import_json_data(
    data: String,
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<Vec<SheetInfo>, String> {
    let imported = sheets_json::import_workbook_json(&data).map_err(|e| e.to_string())?;
    replace_loaded_workbook(state.inner(), comments.inner(), imported)
}

#[tauri::command]
fn export_json(state: State<AppState>) -> Result<String, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    sheets_json::export_workbook_json(&wb).map_err(|e| e.to_string())
}

#[tauri::command]
fn import_json_file(
    file_path: String,
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<Vec<SheetInfo>, String> {
    let path = checked_absolute_path(&file_path)?;
    ensure_allowed_extension(&path, &["json"])?;
    let data = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    import_json_data(data, state, comments)
}

#[tauri::command]
fn export_json_file(file_path: String, state: State<AppState>) -> Result<(), String> {
    let path = checked_absolute_path(&file_path)?;
    ensure_allowed_extension(&path, &["json"])?;
    let data = export_json(state)?;
    std::fs::write(path, data).map_err(|e| e.to_string())
}

#[tauri::command]
fn import_native_file(
    file_path: String,
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<NativeOpenResult, String> {
    let path = checked_absolute_path(&file_path)?;
    ensure_allowed_extension(&path, &["900sheets"])?;
    let data = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let (imported, metadata) = sheets_json::import_native_workbook_with_metadata(&data)
        .map_err(|error| error.to_string())?;
    let sheets = replace_loaded_workbook(state.inner(), comments.inner(), imported)?;
    if let Some(saved_comments) = metadata.get("comments").and_then(|value| value.as_array()) {
        let mut manager = comments.lock().map_err(|error| error.to_string())?;
        for saved in saved_comments {
            let Some(sheet_id) = saved.get("sheet_id").and_then(|value| value.as_u64()) else {
                continue;
            };
            let Ok(comment) = serde_json::from_value::<CellComment>(saved.clone()) else {
                continue;
            };
            manager
                .comments
                .insert((sheet_id as u32, comment.row, comment.col), comment);
        }
    }
    if let Some(saved) = metadata.get("protections") {
        if let Ok(values) = serde_json::from_value::<Vec<SheetProtection>>(saved.clone()) {
            *state
                .protections
                .lock()
                .map_err(|error| error.to_string())? = values;
        }
    }
    if let Some(saved_locks) = metadata
        .get("cell_locks")
        .and_then(|value| value.as_array())
    {
        let mut managers = Vec::new();
        for sheet_locks in saved_locks {
            let mut manager = CellLockManager::new();
            if let Some(entries) = sheet_locks.as_array() {
                for entry in entries {
                    let Some(values) = entry.as_array() else {
                        continue;
                    };
                    if values.len() != 3 {
                        continue;
                    }
                    let (Some(row), Some(col), Some(locked)) =
                        (values[0].as_u64(), values[1].as_u64(), values[2].as_bool())
                    else {
                        continue;
                    };
                    manager.set_locked(row as u32, col as u32, locked);
                }
            }
            managers.push(manager);
        }
        *state.cell_locks.lock().map_err(|error| error.to_string())? = managers;
    }
    Ok(NativeOpenResult { sheets, metadata })
}

#[tauri::command]
fn export_native_file(
    file_path: String,
    mut metadata: serde_json::Value,
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<(), String> {
    let path = checked_absolute_path(&file_path)?;
    ensure_allowed_extension(&path, &["900sheets"])?;
    if !metadata.is_object() {
        metadata = serde_json::json!({});
    }
    let workbook = state.workbook.lock().map_err(|error| error.to_string())?;
    let comment_guard = comments.lock().map_err(|error| error.to_string())?;
    let protection_guard = state
        .protections
        .lock()
        .map_err(|error| error.to_string())?;
    let lock_guard = state.cell_locks.lock().map_err(|error| error.to_string())?;
    let object = metadata.as_object_mut().expect("metadata object");
    let saved_comments: Vec<serde_json::Value> = comment_guard
        .comments
        .iter()
        .filter_map(|(&(sheet_id, _, _), comment)| {
            let mut value = serde_json::to_value(comment).ok()?;
            value
                .as_object_mut()?
                .insert("sheet_id".into(), sheet_id.into());
            Some(value)
        })
        .collect();
    object.insert("comments".into(), saved_comments.into());
    object.insert(
        "protections".into(),
        serde_json::to_value(&*protection_guard).map_err(|error| error.to_string())?,
    );
    let cell_locks: Vec<Vec<(u32, u32, bool)>> = lock_guard
        .iter()
        .map(|manager| {
            manager
                .iter_overrides()
                .map(|((row, col), locked)| (row, col, locked))
                .collect()
        })
        .collect();
    object.insert(
        "cell_locks".into(),
        serde_json::to_value(cell_locks).map_err(|error| error.to_string())?,
    );
    let data = sheets_json::export_native_workbook_with_metadata(&workbook, metadata)
        .map_err(|error| error.to_string())?;
    write_atomic(&path, data.as_bytes())
}

#[tauri::command]
fn set_cell_format(
    sheet_id: u32,
    row: u32,
    col: u32,
    format: CellFormat,
    state: State<AppState>,
) -> Result<(), String> {
    ensure_can_perform_protected_action(state.inner(), sheet_id, ProtectionAction::FormatCells)?;
    let mut wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet_mut(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    sheet.set_format(row, col, format);
    Ok(())
}

#[tauri::command]
fn get_cell_format(
    sheet_id: u32,
    row: u32,
    col: u32,
    state: State<AppState>,
) -> Result<Option<CellFormat>, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    Ok(sheet.get_format(row, col).cloned())
}

#[derive(serde::Deserialize)]
struct SortRange {
    sort_col: u32,
    start_row: u32,
    end_row: u32,
    start_col: u32,
    end_col: u32,
}

#[tauri::command]
fn sort_data(
    sheet_id: u32,
    range: SortRange,
    ascending: bool,
    state: State<AppState>,
) -> Result<(), String> {
    ensure_can_perform_protected_action(state.inner(), sheet_id, ProtectionAction::Sort)?;
    let mut wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let mut next = wb
        .sheet(sheet_id as usize)
        .cloned()
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    sheets_core::data_tools::apply_sort_range(
        &mut next,
        range.sort_col,
        range.start_row,
        range.end_row,
        range.start_col,
        range.end_col,
        ascending,
    );
    let graph = dependency_graph_for_sheet(&next)?;
    *wb.sheet_mut(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))? = next;
    let count = wb.sheet_count();
    drop(wb);
    let mut graphs = state.dep_graphs.lock().map_err(|e| e.to_string())?;
    ensure_dependency_graphs(&mut graphs, count);
    graphs[sheet_id as usize] = graph;
    Ok(())
}

#[tauri::command]
fn find_in_sheet_cmd(
    sheet_id: u32,
    query: String,
    match_case: bool,
    state: State<AppState>,
) -> Result<Vec<SearchResultData>, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    let results = sheets_core::data_tools::find_in_sheet(sheet, &query, match_case);
    Ok(results
        .into_iter()
        .map(|r| SearchResultData {
            row: r.row,
            col: r.col,
            matched_text: r.matched_text,
        })
        .collect())
}

#[tauri::command]
fn replace_in_sheet_cmd(
    sheet_id: u32,
    find: String,
    replace: String,
    match_case: bool,
    state: State<AppState>,
) -> Result<usize, String> {
    ensure_sheet_not_protected(state.inner(), sheet_id)?;
    let mut wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let mut next = wb
        .sheet(sheet_id as usize)
        .cloned()
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    let replaced =
        sheets_core::data_tools::replace_in_sheet(&mut next, &find, &replace, match_case);
    let graph = dependency_graph_for_sheet(&next)?;
    *wb.sheet_mut(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))? = next;
    let count = wb.sheet_count();
    drop(wb);
    let mut graphs = state.dep_graphs.lock().map_err(|e| e.to_string())?;
    ensure_dependency_graphs(&mut graphs, count);
    graphs[sheet_id as usize] = graph;
    Ok(replaced)
}

#[tauri::command]
fn create_pivot(
    sheet_id: u32,
    config: PivotConfig,
    state: State<AppState>,
) -> Result<PivotResult, String> {
    ensure_can_perform_protected_action(state.inner(), sheet_id, ProtectionAction::UsePivot)?;
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    sheets_pivot::build_pivot(sheet, &config).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_pivot_sheet(
    sheet_id: u32,
    config: PivotConfig,
    state: State<AppState>,
) -> Result<Vec<SheetInfo>, String> {
    ensure_can_perform_protected_action(state.inner(), sheet_id, ProtectionAction::UsePivot)?;
    let mut wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    let result = sheets_pivot::build_pivot(sheet, &config).map_err(|e| e.to_string())?;
    let pivot_name = format!(
        "Pivot ({})",
        wb.sheet(sheet_id as usize).map(|s| s.name()).unwrap_or("")
    );
    let idx = wb.add_sheet(pivot_name);
    if let Some(pivot_sheet) = wb.sheet_mut(idx) {
        sheets_pivot::write_pivot_to_sheet(pivot_sheet, &result, 0, 0);
    }
    let graph = dependency_graph_for_sheet(
        wb.sheet(idx)
            .ok_or_else(|| format!("Sheet {} not found", idx))?,
    )?;
    let infos = sheet_infos(&wb);
    let count = wb.sheet_count();
    drop(wb);
    let mut graphs = state.dep_graphs.lock().map_err(|e| e.to_string())?;
    ensure_dependency_graphs(&mut graphs, count);
    graphs[idx] = graph;
    Ok(infos)
}

#[tauri::command]
fn get_pivot_columns(
    sheet_id: u32,
    header_row: u32,
    start_col: u32,
    end_col: u32,
    state: State<AppState>,
) -> Result<Vec<ColumnInfo>, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    Ok(
        sheets_pivot::auto_detect_columns(sheet, header_row, start_col, end_col)
            .into_iter()
            .map(|(col, name)| ColumnInfo { col, name })
            .collect(),
    )
}

#[derive(serde::Serialize)]
struct ColumnInfo {
    col: u32,
    name: String,
}

#[tauri::command]
fn create_chart(
    sheet_id: u32,
    config: ChartConfig,
    state: State<AppState>,
) -> Result<ChartResult, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    sheets_chart::build_chart(sheet, &config).map_err(|e| e.to_string())
}

// --- Data Validation IPC ---

#[derive(serde::Serialize)]
struct ValidationResult {
    row: u32,
    col: u32,
    error: String,
}

#[tauri::command]
fn validate_cell_value(
    sheet_id: u32,
    row: u32,
    col: u32,
    validation: DataValidation,
    state: State<AppState>,
) -> Result<bool, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    Ok(sheets_validation::validate_cell(sheet, row, col, &validation).is_ok())
}

#[tauri::command]
fn validate_range_cmd(
    sheet_id: u32,
    rule: ValidationRule,
    state: State<AppState>,
) -> Result<Vec<ValidationResult>, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    let errors = sheets_validation::validate_range(sheet, &rule);
    Ok(errors
        .into_iter()
        .map(|((r, c), e)| ValidationResult {
            row: r,
            col: c,
            error: e.to_string(),
        })
        .collect())
}

#[tauri::command]
fn check_input_value(validation: DataValidation, value: String) -> Result<bool, String> {
    Ok(sheets_validation::check_new_value(&validation, &value).is_ok())
}

// --- Conditional Formatting IPC ---

#[derive(serde::Serialize)]
struct ConditionalFormatCellResult {
    row: u32,
    col: u32,
    format: CellFormat,
    data_bar_value: Option<f64>,
    icon_index: Option<usize>,
    scale_color: Option<String>,
}

#[tauri::command]
fn evaluate_conditional_formats(
    sheet_id: u32,
    row: u32,
    col: u32,
    rules: Vec<ConditionalFormat>,
    state: State<AppState>,
) -> Result<ConditionalFormatCellResult, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    let base_format = sheet.get_format(row, col);
    let results = sheets_validation::evaluate_all_conditions(sheet, row, col, &rules);
    let mut merged = base_format.cloned().unwrap_or_default();
    let mut data_bar_value = None;
    let mut icon_index = None;
    let mut scale_color = None;
    for result in results {
        merged = merged.merge(&result.format);
        if let Some(v) = result.data_bar_value {
            data_bar_value = Some(v);
        }
        if let Some(i) = result.icon_index {
            icon_index = Some(i);
        }
        if let Some(c) = result.scale_color {
            scale_color = Some(c.clone());
            merged.bg_color = Some(c);
        }
    }
    Ok(ConditionalFormatCellResult {
        row,
        col,
        format: merged,
        data_bar_value,
        icon_index,
        scale_color,
    })
}

#[tauri::command]
fn find_conditional_format_matches(
    sheet_id: u32,
    rule: ConditionalFormat,
    state: State<AppState>,
) -> Result<Vec<(u32, u32)>, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    Ok(sheets_validation::find_matching_cells(sheet, &rule))
}

// --- i18n & Accessibility IPC ---

#[derive(serde::Serialize)]
struct LocaleInfo {
    code: String,
    language: String,
    is_rtl: bool,
    date_format: String,
    currency_symbol: String,
    first_day_of_week: u32,
}

#[tauri::command]
fn get_available_locales() -> Vec<LocaleInfo> {
    Locale::all()
        .into_iter()
        .map(|l| LocaleInfo {
            code: l.code().to_string(),
            language: l.language().to_string(),
            is_rtl: l.is_rtl(),
            date_format: l.date_format().to_string(),
            currency_symbol: l.currency_symbol().to_string(),
            first_day_of_week: l.first_day_of_week(),
        })
        .collect()
}

#[tauri::command]
fn get_translations(locale_code: String) -> Result<Vec<(String, String)>, String> {
    let locale = Locale::from_code(&locale_code)
        .ok_or_else(|| format!("Unknown locale: {}", locale_code))?;
    let provider = TranslationProvider::new(locale);
    Ok(provider.all_translations())
}

#[tauri::command]
fn translate_key(locale_code: String, key: TranslationKey) -> Result<String, String> {
    let locale = Locale::from_code(&locale_code)
        .ok_or_else(|| format!("Unknown locale: {}", locale_code))?;
    let provider = TranslationProvider::new(locale);
    Ok(provider.translate(key).to_string())
}

#[tauri::command]
fn format_number_i18n(value: f64, locale_code: String) -> Result<String, String> {
    let locale = Locale::from_code(&locale_code)
        .ok_or_else(|| format!("Unknown locale: {}", locale_code))?;
    Ok(sheets_i18n::format_number_locale(value, locale))
}

#[tauri::command]
fn format_currency_i18n(value: f64, locale_code: String) -> Result<String, String> {
    let locale = Locale::from_code(&locale_code)
        .ok_or_else(|| format!("Unknown locale: {}", locale_code))?;
    Ok(sheets_i18n::format_currency(value, locale))
}

#[tauri::command]
fn format_percentage_i18n(value: f64, locale_code: String) -> Result<String, String> {
    let locale = Locale::from_code(&locale_code)
        .ok_or_else(|| format!("Unknown locale: {}", locale_code))?;
    Ok(sheets_i18n::format_percentage(value, locale))
}

#[tauri::command]
fn format_date_i18n(serial: f64, locale_code: String) -> Result<String, String> {
    let locale = Locale::from_code(&locale_code)
        .ok_or_else(|| format!("Unknown locale: {}", locale_code))?;
    Ok(sheets_i18n::format_date(serial, locale))
}

#[tauri::command]
fn format_time_i18n(serial: f64, locale_code: String) -> Result<String, String> {
    let locale = Locale::from_code(&locale_code)
        .ok_or_else(|| format!("Unknown locale: {}", locale_code))?;
    Ok(sheets_i18n::format_time(serial, locale))
}

#[derive(serde::Serialize)]
struct AccessibilityLabelResult {
    role: String,
    label: String,
    description: Option<String>,
}

#[tauri::command]
fn get_cell_accessibility_label(
    row: u32,
    col: u32,
    value: String,
    locale_code: String,
) -> Result<AccessibilityLabelResult, String> {
    let locale = Locale::from_code(&locale_code)
        .ok_or_else(|| format!("Unknown locale: {}", locale_code))?;
    let result = sheets_i18n::cell_label(row, col, &value, locale);
    Ok(AccessibilityLabelResult {
        role: result.role,
        label: result.label,
        description: result.description,
    })
}

#[tauri::command]
fn get_selected_cell_label(
    row: u32,
    col: u32,
    value: String,
    locale_code: String,
) -> Result<String, String> {
    let locale = Locale::from_code(&locale_code)
        .ok_or_else(|| format!("Unknown locale: {}", locale_code))?;
    Ok(sheets_i18n::selected_cell_label(row, col, &value, locale))
}

#[tauri::command]
fn get_editing_cell_label(row: u32, col: u32, locale_code: String) -> Result<String, String> {
    let locale = Locale::from_code(&locale_code)
        .ok_or_else(|| format!("Unknown locale: {}", locale_code))?;
    Ok(sheets_i18n::editing_cell_label(row, col, locale))
}

#[tauri::command]
fn get_navigation_direction_name(
    direction: NavigationDirection,
    locale_code: String,
) -> Result<String, String> {
    let locale = Locale::from_code(&locale_code)
        .ok_or_else(|| format!("Unknown locale: {}", locale_code))?;
    Ok(sheets_i18n::navigation_direction_name(direction, locale).to_string())
}

// --- Print & PDF Export IPC ---

#[tauri::command]
fn get_print_preview(
    sheet_id: u32,
    config: PrintConfig,
    state: State<AppState>,
) -> Result<PrintPreview, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    sheets_print::generate_preview(sheet, &config).map_err(|e| e.to_string())
}

#[tauri::command]
fn render_print_html(
    sheet_id: u32,
    config: PrintConfig,
    state: State<AppState>,
) -> Result<String, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    sheets_print::render_html(sheet, &config).map_err(|e| e.to_string())
}

#[tauri::command]
fn export_pdf(
    sheet_id: u32,
    config: PrintConfig,
    state: State<AppState>,
) -> Result<Vec<u8>, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    sheets_print::render_pdf(sheet, &config).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_page_count(
    sheet_id: u32,
    config: PrintConfig,
    state: State<AppState>,
) -> Result<usize, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    let layout = sheets_print::calculate_pages(sheet, &config).map_err(|e| e.to_string())?;
    Ok(layout.total_pages)
}

#[tauri::command]
fn save_pdf_to_file(
    sheet_id: u32,
    config: PrintConfig,
    file_path: String,
    state: State<AppState>,
) -> Result<(), String> {
    let path = checked_absolute_path(&file_path)?;
    ensure_allowed_extension(&path, &["pdf"])?;
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    let pdf_bytes = sheets_print::render_pdf(sheet, &config).map_err(|e| e.to_string())?;
    std::fs::write(path, pdf_bytes).map_err(|e| e.to_string())
}

// --- Advanced Features IPC ---

#[tauri::command]
fn protect_sheet(sheet_id: u32, password: String, state: State<AppState>) -> Result<(), String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet_count = wb.sheet_count();
    if sheet_id as usize >= sheet_count {
        return Err(format!("Sheet {} not found", sheet_id));
    }
    let mut protections = state.protections.lock().map_err(|e| e.to_string())?;
    while protections.len() < sheet_count {
        protections.push(SheetProtection::default());
    }
    if let Some(protection) = protections.get_mut(sheet_id as usize) {
        protection.protect(&password);
    }
    let mut locks = state.cell_locks.lock().map_err(|e| e.to_string())?;
    while locks.len() < sheet_count {
        locks.push(CellLockManager::new());
    }
    Ok(())
}

#[tauri::command]
fn unprotect_sheet(
    sheet_id: u32,
    password: String,
    state: State<AppState>,
) -> Result<bool, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    if sheet_id as usize >= wb.sheet_count() {
        return Err(format!("Sheet {} not found", sheet_id));
    }
    let mut protections = state.protections.lock().map_err(|e| e.to_string())?;
    if (sheet_id as usize) >= protections.len() {
        return Ok(true);
    }
    if let Some(protection) = protections.get_mut(sheet_id as usize) {
        Ok(protection.unprotect(&password))
    } else {
        Ok(true)
    }
}

#[tauri::command]
fn goal_seek_cmd(
    sheet_id: u32,
    config: GoalSeekConfig,
    state: State<AppState>,
) -> Result<GoalSeekResult, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet = wb
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    sheets_advanced::goal_seek(sheet, &config).map_err(|e| e.to_string())
}

#[tauri::command]
fn apply_scenario(sheet_id: u32, scenario: Scenario, state: State<AppState>) -> Result<(), String> {
    let mut wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let mut next = wb
        .sheet(sheet_id as usize)
        .cloned()
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    for cell in &scenario.cells {
        ensure_can_edit_cell(state.inner(), sheet_id, cell.row, cell.col)?;
        next.set_cell_value(cell.row, cell.col, cell.value.clone());
    }
    let graph = dependency_graph_for_sheet(&next)?;
    *wb.sheet_mut(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))? = next;
    let count = wb.sheet_count();
    drop(wb);
    let mut graphs = state.dep_graphs.lock().map_err(|e| e.to_string())?;
    ensure_dependency_graphs(&mut graphs, count);
    graphs[sheet_id as usize] = graph;
    Ok(())
}

#[tauri::command]
fn get_cell_comment(
    sheet_id: u32,
    row: u32,
    col: u32,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<Option<CellComment>, String> {
    let mgr = comments.lock().map_err(|e| e.to_string())?;
    Ok(mgr.comments.get(&(sheet_id, row, col)).cloned())
}

#[tauri::command]
fn add_cell_comment(
    sheet_id: u32,
    row: u32,
    col: u32,
    text: String,
    author: String,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<(), String> {
    let mut mgr = comments.lock().map_err(|e| e.to_string())?;
    mgr.comments.insert(
        (sheet_id, row, col),
        CellComment {
            row,
            col,
            text,
            author,
            visible: false,
        },
    );
    Ok(())
}

#[tauri::command]
fn remove_cell_comment(
    sheet_id: u32,
    row: u32,
    col: u32,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<bool, String> {
    let mut mgr = comments.lock().map_err(|e| e.to_string())?;
    Ok(mgr.comments.remove(&(sheet_id, row, col)).is_some())
}

#[tauri::command]
fn list_comments(
    sheet_id: u32,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<Vec<CellComment>, String> {
    let mgr = comments.lock().map_err(|e| e.to_string())?;
    Ok(mgr
        .comments
        .iter()
        .filter(|((candidate, _, _), _)| *candidate == sheet_id)
        .map(|(_, comment)| comment.clone())
        .collect())
}

#[tauri::command]
fn set_cell_locked(
    sheet_id: u32,
    row: u32,
    col: u32,
    locked: bool,
    state: State<AppState>,
) -> Result<(), String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet_count = wb.sheet_count();
    if sheet_id as usize >= sheet_count {
        return Err(format!("Sheet {} not found", sheet_id));
    }
    let mut locks = state.cell_locks.lock().map_err(|e| e.to_string())?;
    while locks.len() < sheet_count {
        locks.push(CellLockManager::new());
    }
    if let Some(mgr) = locks.get_mut(sheet_id as usize) {
        mgr.set_locked(row, col, locked);
    }
    Ok(())
}

#[tauri::command]
fn lock_cell_range(
    sheet_id: u32,
    start_row: u32,
    start_col: u32,
    end_row: u32,
    end_col: u32,
    locked: bool,
    state: State<AppState>,
) -> Result<(), String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    let sheet_count = wb.sheet_count();
    if sheet_id as usize >= sheet_count {
        return Err(format!("Sheet {} not found", sheet_id));
    }
    let mut locks = state.cell_locks.lock().map_err(|e| e.to_string())?;
    while locks.len() < sheet_count {
        locks.push(CellLockManager::new());
    }
    if let Some(mgr) = locks.get_mut(sheet_id as usize) {
        if locked {
            mgr.lock_range(start_row, start_col, end_row, end_col);
        } else {
            mgr.unlock_range(start_row, start_col, end_row, end_col);
        }
    }
    Ok(())
}

#[tauri::command]
fn is_cell_locked(
    sheet_id: u32,
    row: u32,
    col: u32,
    state: State<AppState>,
) -> Result<bool, String> {
    let locks = state.cell_locks.lock().map_err(|e| e.to_string())?;
    Ok(locks
        .get(sheet_id as usize)
        .map(|m| m.is_locked(row, col))
        .unwrap_or(true))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            workbook: Mutex::new(Workbook::new()),
            dep_graphs: Mutex::new(Vec::new()),
            protections: Mutex::new(Vec::new()),
            cell_locks: Mutex::new(Vec::new()),
        })
        .manage(Mutex::new(SheetComments::default()))
        .invoke_handler(tauri::generate_handler![
            new_workbook,
            get_sheets,
            set_active_sheet,
            add_sheet,
            delete_sheet,
            rename_sheet,
            get_cell,
            set_cell,
            clear_cell,
            batch_set_cells,
            batch_set_formats,
            replace_sheet_snapshot,
            edit_sheet_structure,
            get_sheet_data,
            evaluate_formula,
            import_xlsx,
            export_xlsx,
            import_xlsx_file,
            export_xlsx_file,
            import_csv_data,
            export_csv,
            import_csv_file,
            export_csv_file,
            import_json_data,
            export_json,
            import_json_file,
            export_json_file,
            import_native_file,
            export_native_file,
            set_cell_format,
            get_cell_format,
            sort_data,
            find_in_sheet_cmd,
            replace_in_sheet_cmd,
            create_pivot,
            create_pivot_sheet,
            get_pivot_columns,
            create_chart,
            validate_cell_value,
            validate_range_cmd,
            check_input_value,
            evaluate_conditional_formats,
            find_conditional_format_matches,
            get_available_locales,
            get_translations,
            translate_key,
            format_number_i18n,
            format_currency_i18n,
            format_percentage_i18n,
            format_date_i18n,
            format_time_i18n,
            get_cell_accessibility_label,
            get_selected_cell_label,
            get_editing_cell_label,
            get_navigation_direction_name,
            get_print_preview,
            render_print_html,
            export_pdf,
            get_page_count,
            save_pdf_to_file,
            protect_sheet,
            unprotect_sheet,
            goal_seek_cmd,
            apply_scenario,
            get_cell_comment,
            add_cell_comment,
            remove_cell_comment,
            list_comments,
            set_cell_locked,
            lock_cell_range,
            is_cell_locked,
        ])
        .run(tauri::generate_context!())
        .expect("error while running 900Sheets");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> AppState {
        AppState {
            workbook: Mutex::new(Workbook::new()),
            dep_graphs: Mutex::new(Vec::new()),
            protections: Mutex::new(Vec::new()),
            cell_locks: Mutex::new(Vec::new()),
        }
    }

    #[test]
    fn protected_sheet_blocks_cell_edit() {
        let state = test_state();
        assert!(ensure_can_edit_cell(&state, 0, 0, 0).is_ok());

        {
            let mut protections = state.protections.lock().unwrap();
            let mut protection = SheetProtection::default();
            protection.protect("secret");
            protections.push(protection);
        }

        assert!(ensure_can_edit_cell(&state, 0, 0, 0).is_err());
    }

    #[test]
    fn protected_sheet_allows_unlocked_cell() {
        let state = test_state();
        {
            let mut protections = state.protections.lock().unwrap();
            let mut protection = SheetProtection::default();
            protection.protect("secret");
            protections.push(protection);
        }
        {
            let mut locks = state.cell_locks.lock().unwrap();
            let mut mgr = CellLockManager::new();
            mgr.set_locked(0, 0, false);
            locks.push(mgr);
        }
        assert!(ensure_can_edit_cell(&state, 0, 0, 0).is_ok());
        assert!(ensure_can_edit_cell(&state, 0, 1, 0).is_err());
    }

    #[test]
    fn protected_sheet_blocks_disallowed_actions() {
        let state = test_state();
        {
            let mut protections = state.protections.lock().unwrap();
            let mut protection = SheetProtection::default();
            protection.protect("secret");
            protections.push(protection);
        }

        assert!(
            ensure_can_perform_protected_action(&state, 0, ProtectionAction::FormatCells).is_err()
        );
        assert!(
            ensure_can_perform_protected_action(&state, 0, ProtectionAction::SelectCells).is_ok()
        );
    }

    #[test]
    fn checked_absolute_path_rejects_relative_and_parent_paths() {
        assert!(checked_absolute_path("relative/file.xlsx").is_err());
        assert!(checked_absolute_path("/tmp/../secret.xlsx").is_err());
        assert_eq!(
            checked_absolute_path("/tmp/900sheets.xlsx").unwrap(),
            PathBuf::from("/tmp/900sheets.xlsx")
        );
    }

    #[test]
    fn allowed_extension_is_case_insensitive() {
        assert!(ensure_allowed_extension(&PathBuf::from("/tmp/data.XLSX"), &["xlsx"]).is_ok());
        assert!(ensure_allowed_extension(&PathBuf::from("/tmp/data.exe"), &["xlsx"]).is_err());
    }

    #[test]
    fn set_cell_rejects_and_does_not_store_circular_formula() {
        let state = test_state();

        set_cell_value_in_workbook(&state, 0, 0, 0, "=B1".into()).unwrap();
        let result = set_cell_value_in_workbook(&state, 0, 0, 1, "=A1".into());

        assert!(result.is_err());
        {
            let wb = state.workbook.lock().unwrap();
            let sheet = wb.sheet(0).unwrap();
            assert_eq!(sheet.cell_value(0, 0), Some("=B1".into()));
            assert_eq!(sheet.cell_value(0, 1), None);
        }
        let graphs = state.dep_graphs.lock().unwrap();
        let graph = graphs.first().unwrap();
        assert!(graph.get_dependencies(0, 0).unwrap().contains(&(0, 1)));
        assert!(graph.get_dependencies(0, 1).is_none());
    }

    #[test]
    fn clear_cell_removes_formula_edges_from_dependency_graph() {
        let state = test_state();

        set_cell_value_in_workbook(&state, 0, 0, 0, "=B1".into()).unwrap();
        clear_cell_in_workbook(&state, 0, 0, 0).unwrap();

        let graphs = state.dep_graphs.lock().unwrap();
        let graph = graphs.first().unwrap();
        assert!(graph.get_dependencies(0, 0).is_none());
        assert!(graph
            .get_dependents(0, 1)
            .map(|deps| deps.is_empty())
            .unwrap_or(true));
    }

    #[test]
    fn non_formula_edit_removes_prior_formula_edges() {
        let state = test_state();

        set_cell_value_in_workbook(&state, 0, 0, 0, "=B1".into()).unwrap();
        set_cell_value_in_workbook(&state, 0, 0, 0, "plain text".into()).unwrap();

        let graphs = state.dep_graphs.lock().unwrap();
        let graph = graphs.first().unwrap();
        assert!(graph.get_dependencies(0, 0).is_none());
        assert!(graph
            .get_dependents(0, 1)
            .map(|deps| deps.is_empty())
            .unwrap_or(true));
    }

    #[test]
    fn full_workbook_reset_clears_comments() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        comments.lock().unwrap().comments.insert(
            (0, 0, 0),
            CellComment {
                row: 0,
                col: 0,
                text: "stale".into(),
                author: "tester".into(),
                visible: false,
            },
        );

        clear_full_workbook_state(&state, &comments).unwrap();

        assert!(comments.lock().unwrap().comments.is_empty());
    }

    #[test]
    fn comments_are_scoped_by_sheet() {
        let mut comments = SheetComments::default();
        for sheet_id in 0..=1 {
            comments.comments.insert(
                (sheet_id, 0, 0),
                CellComment {
                    row: 0,
                    col: 0,
                    text: format!("sheet {sheet_id}"),
                    author: "tester".into(),
                    visible: false,
                },
            );
        }
        assert_eq!(comments.comments.get(&(0, 0, 0)).unwrap().text, "sheet 0");
        assert_eq!(comments.comments.get(&(1, 0, 0)).unwrap().text, "sheet 1");
    }

    #[test]
    fn deleting_sheet_removes_and_reindexes_comments() {
        let mut comments = SheetComments::default();
        for sheet_id in 0..=2 {
            comments.comments.insert(
                (sheet_id, 0, 0),
                CellComment {
                    row: 0,
                    col: 0,
                    text: format!("sheet {sheet_id}"),
                    author: "tester".into(),
                    visible: false,
                },
            );
        }
        delete_sheet_comments(&mut comments, 1);
        assert_eq!(comments.comments.len(), 2);
        assert_eq!(comments.comments.get(&(0, 0, 0)).unwrap().text, "sheet 0");
        assert_eq!(comments.comments.get(&(1, 0, 0)).unwrap().text, "sheet 2");
    }

    #[test]
    fn imported_workbook_dependency_rebuild_rejects_existing_cycle() {
        let mut workbook = Workbook::new();
        let sheet = workbook.sheet_mut(0).unwrap();
        sheet.set_cell_value(0, 0, "=B1".into());
        sheet.set_cell_value(0, 1, "=A1".into());
        assert!(dependency_graphs_for_workbook(&workbook).is_err());
    }

    #[test]
    fn dependency_rebuild_rejects_cycle_created_by_formula_move() {
        let mut sheet = sheets_core::sheet::Sheet::new("Data");
        sheet.set_cell_value(0, 0, "=A2".into());
        sheet.set_cell_value(1, 0, "1".into());
        sheets_core::data_tools::apply_sort_range(&mut sheet, 0, 0, 1, 0, 0, true);
        assert_eq!(sheet.cell_value(1, 0), Some("=A2".into()));
        assert!(dependency_graph_for_sheet(&sheet).is_err());
    }

    #[test]
    fn structural_edit_cleanup_removes_coordinate_bound_backend_state() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        comments.lock().unwrap().comments.insert(
            (0, 2, 3),
            CellComment {
                row: 2,
                col: 3,
                text: "note".into(),
                author: "tester".into(),
                visible: false,
            },
        );
        let mut manager = CellLockManager::new();
        manager.set_locked(2, 3, false);
        state.cell_locks.lock().unwrap().push(manager);

        clear_sheet_coordinate_state(&state, &comments, 0).unwrap();

        assert!(comments.lock().unwrap().comments.is_empty());
        assert_eq!(
            state.cell_locks.lock().unwrap()[0].iter_overrides().count(),
            0
        );
    }

    #[test]
    fn native_file_atomic_write_and_open_preserve_latest_cell_and_metadata() {
        let mut workbook = Workbook::new();
        workbook
            .sheet_mut(0)
            .unwrap()
            .set_cell_value(0, 0, "latest".into());
        let metadata = serde_json::json!({"sheet_states": {"1": {"frozenRowCount": 1}}});
        let data =
            sheets_json::export_native_workbook_with_metadata(&workbook, metadata.clone()).unwrap();
        let path = std::env::temp_dir().join(format!(
            "900sheets-native-command-{}.900sheets",
            std::process::id()
        ));

        write_atomic(&path, data.as_bytes()).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        let (reopened, restored_metadata) =
            sheets_json::import_native_workbook_with_metadata(&saved).unwrap();

        assert_eq!(
            reopened.sheet(0).unwrap().cell_value(0, 0),
            Some("latest".into())
        );
        assert_eq!(restored_metadata, metadata);
        let _ = std::fs::remove_file(path);
    }
}
