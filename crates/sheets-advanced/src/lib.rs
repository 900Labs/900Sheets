use serde::{Deserialize, Serialize};
use sheets_core::sheet::Sheet;
use sheets_formula::evaluator::{CellProvider, Evaluator, Value};
use sheets_formula::parser::Parser;
use std::collections::HashMap;
use thiserror::Error;

// ============================================================================
// Sheet Protection
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetProtection {
    pub protected: bool,
    pub password_hash: Option<String>,
    pub allow_select_cells: bool,
    pub allow_format_cells: bool,
    pub allow_insert_rows: bool,
    pub allow_insert_cols: bool,
    pub allow_delete_rows: bool,
    pub allow_delete_cols: bool,
    pub allow_sort: bool,
    pub allow_filter: bool,
    pub allow_use_pivot: bool,
}

impl Default for SheetProtection {
    fn default() -> Self {
        Self {
            protected: false,
            password_hash: None,
            allow_select_cells: true,
            allow_format_cells: false,
            allow_insert_rows: false,
            allow_insert_cols: false,
            allow_delete_rows: false,
            allow_delete_cols: false,
            allow_sort: false,
            allow_filter: false,
            allow_use_pivot: false,
        }
    }
}

impl SheetProtection {
    pub fn protect(&mut self, password: &str) {
        self.protected = true;
        self.password_hash = Some(hash_password(password));
    }

    pub fn unprotect(&mut self, password: &str) -> bool {
        if let Some(ref hash) = self.password_hash {
            if hash == &hash_password(password) {
                self.protected = false;
                self.password_hash = None;
                return true;
            }
            false
        } else {
            self.protected = false;
            true
        }
    }

    pub fn unprotect_no_password(&mut self) {
        self.protected = false;
        self.password_hash = None;
    }

    pub fn can_edit_cell(&self, cell_locked: bool) -> bool {
        if !self.protected {
            return true;
        }
        !cell_locked
    }

    pub fn can_perform(&self, action: ProtectionAction) -> bool {
        if !self.protected {
            return true;
        }
        match action {
            ProtectionAction::SelectCells => self.allow_select_cells,
            ProtectionAction::FormatCells => self.allow_format_cells,
            ProtectionAction::InsertRows => self.allow_insert_rows,
            ProtectionAction::InsertCols => self.allow_insert_cols,
            ProtectionAction::DeleteRows => self.allow_delete_rows,
            ProtectionAction::DeleteCols => self.allow_delete_cols,
            ProtectionAction::Sort => self.allow_sort,
            ProtectionAction::Filter => self.allow_filter,
            ProtectionAction::UsePivot => self.allow_use_pivot,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ProtectionAction {
    SelectCells,
    FormatCells,
    InsertRows,
    InsertCols,
    DeleteRows,
    DeleteCols,
    Sort,
    Filter,
    UsePivot,
}

/// Non-cryptographic hash for sheet protection passwords.
/// Uses a simple polynomial rolling hash, sufficient for deterrence in a
/// local-first desktop app but NOT suitable for storing sensitive secrets.
fn hash_password(password: &str) -> String {
    let mut hash: u32 = 0;
    for byte in password.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
    }
    format!("{:08x}", hash)
}

// ============================================================================
// Cell Locking
// ============================================================================

#[derive(Debug, Clone, Default)]
pub struct CellLockManager {
    locked_cells: HashMap<(u32, u32), bool>,
}

impl CellLockManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_locked(&mut self, row: u32, col: u32, locked: bool) {
        self.locked_cells.insert((row, col), locked);
    }

    pub fn is_locked(&self, row: u32, col: u32) -> bool {
        self.locked_cells.get(&(row, col)).copied().unwrap_or(true)
    }

    pub fn lock_range(&mut self, start_row: u32, start_col: u32, end_row: u32, end_col: u32) {
        for r in start_row..=end_row {
            for c in start_col..=end_col {
                self.locked_cells.insert((r, c), true);
            }
        }
    }

    pub fn unlock_range(&mut self, start_row: u32, start_col: u32, end_row: u32, end_col: u32) {
        for r in start_row..=end_row {
            for c in start_col..=end_col {
                self.locked_cells.insert((r, c), false);
            }
        }
    }

    pub fn locked_count(&self) -> usize {
        self.locked_cells.values().filter(|&&v| v).count()
    }

    pub fn clear(&mut self) {
        self.locked_cells.clear();
    }

    pub fn iter_overrides(&self) -> impl Iterator<Item = ((u32, u32), bool)> + '_ {
        self.locked_cells
            .iter()
            .map(|(&(row, col), &locked)| ((row, col), locked))
    }
}

// ============================================================================
// Goal Seek
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalSeekConfig {
    pub target_cell_row: u32,
    pub target_cell_col: u32,
    pub target_value: f64,
    pub input_cell_row: u32,
    pub input_cell_col: u32,
    pub max_iterations: u32,
    pub tolerance: f64,
}

impl Default for GoalSeekConfig {
    fn default() -> Self {
        Self {
            target_cell_row: 0,
            target_cell_col: 0,
            target_value: 0.0,
            input_cell_row: 0,
            input_cell_col: 0,
            max_iterations: 100,
            tolerance: 0.001,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalSeekResult {
    pub success: bool,
    pub input_value: f64,
    pub achieved_value: f64,
    pub iterations: u32,
    pub error: Option<String>,
}

#[derive(Debug, Error, Clone, PartialEq)]
pub enum GoalSeekError {
    #[error("Target cell is not a formula")]
    TargetNotFormula,
    #[error("Target cell not found")]
    TargetNotFound,
    #[error("Input cell not found")]
    InputNotFound,
    #[error("Goal seek did not converge after {0} iterations")]
    NotConverged(u32),
    #[error("Formula evaluation error: {0}")]
    EvaluationError(String),
}

/// Perform goal seek using the bisection method with Newton's method fallback.
/// Given a sheet with formulas, finds the input value that makes the target cell
/// equal to the target value.
pub fn goal_seek(sheet: &Sheet, config: &GoalSeekConfig) -> Result<GoalSeekResult, GoalSeekError> {
    let target_cell = sheet
        .cell(config.target_cell_row, config.target_cell_col)
        .ok_or(GoalSeekError::TargetNotFound)?;

    if !target_cell.is_formula() {
        return Err(GoalSeekError::TargetNotFormula);
    }

    // Get current input value as starting point
    let current_input = sheet
        .cell(config.input_cell_row, config.input_cell_col)
        .and_then(|c| c.as_number())
        .unwrap_or(0.0);

    // Create a mutable provider with the sheet's data
    let mut provider = SheetProvider::from_sheet(sheet);

    let evaluate_target = |input_val: f64, prov: &mut SheetProvider| -> f64 {
        prov.set_number(config.input_cell_row, config.input_cell_col, input_val);
        let expr = Parser::parse_formula(&target_cell.raw);
        if let Ok(expr) = expr {
            let evaluator = Evaluator::new();
            let result = evaluator.evaluate(&expr, prov);
            result.as_number().unwrap_or(f64::NAN)
        } else {
            f64::NAN
        }
    };

    // Try Newton's method first with numerical derivative
    let mut current = current_input;
    let mut best_value = evaluate_target(current, &mut provider);
    let mut iterations = 0u32;

    if (best_value - config.target_value).abs() < config.tolerance {
        return Ok(GoalSeekResult {
            success: true,
            input_value: current,
            achieved_value: best_value,
            iterations: 0,
            error: None,
        });
    }

    let h = 1e-6;
    let mut prev_diff = best_value - config.target_value;

    while iterations < config.max_iterations {
        iterations += 1;

        // Numerical derivative
        let f_plus = evaluate_target(current + h, &mut provider);
        let derivative = (f_plus - best_value) / h;

        if derivative.abs() < 1e-15 {
            // Fall back to bisection
            break;
        }

        // Newton step
        let step = prev_diff / derivative;
        let new_input = current - step;

        let new_value = evaluate_target(new_input, &mut provider);
        let new_diff = new_value - config.target_value;

        if new_diff.abs() < config.tolerance {
            return Ok(GoalSeekResult {
                success: true,
                input_value: new_input,
                achieved_value: new_value,
                iterations,
                error: None,
            });
        }

        current = new_input;
        best_value = new_value;
        prev_diff = new_diff;
    }

    // Bisection fallback
    let mut lo = current_input - 1000.0;
    let mut hi = current_input + 1000.0;

    let f_lo = evaluate_target(lo, &mut provider);
    let f_hi = evaluate_target(hi, &mut provider);

    let target = config.target_value;

    // Check if we bracket the root
    if (f_lo - target).signum() == (f_hi - target).signum() {
        // Try expanding the range
        let mut found = false;
        for _ in 0..20 {
            lo -= 1000.0;
            hi += 1000.0;
            let f_lo2 = evaluate_target(lo, &mut provider);
            let f_hi2 = evaluate_target(hi, &mut provider);
            if (f_lo2 - target).signum() != (f_hi2 - target).signum() {
                found = true;
                break;
            }
        }
        if !found {
            return Ok(GoalSeekResult {
                success: false,
                input_value: current,
                achieved_value: best_value,
                iterations,
                error: Some(format!(
                    "Could not bracket solution. Last value: {} (target: {})",
                    best_value, target
                )),
            });
        }
    }

    while iterations < config.max_iterations {
        iterations += 1;
        let mid = (lo + hi) / 2.0;
        let f_mid = evaluate_target(mid, &mut provider);

        if (f_mid - target).abs() < config.tolerance {
            return Ok(GoalSeekResult {
                success: true,
                input_value: mid,
                achieved_value: f_mid,
                iterations,
                error: None,
            });
        }

        if (f_mid - target).signum() != (evaluate_target(lo, &mut provider) - target).signum() {
            hi = mid;
        } else {
            lo = mid;
        }
    }

    let final_input = (lo + hi) / 2.0;
    let final_value = evaluate_target(final_input, &mut provider);
    Ok(GoalSeekResult {
        success: false,
        input_value: final_input,
        achieved_value: final_value,
        iterations,
        error: Some(GoalSeekError::NotConverged(config.max_iterations).to_string()),
    })
}

// ============================================================================
// Scenarios (What-If Analysis)
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioCell {
    pub row: u32,
    pub col: u32,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub comment: String,
    pub cells: Vec<ScenarioCell>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScenarioManager {
    pub scenarios: Vec<Scenario>,
    pub active_scenario: Option<String>,
}

impl ScenarioManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, scenario: Scenario) {
        self.scenarios.retain(|s| s.name != scenario.name);
        self.scenarios.push(scenario);
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.scenarios.len();
        self.scenarios.retain(|s| s.name != name);
        if self.active_scenario.as_deref() == Some(name) {
            self.active_scenario = None;
        }
        self.scenarios.len() < before
    }

    pub fn get(&self, name: &str) -> Option<&Scenario> {
        self.scenarios.iter().find(|s| s.name == name)
    }

    pub fn apply(&mut self, name: &str, sheet: &mut Sheet) -> Result<(), String> {
        let scenario = self
            .get(name)
            .ok_or_else(|| format!("Scenario '{}' not found", name))?
            .clone();

        for cell in &scenario.cells {
            sheet.set_cell_value(cell.row, cell.col, cell.value.clone());
        }

        self.active_scenario = Some(name.to_string());
        Ok(())
    }

    pub fn create_from_sheet(
        &mut self,
        name: &str,
        comment: &str,
        sheet: &Sheet,
        cells: &[(u32, u32)],
    ) {
        let scenario_cells: Vec<ScenarioCell> = cells
            .iter()
            .map(|&(row, col)| {
                let value = sheet.cell_value(row, col).unwrap_or_default();
                ScenarioCell { row, col, value }
            })
            .collect();

        self.add(Scenario {
            name: name.to_string(),
            comment: comment.to_string(),
            cells: scenario_cells,
        });
    }

    pub fn list(&self) -> Vec<(String, String)> {
        self.scenarios
            .iter()
            .map(|s| (s.name.clone(), s.comment.clone()))
            .collect()
    }

    pub fn summary(&self) -> String {
        let mut s = String::new();
        s.push_str("Scenario Summary\n");
        s.push_str("================\n\n");
        for scenario in &self.scenarios {
            let active = if self.active_scenario.as_deref() == Some(&scenario.name) {
                " (active)"
            } else {
                ""
            };
            s.push_str(&format!("{}{}\n", scenario.name, active));
            if !scenario.comment.is_empty() {
                s.push_str(&format!("  Comment: {}\n", scenario.comment));
            }
            for cell in &scenario.cells {
                s.push_str(&format!(
                    "  Cell ({},{}): {}\n",
                    cell.row, cell.col, cell.value
                ));
            }
            s.push('\n');
        }
        s
    }
}

// ============================================================================
// Cell Comments / Notes
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellComment {
    pub row: u32,
    pub col: u32,
    pub text: String,
    pub author: String,
    pub visible: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommentManager {
    comments: HashMap<(u32, u32), CellComment>,
}

impl CommentManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, row: u32, col: u32, text: &str, author: &str) {
        self.comments.insert(
            (row, col),
            CellComment {
                row,
                col,
                text: text.to_string(),
                author: author.to_string(),
                visible: false,
            },
        );
    }

    pub fn get(&self, row: u32, col: u32) -> Option<&CellComment> {
        self.comments.get(&(row, col))
    }

    pub fn remove(&mut self, row: u32, col: u32) -> bool {
        self.comments.remove(&(row, col)).is_some()
    }

    pub fn update_text(&mut self, row: u32, col: u32, text: &str) -> bool {
        if let Some(comment) = self.comments.get_mut(&(row, col)) {
            comment.text = text.to_string();
            true
        } else {
            false
        }
    }

    pub fn set_visible(&mut self, row: u32, col: u32, visible: bool) {
        if let Some(comment) = self.comments.get_mut(&(row, col)) {
            comment.visible = visible;
        }
    }

    pub fn toggle_visible(&mut self, row: u32, col: u32) {
        if let Some(comment) = self.comments.get_mut(&(row, col)) {
            comment.visible = !comment.visible;
        }
    }

    pub fn list(&self) -> Vec<&CellComment> {
        self.comments.values().collect()
    }

    pub fn count(&self) -> usize {
        self.comments.len()
    }

    pub fn clear(&mut self) {
        self.comments.clear();
    }

    pub fn iter(&self) -> impl Iterator<Item = (&(u32, u32), &CellComment)> {
        self.comments.iter()
    }
}

// ============================================================================
// Sheet Provider (for goal seek evaluation)
// ============================================================================

struct SheetProvider {
    cells: HashMap<(u32, u32), Value>,
    raw: HashMap<(u32, u32), String>,
    formulas: HashMap<(u32, u32), String>,
}

impl SheetProvider {
    fn from_sheet(sheet: &Sheet) -> Self {
        let mut cells = HashMap::new();
        let mut raw = HashMap::new();
        let mut formulas = HashMap::new();

        for ((r, c), cell) in sheet.iter_cells() {
            raw.insert((r, c), cell.raw.clone());
            match cell.cell_type {
                sheets_core::cell::CellType::Number => {
                    cells.insert((r, c), Value::Number(cell.as_number().unwrap_or(0.0)));
                }
                sheets_core::cell::CellType::Text => {
                    cells.insert((r, c), Value::String(cell.raw.clone()));
                }
                sheets_core::cell::CellType::Boolean => {
                    cells.insert(
                        (r, c),
                        Value::Boolean(cell.raw.eq_ignore_ascii_case("true")),
                    );
                }
                sheets_core::cell::CellType::Formula => {
                    formulas.insert((r, c), cell.raw.clone());
                    cells.insert((r, c), Value::Empty);
                }
                _ => {}
            }
        }

        Self {
            cells,
            raw,
            formulas,
        }
    }

    fn set_number(&mut self, row: u32, col: u32, n: f64) {
        self.cells.insert((row, col), Value::Number(n));
        self.raw.insert((row, col), n.to_string());
    }
}

impl CellProvider for SheetProvider {
    fn get_cell(&self, row: u32, col: u32) -> Value {
        if let Some(formula) = self.formulas.get(&(row, col)) {
            if let Ok(expr) = Parser::parse_formula(formula) {
                let evaluator = Evaluator::new();
                return evaluator.evaluate(&expr, self);
            }
            return Value::Error(sheets_formula::error::FormulaError::ParseError(
                "Invalid formula".into(),
            ));
        }
        self.cells.get(&(row, col)).cloned().unwrap_or(Value::Empty)
    }

    fn get_raw(&self, row: u32, col: u32) -> String {
        self.raw.get(&(row, col)).cloned().unwrap_or_default()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Protection Tests ---

    #[test]
    fn test_protection_default() {
        let p = SheetProtection::default();
        assert!(!p.protected);
        assert!(p.allow_select_cells);
        assert!(!p.allow_format_cells);
    }

    #[test]
    fn test_protect_with_password() {
        let mut p = SheetProtection::default();
        p.protect("secret");
        assert!(p.protected);
        assert!(p.password_hash.is_some());
    }

    #[test]
    fn test_unprotect_correct_password() {
        let mut p = SheetProtection::default();
        p.protect("secret");
        assert!(p.unprotect("secret"));
        assert!(!p.protected);
    }

    #[test]
    fn test_unprotect_wrong_password() {
        let mut p = SheetProtection::default();
        p.protect("secret");
        assert!(!p.unprotect("wrong"));
        assert!(p.protected);
    }

    #[test]
    fn test_unprotect_no_password() {
        let mut p = SheetProtection {
            protected: true,
            ..SheetProtection::default()
        };
        p.unprotect_no_password();
        assert!(!p.protected);
    }

    #[test]
    fn test_can_edit_cell() {
        let mut p = SheetProtection::default();
        p.protect("pass");
        assert!(!p.can_edit_cell(true));
        assert!(p.can_edit_cell(false));
    }

    #[test]
    fn test_can_edit_cell_unprotected() {
        let p = SheetProtection::default();
        assert!(p.can_edit_cell(true));
        assert!(p.can_edit_cell(false));
    }

    #[test]
    fn test_can_perform() {
        let mut p = SheetProtection::default();
        p.protect("pass");
        assert!(p.can_perform(ProtectionAction::SelectCells));
        assert!(!p.can_perform(ProtectionAction::FormatCells));
    }

    #[test]
    fn test_password_hash_deterministic() {
        assert_eq!(hash_password("test"), hash_password("test"));
        assert_ne!(hash_password("test"), hash_password("test2"));
    }

    // --- Cell Lock Tests ---

    #[test]
    fn test_lock_manager_default() {
        let mgr = CellLockManager::new();
        assert!(mgr.is_locked(0, 0));
    }

    #[test]
    fn test_lock_unlock() {
        let mut mgr = CellLockManager::new();
        mgr.set_locked(0, 0, false);
        assert!(!mgr.is_locked(0, 0));
        mgr.set_locked(0, 0, true);
        assert!(mgr.is_locked(0, 0));
    }

    #[test]
    fn test_lock_range() {
        let mut mgr = CellLockManager::new();
        mgr.unlock_range(0, 0, 2, 2);
        assert!(!mgr.is_locked(0, 0));
        assert!(!mgr.is_locked(2, 2));
        mgr.lock_range(0, 0, 2, 2);
        assert!(mgr.is_locked(0, 0));
        assert!(mgr.is_locked(2, 2));
    }

    #[test]
    fn test_locked_count() {
        let mut mgr = CellLockManager::new();
        mgr.set_locked(0, 0, true);
        mgr.set_locked(1, 1, true);
        mgr.set_locked(2, 2, false);
        assert_eq!(mgr.locked_count(), 2);
    }

    // --- Goal Seek Tests ---

    #[test]
    fn test_goal_seek_simple() {
        let mut sheet = Sheet::new("Test");
        // A1 = input, B1 = =A1*2, target: B1 = 10
        sheet.set_cell_value(0, 0, "1".into());
        sheet.set_cell_value(0, 1, "=A1*2".into());

        let config = GoalSeekConfig {
            target_cell_row: 0,
            target_cell_col: 1,
            target_value: 10.0,
            input_cell_row: 0,
            input_cell_col: 0,
            ..GoalSeekConfig::default()
        };

        let result = goal_seek(&sheet, &config).unwrap();
        assert!(result.success);
        assert!((result.input_value - 5.0).abs() < 0.01);
        assert!((result.achieved_value - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_goal_seek_linear() {
        let mut sheet = Sheet::new("Test");
        // A1 = input, B1 = =A1+5, target: B1 = 20
        sheet.set_cell_value(0, 0, "0".into());
        sheet.set_cell_value(0, 1, "=A1+5".into());

        let config = GoalSeekConfig {
            target_cell_row: 0,
            target_cell_col: 1,
            target_value: 20.0,
            input_cell_row: 0,
            input_cell_col: 0,
            ..GoalSeekConfig::default()
        };

        let result = goal_seek(&sheet, &config).unwrap();
        assert!(result.success);
        assert!((result.input_value - 15.0).abs() < 0.01);
    }

    #[test]
    fn test_goal_seek_with_sum() {
        let mut sheet = Sheet::new("Test");
        // A1 = input, A2..A4 = 10, 20, 30, B1 = =SUM(A1:A4), target: B1 = 100
        sheet.set_cell_value(0, 0, "0".into());
        sheet.set_cell_value(1, 0, "10".into());
        sheet.set_cell_value(2, 0, "20".into());
        sheet.set_cell_value(3, 0, "30".into());
        sheet.set_cell_value(0, 1, "=SUM(A1:A4)".into());

        let config = GoalSeekConfig {
            target_cell_row: 0,
            target_cell_col: 1,
            target_value: 100.0,
            input_cell_row: 0,
            input_cell_col: 0,
            ..GoalSeekConfig::default()
        };

        let result = goal_seek(&sheet, &config).unwrap();
        assert!(result.success);
        assert!((result.input_value - 40.0).abs() < 0.01);
    }

    #[test]
    fn test_goal_seek_not_formula() {
        let mut sheet = Sheet::new("Test");
        sheet.set_cell_value(0, 0, "5".into());

        let config = GoalSeekConfig {
            target_cell_row: 0,
            target_cell_col: 0,
            target_value: 10.0,
            input_cell_row: 0,
            input_cell_col: 1,
            ..GoalSeekConfig::default()
        };

        let result = goal_seek(&sheet, &config);
        assert!(matches!(result, Err(GoalSeekError::TargetNotFormula)));
    }

    #[test]
    fn test_goal_seek_already_at_target() {
        let mut sheet = Sheet::new("Test");
        sheet.set_cell_value(0, 0, "5".into());
        sheet.set_cell_value(0, 1, "=A1*2".into());

        let config = GoalSeekConfig {
            target_cell_row: 0,
            target_cell_col: 1,
            target_value: 10.0,
            input_cell_row: 0,
            input_cell_col: 0,
            ..GoalSeekConfig::default()
        };

        let result = goal_seek(&sheet, &config).unwrap();
        assert!(result.success);
        assert_eq!(result.iterations, 0);
    }

    // --- Scenario Tests ---

    #[test]
    fn test_scenario_add_remove() {
        let mut mgr = ScenarioManager::new();
        mgr.add(Scenario {
            name: "Best Case".into(),
            comment: "Optimistic".into(),
            cells: vec![ScenarioCell {
                row: 0,
                col: 0,
                value: "100".into(),
            }],
        });
        assert_eq!(mgr.scenarios.len(), 1);
        assert!(mgr.remove("Best Case"));
        assert_eq!(mgr.scenarios.len(), 0);
    }

    #[test]
    fn test_scenario_apply() {
        let mut mgr = ScenarioManager::new();
        mgr.add(Scenario {
            name: "Test".into(),
            comment: "".into(),
            cells: vec![
                ScenarioCell {
                    row: 0,
                    col: 0,
                    value: "42".into(),
                },
                ScenarioCell {
                    row: 1,
                    col: 0,
                    value: "Hello".into(),
                },
            ],
        });

        let mut sheet = Sheet::new("Sheet1");
        mgr.apply("Test", &mut sheet).unwrap();
        assert_eq!(sheet.cell_value(0, 0), Some("42".into()));
        assert_eq!(sheet.cell_value(1, 0), Some("Hello".into()));
        assert_eq!(mgr.active_scenario, Some("Test".to_string()));
    }

    #[test]
    fn test_scenario_create_from_sheet() {
        let mut mgr = ScenarioManager::new();
        let mut sheet = Sheet::new("Sheet1");
        sheet.set_cell_value(0, 0, "10".into());
        sheet.set_cell_value(1, 1, "20".into());

        mgr.create_from_sheet("Scenario1", "Test", &sheet, &[(0, 0), (1, 1)]);
        let s = mgr.get("Scenario1").unwrap();
        assert_eq!(s.cells.len(), 2);
        assert_eq!(s.cells[0].value, "10");
    }

    #[test]
    fn test_scenario_list() {
        let mut mgr = ScenarioManager::new();
        mgr.add(Scenario {
            name: "A".into(),
            comment: "Comment A".into(),
            cells: vec![],
        });
        mgr.add(Scenario {
            name: "B".into(),
            comment: "Comment B".into(),
            cells: vec![],
        });
        let list = mgr.list();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_scenario_summary() {
        let mut mgr = ScenarioManager::new();
        mgr.add(Scenario {
            name: "Test".into(),
            comment: "Test comment".into(),
            cells: vec![ScenarioCell {
                row: 0,
                col: 0,
                value: "100".into(),
            }],
        });
        let summary = mgr.summary();
        assert!(summary.contains("Test"));
        assert!(summary.contains("Test comment"));
        assert!(summary.contains("100"));
    }

    #[test]
    fn test_scenario_replace_same_name() {
        let mut mgr = ScenarioManager::new();
        mgr.add(Scenario {
            name: "Test".into(),
            comment: "Original".into(),
            cells: vec![],
        });
        mgr.add(Scenario {
            name: "Test".into(),
            comment: "Updated".into(),
            cells: vec![],
        });
        assert_eq!(mgr.scenarios.len(), 1);
        assert_eq!(mgr.get("Test").unwrap().comment, "Updated");
    }

    #[test]
    fn test_scenario_apply_nonexistent() {
        let mut mgr = ScenarioManager::new();
        let mut sheet = Sheet::new("Sheet1");
        let result = mgr.apply("Nonexistent", &mut sheet);
        assert!(result.is_err());
    }

    // --- Comment Tests ---

    #[test]
    fn test_comment_add_get() {
        let mut mgr = CommentManager::new();
        mgr.add(0, 0, "This is a comment", "Alice");
        let comment = mgr.get(0, 0).unwrap();
        assert_eq!(comment.text, "This is a comment");
        assert_eq!(comment.author, "Alice");
        assert!(!comment.visible);
    }

    #[test]
    fn test_comment_remove() {
        let mut mgr = CommentManager::new();
        mgr.add(0, 0, "Test", "Bob");
        assert!(mgr.remove(0, 0));
        assert!(mgr.get(0, 0).is_none());
        assert!(!mgr.remove(0, 0));
    }

    #[test]
    fn test_comment_update_text() {
        let mut mgr = CommentManager::new();
        mgr.add(0, 0, "Original", "Bob");
        assert!(mgr.update_text(0, 0, "Updated"));
        assert_eq!(mgr.get(0, 0).unwrap().text, "Updated");
    }

    #[test]
    fn test_comment_visibility() {
        let mut mgr = CommentManager::new();
        mgr.add(0, 0, "Test", "Bob");
        mgr.set_visible(0, 0, true);
        assert!(mgr.get(0, 0).unwrap().visible);
        mgr.toggle_visible(0, 0);
        assert!(!mgr.get(0, 0).unwrap().visible);
    }

    #[test]
    fn test_comment_list() {
        let mut mgr = CommentManager::new();
        mgr.add(0, 0, "A", "X");
        mgr.add(1, 1, "B", "Y");
        assert_eq!(mgr.count(), 2);
        assert_eq!(mgr.list().len(), 2);
    }

    #[test]
    fn test_comment_clear() {
        let mut mgr = CommentManager::new();
        mgr.add(0, 0, "A", "X");
        mgr.add(1, 1, "B", "Y");
        mgr.clear();
        assert_eq!(mgr.count(), 0);
    }

    #[test]
    fn test_comment_iter() {
        let mut mgr = CommentManager::new();
        mgr.add(0, 0, "A", "X");
        mgr.add(1, 1, "B", "Y");
        let count = mgr.iter().count();
        assert_eq!(count, 2);
    }
}
