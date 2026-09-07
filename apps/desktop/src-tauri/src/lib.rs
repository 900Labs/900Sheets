mod export_preflight;

use export_preflight::{preflight_workbook, ExportKind, ExportPreflight};
use sheets_advanced::{
    CellComment, CellLockManager, GoalSeekConfig, GoalSeekResult, ProtectionAction, Scenario,
    SheetProtection,
};
use sheets_chart::{ChartConfig, ChartObject, ChartResult};
use sheets_core::format::CellFormat;
use sheets_core::number_format::NumberFormat;
use sheets_core::workbook::Workbook;
use sheets_core::{CellValue, Sheet};
use sheets_formula::dependency::{CellKey, DependencyGraph};
use sheets_formula::evaluator::{CellProvider, Evaluator, Value};
use sheets_formula::parser::Parser;
use sheets_i18n::{Locale, NavigationDirection, TranslationKey, TranslationProvider};
use sheets_pivot::{PivotConfig, PivotResult};
use sheets_print::{PrintConfig, PrintPreview};
use sheets_validation::{ConditionalFormat, DataValidation, ValidationRule};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};

struct AppState {
    workbook: Mutex<Workbook>,
    dep_graphs: Mutex<DependencyGraph>,
    protections: Mutex<Vec<SheetProtection>>,
    cell_locks: Mutex<Vec<CellLockManager>>,
    history: Mutex<TransactionHistory>,
}

#[derive(Clone, Default)]
struct SheetComments {
    comments: HashMap<(u32, u32, u32), CellComment>,
}

#[derive(Clone)]
struct CellDelta {
    sheet_stable_id: u64,
    row: u32,
    col: u32,
    before: Option<CellValue>,
    after: Option<CellValue>,
}

#[derive(Clone)]
struct FormatDelta {
    sheet_stable_id: u64,
    row: u32,
    col: u32,
    before: Option<CellFormat>,
    after: Option<CellFormat>,
}

#[derive(Clone)]
enum WorkbookDelta {
    Cells(Vec<CellDelta>),
    Formats(Vec<FormatDelta>),
    AddedSheet {
        index: usize,
        sheet: Sheet,
    },
    DeletedSheet {
        index: usize,
        sheet: Sheet,
    },
    RenamedSheet {
        sheet_stable_id: u64,
        before: String,
        after: String,
    },
}

#[derive(Clone)]
struct WorkbookTransaction {
    operations: Vec<WorkbookDelta>,
    metadata_before: serde_json::Value,
    metadata_after: serde_json::Value,
    backend_state_before: serde_json::Value,
    backend_state_after: serde_json::Value,
    serialized_bytes: usize,
}

struct PendingTransaction {
    metadata_before: serde_json::Value,
    workbook: Workbook,
    dep_graph: DependencyGraph,
    protections: Vec<SheetProtection>,
    cell_locks: Vec<CellLockManager>,
    comments: SheetComments,
}

#[derive(Default)]
struct TransactionHistory {
    undo: Vec<WorkbookTransaction>,
    redo: Vec<WorkbookTransaction>,
    pending: Option<PendingTransaction>,
}

const MAX_TRANSACTION_HISTORY: usize = 100;
const MAX_TRANSACTION_CELLS: usize = 200_000;
const MAX_TRANSACTION_BYTES: usize = 32 * 1024 * 1024;
const MAX_HISTORY_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone)]
struct RecoveryStore {
    root: PathBuf,
    io_lock: Arc<Mutex<()>>,
    #[cfg(test)]
    fail_before_rename: Arc<std::sync::atomic::AtomicBool>,
    #[cfg(test)]
    fail_cleanup_after_retire: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Clone)]
struct NativeBackupStore {
    root: PathBuf,
    io_lock: Arc<Mutex<()>>,
    #[cfg(test)]
    fail_before_replace: Arc<std::sync::atomic::AtomicBool>,
    #[cfg(test)]
    fail_rotation_cleanup: Arc<std::sync::atomic::AtomicBool>,
}

static PRIVATE_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const MAX_NATIVE_BACKUPS_PER_DOCUMENT: usize = 5;

#[derive(Clone, serde::Deserialize, serde::Serialize)]
struct RecoveryEntry {
    id: String,
    modified_millis: u128,
    size_bytes: u64,
}

#[derive(serde::Serialize)]
struct RecoveryInspection {
    entry: RecoveryEntry,
    sheets: Vec<SheetInfo>,
    metadata: serde_json::Value,
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
struct NativeBackupEntry {
    id: String,
    document_id: String,
    document_name: String,
    created_millis: u128,
    size_bytes: u64,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct NativeBackupEnvelope {
    schema_version: u32,
    entry: NativeBackupEntry,
    native_workbook: String,
}

#[derive(serde::Serialize)]
struct NativeBackupInspection {
    entry: NativeBackupEntry,
    sheets: Vec<SheetInfo>,
    metadata: serde_json::Value,
}

#[derive(serde::Serialize)]
struct NativeBackupWriteResult {
    entry: NativeBackupEntry,
    #[serde(skip_serializing_if = "Option::is_none")]
    rotation_warning: Option<String>,
}

#[derive(serde::Serialize)]
struct NativeSaveResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    backup: Option<NativeBackupEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    backup_warning: Option<String>,
}

#[derive(serde::Serialize)]
struct TransactionStatus {
    can_undo: bool,
    can_redo: bool,
}

#[derive(serde::Serialize)]
struct TransactionRestoreResult {
    sheets: Vec<SheetInfo>,
    metadata: serde_json::Value,
    can_undo: bool,
    can_redo: bool,
}

impl SheetComments {
    fn clear(&mut self) {
        self.comments.clear();
    }
}

const MAX_EVALUATION_DEPTH: usize = 128;
const MAX_EVALUATION_STEPS: usize = 1_000_000;
const MAX_EVALUATION_VALUE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Default)]
struct EvaluationSession {
    cached: RefCell<HashMap<CellKey, CachedValue>>,
    visiting: RefCell<HashSet<CellKey>>,
    depth: Cell<usize>,
    steps: Cell<usize>,
    value_bytes: Cell<usize>,
}

struct CachedValue {
    value: Value,
    payload_bytes: usize,
}

impl EvaluationSession {
    fn reserve_value_bytes(&self, bytes: usize) -> Result<(), sheets_formula::error::FormulaError> {
        let total = self.value_bytes.get().saturating_add(bytes);
        if total > MAX_EVALUATION_VALUE_BYTES {
            return Err(sheets_formula::error::FormulaError::EvalError(format!(
                "Formula calculation exceeds safe value-copy limit of {MAX_EVALUATION_VALUE_BYTES} bytes"
            )));
        }
        self.value_bytes.set(total);
        Ok(())
    }

    fn consume_step(&self) -> Result<(), sheets_formula::error::FormulaError> {
        if self.steps.get() >= MAX_EVALUATION_STEPS {
            return Err(sheets_formula::error::FormulaError::EvalError(format!(
                "Formula calculation exceeds safe work limit of {MAX_EVALUATION_STEPS} steps"
            )));
        }
        self.steps.set(self.steps.get() + 1);
        Ok(())
    }
}

fn value_payload_bytes(value: &Value) -> usize {
    match value {
        Value::String(text) => text.len(),
        Value::Array(values) => values.iter().fold(
            values.len().saturating_mul(std::mem::size_of::<Value>()),
            |bytes, value| bytes.saturating_add(value_payload_bytes(value)),
        ),
        _ => 0,
    }
}

struct TauriProvider<'a> {
    workbook: &'a Workbook,
    current_sheet_id: u64,
    session: &'a EvaluationSession,
}

impl<'a> CellProvider for TauriProvider<'a> {
    fn enter_evaluation(&self) -> Result<(), sheets_formula::error::FormulaError> {
        self.session.consume_step()?;
        if self.session.depth.get() >= MAX_EVALUATION_DEPTH {
            return Err(sheets_formula::error::FormulaError::EvalError(format!(
                "Formula calculation exceeds safe nesting limit of {MAX_EVALUATION_DEPTH}"
            )));
        }
        self.session.depth.set(self.session.depth.get() + 1);
        Ok(())
    }

    fn leave_evaluation(&self) {
        self.session.depth.set(self.session.depth.get() - 1);
    }

    fn get_cell(&self, row: u32, col: u32) -> Value {
        self.get_cell_by_sheet_id(self.current_sheet_id, row, col)
    }

    fn get_cell_on_sheet(&self, sheet: &str, row: u32, col: u32) -> Value {
        let Some(index) = self.workbook.sheet_index_by_name(sheet) else {
            return Value::Error(sheets_formula::error::FormulaError::RefError(format!(
                "Sheet '{sheet}' was not found"
            )));
        };
        let stable_id = self
            .workbook
            .sheet(index)
            .expect("resolved sheet")
            .stable_id();
        self.get_cell_by_sheet_id(stable_id, row, col)
    }

    fn get_raw(&self, row: u32, col: u32) -> String {
        self.workbook
            .sheet_by_stable_id(self.current_sheet_id)
            .and_then(|sheet| sheet.cell_value(row, col))
            .unwrap_or_default()
    }

    fn get_raw_on_sheet(&self, sheet: &str, row: u32, col: u32) -> String {
        self.workbook
            .sheet_index_by_name(sheet)
            .and_then(|index| self.workbook.sheet(index))
            .and_then(|sheet| sheet.cell_value(row, col))
            .unwrap_or_default()
    }
}

impl TauriProvider<'_> {
    fn cached_value(&self, key: CellKey) -> Option<Value> {
        if let Some(cached) = self.session.cached.borrow().get(&key) {
            if let Err(error) = self.session.reserve_value_bytes(cached.payload_bytes) {
                return Some(Value::Error(error));
            }
            return Some(cached.value.clone());
        }
        None
    }

    fn snapshot_cell(&self, row: u32, col: u32) -> Value {
        // Preserve already calculated results when the remaining calculation
        // budget is exhausted. Nested formula reads still consume work below.
        self.cached_value(CellKey::new(self.current_sheet_id, row, col))
            .unwrap_or_else(|| self.get_cell(row, col))
    }

    fn get_cell_by_sheet_id(&self, sheet_id: u64, row: u32, col: u32) -> Value {
        if let Err(error) = self.session.consume_step() {
            return Value::Error(error);
        }
        let key = CellKey::new(sheet_id, row, col);
        if let Some(value) = self.cached_value(key) {
            return value;
        }
        let Some(sheet) = self.workbook.sheet_by_stable_id(sheet_id) else {
            return Value::Error(sheets_formula::error::FormulaError::RefError(
                "Sheet was not found".into(),
            ));
        };
        if let Some(cell) = sheet.cell(row, col) {
            match cell.cell_type {
                sheets_core::cell::CellType::Number => {
                    Value::Number(cell.as_number().unwrap_or(0.0))
                }
                sheets_core::cell::CellType::Text => {
                    if let Err(error) = self.session.reserve_value_bytes(cell.raw.len()) {
                        return Value::Error(error);
                    }
                    Value::String(cell.raw.clone())
                }
                sheets_core::cell::CellType::Boolean => {
                    Value::Boolean(cell.raw.eq_ignore_ascii_case("true"))
                }
                sheets_core::cell::CellType::Formula => {
                    if !self.session.visiting.borrow_mut().insert(key) {
                        return Value::Error(
                            sheets_formula::error::FormulaError::CircularReference,
                        );
                    }
                    let evaluator = Evaluator::new();
                    let value = if let Ok(expr) = Parser::parse_formula(&cell.raw) {
                        let provider = TauriProvider {
                            workbook: self.workbook,
                            current_sheet_id: sheet_id,
                            session: self.session,
                        };
                        evaluator.evaluate(&expr, &provider)
                    } else {
                        Value::Error(sheets_formula::error::FormulaError::ParseError(
                            "Invalid formula".into(),
                        ))
                    };
                    self.session.visiting.borrow_mut().remove(&key);
                    // A budget failure can depend on the current call depth;
                    // it must not poison another independently evaluated cell.
                    if !value.is_error() {
                        // Account for both the retained cache copy and the
                        // returned payload before cloning large strings/arrays.
                        let payload_bytes = value_payload_bytes(&value);
                        if let Err(error) = self
                            .session
                            .reserve_value_bytes(payload_bytes.saturating_mul(2))
                        {
                            return Value::Error(error);
                        }
                        self.session.cached.borrow_mut().insert(
                            key,
                            CachedValue {
                                value: value.clone(),
                                payload_bytes,
                            },
                        );
                    }
                    value
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
    numeric_value: Option<f64>,
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

#[cfg(test)]
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
    *state.history.lock().map_err(|error| error.to_string())? = TransactionHistory::default();
    Ok(())
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

fn ensure_regular_file_or_missing(path: &Path, label: &str) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(format!("{label} must be a regular file"))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn unique_sibling_temporary_path(path: &Path) -> Result<PathBuf, String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Destination must have a parent directory".to_string())?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Destination file name must be valid UTF-8".to_string())?;
    let sequence = PRIVATE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    Ok(parent.join(format!(
        ".{file_name}.{}.{}.{}.tmp",
        std::process::id(),
        nanos,
        sequence
    )))
}

#[cfg(windows)]
fn replace_file_platform(temporary: &Path, target: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let source: Vec<u16> = temporary
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let destination: Vec<u16> = target
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file_platform(temporary: &Path, target: &Path) -> Result<(), String> {
    std::fs::rename(temporary, target).map_err(|error| error.to_string())
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Destination must have a parent directory".to_string())?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    use std::os::unix::fs::OpenOptionsExt;
    options.custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW);
    options
        .open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn write_atomic_with_faults(
    path: &Path,
    data: &[u8],
    interrupt_before_replace: bool,
    fail_parent_sync_after_replace: bool,
) -> Result<Option<String>, String> {
    use std::io::Write;
    ensure_regular_file_or_missing(path, "Destination")?;
    let temporary = unique_sibling_temporary_path(path)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|error| error.to_string())?;
    let result = (|| -> Result<Option<String>, String> {
        file.write_all(data).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        if interrupt_before_replace {
            return Err("injected interruption before atomic replacement".into());
        }
        replace_file_platform(&temporary, path)?;
        let parent_sync = if fail_parent_sync_after_replace {
            Err("injected parent-directory sync failure".to_string())
        } else {
            sync_parent_directory(path)
        };
        Ok(parent_sync.err().map(|error| {
            format!(
                "The file was replaced, but directory durability could not be confirmed: {error}"
            )
        }))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn write_atomic_with_interruption(
    path: &Path,
    data: &[u8],
    interrupt_before_replace: bool,
) -> Result<(), String> {
    write_atomic_with_faults(path, data, interrupt_before_replace, false).map(|_| ())
}

fn write_atomic(path: &Path, data: &[u8]) -> Result<(), String> {
    write_atomic_with_interruption(path, data, false)
}

fn combine_warnings(left: Option<String>, right: Option<String>) -> Option<String> {
    match (left, right) {
        (Some(left), Some(right)) => Some(format!("{left} {right}")),
        (Some(warning), None) | (None, Some(warning)) => Some(warning),
        (None, None) => None,
    }
}

impl RecoveryStore {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            io_lock: Arc::new(Mutex::new(())),
            #[cfg(test)]
            fail_before_rename: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            #[cfg(test)]
            fail_cleanup_after_retire: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn validate_id(id: &str) -> Result<(), String> {
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err("Recovery ID must contain only letters, numbers, '-' or '_'".into());
        }
        Ok(())
    }

    fn path(&self, id: &str) -> Result<PathBuf, String> {
        Self::validate_id(id)?;
        Ok(self.root.join(format!("{id}.900sheets.recovery")))
    }

    fn cleanup_path(&self, id: &str) -> Result<PathBuf, String> {
        Self::validate_id(id)?;
        Ok(self.root.join(format!("{id}.900sheets.cleanup-pending")))
    }

    fn ensure_root(&self) -> Result<(), String> {
        match std::fs::symlink_metadata(&self.root) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err("Recovery directory must be a real directory".into());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&self.root).map_err(|error| error.to_string())?;
            }
            Err(error) => return Err(error.to_string()),
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.root, std::fs::Permissions::from_mode(0o700))
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn ensure_regular_target_or_missing(path: &Path) -> Result<(), String> {
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                Err("Recovery target must be a regular file".into())
            }
            Ok(_) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }

    fn unique_temporary_path(&self, id: &str) -> PathBuf {
        let sequence = PRIVATE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        self.root.join(format!(
            ".{id}.{}.{}.{}.tmp",
            std::process::id(),
            nanos,
            sequence
        ))
    }

    #[cfg(unix)]
    fn sync_root(&self) -> Result<(), String> {
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW);
        options
            .open(&self.root)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| error.to_string())
    }

    #[cfg(windows)]
    fn sync_root(&self) -> Result<(), String> {
        // Windows has no portable directory-fsync equivalent. The recovery file is
        // flushed before replacement and MoveFileExW uses MOVEFILE_WRITE_THROUGH.
        Ok(())
    }

    #[cfg(not(any(unix, windows)))]
    fn sync_root(&self) -> Result<(), String> {
        Ok(())
    }

    #[cfg(windows)]
    fn replace_file(temporary: &Path, target: &Path) -> Result<(), String> {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };
        let source: Vec<u16> = temporary
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let destination: Vec<u16> = target
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let result = unsafe {
            MoveFileExW(
                source.as_ptr(),
                destination.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if result == 0 {
            Err(std::io::Error::last_os_error().to_string())
        } else {
            Ok(())
        }
    }

    #[cfg(not(windows))]
    fn replace_file(temporary: &Path, target: &Path) -> Result<(), String> {
        std::fs::rename(temporary, target).map_err(|error| error.to_string())
    }

    fn write(&self, id: &str, data: &[u8]) -> Result<(), String> {
        use std::io::Write;
        let _guard = self.io_lock.lock().map_err(|error| error.to_string())?;
        self.ensure_root()?;
        let target = self.path(id)?;
        Self::ensure_regular_target_or_missing(&target)?;
        let temporary = self.unique_temporary_path(id);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        let write_result = (|| -> Result<(), String> {
            file.write_all(data).map_err(|error| error.to_string())?;
            file.sync_all().map_err(|error| error.to_string())?;
            drop(file);
            #[cfg(test)]
            if self.fail_before_rename.swap(false, Ordering::SeqCst) {
                return Err("injected recovery write failure".into());
            }
            Self::replace_file(&temporary, &target)?;
            self.sync_root()?;
            Ok(())
        })();
        if write_result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        write_result
    }

    fn read(&self, id: &str) -> Result<String, String> {
        use std::io::Read;
        let _guard = self.io_lock.lock().map_err(|error| error.to_string())?;
        self.ensure_root()?;
        let path = self.path(id)?;
        Self::ensure_regular_target_or_missing(&path)?;
        let mut contents = String::new();
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        options
            .open(path)
            .and_then(|mut file| file.read_to_string(&mut contents))
            .map_err(|error| error.to_string())?;
        Ok(contents)
    }

    fn discard(&self, id: &str) -> Result<(), String> {
        let _guard = self.io_lock.lock().map_err(|error| error.to_string())?;
        self.ensure_root()?;
        let source = self.path(id)?;
        let retired = self.cleanup_path(id)?;
        Self::ensure_regular_target_or_missing(&source)?;
        Self::ensure_regular_target_or_missing(&retired)?;
        if source.exists() {
            Self::replace_file(&source, &retired)?;
            self.sync_root()?;
        }
        if !retired.exists() {
            return Ok(());
        }
        #[cfg(test)]
        if self.fail_cleanup_after_retire.swap(false, Ordering::SeqCst) {
            return Err("injected recovery cleanup failure".into());
        }
        std::fs::remove_file(retired).map_err(|error| error.to_string())?;
        self.sync_root()
    }

    fn quarantine(&self, id: &str) -> Result<(), String> {
        let _guard = self.io_lock.lock().map_err(|error| error.to_string())?;
        self.ensure_root()?;
        let source = self.path(id)?;
        Self::ensure_regular_target_or_missing(&source)?;
        let sequence = PRIVATE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let target = self.root.join(format!("{id}.900sheets.corrupt.{sequence}"));
        Self::replace_file(&source, &target)?;
        self.sync_root()
    }

    fn list(&self) -> Result<Vec<RecoveryEntry>, String> {
        let _guard = self.io_lock.lock().map_err(|error| error.to_string())?;
        self.ensure_root()?;
        let entries = match std::fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.to_string()),
        };
        let mut result = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some(id) = name.strip_suffix(".900sheets.recovery") else {
                continue;
            };
            let metadata = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                continue;
            }
            if Self::validate_id(id).is_err() {
                continue;
            }
            let modified_millis = metadata
                .modified()
                .ok()
                .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis())
                .unwrap_or_default();
            result.push(RecoveryEntry {
                id: id.to_string(),
                modified_millis,
                size_bytes: metadata.len(),
            });
        }
        result.sort_by(|left, right| right.modified_millis.cmp(&left.modified_millis));
        Ok(result)
    }

    fn describe(&self, id: &str) -> Result<RecoveryEntry, String> {
        let _guard = self.io_lock.lock().map_err(|error| error.to_string())?;
        self.ensure_root()?;
        let path = self.path(id)?;
        Self::ensure_regular_target_or_missing(&path)?;
        let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
        let modified_millis = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        Ok(RecoveryEntry {
            id: id.to_string(),
            modified_millis,
            size_bytes: metadata.len(),
        })
    }
}

impl NativeBackupStore {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            io_lock: Arc::new(Mutex::new(())),
            #[cfg(test)]
            fail_before_replace: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            #[cfg(test)]
            fail_rotation_cleanup: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn ensure_root(&self) -> Result<(), String> {
        match std::fs::symlink_metadata(&self.root) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err("Backup directory must be a real directory".into());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&self.root).map_err(|error| error.to_string())?;
            }
            Err(error) => return Err(error.to_string()),
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.root, std::fs::Permissions::from_mode(0o700))
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn validate_id(id: &str) -> Result<(), String> {
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err("Backup ID must contain only letters, numbers, '-' or '_'".into());
        }
        Ok(())
    }

    fn path(&self, id: &str) -> Result<PathBuf, String> {
        Self::validate_id(id)?;
        Ok(self.root.join(format!("{id}.900sheets.backup")))
    }

    fn document_id(path: &Path) -> String {
        // FNV-1a is used only as an opaque, stable grouping key. No source path is
        // persisted in the private backup directory.
        let mut hash = 0xcbf29ce484222325_u64;
        for byte in path.as_os_str().to_string_lossy().as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        format!("{hash:016x}")
    }

    fn document_name(path: &Path) -> String {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Workbook.900sheets");
        name.chars().take(128).collect()
    }

    fn read_envelope_unlocked(&self, id: &str) -> Result<NativeBackupEnvelope, String> {
        use std::io::Read;
        let path = self.path(id)?;
        ensure_regular_file_or_missing(&path, "Backup")?;
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let mut contents = String::new();
        options
            .open(path)
            .and_then(|mut file| file.read_to_string(&mut contents))
            .map_err(|error| error.to_string())?;
        let envelope: NativeBackupEnvelope =
            serde_json::from_str(&contents).map_err(|error| error.to_string())?;
        if envelope.schema_version != 1 || envelope.entry.id != id {
            return Err("Backup envelope is invalid".into());
        }
        sheets_json::import_native_workbook_with_metadata(&envelope.native_workbook)
            .map_err(|error| format!("Backup workbook is invalid: {error}"))?;
        Ok(envelope)
    }

    fn list_unlocked(&self) -> Result<Vec<NativeBackupEntry>, String> {
        let mut result = Vec::new();
        for entry in std::fs::read_dir(&self.root).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some(id) = name.strip_suffix(".900sheets.backup") else {
                continue;
            };
            if Self::validate_id(id).is_err() {
                continue;
            }
            if let Ok(envelope) = self.read_envelope_unlocked(id) {
                result.push(envelope.entry);
            }
        }
        result.sort_by(|left, right| {
            right
                .created_millis
                .cmp(&left.created_millis)
                .then_with(|| right.id.cmp(&left.id))
        });
        Ok(result)
    }

    fn list(&self) -> Result<Vec<NativeBackupEntry>, String> {
        let _guard = self.io_lock.lock().map_err(|error| error.to_string())?;
        self.ensure_root()?;
        self.list_unlocked()
    }

    fn read(&self, id: &str) -> Result<NativeBackupEnvelope, String> {
        let _guard = self.io_lock.lock().map_err(|error| error.to_string())?;
        self.ensure_root()?;
        self.read_envelope_unlocked(id)
    }

    fn delete(&self, id: &str) -> Result<(), String> {
        let _guard = self.io_lock.lock().map_err(|error| error.to_string())?;
        self.ensure_root()?;
        let path = self.path(id)?;
        ensure_regular_file_or_missing(&path, "Backup")?;
        match std::fs::remove_file(&path) {
            Ok(()) => sync_parent_directory(&path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }

    fn write_for_path(
        &self,
        source_path: &Path,
        data: &str,
    ) -> Result<NativeBackupWriteResult, String> {
        // Reject an invalid native payload before it can enter backup rotation.
        sheets_json::import_native_workbook_with_metadata(data)
            .map_err(|error| format!("Cannot back up an invalid native workbook: {error}"))?;
        let _guard = self.io_lock.lock().map_err(|error| error.to_string())?;
        self.ensure_root()?;
        let document_id = Self::document_id(source_path);
        let created_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        let sequence = PRIVATE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let id = format!("{document_id}_{created_millis}_{sequence:020}");
        let entry = NativeBackupEntry {
            id: id.clone(),
            document_id: document_id.clone(),
            document_name: Self::document_name(source_path),
            created_millis,
            size_bytes: data.len() as u64,
        };
        let envelope = NativeBackupEnvelope {
            schema_version: 1,
            entry: entry.clone(),
            native_workbook: data.to_string(),
        };
        let encoded = serde_json::to_vec(&envelope).map_err(|error| error.to_string())?;
        let destination = self.path(&id)?;
        #[cfg(test)]
        let interrupt = self.fail_before_replace.swap(false, Ordering::SeqCst);
        #[cfg(not(test))]
        let interrupt = false;
        write_atomic_with_interruption(&destination, &encoded, interrupt)?;

        let mut rotation_warning = None;
        let mut document_backups: Vec<_> = self
            .list_unlocked()?
            .into_iter()
            .filter(|candidate| candidate.document_id == document_id)
            .collect();
        document_backups.sort_by(|left, right| {
            right
                .created_millis
                .cmp(&left.created_millis)
                .then_with(|| right.id.cmp(&left.id))
        });
        if document_backups.len() > MAX_NATIVE_BACKUPS_PER_DOCUMENT {
            #[cfg(test)]
            let inject_cleanup_failure = self.fail_rotation_cleanup.swap(false, Ordering::SeqCst);
            #[cfg(not(test))]
            let inject_cleanup_failure = false;
            if inject_cleanup_failure {
                rotation_warning = Some(
                    "The newest backup was kept, but older backups could not be rotated".into(),
                );
            } else {
                let mut removed_any = false;
                for stale in document_backups
                    .into_iter()
                    .skip(MAX_NATIVE_BACKUPS_PER_DOCUMENT)
                {
                    let stale_path = self.path(&stale.id)?;
                    match std::fs::remove_file(&stale_path) {
                        Ok(()) => removed_any = true,
                        Err(error) => {
                            rotation_warning = Some(format!(
                                "The newest backup was kept, but an older backup could not be removed: {error}"
                            ));
                            break;
                        }
                    }
                }
                if removed_any {
                    if let Err(error) = sync_parent_directory(&destination) {
                        rotation_warning = Some(format!(
                            "The newest backup was kept, but backup rotation could not be fully synchronized: {error}"
                        ));
                    }
                }
            }
        }
        Ok(NativeBackupWriteResult {
            entry,
            rotation_warning,
        })
    }
}

fn ensure_can_edit_cell(state: &AppState, sheet_id: u32, row: u32, col: u32) -> Result<(), String> {
    with_pending_mutation(state, |pending| {
        ensure_pending_cell_edit(pending, sheet_id, row, col)
    })
}

fn ensure_pending_cell_edit(
    pending: &PendingTransaction,
    sheet_id: u32,
    row: u32,
    col: u32,
) -> Result<(), String> {
    if let Some(protection) = pending.protections.get(sheet_id as usize) {
        if protection.protected {
            let cell_locked = pending
                .cell_locks
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
    with_pending_mutation(state, |pending| {
        if let Some(protection) = pending.protections.get(sheet_id as usize) {
            if protection.protected {
                return Err(format!("Sheet {} is protected", sheet_id));
            }
        }
        Ok(())
    })
}

fn ensure_can_perform_protected_action(
    state: &AppState,
    sheet_id: u32,
    action: ProtectionAction,
) -> Result<(), String> {
    with_pending_mutation(state, |pending| {
        if let Some(protection) = pending.protections.get(sheet_id as usize) {
            if !protection.can_perform(action) {
                return Err(format!("Sheet {} is protected for {:?}", sheet_id, action));
            }
        }
        Ok(())
    })
}

fn ensure_can_perform_live(
    state: &AppState,
    sheet_id: u32,
    action: ProtectionAction,
) -> Result<(), String> {
    let protections = state
        .protections
        .lock()
        .map_err(|error| error.to_string())?;
    if let Some(protection) = protections.get(sheet_id as usize) {
        if !protection.can_perform(action) {
            return Err(format!("Sheet {} is protected for {:?}", sheet_id, action));
        }
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
    with_pending_mutation(state, |pending| {
        let mut candidate = pending.workbook.clone();
        let sheet = candidate
            .sheet_mut(sheet_id as usize)
            .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
        if !sheet.in_bounds(row, col) {
            return Ok(());
        }
        sheet.set_cell_value(row, col, value);
        let graph = dependency_graphs_for_workbook(&candidate)?;
        pending.workbook = candidate;
        pending.dep_graph = graph;
        Ok(())
    })
}

fn clear_cell_in_workbook(
    state: &AppState,
    sheet_id: u32,
    row: u32,
    col: u32,
) -> Result<(), String> {
    with_pending_mutation(state, |pending| {
        let mut candidate = pending.workbook.clone();
        candidate
            .sheet_mut(sheet_id as usize)
            .ok_or_else(|| format!("Sheet {} not found", sheet_id))?
            .clear_value(row, col);
        let graph = dependency_graphs_for_workbook(&candidate)?;
        pending.workbook = candidate;
        pending.dep_graph = graph;
        Ok(())
    })
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

#[cfg(test)]
fn dependency_graph_for_sheet(
    sheet: &sheets_core::sheet::Sheet,
) -> Result<DependencyGraph, String> {
    let mut graph = DependencyGraph::new();
    for ((row, col), cell) in sheet.iter_cells() {
        if cell.is_formula() {
            Parser::parse_formula(&cell.raw).map_err(|error| error.to_string())?;
            graph.set_formula_on_sheet(sheet.stable_id(), row, col, &cell.raw, |_| None)?;
        }
    }
    Ok(graph)
}

fn dependency_graphs_for_workbook(workbook: &Workbook) -> Result<DependencyGraph, String> {
    let mut graph = DependencyGraph::new();
    for sheet in workbook.sheets() {
        for ((row, col), cell) in sheet.iter_cells() {
            if cell.is_formula() {
                Parser::parse_formula(&cell.raw).map_err(|error| error.to_string())?;
                graph.set_formula_on_sheet(sheet.stable_id(), row, col, &cell.raw, |name| {
                    workbook
                        .sheet_index_by_name(name)
                        .and_then(|index| workbook.sheet(index))
                        .map(|sheet| sheet.stable_id())
                })?;
            }
        }
    }
    Ok(graph)
}

fn transaction_status(history: &TransactionHistory) -> TransactionStatus {
    TransactionStatus {
        can_undo: !history.undo.is_empty(),
        can_redo: !history.redo.is_empty(),
    }
}

#[tauri::command]
fn begin_workbook_transaction(
    metadata: serde_json::Value,
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<TransactionStatus, String> {
    begin_transaction(metadata, state.inner(), comments.inner())
}

fn begin_transaction(
    metadata: serde_json::Value,
    state: &AppState,
    comments: &Mutex<SheetComments>,
) -> Result<TransactionStatus, String> {
    let mut history = state.history.lock().map_err(|error| error.to_string())?;
    if history.pending.is_some() {
        return Err("A workbook transaction is already active".into());
    }
    let workbook = state
        .workbook
        .lock()
        .map_err(|error| error.to_string())?
        .clone();
    let dep_graph = state
        .dep_graphs
        .lock()
        .map_err(|error| error.to_string())?
        .clone();
    let protections = state
        .protections
        .lock()
        .map_err(|error| error.to_string())?
        .clone();
    let cell_locks = state
        .cell_locks
        .lock()
        .map_err(|error| error.to_string())?
        .clone();
    let comments = comments.lock().map_err(|error| error.to_string())?.clone();
    history.pending = Some(PendingTransaction {
        metadata_before: metadata,
        workbook,
        dep_graph,
        protections,
        cell_locks,
        comments,
    });
    Ok(transaction_status(&history))
}

#[tauri::command]
fn commit_workbook_transaction(
    metadata: serde_json::Value,
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<TransactionStatus, String> {
    commit_transaction(metadata, state.inner(), comments.inner())
}

fn commit_transaction(
    metadata: serde_json::Value,
    state: &AppState,
    comments: &Mutex<SheetComments>,
) -> Result<TransactionStatus, String> {
    let mut history = state.history.lock().map_err(|error| error.to_string())?;
    let Some(pending) = history.pending.as_mut() else {
        return Err("No workbook transaction is active".into());
    };
    pending.dep_graph = dependency_graphs_for_workbook(&pending.workbook)?;

    let mut workbook = state.workbook.lock().map_err(|error| error.to_string())?;
    let mut dep_graph = state.dep_graphs.lock().map_err(|error| error.to_string())?;
    let mut protections = state
        .protections
        .lock()
        .map_err(|error| error.to_string())?;
    let mut cell_locks = state.cell_locks.lock().map_err(|error| error.to_string())?;
    let mut live_comments = comments.lock().map_err(|error| error.to_string())?;
    let operations = workbook_deltas_between(&workbook, &pending.workbook);
    let backend_state_before = metadata_with_backend_sheet_states(
        &workbook,
        serde_json::json!({}),
        &live_comments,
        &protections,
        &cell_locks,
    )?;
    let backend_state_after = metadata_with_backend_sheet_states(
        &pending.workbook,
        serde_json::json!({}),
        &pending.comments,
        &pending.protections,
        &pending.cell_locks,
    )?;
    let _ = parse_backend_sheet_states(&pending.workbook, &backend_state_after)?;
    let mut transaction = WorkbookTransaction {
        operations,
        metadata_before: pending.metadata_before.clone(),
        metadata_after: metadata,
        backend_state_before,
        backend_state_after,
        serialized_bytes: 0,
    };
    let changed = !transaction.operations.is_empty()
        || transaction.metadata_before != transaction.metadata_after
        || transaction.backend_state_before != transaction.backend_state_after;
    if !changed {
        history.pending = None;
        return Ok(transaction_status(&history));
    }
    if transaction_coordinate_count(&transaction) > MAX_TRANSACTION_CELLS {
        return Err("Operation exceeds the bounded undo coordinate budget".into());
    }
    transaction.serialized_bytes = transaction_serialized_size(&transaction)?;
    if transaction.serialized_bytes > MAX_TRANSACTION_BYTES {
        return Err("Operation exceeds the bounded undo history budget".into());
    }

    let pending = history
        .pending
        .take()
        .expect("validated pending transaction");
    *workbook = pending.workbook;
    *dep_graph = pending.dep_graph;
    *protections = pending.protections;
    *cell_locks = pending.cell_locks;
    *live_comments = pending.comments;
    history.undo.push(transaction);
    history.redo.clear();
    evict_history_to_budget(&mut history.undo);
    Ok(transaction_status(&history))
}

#[tauri::command]
fn abort_workbook_transaction(
    state: State<AppState>,
    _comments: State<'_, Mutex<SheetComments>>,
) -> Result<TransactionStatus, String> {
    abort_transaction(state.inner())
}

fn abort_transaction(state: &AppState) -> Result<TransactionStatus, String> {
    let mut history = state.history.lock().map_err(|error| error.to_string())?;
    history.pending = None;
    Ok(transaction_status(&history))
}

fn cell_deltas_between_workbooks(before: &Workbook, after: &Workbook) -> Vec<CellDelta> {
    let mut deltas = Vec::new();
    for before_sheet in before.sheets() {
        let Some(after_sheet) = after.sheet_by_stable_id(before_sheet.stable_id()) else {
            continue;
        };
        let keys: HashSet<_> = before_sheet
            .iter_cells()
            .map(|(key, _)| key)
            .chain(after_sheet.iter_cells().map(|(key, _)| key))
            .collect();
        for (row, col) in keys {
            let before_value = before_sheet.cell(row, col).cloned();
            let after_value = after_sheet.cell(row, col).cloned();
            if before_value != after_value {
                deltas.push(CellDelta {
                    sheet_stable_id: before_sheet.stable_id(),
                    row,
                    col,
                    before: before_value,
                    after: after_value,
                });
            }
        }
    }
    deltas
}

fn format_deltas_between_sheets(before: &Sheet, after: &Sheet) -> Vec<FormatDelta> {
    let keys: HashSet<_> = before
        .iter_formats()
        .map(|(key, _)| key)
        .chain(after.iter_formats().map(|(key, _)| key))
        .collect();
    keys.into_iter()
        .filter_map(|(row, col)| {
            let before_value = before.get_format(row, col).cloned();
            let after_value = after.get_format(row, col).cloned();
            (before_value != after_value).then_some(FormatDelta {
                sheet_stable_id: before.stable_id(),
                row,
                col,
                before: before_value,
                after: after_value,
            })
        })
        .collect()
}

fn workbook_deltas_between(before: &Workbook, after: &Workbook) -> Vec<WorkbookDelta> {
    let mut operations = Vec::new();
    let cells = cell_deltas_between_workbooks(before, after);
    if !cells.is_empty() {
        operations.push(WorkbookDelta::Cells(cells));
    }
    let mut formats = Vec::new();
    for before_sheet in before.sheets() {
        if let Some(after_sheet) = after.sheet_by_stable_id(before_sheet.stable_id()) {
            formats.extend(format_deltas_between_sheets(before_sheet, after_sheet));
            if before_sheet.name() != after_sheet.name() {
                operations.push(WorkbookDelta::RenamedSheet {
                    sheet_stable_id: before_sheet.stable_id(),
                    before: before_sheet.name().to_string(),
                    after: after_sheet.name().to_string(),
                });
            }
        }
    }
    if !formats.is_empty() {
        operations.push(WorkbookDelta::Formats(formats));
    }
    let mut deleted: Vec<_> = before
        .sheets()
        .iter()
        .enumerate()
        .filter(|(_, sheet)| after.sheet_by_stable_id(sheet.stable_id()).is_none())
        .map(|(index, sheet)| (index, sheet.clone()))
        .collect();
    deleted.sort_by(|left, right| right.0.cmp(&left.0));
    operations.extend(
        deleted
            .into_iter()
            .map(|(index, sheet)| WorkbookDelta::DeletedSheet { index, sheet }),
    );
    operations.extend(
        after
            .sheets()
            .iter()
            .enumerate()
            .filter(|(_, sheet)| before.sheet_by_stable_id(sheet.stable_id()).is_none())
            .map(|(index, sheet)| WorkbookDelta::AddedSheet {
                index,
                sheet: sheet.clone(),
            }),
    );
    operations
}

fn transaction_coordinate_count(transaction: &WorkbookTransaction) -> usize {
    transaction
        .operations
        .iter()
        .map(|operation| match operation {
            WorkbookDelta::Cells(values) => values.len(),
            WorkbookDelta::Formats(values) => values.len(),
            WorkbookDelta::AddedSheet { sheet, .. } | WorkbookDelta::DeletedSheet { sheet, .. } => {
                sheet.cell_count().saturating_add(sheet.format_count())
            }
            WorkbookDelta::RenamedSheet { .. } => 0,
        })
        .sum()
}

fn sheet_history_value(sheet: &Sheet) -> serde_json::Value {
    let cells: Vec<_> = sheet
        .iter_cells()
        .map(|((row, col), value)| serde_json::json!([row, col, value]))
        .collect();
    let formats: Vec<_> = sheet
        .iter_formats()
        .map(|((row, col), format)| serde_json::json!([row, col, format]))
        .collect();
    serde_json::json!({
        "stable_id": sheet.stable_id(),
        "name": sheet.name(),
        "max_rows": 1_000_000u32,
        "max_cols": 16_384u32,
        "cells": cells,
        "formats": formats,
    })
}

fn transaction_serialized_size(transaction: &WorkbookTransaction) -> Result<usize, String> {
    let operations: Vec<_> = transaction
        .operations
        .iter()
        .map(|operation| match operation {
            WorkbookDelta::Cells(values) => serde_json::json!({
                "kind": "cells",
                "values": values.iter().map(|value| serde_json::json!({
                    "sheet": value.sheet_stable_id,
                    "row": value.row,
                    "col": value.col,
                    "before": value.before,
                    "after": value.after,
                })).collect::<Vec<_>>()
            }),
            WorkbookDelta::Formats(values) => serde_json::json!({
                "kind": "formats",
                "values": values.iter().map(|value| serde_json::json!({
                    "sheet": value.sheet_stable_id,
                    "row": value.row,
                    "col": value.col,
                    "before": value.before,
                    "after": value.after,
                })).collect::<Vec<_>>()
            }),
            WorkbookDelta::AddedSheet { index, sheet } => serde_json::json!({
                "kind": "added_sheet",
                "index": index,
                "sheet": sheet_history_value(sheet),
            }),
            WorkbookDelta::DeletedSheet { index, sheet } => serde_json::json!({
                "kind": "deleted_sheet",
                "index": index,
                "sheet": sheet_history_value(sheet),
            }),
            WorkbookDelta::RenamedSheet {
                sheet_stable_id,
                before,
                after,
            } => serde_json::json!({
                "kind": "renamed_sheet",
                "sheet": sheet_stable_id,
                "before": before,
                "after": after,
            }),
        })
        .collect();
    serde_json::to_vec(&serde_json::json!({
        "operations": operations,
        "metadata_before": transaction.metadata_before,
        "metadata_after": transaction.metadata_after,
        "backend_state_before": transaction.backend_state_before,
        "backend_state_after": transaction.backend_state_after,
    }))
    .map(|bytes| bytes.len())
    .map_err(|error| error.to_string())
}

fn evict_history_to_budget(history: &mut Vec<WorkbookTransaction>) {
    while history.len() > MAX_TRANSACTION_HISTORY
        || history
            .iter()
            .map(|transaction| transaction.serialized_bytes)
            .sum::<usize>()
            > MAX_HISTORY_BYTES
    {
        history.remove(0);
    }
}

fn with_pending_mutation<T>(
    state: &AppState,
    operation: impl FnOnce(&mut PendingTransaction) -> Result<T, String>,
) -> Result<T, String> {
    let mut history = state.history.lock().map_err(|error| error.to_string())?;
    let pending = history
        .pending
        .as_mut()
        .ok_or_else(|| "Mutating command requires an active workbook transaction".to_string())?;
    operation(pending)
}

fn apply_workbook_transaction(
    workbook: &mut Workbook,
    transaction: &WorkbookTransaction,
    forward: bool,
) -> Result<(), String> {
    let apply_delta = |workbook: &mut Workbook, delta: &WorkbookDelta, forward: bool| {
        apply_workbook_delta(workbook, delta, forward)
    };
    if forward {
        for delta in &transaction.operations {
            apply_delta(workbook, delta, true)?;
        }
    } else {
        for delta in transaction.operations.iter().rev() {
            apply_delta(workbook, delta, false)?;
        }
    }
    Ok(())
}

fn apply_workbook_delta(
    workbook: &mut Workbook,
    delta: &WorkbookDelta,
    forward: bool,
) -> Result<(), String> {
    match delta {
        WorkbookDelta::Cells(cells) => {
            for delta in cells {
                let index = workbook
                    .sheets()
                    .iter()
                    .position(|sheet| sheet.stable_id() == delta.sheet_stable_id)
                    .ok_or_else(|| "Undo target sheet no longer exists".to_string())?;
                let value = if forward { &delta.after } else { &delta.before };
                let sheet = workbook.sheet_mut(index).expect("resolved sheet");
                if let Some(value) = value {
                    sheet.set_cell(delta.row, delta.col, value.clone());
                } else {
                    sheet.clear_value(delta.row, delta.col);
                }
            }
        }
        WorkbookDelta::Formats(formats) => {
            for delta in formats {
                let index = workbook
                    .sheets()
                    .iter()
                    .position(|sheet| sheet.stable_id() == delta.sheet_stable_id)
                    .ok_or_else(|| "Undo target sheet no longer exists".to_string())?;
                let value = if forward { &delta.after } else { &delta.before };
                let sheet = workbook.sheet_mut(index).expect("resolved sheet");
                if let Some(value) = value {
                    sheet.set_format(delta.row, delta.col, value.clone());
                } else {
                    sheet.clear_format(delta.row, delta.col);
                }
            }
        }
        WorkbookDelta::AddedSheet { index, sheet } => {
            if forward {
                workbook
                    .insert_existing_sheet(*index, sheet.clone())
                    .map_err(|error| error.to_string())?;
            } else {
                let current = workbook
                    .sheets()
                    .iter()
                    .position(|candidate| candidate.stable_id() == sheet.stable_id())
                    .ok_or_else(|| "Added sheet no longer exists".to_string())?;
                if !workbook.delete_sheet(current) {
                    return Err("Unable to undo added sheet".into());
                }
            }
        }
        WorkbookDelta::DeletedSheet { index, sheet } => {
            if forward {
                let current = workbook
                    .sheets()
                    .iter()
                    .position(|candidate| candidate.stable_id() == sheet.stable_id())
                    .ok_or_else(|| "Deleted sheet no longer exists".to_string())?;
                if !workbook.delete_sheet(current) {
                    return Err("Unable to redo deleted sheet".into());
                }
            } else {
                workbook
                    .insert_existing_sheet(*index, sheet.clone())
                    .map_err(|error| error.to_string())?;
            }
        }
        WorkbookDelta::RenamedSheet {
            sheet_stable_id,
            before,
            after,
        } => {
            let index = workbook
                .sheets()
                .iter()
                .position(|sheet| sheet.stable_id() == *sheet_stable_id)
                .ok_or_else(|| "Renamed sheet no longer exists".to_string())?;
            workbook
                .rename_sheet(index, if forward { after } else { before })
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
fn undo_workbook_transaction(
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<TransactionRestoreResult, String> {
    restore_history_transaction(state.inner(), comments.inner(), false)
}

#[tauri::command]
fn redo_workbook_transaction(
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<TransactionRestoreResult, String> {
    restore_history_transaction(state.inner(), comments.inner(), true)
}

fn restore_history_transaction(
    state: &AppState,
    comments: &Mutex<SheetComments>,
    redo: bool,
) -> Result<TransactionRestoreResult, String> {
    let mut history = state.history.lock().map_err(|error| error.to_string())?;
    if history.pending.is_some() {
        return Err("Cannot undo while a transaction is active".into());
    }
    let transaction = if redo {
        history.redo.last()
    } else {
        history.undo.last()
    }
    .cloned()
    .ok_or_else(|| {
        if redo {
            "Nothing to redo"
        } else {
            "Nothing to undo"
        }
        .to_string()
    })?;
    let mut live_workbook = state.workbook.lock().map_err(|error| error.to_string())?;
    let mut live_graph = state.dep_graphs.lock().map_err(|error| error.to_string())?;
    let mut live_protections = state
        .protections
        .lock()
        .map_err(|error| error.to_string())?;
    let mut live_locks = state.cell_locks.lock().map_err(|error| error.to_string())?;
    let mut live_comments = comments.lock().map_err(|error| error.to_string())?;

    let mut candidate_workbook = live_workbook.clone();
    apply_workbook_transaction(&mut candidate_workbook, &transaction, redo)?;
    let candidate_graph = dependency_graphs_for_workbook(&candidate_workbook)?;
    let backend_state = if redo {
        &transaction.backend_state_after
    } else {
        &transaction.backend_state_before
    };
    let (candidate_comments, candidate_protections, candidate_locks) =
        parse_backend_sheet_states(&candidate_workbook, backend_state)?;
    let metadata = if redo {
        transaction.metadata_after.clone()
    } else {
        transaction.metadata_before.clone()
    };
    let sheets = sheet_infos(&candidate_workbook);

    *live_workbook = candidate_workbook;
    *live_graph = candidate_graph;
    *live_protections = candidate_protections;
    *live_locks = candidate_locks;
    *live_comments = candidate_comments;
    if redo {
        history.redo.pop();
        history.undo.push(transaction)
    } else {
        history.undo.pop();
        history.redo.push(transaction)
    }
    Ok(TransactionRestoreResult {
        sheets,
        metadata,
        can_undo: !history.undo.is_empty(),
        can_redo: !history.redo.is_empty(),
    })
}

fn replace_loaded_workbook(
    state: &AppState,
    comments: &Mutex<SheetComments>,
    imported: Workbook,
) -> Result<Vec<SheetInfo>, String> {
    let graphs = dependency_graphs_for_workbook(&imported)?;
    let infos = sheet_infos(&imported);
    let mut history = state.history.lock().map_err(|error| error.to_string())?;
    if history.pending.is_some() {
        return Err("Workbook replacement is a non-transactional session operation".into());
    }
    let mut workbook = state.workbook.lock().map_err(|error| error.to_string())?;
    let mut dep_graph = state.dep_graphs.lock().map_err(|error| error.to_string())?;
    let mut protections = state
        .protections
        .lock()
        .map_err(|error| error.to_string())?;
    let mut cell_locks = state.cell_locks.lock().map_err(|error| error.to_string())?;
    let mut live_comments = comments.lock().map_err(|error| error.to_string())?;
    *workbook = imported;
    *dep_graph = graphs;
    protections.clear();
    cell_locks.clear();
    live_comments.clear();
    *history = TransactionHistory::default();
    Ok(infos)
}

#[tauri::command]
fn batch_set_cells(
    sheet_id: u32,
    changes: Vec<CellChange>,
    state: State<AppState>,
) -> Result<(), String> {
    with_pending_mutation(state.inner(), |pending| {
        apply_cell_changes(pending, sheet_id, &changes)
    })
}

fn apply_cell_changes(
    pending: &mut PendingTransaction,
    sheet_id: u32,
    changes: &[CellChange],
) -> Result<(), String> {
    if changes.len() > MAX_TRANSACTION_CELLS {
        return Err(format!(
            "Cell edit exceeds safe transaction limit of {MAX_TRANSACTION_CELLS} coordinates"
        ));
    }
    let sheet = pending
        .workbook
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {sheet_id} not found"))?;
    for change in changes {
        if !sheet.in_bounds(change.row, change.col) {
            return Err("Cell edit is outside workbook limits".into());
        }
        ensure_pending_cell_edit(pending, sheet_id, change.row, change.col)?;
    }
    let mut candidate = pending.workbook.clone();
    let sheet = candidate
        .sheet_mut(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {sheet_id} not found"))?;
    for change in changes {
        if change.value.is_empty() {
            sheet.clear_value(change.row, change.col);
        } else {
            sheet.set_cell_value(change.row, change.col, change.value.clone());
        }
    }
    pending.dep_graph = dependency_graphs_for_workbook(&candidate)?;
    pending.workbook = candidate;
    Ok(())
}

#[tauri::command]
fn move_cells(
    sheet_id: u32,
    source_stable_sheet_id: String,
    source: Vec<CellChange>,
    changes: Vec<CellChange>,
    state: State<AppState>,
) -> Result<(), String> {
    move_cells_in_workbook(
        state.inner(),
        sheet_id,
        &source_stable_sheet_id,
        &source,
        &changes,
    )
}

fn move_cells_in_workbook(
    state: &AppState,
    sheet_id: u32,
    source_stable_sheet_id: &str,
    source: &[CellChange],
    changes: &[CellChange],
) -> Result<(), String> {
    with_pending_mutation(state, |pending| {
        if has_workbook_formulas(&pending.workbook) {
            return Err(
                "Cut is unavailable while the workbook contains formulas. Copy the cells instead."
                    .into(),
            );
        }
        if source.is_empty() || source.len() > MAX_TRANSACTION_CELLS {
            return Err("Cut source is empty or exceeds the safe coordinate limit".into());
        }
        let sheet = pending
            .workbook
            .sheet(sheet_id as usize)
            .ok_or_else(|| format!("Sheet {sheet_id} not found"))?;
        if sheet.stable_id().to_string() != source_stable_sheet_id {
            return Err("The cut source sheet has changed. Cut the cells again.".into());
        }
        for expected in source {
            if !sheet.in_bounds(expected.row, expected.col)
                || sheet
                    .cell(expected.row, expected.col)
                    .map(|cell| cell.raw.as_str())
                    .unwrap_or("")
                    != expected.value
            {
                return Err("The cut source has changed. Cut the cells again.".into());
            }
            ensure_pending_cell_edit(pending, sheet_id, expected.row, expected.col)?;
        }
        if changes
            .iter()
            .any(|change| change.value.trim_start().starts_with('='))
        {
            return Err("Cut cannot convert text into formulas. Copy the cells instead.".into());
        }
        apply_cell_changes(pending, sheet_id, changes)
    })
}

#[tauri::command]
fn batch_set_formats(
    sheet_id: u32,
    changes: Vec<FormatChange>,
    state: State<AppState>,
) -> Result<(), String> {
    ensure_can_perform_protected_action(state.inner(), sheet_id, ProtectionAction::FormatCells)?;
    with_pending_mutation(state.inner(), |pending| {
        let sheet = pending
            .workbook
            .sheet_mut(sheet_id as usize)
            .ok_or_else(|| format!("Sheet {sheet_id} not found"))?;
        for change in changes {
            sheet.set_format(change.row, change.col, change.format);
        }
        Ok(())
    })
}

#[tauri::command]
fn replace_sheet_snapshot(
    sheet_id: u32,
    cells: Vec<CellChange>,
    formats: Vec<FormatChange>,
    state: State<AppState>,
) -> Result<(), String> {
    ensure_sheet_not_protected(state.inner(), sheet_id)?;
    with_pending_mutation(state.inner(), |pending| {
        let current = pending
            .workbook
            .sheet(sheet_id as usize)
            .ok_or_else(|| format!("Sheet {sheet_id} not found"))?;
        let mut replacement =
            sheets_core::sheet::Sheet::with_stable_id(current.stable_id(), current.name());
        for cell in cells {
            replacement.set_cell_value(cell.row, cell.col, cell.value);
        }
        for format in formats {
            replacement.set_format(format.row, format.col, format.format);
        }
        let mut candidate = pending.workbook.clone();
        *candidate
            .sheet_mut(sheet_id as usize)
            .ok_or_else(|| format!("Sheet {sheet_id} not found"))? = replacement;
        pending.dep_graph = dependency_graphs_for_workbook(&candidate)?;
        pending.workbook = candidate;
        Ok(())
    })
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
    let _ = comments;
    with_pending_mutation(state.inner(), |pending| {
        let mut candidate = pending.workbook.clone();
        if !candidate.apply_sheet_structure_edit(sheet_id as usize, edit) {
            return Err(format!("Sheet {sheet_id} not found"));
        }
        pending.dep_graph = dependency_graphs_for_workbook(&candidate)?;
        pending.workbook = candidate;
        if let Some(manager) = pending.cell_locks.get_mut(sheet_id as usize) {
            manager.clear();
        }
        pending
            .comments
            .comments
            .retain(|(candidate, _, _), _| *candidate != sheet_id);
        Ok(())
    })
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
    replace_loaded_workbook(state.inner(), comments.inner(), Workbook::new())
}

#[tauri::command]
fn get_sheets(state: State<AppState>) -> Result<Vec<SheetInfo>, String> {
    let wb = state.workbook.lock().map_err(|e| e.to_string())?;
    Ok(sheet_infos(&wb))
}

#[tauri::command]
fn set_active_sheet(sheet_id: u32, state: State<AppState>) -> Result<(), String> {
    let history = state.history.lock().map_err(|error| error.to_string())?;
    if history.pending.is_some() {
        return Err("Active-sheet selection is a non-transactional session operation".into());
    }
    let mut workbook = state.workbook.lock().map_err(|error| error.to_string())?;
    if sheet_id as usize >= workbook.sheet_count() {
        return Err(format!("Sheet {sheet_id} not found"));
    }
    workbook.set_active_sheet(sheet_id as usize);
    Ok(())
}

#[tauri::command]
fn add_sheet(name: String, state: State<AppState>) -> Result<Vec<SheetInfo>, String> {
    with_pending_mutation(state.inner(), |pending| {
        let mut candidate = pending.workbook.clone();
        candidate
            .add_sheet(&name)
            .map_err(|error| error.to_string())?;
        pending.dep_graph = dependency_graphs_for_workbook(&candidate)?;
        pending.workbook = candidate;
        Ok(sheet_infos(&pending.workbook))
    })
}

#[tauri::command]
fn add_generated_sheet(state: State<AppState>) -> Result<Vec<SheetInfo>, String> {
    with_pending_mutation(state.inner(), |pending| {
        let mut candidate = pending.workbook.clone();
        add_generated_sheet_to_workbook(&mut candidate)?;
        pending.dep_graph = dependency_graphs_for_workbook(&candidate)?;
        pending.workbook = candidate;
        Ok(sheet_infos(&pending.workbook))
    })
}

fn add_generated_sheet_to_workbook(workbook: &mut Workbook) -> Result<usize, String> {
    let preferred = format!("Sheet{}", workbook.sheet_count() + 1);
    let name = workbook.next_available_sheet_name(&preferred);
    workbook.add_sheet(name).map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_sheet(
    sheet_id: u32,
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<Vec<SheetInfo>, String> {
    ensure_sheet_not_protected(state.inner(), sheet_id)?;
    let _ = comments;
    with_pending_mutation(state.inner(), |pending| {
        if sheet_id as usize >= pending.workbook.sheet_count() {
            return Err(format!("Sheet {} not found", sheet_id));
        }
        if pending.workbook.sheet_count() <= 1 {
            return Err("Cannot delete the last sheet".into());
        }
        let mut candidate = pending.workbook.clone();
        candidate.delete_sheet(sheet_id as usize);
        let graph = dependency_graphs_for_workbook(&candidate)?;
        if (sheet_id as usize) < pending.protections.len() {
            pending.protections.remove(sheet_id as usize);
        }
        if (sheet_id as usize) < pending.cell_locks.len() {
            pending.cell_locks.remove(sheet_id as usize);
        }
        delete_sheet_comments(&mut pending.comments, sheet_id);
        pending.workbook = candidate;
        pending.dep_graph = graph;
        Ok(sheet_infos(&pending.workbook))
    })
}

#[tauri::command]
fn rename_sheet(
    sheet_id: u32,
    name: String,
    state: State<AppState>,
) -> Result<Vec<SheetInfo>, String> {
    with_pending_mutation(state.inner(), |pending| {
        let mut candidate = pending.workbook.clone();
        candidate
            .rename_sheet(sheet_id as usize, &name)
            .map_err(|error| error.to_string())?;
        pending.dep_graph = dependency_graphs_for_workbook(&candidate)?;
        pending.workbook = candidate;
        Ok(sheet_infos(&pending.workbook))
    })
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
    sheet_data(&wb, sheet_id)
}

fn sheet_data(workbook: &Workbook, sheet_id: u32) -> Result<Vec<CellData>, String> {
    let sheet = workbook
        .sheet(sheet_id as usize)
        .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
    let current_sheet_id = sheet.stable_id();
    let session = EvaluationSession::default();
    let provider = TauriProvider {
        workbook,
        current_sheet_id,
        session: &session,
    };
    // Resource-limit results must not depend on HashMap insertion order or
    // the random hash seed assigned when reopening the same workbook.
    let mut cells: Vec<_> = sheet.iter_cells().collect();
    cells.sort_unstable_by_key(|(coordinate, _)| *coordinate);
    let mut data: Vec<_> = cells
        .into_iter()
        .map(|((row, col), cell)| {
            let mut numeric_value = None;
            let display = if cell.cell_type == sheets_core::cell::CellType::Formula {
                match provider.snapshot_cell(row, col) {
                    Value::Number(n) => {
                        numeric_value = n.is_finite().then_some(n);
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
            } else if cell.cell_type == sheets_core::cell::CellType::Number {
                numeric_value = cell.as_number().filter(|value| value.is_finite());
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
                numeric_value,
                cell_type: format!("{:?}", cell.cell_type).to_lowercase(),
                format: sheet.get_format(row, col).cloned(),
            }
        })
        .chain(
            sheet
                .iter_formats()
                .filter(|((row, col), _)| sheet.cell(*row, *col).is_none())
                .map(|((row, col), format)| CellData {
                    row,
                    col,
                    value: String::new(),
                    display: String::new(),
                    numeric_value: None,
                    cell_type: "empty".into(),
                    format: Some(format.clone()),
                }),
        )
        .collect();
    data.sort_unstable_by_key(|cell| (cell.row, cell.col));
    Ok(data)
}

fn has_workbook_formulas(workbook: &Workbook) -> bool {
    workbook
        .sheets()
        .iter()
        .any(|sheet| sheet.iter_cells().any(|(_, cell)| cell.is_formula()))
}

#[tauri::command]
fn workbook_has_formulas(state: State<AppState>) -> Result<bool, String> {
    let workbook = state.workbook.lock().map_err(|error| error.to_string())?;
    Ok(has_workbook_formulas(&workbook))
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
    let session = EvaluationSession::default();
    let provider = TauriProvider {
        workbook: &wb,
        current_sheet_id: sheet.stable_id(),
        session: &session,
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
fn get_export_preflight(
    format: String,
    sheet_id: Option<u32>,
    print_area: Option<[u32; 4]>,
    state: State<AppState>,
) -> Result<ExportPreflight, String> {
    let workbook = state.workbook.lock().map_err(|error| error.to_string())?;
    preflight_workbook(
        &workbook,
        ExportKind::parse(&format)?,
        sheet_id.map(|value| value as usize),
        print_area,
    )
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrontendFeatureRange {
    start_row: u32,
    start_col: u32,
    end_row: u32,
    end_col: u32,
}

impl FrontendFeatureRange {
    fn tuple(&self) -> (u32, u32, u32, u32) {
        (self.start_row, self.start_col, self.end_row, self.end_col)
    }
}

#[derive(serde::Deserialize)]
struct FrontendValidationRule {
    range: FrontendFeatureRange,
    validation: DataValidation,
}

#[derive(serde::Deserialize)]
struct FrontendConditionalRule {
    rule: ConditionalFormat,
}

#[derive(serde::Deserialize)]
struct FrontendTable {
    table: sheets_tables::Table,
}

#[derive(serde::Deserialize)]
struct FrontendChart {
    chart: ChartObject,
}

fn xlsx_features_from_metadata(
    workbook: &Workbook,
    metadata: &serde_json::Value,
) -> Result<Vec<sheets_xlsx::XlsxSheetFeatures>, String> {
    let states = metadata
        .get("sheet_states")
        .and_then(serde_json::Value::as_object);
    workbook
        .sheets()
        .iter()
        .map(|sheet| {
            let Some(state) = states.and_then(|states| states.get(&sheet.stable_id().to_string()))
            else {
                return Ok(sheets_xlsx::XlsxSheetFeatures::default());
            };
            let validations = match state.get("validationRules") {
                None => Vec::new(),
                Some(value) => serde_json::from_value::<Vec<FrontendValidationRule>>(value.clone())
                    .map_err(|error| {
                        format!("Invalid validation metadata for XLSX export: {error}")
                    })?
                    .into_iter()
                    .map(|stored| ValidationRule {
                        range: stored.range.tuple(),
                        validation: stored.validation,
                    })
                    .collect(),
            };
            let conditional_formats = match state.get("conditionalRules") {
                None => Vec::new(),
                Some(value) => {
                    serde_json::from_value::<Vec<FrontendConditionalRule>>(value.clone())
                        .map_err(|error| {
                            format!("Invalid conditional-format metadata for XLSX export: {error}")
                        })?
                        .into_iter()
                        .map(|stored| stored.rule)
                        .collect()
                }
            };
            let tables = match state.get("tables") {
                None => Vec::new(),
                Some(value) => serde_json::from_value::<Vec<FrontendTable>>(value.clone())
                    .map_err(|error| format!("Invalid table metadata for XLSX export: {error}"))?
                    .into_iter()
                    .map(|stored| stored.table)
                    .collect(),
            };
            let charts = match state.get("charts") {
                None => Vec::new(),
                Some(value) => serde_json::from_value::<Vec<FrontendChart>>(value.clone())
                    .map_err(|error| format!("Invalid chart metadata for XLSX export: {error}"))?
                    .into_iter()
                    .map(|stored| stored.chart)
                    .collect(),
            };
            Ok(sheets_xlsx::XlsxSheetFeatures {
                validations,
                conditional_formats,
                tables,
                charts,
            })
        })
        .collect()
}

fn xlsx_metadata_from_document(document: &sheets_xlsx::XlsxDocument) -> serde_json::Value {
    let mut states = serde_json::Map::new();
    for (sheet_index, features) in document.sheet_features.iter().enumerate() {
        if features.validations.is_empty()
            && features.conditional_formats.is_empty()
            && features.tables.is_empty()
            && features.charts.is_empty()
        {
            continue;
        }
        let Some(sheet) = document.workbook.sheet(sheet_index) else {
            continue;
        };
        let validation_rules: Vec<_> = features
            .validations
            .iter()
            .enumerate()
            .map(|(index, rule)| {
                let (start_row, start_col, end_row, end_col) = rule.range;
                serde_json::json!({
                    "id": format!("xlsx-validation-{index}"),
                    "label": "Imported XLSX validation",
                    "range": {
                        "startRow": start_row,
                        "startCol": start_col,
                        "endRow": end_row,
                        "endCol": end_col,
                    },
                    "validation": rule.validation,
                })
            })
            .collect();
        let conditional_rules: Vec<_> = features
            .conditional_formats
            .iter()
            .enumerate()
            .map(|(index, rule)| {
                serde_json::json!({
                    "id": format!("xlsx-conditional-{index}"),
                    "label": "Imported XLSX conditional format",
                    "rule": rule,
                })
            })
            .collect();
        let tables: Vec<_> = features
            .tables
            .iter()
            .map(|table| {
                serde_json::json!({
                    "id": format!("xlsx-table-{}", table.name),
                    "label": "Imported XLSX table",
                    "table": table,
                })
            })
            .collect();
        let charts: Vec<_> = features
            .charts
            .iter()
            .enumerate()
            .map(|(index, chart)| {
                serde_json::json!({
                    "id": format!("xlsx-chart-{index}"),
                    "label": "Imported XLSX chart",
                    "chart": chart,
                })
            })
            .collect();
        states.insert(
            sheet.stable_id().to_string(),
            serde_json::json!({
                "validationRules": validation_rules,
                "conditionalRules": conditional_rules,
                "tables": tables,
                "charts": charts,
            }),
        );
    }
    serde_json::json!({ "sheet_states": states })
}

#[tauri::command]
fn import_xlsx(
    data: Vec<u8>,
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<NativeOpenResult, String> {
    let document = sheets_xlsx::import_document(&data).map_err(|error| error.to_string())?;
    let metadata = xlsx_metadata_from_document(&document);
    let sheets = replace_loaded_workbook(state.inner(), comments.inner(), document.workbook)?;
    Ok(NativeOpenResult { sheets, metadata })
}

#[tauri::command]
fn export_xlsx(metadata: serde_json::Value, state: State<AppState>) -> Result<Vec<u8>, String> {
    let workbook = state.workbook.lock().map_err(|error| error.to_string())?;
    let sheet_features = xlsx_features_from_metadata(&workbook, &metadata)?;
    sheets_xlsx::export_document(&sheets_xlsx::XlsxDocument {
        workbook: workbook.clone(),
        sheet_features,
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn import_xlsx_file(
    file_path: String,
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
) -> Result<NativeOpenResult, String> {
    let path = checked_absolute_path(&file_path)?;
    ensure_allowed_extension(&path, &["xlsx"])?;
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    import_xlsx(data, state, comments)
}

#[tauri::command]
fn export_xlsx_file(
    file_path: String,
    metadata: serde_json::Value,
    state: State<AppState>,
) -> Result<(), String> {
    let path = checked_absolute_path(&file_path)?;
    ensure_allowed_extension(&path, &["xlsx"])?;
    let data = export_xlsx(metadata, state)?;
    write_atomic(&path, &data)
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
    import_csv_data_inner(&data, delim, sheet_id, state.inner())
}

fn import_csv_data_inner(
    data: &str,
    delim: char,
    sheet_id: Option<u32>,
    state: &AppState,
) -> Result<Vec<SheetInfo>, String> {
    let sheet = sheets_csv::import_csv(data, delim).map_err(|e| e.to_string())?;
    with_pending_mutation(state, |pending| {
        let mut candidate = pending.workbook.clone();
        let target = if let Some(id) = sheet_id {
            id as usize
        } else {
            let sheet_name = candidate
                .next_available_sheet_name(&format!("Sheet{}", candidate.sheet_count() + 1));
            candidate
                .add_sheet(&sheet_name)
                .map_err(|error| error.to_string())?
        };
        let target_sheet = candidate
            .sheet_mut(target)
            .ok_or_else(|| format!("Sheet {} not found", target))?;
        for ((row, col), cell) in sheet.iter_cells() {
            target_sheet.set_cell(row, col, cell.clone());
        }
        pending.dep_graph = dependency_graphs_for_workbook(&candidate)?;
        pending.workbook = candidate;
        Ok(sheet_infos(&pending.workbook))
    })
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
    write_atomic(&path, data.as_bytes())
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
    write_atomic(&path, data.as_bytes())
}

fn metadata_with_backend_sheet_states(
    workbook: &Workbook,
    mut metadata: serde_json::Value,
    comments: &SheetComments,
    protections: &[SheetProtection],
    locks: &[CellLockManager],
) -> Result<serde_json::Value, String> {
    if !metadata.is_object() {
        metadata = serde_json::json!({});
    }
    let object = metadata.as_object_mut().expect("metadata object");
    object.remove("comments");
    object.remove("protections");
    object.remove("cell_locks");
    let sheet_states = object
        .entry("sheet_states")
        .or_insert_with(|| serde_json::json!({}));
    if !sheet_states.is_object() {
        *sheet_states = serde_json::json!({});
    }
    let states = sheet_states.as_object_mut().expect("sheet states object");
    let current_ids: HashSet<String> = workbook
        .sheets()
        .iter()
        .map(|sheet| sheet.stable_id().to_string())
        .collect();
    states.retain(|stable_id, _| current_ids.contains(stable_id));
    for (index, sheet) in workbook.sheets().iter().enumerate() {
        let state = states
            .entry(sheet.stable_id().to_string())
            .or_insert_with(|| serde_json::json!({}));
        if !state.is_object() {
            *state = serde_json::json!({});
        }
        let state = state.as_object_mut().expect("sheet state object");
        let saved_comments: Vec<_> = comments
            .comments
            .iter()
            .filter(|((sheet_id, _, _), _)| *sheet_id == index as u32)
            .map(|(_, comment)| comment.clone())
            .collect();
        state.insert(
            "comments".into(),
            serde_json::to_value(saved_comments).map_err(|error| error.to_string())?,
        );
        if let Some(protection) = protections.get(index) {
            state.insert(
                "protection".into(),
                serde_json::to_value(protection).map_err(|error| error.to_string())?,
            );
        }
        let saved_locks: Vec<_> = locks
            .get(index)
            .map(|manager| {
                manager
                    .iter_overrides()
                    .map(|((row, col), locked)| (row, col, locked))
                    .collect()
            })
            .unwrap_or_default();
        state.insert(
            "cell_locks".into(),
            serde_json::to_value(saved_locks).map_err(|error| error.to_string())?,
        );
    }
    Ok(metadata)
}

fn parse_backend_sheet_states(
    workbook: &Workbook,
    metadata: &serde_json::Value,
) -> Result<(SheetComments, Vec<SheetProtection>, Vec<CellLockManager>), String> {
    let mut restored_comments = SheetComments::default();
    let mut restored_protections: Vec<SheetProtection> = (0..workbook.sheet_count())
        .map(|_| SheetProtection::default())
        .collect();
    let mut restored_locks: Vec<CellLockManager> = (0..workbook.sheet_count())
        .map(|_| CellLockManager::new())
        .collect();

    // v0.3 compatibility: these fields were stored globally by sheet index.
    if let Some(saved_comments) = metadata.get("comments").and_then(|value| value.as_array()) {
        for saved in saved_comments {
            let Some(sheet_id) = saved.get("sheet_id").and_then(|value| value.as_u64()) else {
                continue;
            };
            if let Ok(comment) = serde_json::from_value::<CellComment>(saved.clone()) {
                restored_comments
                    .comments
                    .insert((sheet_id as u32, comment.row, comment.col), comment);
            }
        }
    }
    if let Some(saved) = metadata.get("protections") {
        if let Ok(values) = serde_json::from_value::<Vec<SheetProtection>>(saved.clone()) {
            for (index, protection) in values.into_iter().enumerate() {
                if index < restored_protections.len() {
                    restored_protections[index] = protection;
                }
            }
        }
    }
    if let Some(saved_locks) = metadata
        .get("cell_locks")
        .and_then(|value| value.as_array())
    {
        for (index, sheet_locks) in saved_locks.iter().enumerate() {
            if index >= restored_locks.len() {
                break;
            }
            if let Some(entries) = sheet_locks.as_array() {
                for entry in entries {
                    let Some(values) = entry.as_array() else {
                        continue;
                    };
                    if let [row, col, locked] = values.as_slice() {
                        if let (Some(row), Some(col), Some(locked)) =
                            (row.as_u64(), col.as_u64(), locked.as_bool())
                        {
                            restored_locks[index].set_locked(row as u32, col as u32, locked);
                        }
                    }
                }
            }
        }
    }

    // v0.4 state is stable-ID keyed. It overlays any compatible legacy state.
    if let Some(sheet_states) = metadata
        .get("sheet_states")
        .and_then(|value| value.as_object())
    {
        for (index, sheet) in workbook.sheets().iter().enumerate() {
            let Some(sheet_state) = sheet_states.get(&sheet.stable_id().to_string()) else {
                continue;
            };
            if let Some(saved_comments) = sheet_state.get("comments").and_then(|v| v.as_array()) {
                for saved in saved_comments {
                    if let Ok(comment) = serde_json::from_value::<CellComment>(saved.clone()) {
                        restored_comments
                            .comments
                            .insert((index as u32, comment.row, comment.col), comment);
                    }
                }
            }
            if let Some(saved) = sheet_state.get("protection") {
                if let Ok(protection) = serde_json::from_value::<SheetProtection>(saved.clone()) {
                    restored_protections[index] = protection;
                }
            }
            if let Some(saved_locks) = sheet_state.get("cell_locks").and_then(|v| v.as_array()) {
                for entry in saved_locks {
                    let Some(values) = entry.as_array() else {
                        continue;
                    };
                    if let [row, col, locked] = values.as_slice() {
                        if let (Some(row), Some(col), Some(locked)) =
                            (row.as_u64(), col.as_u64(), locked.as_bool())
                        {
                            restored_locks[index].set_locked(row as u32, col as u32, locked);
                        }
                    }
                }
            }
        }
    }
    Ok((restored_comments, restored_protections, restored_locks))
}

fn export_native_contents(
    metadata: serde_json::Value,
    state: &AppState,
    comments: &Mutex<SheetComments>,
) -> Result<String, String> {
    let workbook = state.workbook.lock().map_err(|error| error.to_string())?;
    let comment_guard = comments.lock().map_err(|error| error.to_string())?;
    let protection_guard = state
        .protections
        .lock()
        .map_err(|error| error.to_string())?;
    let lock_guard = state.cell_locks.lock().map_err(|error| error.to_string())?;
    let metadata = metadata_with_backend_sheet_states(
        &workbook,
        metadata,
        &comment_guard,
        &protection_guard,
        &lock_guard,
    )?;
    sheets_json::export_native_workbook_with_metadata(&workbook, metadata)
        .map_err(|error| error.to_string())
}

fn inspect_native_contents(data: &str) -> Result<(Vec<SheetInfo>, serde_json::Value), String> {
    let (workbook, metadata) = sheets_json::import_native_workbook_with_metadata(data)
        .map_err(|error| error.to_string())?;
    dependency_graphs_for_workbook(&workbook)?;
    parse_backend_sheet_states(&workbook, &metadata)?;
    Ok((sheet_infos(&workbook), metadata))
}

fn import_native_contents(
    data: &str,
    state: &AppState,
    comments: &Mutex<SheetComments>,
) -> Result<NativeOpenResult, String> {
    let (imported, metadata) = sheets_json::import_native_workbook_with_metadata(data)
        .map_err(|error| error.to_string())?;
    // Build and validate every candidate component before acquiring the live
    // state locks. A malformed backup, recovery, or native file can therefore
    // never leave a partially replaced workbook behind.
    let candidate_graphs = dependency_graphs_for_workbook(&imported)?;
    let (candidate_comments, candidate_protections, candidate_locks) =
        parse_backend_sheet_states(&imported, &metadata)?;
    let sheets = sheet_infos(&imported);

    let mut history = state.history.lock().map_err(|error| error.to_string())?;
    if history.pending.is_some() {
        return Err("Workbook replacement is a non-transactional session operation".into());
    }
    let mut workbook = state.workbook.lock().map_err(|error| error.to_string())?;
    let mut dep_graph = state.dep_graphs.lock().map_err(|error| error.to_string())?;
    let mut protections = state
        .protections
        .lock()
        .map_err(|error| error.to_string())?;
    let mut cell_locks = state.cell_locks.lock().map_err(|error| error.to_string())?;
    let mut live_comments = comments.lock().map_err(|error| error.to_string())?;

    *workbook = imported;
    *dep_graph = candidate_graphs;
    *protections = candidate_protections;
    *cell_locks = candidate_locks;
    *live_comments = candidate_comments;
    *history = TransactionHistory::default();
    Ok(NativeOpenResult { sheets, metadata })
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
    import_native_contents(&data, state.inner(), comments.inner())
}

#[tauri::command]
fn export_native_file(
    file_path: String,
    metadata: serde_json::Value,
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
    backups: State<NativeBackupStore>,
) -> Result<NativeSaveResult, String> {
    let path = checked_absolute_path(&file_path)?;
    ensure_allowed_extension(&path, &["900sheets"])?;
    let data = export_native_contents(metadata, state.inner(), comments.inner())?;
    let durability_warning = write_atomic_with_faults(&path, data.as_bytes(), false, false)?;
    // A successful primary save remains successful if backup maintenance fails.
    // The result lets the UI surface the warning without telling the user that
    // their already-written workbook was not saved.
    match backups.write_for_path(&path, &data) {
        Ok(result) => Ok(NativeSaveResult {
            backup: Some(result.entry),
            backup_warning: combine_warnings(durability_warning, result.rotation_warning),
        }),
        Err(error) => Ok(NativeSaveResult {
            backup: None,
            backup_warning: combine_warnings(
                durability_warning,
                Some(format!(
                    "Workbook saved, but its rotating backup could not be created: {error}"
                )),
            ),
        }),
    }
}

#[tauri::command]
fn write_recovery_snapshot(
    recovery_id: String,
    metadata: serde_json::Value,
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
    recovery: State<RecoveryStore>,
) -> Result<(), String> {
    let data = export_native_contents(metadata, state.inner(), comments.inner())?;
    recovery.write(&recovery_id, data.as_bytes())
}

#[tauri::command]
fn list_recovery_snapshots(recovery: State<RecoveryStore>) -> Result<Vec<RecoveryEntry>, String> {
    recovery.list()
}

#[tauri::command]
fn inspect_recovery_snapshot(
    recovery_id: String,
    recovery: State<RecoveryStore>,
) -> Result<RecoveryInspection, String> {
    let data = recovery.read(&recovery_id)?;
    let (sheets, metadata) = inspect_native_contents(&data)?;
    Ok(RecoveryInspection {
        entry: recovery.describe(&recovery_id)?,
        sheets,
        metadata,
    })
}

#[tauri::command]
fn restore_recovery_snapshot(
    recovery_id: String,
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
    recovery: State<RecoveryStore>,
) -> Result<NativeOpenResult, String> {
    let data = recovery.read(&recovery_id)?;
    if let Err(error) = sheets_json::import_native_workbook_with_metadata(&data) {
        recovery.quarantine(&recovery_id)?;
        return Err(format!(
            "Recovery snapshot was corrupted and has been quarantined: {error}"
        ));
    }
    import_native_contents(&data, state.inner(), comments.inner())
}

#[tauri::command]
fn discard_recovery_snapshot(
    recovery_id: String,
    recovery: State<RecoveryStore>,
) -> Result<(), String> {
    recovery.discard(&recovery_id)
}

#[tauri::command]
fn delete_recovery_snapshot(
    recovery_id: String,
    recovery: State<RecoveryStore>,
) -> Result<(), String> {
    recovery.discard(&recovery_id)
}

#[tauri::command]
fn create_native_backup(
    file_path: String,
    backups: State<NativeBackupStore>,
) -> Result<NativeBackupWriteResult, String> {
    let path = checked_absolute_path(&file_path)?;
    ensure_allowed_extension(&path, &["900sheets"])?;
    ensure_regular_file_or_missing(&path, "Native workbook")?;
    let data = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    backups.write_for_path(&path, &data)
}

#[tauri::command]
fn list_native_backups(
    file_path: Option<String>,
    backups: State<NativeBackupStore>,
) -> Result<Vec<NativeBackupEntry>, String> {
    let mut entries = backups.list()?;
    if let Some(file_path) = file_path {
        let path = checked_absolute_path(&file_path)?;
        ensure_allowed_extension(&path, &["900sheets"])?;
        let document_id = NativeBackupStore::document_id(&path);
        entries.retain(|entry| entry.document_id == document_id);
    }
    Ok(entries)
}

#[tauri::command]
fn inspect_native_backup(
    backup_id: String,
    backups: State<NativeBackupStore>,
) -> Result<NativeBackupInspection, String> {
    let envelope = backups.read(&backup_id)?;
    let (sheets, metadata) = inspect_native_contents(&envelope.native_workbook)?;
    Ok(NativeBackupInspection {
        entry: envelope.entry,
        sheets,
        metadata,
    })
}

#[tauri::command]
fn restore_native_backup(
    backup_id: String,
    state: State<AppState>,
    comments: State<'_, Mutex<SheetComments>>,
    backups: State<NativeBackupStore>,
) -> Result<NativeOpenResult, String> {
    let envelope = backups.read(&backup_id)?;
    import_native_contents(&envelope.native_workbook, state.inner(), comments.inner())
}

#[tauri::command]
fn delete_native_backup(
    backup_id: String,
    backups: State<NativeBackupStore>,
) -> Result<(), String> {
    backups.delete(&backup_id)
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
    with_pending_mutation(state.inner(), |pending| {
        pending
            .workbook
            .sheet_mut(sheet_id as usize)
            .ok_or_else(|| format!("Sheet {} not found", sheet_id))?
            .set_format(row, col, format);
        Ok(())
    })
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
    with_pending_mutation(state.inner(), |pending| {
        let mut candidate = pending.workbook.clone();
        let sheet = candidate
            .sheet_mut(sheet_id as usize)
            .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
        sheets_core::data_tools::apply_sort_range(
            sheet,
            range.sort_col,
            range.start_row,
            range.end_row,
            range.start_col,
            range.end_col,
            ascending,
        );
        pending.dep_graph = dependency_graphs_for_workbook(&candidate)?;
        pending.workbook = candidate;
        Ok(())
    })
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
    with_pending_mutation(state.inner(), |pending| {
        let mut candidate = pending.workbook.clone();
        let replaced = sheets_core::data_tools::replace_in_sheet(
            candidate
                .sheet_mut(sheet_id as usize)
                .ok_or_else(|| format!("Sheet {} not found", sheet_id))?,
            &find,
            &replace,
            match_case,
        );
        pending.dep_graph = dependency_graphs_for_workbook(&candidate)?;
        pending.workbook = candidate;
        Ok(replaced)
    })
}

#[tauri::command]
fn create_pivot(
    sheet_id: u32,
    config: PivotConfig,
    state: State<AppState>,
) -> Result<PivotResult, String> {
    ensure_can_perform_live(state.inner(), sheet_id, ProtectionAction::UsePivot)?;
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
    with_pending_mutation(state.inner(), |pending| {
        let mut candidate = pending.workbook.clone();
        let result = sheets_pivot::build_pivot(
            candidate
                .sheet(sheet_id as usize)
                .ok_or_else(|| format!("Sheet {} not found", sheet_id))?,
            &config,
        )
        .map_err(|e| e.to_string())?;
        add_pivot_result_sheet(&mut candidate, sheet_id as usize, &result)?;
        pending.dep_graph = dependency_graphs_for_workbook(&candidate)?;
        pending.workbook = candidate;
        Ok(sheet_infos(&pending.workbook))
    })
}

fn add_pivot_result_sheet(
    workbook: &mut Workbook,
    source_sheet: usize,
    result: &PivotResult,
) -> Result<usize, String> {
    let preferred_pivot_name = format!(
        "Pivot ({})",
        workbook
            .sheet(source_sheet)
            .map(|sheet| sheet.name())
            .ok_or_else(|| format!("Sheet {source_sheet} not found"))?
    );
    let pivot_name = workbook.next_available_sheet_name(&preferred_pivot_name);
    let index = workbook
        .add_sheet(pivot_name)
        .map_err(|error| error.to_string())?;
    if let Some(pivot_sheet) = workbook.sheet_mut(index) {
        sheets_pivot::write_pivot_to_sheet(pivot_sheet, result, 0, 0);
    }
    Ok(index)
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

fn bounded_range_cells(
    sheet: &Sheet,
    range: (u32, u32, u32, u32),
    operation: &str,
) -> Result<usize, String> {
    let (start_row, start_col, end_row, end_col) = range;
    if start_row > end_row || start_col > end_col {
        return Err(format!("{operation} range start must not be after its end"));
    }
    if end_row >= sheet.max_rows() || end_col >= sheet.max_cols() {
        return Err(format!("{operation} range is outside the workbook grid"));
    }
    let rows = u64::from(end_row - start_row + 1);
    let columns = u64::from(end_col - start_col + 1);
    let cells = rows.saturating_mul(columns);
    if cells > MAX_TRANSACTION_CELLS as u64 {
        return Err(format!(
            "{operation} range contains {cells} cells; the interactive limit is {MAX_TRANSACTION_CELLS}"
        ));
    }
    Ok(cells as usize)
}

fn ensure_conditional_rule_budget(
    sheet: &Sheet,
    rules: &[ConditionalFormat],
) -> Result<(), String> {
    let mut total = 0usize;
    for rule in rules {
        total = total.saturating_add(bounded_range_cells(
            sheet,
            rule.range,
            "Conditional formatting",
        )?);
        if total > MAX_TRANSACTION_CELLS {
            return Err(format!(
                "Conditional formatting rules cover more than {MAX_TRANSACTION_CELLS} cells in one request"
            ));
        }
    }
    Ok(())
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
    bounded_range_cells(sheet, rule.range, "Validation")?;
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
    ensure_conditional_rule_budget(sheet, &rules)?;
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
    ensure_conditional_rule_budget(sheet, std::slice::from_ref(&rule))?;
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
    write_atomic(&path, &pdf_bytes)
}

// --- Advanced Features IPC ---

#[tauri::command]
fn protect_sheet(sheet_id: u32, password: String, state: State<AppState>) -> Result<(), String> {
    with_pending_mutation(state.inner(), |pending| {
        let sheet_count = pending.workbook.sheet_count();
        if sheet_id as usize >= sheet_count {
            return Err(format!("Sheet {} not found", sheet_id));
        }
        while pending.protections.len() < sheet_count {
            pending.protections.push(SheetProtection::default());
        }
        pending.protections[sheet_id as usize].protect(&password);
        while pending.cell_locks.len() < sheet_count {
            pending.cell_locks.push(CellLockManager::new());
        }
        Ok(())
    })
}

#[tauri::command]
fn unprotect_sheet(
    sheet_id: u32,
    password: String,
    state: State<AppState>,
) -> Result<bool, String> {
    with_pending_mutation(state.inner(), |pending| {
        if sheet_id as usize >= pending.workbook.sheet_count() {
            return Err(format!("Sheet {} not found", sheet_id));
        }
        Ok(pending
            .protections
            .get_mut(sheet_id as usize)
            .map(|protection| protection.unprotect(&password))
            .unwrap_or(true))
    })
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
    for cell in &scenario.cells {
        ensure_can_edit_cell(state.inner(), sheet_id, cell.row, cell.col)?;
    }
    with_pending_mutation(state.inner(), |pending| {
        let mut candidate = pending.workbook.clone();
        let sheet = candidate
            .sheet_mut(sheet_id as usize)
            .ok_or_else(|| format!("Sheet {} not found", sheet_id))?;
        for cell in scenario.cells {
            sheet.set_cell_value(cell.row, cell.col, cell.value);
        }
        pending.dep_graph = dependency_graphs_for_workbook(&candidate)?;
        pending.workbook = candidate;
        Ok(())
    })
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
    state: State<AppState>,
) -> Result<(), String> {
    with_pending_mutation(state.inner(), |pending| {
        pending.comments.comments.insert(
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
    })
}

#[tauri::command]
fn remove_cell_comment(
    sheet_id: u32,
    row: u32,
    col: u32,
    state: State<AppState>,
) -> Result<bool, String> {
    with_pending_mutation(state.inner(), |pending| {
        Ok(pending
            .comments
            .comments
            .remove(&(sheet_id, row, col))
            .is_some())
    })
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
    with_pending_mutation(state.inner(), |pending| {
        let sheet_count = pending.workbook.sheet_count();
        if sheet_id as usize >= sheet_count {
            return Err(format!("Sheet {} not found", sheet_id));
        }
        while pending.cell_locks.len() < sheet_count {
            pending.cell_locks.push(CellLockManager::new());
        }
        pending.cell_locks[sheet_id as usize].set_locked(row, col, locked);
        Ok(())
    })
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
    with_pending_mutation(state.inner(), |pending| {
        let sheet_count = pending.workbook.sheet_count();
        if sheet_id as usize >= sheet_count {
            return Err(format!("Sheet {} not found", sheet_id));
        }
        while pending.cell_locks.len() < sheet_count {
            pending.cell_locks.push(CellLockManager::new());
        }
        let mgr = &mut pending.cell_locks[sheet_id as usize];
        if locked {
            mgr.lock_range(start_row, start_col, end_row, end_col);
        } else {
            mgr.unlock_range(start_row, start_col, end_row, end_col);
        }
        Ok(())
    })
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
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&app_data, std::fs::Permissions::from_mode(0o700))?;
            }
            let recovery = RecoveryStore::new(app_data.join("recovery"));
            recovery.ensure_root().map_err(std::io::Error::other)?;
            app.manage(recovery);
            let backups = NativeBackupStore::new(app_data.join("backups"));
            backups.ensure_root().map_err(std::io::Error::other)?;
            app.manage(backups);
            Ok(())
        })
        .manage(AppState {
            workbook: Mutex::new(Workbook::new()),
            dep_graphs: Mutex::new(DependencyGraph::new()),
            protections: Mutex::new(Vec::new()),
            cell_locks: Mutex::new(Vec::new()),
            history: Mutex::new(TransactionHistory::default()),
        })
        .manage(Mutex::new(SheetComments::default()))
        .invoke_handler(tauri::generate_handler![
            new_workbook,
            begin_workbook_transaction,
            commit_workbook_transaction,
            abort_workbook_transaction,
            undo_workbook_transaction,
            redo_workbook_transaction,
            get_sheets,
            set_active_sheet,
            add_sheet,
            add_generated_sheet,
            delete_sheet,
            rename_sheet,
            get_cell,
            set_cell,
            clear_cell,
            batch_set_cells,
            move_cells,
            batch_set_formats,
            replace_sheet_snapshot,
            edit_sheet_structure,
            get_sheet_data,
            workbook_has_formulas,
            evaluate_formula,
            get_export_preflight,
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
            write_recovery_snapshot,
            list_recovery_snapshots,
            inspect_recovery_snapshot,
            restore_recovery_snapshot,
            discard_recovery_snapshot,
            delete_recovery_snapshot,
            create_native_backup,
            list_native_backups,
            inspect_native_backup,
            restore_native_backup,
            delete_native_backup,
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
            dep_graphs: Mutex::new(DependencyGraph::new()),
            protections: Mutex::new(Vec::new()),
            cell_locks: Mutex::new(Vec::new()),
            history: Mutex::new(TransactionHistory::default()),
        }
    }

    fn native_test_data(value: &str) -> String {
        let mut workbook = Workbook::new();
        workbook
            .sheet_mut(0)
            .unwrap()
            .set_cell_value(0, 0, value.into());
        sheets_json::export_native_workbook_with_metadata(
            &workbook,
            serde_json::json!({"sheet_states": {}}),
        )
        .unwrap()
    }

    #[test]
    fn protected_sheet_blocks_cell_edit() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        begin_transaction(serde_json::json!({}), &state, &comments).unwrap();
        assert!(ensure_can_edit_cell(&state, 0, 0, 0).is_ok());
        abort_transaction(&state).unwrap();

        {
            let mut protections = state.protections.lock().unwrap();
            let mut protection = SheetProtection::default();
            protection.protect("secret");
            protections.push(protection);
        }

        begin_transaction(serde_json::json!({}), &state, &comments).unwrap();
        assert!(ensure_can_edit_cell(&state, 0, 0, 0).is_err());
    }

    #[test]
    fn protected_sheet_allows_unlocked_cell() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
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
        begin_transaction(serde_json::json!({}), &state, &comments).unwrap();
        assert!(ensure_can_edit_cell(&state, 0, 0, 0).is_ok());
        assert!(ensure_can_edit_cell(&state, 0, 1, 0).is_err());
    }

    #[test]
    fn protected_sheet_blocks_disallowed_actions() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        {
            let mut protections = state.protections.lock().unwrap();
            let mut protection = SheetProtection::default();
            protection.protect("secret");
            protections.push(protection);
        }

        begin_transaction(serde_json::json!({}), &state, &comments).unwrap();
        assert!(
            ensure_can_perform_protected_action(&state, 0, ProtectionAction::FormatCells).is_err()
        );
        assert!(
            ensure_can_perform_protected_action(&state, 0, ProtectionAction::SelectCells).is_ok()
        );
    }

    #[test]
    fn checked_absolute_path_rejects_relative_and_parent_paths() {
        let root = std::env::current_dir().unwrap();
        let parent_traversal = root.join("nested").join("..").join("secret.xlsx");
        let valid = root.join("900sheets.xlsx");

        assert!(checked_absolute_path("relative/file.xlsx").is_err());
        assert!(checked_absolute_path(parent_traversal.to_str().unwrap()).is_err());
        assert_eq!(
            checked_absolute_path(valid.to_str().unwrap()).unwrap(),
            valid
        );
    }

    #[test]
    fn allowed_extension_is_case_insensitive() {
        assert!(ensure_allowed_extension(&PathBuf::from("data.XLSX"), &["xlsx"]).is_ok());
        assert!(ensure_allowed_extension(&PathBuf::from("data.exe"), &["xlsx"]).is_err());
    }

    #[test]
    fn interactive_validation_and_conditional_ranges_are_bounded() {
        let sheet = Sheet::new("Bounds");
        assert_eq!(
            bounded_range_cells(&sheet, (0, 0, 399, 499), "Validation").unwrap(),
            MAX_TRANSACTION_CELLS
        );
        assert!(bounded_range_cells(&sheet, (0, 0, 400, 499), "Validation").is_err());
        assert!(bounded_range_cells(&sheet, (4, 0, 3, 0), "Validation").is_err());
        assert!(bounded_range_cells(
            &sheet,
            (0, 0, sheet.max_rows(), 0),
            "Conditional formatting"
        )
        .is_err());
    }

    #[test]
    fn xlsx_features_follow_imported_sheet_stable_ids() {
        let mut workbook = Workbook::new();
        workbook.rename_sheet(0, "Budget").unwrap();
        let assumptions = workbook.add_sheet("Assumptions").unwrap();
        let assumption_id = workbook.sheet(assumptions).unwrap().stable_id();
        let validation = DataValidation::list("Low,Medium,High");
        let conditional = ConditionalFormat::cell_value(
            sheets_validation::ConditionOperator::LessThan,
            "0",
            CellFormat::new().bg_color("#FFC7CE"),
            (1, 3, 4, 3),
        );
        let metadata = serde_json::json!({
            "sheet_states": {
                (assumption_id.to_string()): {
                    "validationRules": [{
                        "id": "validation-1",
                        "label": "Priority",
                        "range": {"startRow": 1, "startCol": 1, "endRow": 3, "endCol": 1},
                        "validation": validation,
                    }],
                    "conditionalRules": [{
                        "id": "conditional-1",
                        "label": "Negative",
                        "rule": conditional,
                    }],
                }
            }
        });
        let features = xlsx_features_from_metadata(&workbook, &metadata).unwrap();
        assert!(features[0].validations.is_empty());
        assert_eq!(features[1].validations.len(), 1);
        assert_eq!(features[1].conditional_formats.len(), 1);

        let bytes = sheets_xlsx::export_document(&sheets_xlsx::XlsxDocument {
            workbook,
            sheet_features: features,
        })
        .unwrap();
        let imported = sheets_xlsx::import_document(&bytes).unwrap();
        assert_eq!(imported.workbook.sheet(1).unwrap().name(), "Assumptions");
        let imported_id = imported.workbook.sheet(1).unwrap().stable_id();
        let imported_metadata = xlsx_metadata_from_document(&imported);
        let imported_state = &imported_metadata["sheet_states"][imported_id.to_string()];
        assert_eq!(
            imported_state["validationRules"].as_array().unwrap().len(),
            1
        );
        assert_eq!(
            imported_state["conditionalRules"].as_array().unwrap().len(),
            1
        );
        assert!(imported_metadata["sheet_states"]
            .get(imported.workbook.sheet(0).unwrap().stable_id().to_string())
            .is_none());
    }

    #[test]
    fn table_metadata_roundtrips_through_xlsx_import_export_and_native_format() {
        use sheets_tables::{Table, TableColumn, TotalsRowFunction};

        // Simulate an XLSX document with one table, the way import_document
        // returns it.
        let workbook = Workbook::new();
        let stable_id = workbook.sheet(0).unwrap().stable_id();
        let table = Table {
            name: "Inventory".into(),
            display_name: "Inventory".into(),
            range: (0, 0, 3, 2),
            header_row_count: 1,
            totals_row_shown: true,
            columns: vec![
                TableColumn {
                    id: 1,
                    name: "SKU".into(),
                    totals_row_function: None,
                    totals_row_label: None,
                },
                TableColumn {
                    id: 2,
                    name: "Name".into(),
                    totals_row_function: None,
                    totals_row_label: None,
                },
                TableColumn {
                    id: 3,
                    name: "Qty".into(),
                    totals_row_function: Some(TotalsRowFunction::Sum),
                    totals_row_label: None,
                },
            ],
            style: sheets_tables::TableStyleInfo::default(),
            auto_filter_range: Some((0, 0, 3, 2)),
        };
        let document = sheets_xlsx::XlsxDocument {
            workbook: workbook.clone(),
            sheet_features: vec![sheets_xlsx::XlsxSheetFeatures {
                validations: Vec::new(),
                conditional_formats: Vec::new(),
                tables: vec![table.clone()],
                charts: Vec::new(),
            }],
        };

        // xlsx_metadata_from_document: tables appear in sheet_states metadata.
        let metadata = xlsx_metadata_from_document(&document);
        let state = &metadata["sheet_states"][stable_id.to_string()];
        assert_eq!(state["tables"].as_array().unwrap().len(), 1);
        assert_eq!(state["tables"][0]["table"]["name"], "Inventory");

        // xlsx_features_from_metadata: tables are reconstructed for XLSX export.
        let features = xlsx_features_from_metadata(&workbook, &metadata).unwrap();
        assert_eq!(features[0].tables.len(), 1);
        assert_eq!(features[0].tables[0], table);

        // Full XLSX round-trip through the desktop metadata bridge.
        let bytes = sheets_xlsx::export_document(&sheets_xlsx::XlsxDocument {
            workbook: workbook.clone(),
            sheet_features: features,
        })
        .unwrap();
        let reimported = sheets_xlsx::import_document(&bytes).unwrap();
        assert_eq!(reimported.sheet_features[0].tables.len(), 1);
        assert_eq!(reimported.sheet_features[0].tables[0].name, "Inventory");
        assert_eq!(reimported.sheet_features[0].tables[0].range, (0, 0, 3, 2));
        assert!(reimported.sheet_features[0].tables[0].totals_row_shown);

        // Native-format round-trip: metadata (including tables) is an opaque
        // blob that survives .900sheets save and reopen unchanged.
        let native_json =
            sheets_json::export_native_workbook_with_metadata(&workbook, metadata.clone()).unwrap();
        let (_, restored_metadata) =
            sheets_json::import_native_workbook_with_metadata(&native_json).unwrap();
        assert_eq!(restored_metadata, metadata);
    }

    #[test]
    fn chart_metadata_roundtrips_through_xlsx_import_export_and_native_format() {
        use sheets_chart::{
            ChartAnchor, ChartObject, ChartObjectSeries, ChartType, LegendPosition,
        };

        let workbook = Workbook::new();
        let stable_id = workbook.sheet(0).unwrap().stable_id();
        let chart = ChartObject {
            title: Some("Sales by Month".into()),
            chart_type: ChartType::Column,
            anchor: ChartAnchor::new(0, 4, 18, 12),
            series: vec![ChartObjectSeries {
                name_ref: Some("Sheet1!$B$1".into()),
                category_ref: Some("Sheet1!$A$2:$A$4".into()),
                value_ref: Some("Sheet1!$B$2:$B$4".into()),
            }],
            legend_position: LegendPosition::Right,
        };
        let document = sheets_xlsx::XlsxDocument {
            workbook: workbook.clone(),
            sheet_features: vec![sheets_xlsx::XlsxSheetFeatures {
                validations: Vec::new(),
                conditional_formats: Vec::new(),
                tables: Vec::new(),
                charts: vec![chart.clone()],
            }],
        };

        // xlsx_metadata_from_document: charts appear in sheet_states metadata.
        let metadata = xlsx_metadata_from_document(&document);
        let state = &metadata["sheet_states"][stable_id.to_string()];
        assert_eq!(state["charts"].as_array().unwrap().len(), 1);
        assert_eq!(state["charts"][0]["chart"]["title"], "Sales by Month");

        // xlsx_features_from_metadata: charts are reconstructed for XLSX export.
        let features = xlsx_features_from_metadata(&workbook, &metadata).unwrap();
        assert_eq!(features[0].charts.len(), 1);
        assert_eq!(features[0].charts[0], chart);

        // Full XLSX round-trip through the desktop metadata bridge.
        let bytes = sheets_xlsx::export_document(&sheets_xlsx::XlsxDocument {
            workbook: workbook.clone(),
            sheet_features: features,
        })
        .unwrap();
        let reimported = sheets_xlsx::import_document(&bytes).unwrap();
        assert_eq!(reimported.sheet_features[0].charts.len(), 1);
        assert_eq!(
            reimported.sheet_features[0].charts[0].chart_type,
            ChartType::Column
        );
        assert_eq!(
            reimported.sheet_features[0].charts[0].title.as_deref(),
            Some("Sales by Month")
        );

        // Native-format round-trip: chart metadata survives .900sheets save
        // and reopen unchanged as an opaque blob.
        let native_json =
            sheets_json::export_native_workbook_with_metadata(&workbook, metadata.clone()).unwrap();
        let (_, restored_metadata) =
            sheets_json::import_native_workbook_with_metadata(&native_json).unwrap();
        assert_eq!(restored_metadata, metadata);
    }

    #[test]
    fn set_cell_rejects_and_does_not_store_circular_formula() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        begin_transaction(serde_json::json!({}), &state, &comments).unwrap();

        set_cell_value_in_workbook(&state, 0, 0, 0, "=B1".into()).unwrap();
        let result = set_cell_value_in_workbook(&state, 0, 0, 1, "=A1".into());

        assert!(result.is_err());
        commit_transaction(serde_json::json!({}), &state, &comments).unwrap();
        {
            let wb = state.workbook.lock().unwrap();
            let sheet = wb.sheet(0).unwrap();
            assert_eq!(sheet.cell_value(0, 0), Some("=B1".into()));
            assert_eq!(sheet.cell_value(0, 1), None);
        }
        let graph = state.dep_graphs.lock().unwrap();
        let stable_id = state.workbook.lock().unwrap().sheet(0).unwrap().stable_id();
        assert!(graph
            .get_dependencies_key(CellKey::new(stable_id, 0, 0))
            .unwrap()
            .contains(&CellKey::new(stable_id, 0, 1)));
        assert!(graph
            .get_dependencies_key(CellKey::new(stable_id, 0, 1))
            .is_none());
    }

    #[test]
    fn clear_cell_removes_formula_edges_from_dependency_graph() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        begin_transaction(serde_json::json!({}), &state, &comments).unwrap();

        set_cell_value_in_workbook(&state, 0, 0, 0, "=B1".into()).unwrap();
        clear_cell_in_workbook(&state, 0, 0, 0).unwrap();
        commit_transaction(serde_json::json!({}), &state, &comments).unwrap();

        let graph = state.dep_graphs.lock().unwrap();
        let stable_id = state.workbook.lock().unwrap().sheet(0).unwrap().stable_id();
        assert!(graph
            .get_dependencies_key(CellKey::new(stable_id, 0, 0))
            .is_none());
        assert!(graph
            .get_dependents_key(CellKey::new(stable_id, 0, 1))
            .map(|deps| deps.is_empty())
            .unwrap_or(true));
    }

    #[test]
    fn non_formula_edit_removes_prior_formula_edges() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        begin_transaction(serde_json::json!({}), &state, &comments).unwrap();

        set_cell_value_in_workbook(&state, 0, 0, 0, "=B1".into()).unwrap();
        set_cell_value_in_workbook(&state, 0, 0, 0, "plain text".into()).unwrap();
        commit_transaction(serde_json::json!({}), &state, &comments).unwrap();

        let graph = state.dep_graphs.lock().unwrap();
        let stable_id = state.workbook.lock().unwrap().sheet(0).unwrap().stable_id();
        assert!(graph
            .get_dependencies_key(CellKey::new(stable_id, 0, 0))
            .is_none());
        assert!(graph
            .get_dependents_key(CellKey::new(stable_id, 0, 1))
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
    fn sheet_snapshot_preserves_blank_formats_without_duplicate_cells() {
        let mut workbook = Workbook::new();
        let sheet = workbook.sheet_mut(0).unwrap();
        let format = CellFormat::new().bold(true).bg_color("#ffeecc");
        sheet.set_cell_value(0, 0, "12".into());
        sheet.set_format(0, 0, format.clone());
        sheet.set_format(999_999, 16_383, format.clone());

        let snapshot = sheet_data(&workbook, 0).unwrap();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot.iter().filter(|cell| cell.row == 0).count(), 1);
        let blank = snapshot.iter().find(|cell| cell.row == 999_999).unwrap();
        assert_eq!(blank.col, 16_383);
        assert_eq!(blank.value, "");
        assert_eq!(blank.display, "");
        assert_eq!(blank.cell_type, "empty");
        assert_eq!(blank.format, Some(format));
    }

    #[test]
    fn formula_presence_checks_all_sheets_without_projecting_values() {
        let mut workbook = Workbook::new();
        workbook
            .sheet_mut(0)
            .unwrap()
            .set_cell_value(0, 0, "42".into());
        assert!(!has_workbook_formulas(&workbook));
        let second = workbook.add_sheet("Other").unwrap();
        workbook
            .sheet_mut(second)
            .unwrap()
            .set_cell_value(0, 0, "=1+1".into());
        assert!(has_workbook_formulas(&workbook));
    }

    #[test]
    fn snapshot_numeric_values_use_calculated_numbers_before_display_formatting() {
        let mut workbook = Workbook::new();
        let sheet = workbook.sheet_mut(0).unwrap();
        sheet.set_cell_value(0, 0, "1250".into());
        sheet.set_cell_value(0, 1, "=A1*10%".into());
        sheet.set_cell(0, 2, CellValue::text("1250"));
        sheet.set_cell_value(0, 3, "=1/0".into());
        sheet.set_cell_value(0, 4, "TRUE".into());
        sheet.set_format(0, 0, CellFormat::new().number_format("#,##0.00"));
        sheet.set_format(0, 1, CellFormat::new().number_format("$#,##0.00"));
        let data = sheet_data(&workbook, 0).unwrap();
        let get = |col| data.iter().find(|cell| cell.col == col).unwrap();
        assert_eq!(get(0).numeric_value, Some(1250.0));
        assert_eq!(get(1).numeric_value, Some(125.0));
        assert_ne!(get(0).display, "1250");
        assert_eq!(get(2).numeric_value, None);
        assert_eq!(get(3).numeric_value, None);
        assert_eq!(get(4).numeric_value, None);
    }

    #[test]
    fn repeated_formula_dependencies_are_evaluated_once_per_read() {
        let mut workbook = Workbook::new();
        let sheet = workbook.sheet_mut(0).unwrap();
        sheet.set_cell_value(0, 0, "1".into());
        for row in 1..=40 {
            sheet.set_cell_value(row, 0, format!("=A{row}+A{row}"));
        }
        let session = EvaluationSession::default();
        let provider = TauriProvider {
            workbook: &workbook,
            current_sheet_id: workbook.sheet(0).unwrap().stable_id(),
            session: &session,
        };
        assert_eq!(provider.get_cell(40, 0), Value::Number(2_f64.powi(40)));
        assert_eq!(session.cached.borrow().len(), 40);
        assert!(session.steps.get() < 300);
        assert_eq!(session.depth.get(), 0);
        assert!(session.visiting.borrow().is_empty());
        let steps = session.steps.get();
        assert_eq!(provider.get_cell(40, 0), Value::Number(2_f64.powi(40)));
        assert_eq!(session.steps.get(), steps + 1);
    }

    #[test]
    fn deep_formula_chains_and_work_budget_exhaustion_return_errors() {
        let mut workbook = Workbook::new();
        let sheet = workbook.sheet_mut(0).unwrap();
        sheet.set_cell_value(0, 0, "1".into());
        for row in 1..=200 {
            sheet.set_cell_value(row, 0, format!("=A{row}"));
        }
        let session = EvaluationSession::default();
        let provider = TauriProvider {
            workbook: &workbook,
            current_sheet_id: workbook.sheet(0).unwrap().stable_id(),
            session: &session,
        };
        assert!(matches!(provider.get_cell(200, 0), Value::Error(_)));
        assert_eq!(session.depth.get(), 0);
        assert!(session.visiting.borrow().is_empty());
        assert_eq!(provider.get_cell(1, 0), Value::Number(1.0));
        session.steps.set(MAX_EVALUATION_STEPS);
        assert_eq!(provider.snapshot_cell(1, 0), Value::Number(1.0));
        assert!(matches!(provider.get_cell(1, 0), Value::Error(_)));
        assert!(matches!(provider.get_cell(200, 0), Value::Error(_)));
        assert_eq!(session.steps.get(), MAX_EVALUATION_STEPS);
    }

    #[test]
    fn formula_snapshot_limits_are_deterministic_across_insertion_orders() {
        let build = |reverse: bool| {
            let mut workbook = Workbook::new();
            let sheet = workbook.sheet_mut(0).unwrap();
            for offset in 0..200 {
                let row = if reverse { 199 - offset } else { offset };
                sheet.set_cell_value(
                    row,
                    0,
                    if row == 199 {
                        "1".into()
                    } else {
                        format!("=A{}", row + 2)
                    },
                );
            }
            sheet.set_format(800, 1, CellFormat::new().bold(true));
            workbook
        };
        let forward = sheet_data(&build(false), 0).unwrap();
        let reverse = sheet_data(&build(true), 0).unwrap();
        assert_eq!(
            serde_json::to_value(forward).unwrap(),
            serde_json::to_value(reverse).unwrap()
        );
    }

    #[test]
    fn formula_value_copy_budget_bounds_cached_arrays_and_text() {
        let mut workbook = Workbook::new();
        let sheet = workbook.sheet_mut(0).unwrap();
        sheet.set_cell_value(0, 0, "text".into());
        sheet.set_cell_value(0, 1, "=A1:A1".into());
        let session = EvaluationSession::default();
        let provider = TauriProvider {
            workbook: &workbook,
            current_sheet_id: workbook.sheet(0).unwrap().stable_id(),
            session: &session,
        };
        assert_eq!(
            provider.get_cell(0, 1),
            Value::Array(vec![Value::String("text".into())])
        );
        session.value_bytes.set(MAX_EVALUATION_VALUE_BYTES - 3);
        assert!(matches!(provider.get_cell(0, 1), Value::Error(_)));
        assert!(matches!(provider.get_cell(0, 0), Value::Error(_)));
        assert_eq!(session.value_bytes.get(), MAX_EVALUATION_VALUE_BYTES - 3);
    }

    #[test]
    fn malformed_csv_leaves_pending_workbook_unchanged() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        begin_transaction(serde_json::json!({}), &state, &comments).unwrap();
        set_cell_value_in_workbook(&state, 0, 0, 0, "original".into()).unwrap();
        assert!(
            import_csv_data_inner("replacement\n\"unterminated", ',', Some(0), &state).is_err()
        );
        commit_transaction(serde_json::json!({}), &state, &comments).unwrap();
        assert_eq!(
            state
                .workbook
                .lock()
                .unwrap()
                .sheet(0)
                .unwrap()
                .cell_value(0, 0),
            Some("original".into())
        );
    }

    #[test]
    fn guarded_move_is_one_undoable_transaction_including_overlapping_cells() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        {
            let mut workbook = state.workbook.lock().unwrap();
            let sheet = workbook.sheet_mut(0).unwrap();
            sheet.set_cell_value(0, 0, "first".into());
            sheet.set_cell_value(1, 0, "second".into());
        }
        let source = [
            CellChange {
                row: 0,
                col: 0,
                value: "first".into(),
            },
            CellChange {
                row: 1,
                col: 0,
                value: "second".into(),
            },
        ];
        let changes = [
            CellChange {
                row: 0,
                col: 0,
                value: String::new(),
            },
            CellChange {
                row: 1,
                col: 0,
                value: "first".into(),
            },
            CellChange {
                row: 2,
                col: 0,
                value: "second".into(),
            },
        ];
        begin_transaction(serde_json::json!({}), &state, &comments).unwrap();
        move_cells_in_workbook(&state, 0, "1", &source, &changes).unwrap();
        commit_transaction(serde_json::json!({}), &state, &comments).unwrap();
        {
            let workbook = state.workbook.lock().unwrap();
            let sheet = workbook.sheet(0).unwrap();
            assert_eq!(sheet.cell_value(0, 0), None);
            assert_eq!(sheet.cell_value(1, 0), Some("first".into()));
            assert_eq!(sheet.cell_value(2, 0), Some("second".into()));
        }
        assert_eq!(state.history.lock().unwrap().undo.len(), 1);
        restore_history_transaction(&state, &comments, false).unwrap();
        let workbook = state.workbook.lock().unwrap();
        let sheet = workbook.sheet(0).unwrap();
        assert_eq!(sheet.cell_value(0, 0), Some("first".into()));
        assert_eq!(sheet.cell_value(1, 0), Some("second".into()));
        assert_eq!(sheet.cell_value(2, 0), None);
    }

    #[test]
    fn guarded_move_rechecks_formula_and_source_changes_inside_candidate() {
        for add_formula in [false, true] {
            let state = test_state();
            let comments = Mutex::new(SheetComments::default());
            state
                .workbook
                .lock()
                .unwrap()
                .sheet_mut(0)
                .unwrap()
                .set_cell_value(0, 0, "original".into());
            begin_transaction(serde_json::json!({}), &state, &comments).unwrap();
            if add_formula {
                with_pending_mutation(&state, |pending| {
                    let other = pending.workbook.add_sheet("Other").unwrap();
                    pending
                        .workbook
                        .sheet_mut(other)
                        .unwrap()
                        .set_cell_value(0, 0, "=1+1".into());
                    Ok(())
                })
                .unwrap();
            } else {
                set_cell_value_in_workbook(&state, 0, 0, 0, "new value".into()).unwrap();
            }
            let source = [CellChange {
                row: 0,
                col: 0,
                value: "original".into(),
            }];
            let changes = [
                CellChange {
                    row: 0,
                    col: 0,
                    value: String::new(),
                },
                CellChange {
                    row: 0,
                    col: 1,
                    value: "original".into(),
                },
            ];
            assert!(move_cells_in_workbook(&state, 0, "1", &source, &changes).is_err());
            commit_transaction(serde_json::json!({}), &state, &comments).unwrap();
            let workbook = state.workbook.lock().unwrap();
            let sheet = workbook.sheet(0).unwrap();
            assert_eq!(
                sheet.cell_value(0, 0),
                Some(if add_formula { "original" } else { "new value" }.into())
            );
            assert_eq!(sheet.cell_value(0, 1), None);
        }
    }

    #[test]
    fn guarded_move_rejects_changed_sheet_locked_destinations_and_invalid_coordinates() {
        for failure in ["sheet identity", "locked", "bounds"] {
            let state = test_state();
            let comments = Mutex::new(SheetComments::default());
            state
                .workbook
                .lock()
                .unwrap()
                .sheet_mut(0)
                .unwrap()
                .set_cell_value(0, 0, "original".into());
            begin_transaction(serde_json::json!({}), &state, &comments).unwrap();
            if failure == "locked" {
                with_pending_mutation(&state, |pending| {
                    let mut protection = SheetProtection::default();
                    protection.protect("");
                    pending.protections.push(protection);
                    let mut locks = CellLockManager::new();
                    locks.set_locked(0, 0, false);
                    pending.cell_locks.push(locks);
                    Ok(())
                })
                .unwrap();
            }
            let source = [CellChange {
                row: 0,
                col: 0,
                value: "original".into(),
            }];
            let changes = [
                CellChange {
                    row: 0,
                    col: 0,
                    value: String::new(),
                },
                CellChange {
                    row: if failure == "bounds" { 1_000_000 } else { 0 },
                    col: 1,
                    value: "original".into(),
                },
            ];
            assert!(move_cells_in_workbook(
                &state,
                0,
                if failure == "sheet identity" {
                    "2"
                } else {
                    "1"
                },
                &source,
                &changes
            )
            .is_err());
            with_pending_mutation(&state, |pending| {
                let sheet = pending.workbook.sheet(0).unwrap();
                assert_eq!(sheet.cell_value(0, 0), Some("original".into()));
                assert_eq!(sheet.cell_value(0, 1), None);
                Ok(())
            })
            .unwrap();
            abort_transaction(&state).unwrap();
        }
    }

    #[test]
    fn workbook_provider_evaluates_cross_sheet_chain_and_reflects_source_edits() {
        let mut workbook = Workbook::new();
        workbook.rename_sheet(0, "Annual Budget").unwrap();
        let report = workbook.add_sheet("Report").unwrap();
        workbook
            .sheet_mut(0)
            .unwrap()
            .set_cell_value(0, 0, "10".into());
        workbook
            .sheet_mut(report)
            .unwrap()
            .set_cell_value(0, 0, "='Annual Budget'!A1*2".into());

        let evaluate = |workbook: &Workbook| {
            let report_sheet = workbook.sheet(report).unwrap();
            let session = EvaluationSession::default();
            let provider = TauriProvider {
                workbook,
                current_sheet_id: report_sheet.stable_id(),
                session: &session,
            };
            let expr = Parser::parse_formula(&report_sheet.cell(0, 0).unwrap().raw).unwrap();
            Evaluator::new().evaluate(&expr, &provider)
        };

        assert_eq!(evaluate(&workbook), Value::Number(20.0));
        workbook
            .sheet_mut(0)
            .unwrap()
            .set_cell_value(0, 0, "25".into());
        assert_eq!(evaluate(&workbook), Value::Number(50.0));
    }

    #[test]
    fn dependency_rebuild_rejects_cross_sheet_cycle() {
        let mut workbook = Workbook::new();
        workbook.rename_sheet(0, "First").unwrap();
        let second = workbook.add_sheet("Second").unwrap();
        workbook
            .sheet_mut(0)
            .unwrap()
            .set_cell_value(0, 0, "=Second!A1".into());
        workbook
            .sheet_mut(second)
            .unwrap()
            .set_cell_value(0, 0, "=First!A1".into());
        assert!(dependency_graphs_for_workbook(&workbook).is_err());
    }

    #[test]
    fn new_sheet_csv_import_rolls_back_workbook_and_graph_on_cycle() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        let original_id = state.workbook.lock().unwrap().sheet(0).unwrap().stable_id();
        begin_transaction(serde_json::json!({}), &state, &comments).unwrap();

        let result = import_csv_data_inner("=Sheet2!A1", ',', None, &state);

        assert!(result.is_err());
        abort_transaction(&state).unwrap();
        let workbook = state.workbook.lock().unwrap();
        assert_eq!(workbook.sheet_count(), 1);
        assert_eq!(workbook.sheet(0).unwrap().stable_id(), original_id);
        drop(workbook);
        assert!(state
            .dep_graphs
            .lock()
            .unwrap()
            .topological_sort()
            .is_empty());
    }

    #[test]
    fn new_sheet_csv_import_skips_existing_generated_name() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        state.workbook.lock().unwrap().add_sheet("Sheet2").unwrap();
        begin_transaction(serde_json::json!({}), &state, &comments).unwrap();

        import_csv_data_inner("value", ',', None, &state).unwrap();
        commit_transaction(serde_json::json!({}), &state, &comments).unwrap();

        let workbook = state.workbook.lock().unwrap();
        let names: Vec<_> = workbook.sheets().iter().map(|sheet| sheet.name()).collect();
        assert_eq!(names, vec!["Sheet1", "Sheet2", "Sheet3"]);
    }

    #[test]
    fn repeated_pivot_sheet_creation_allocates_unique_names() {
        let mut workbook = Workbook::new();
        workbook.rename_sheet(0, "Data").unwrap();
        let result = PivotResult {
            row_headers: Vec::new(),
            column_headers: Vec::new(),
            data: Vec::new(),
            grand_totals_row: Vec::new(),
            grand_totals_col: Vec::new(),
            value_field_labels: Vec::new(),
        };

        add_pivot_result_sheet(&mut workbook, 0, &result).unwrap();
        add_pivot_result_sheet(&mut workbook, 0, &result).unwrap();

        assert_eq!(workbook.sheet(1).unwrap().name(), "Pivot (Data)");
        assert_eq!(workbook.sheet(2).unwrap().name(), "Pivot (Data) 2");
    }

    #[test]
    fn generated_desktop_sheet_uses_shared_allocator_after_rename() {
        let mut workbook = Workbook::new();
        workbook.rename_sheet(0, "Sheet2").unwrap();
        workbook.add_sheet("Data").unwrap();

        let index = add_generated_sheet_to_workbook(&mut workbook).unwrap();

        assert_eq!(workbook.sheet(index).unwrap().name(), "Sheet3");
    }

    #[test]
    fn plain_value_edit_preserves_cross_sheet_dependencies_incrementally() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        {
            let mut workbook = state.workbook.lock().unwrap();
            workbook.rename_sheet(0, "Data").unwrap();
            workbook.add_sheet("Report").unwrap();
            *state.dep_graphs.lock().unwrap() = dependency_graphs_for_workbook(&workbook).unwrap();
        }
        begin_transaction(serde_json::json!({}), &state, &comments).unwrap();
        set_cell_value_in_workbook(&state, 1, 0, 0, "=Data!A1".into()).unwrap();
        let (data_id, report_id) = {
            let workbook = state.workbook.lock().unwrap();
            (
                workbook.sheet(0).unwrap().stable_id(),
                workbook.sheet(1).unwrap().stable_id(),
            )
        };

        set_cell_value_in_workbook(&state, 0, 0, 0, "42".into()).unwrap();
        commit_transaction(serde_json::json!({}), &state, &comments).unwrap();

        let graph = state.dep_graphs.lock().unwrap();
        assert!(graph
            .get_dependencies_key(CellKey::new(report_id, 0, 0))
            .unwrap()
            .contains(&CellKey::new(data_id, 0, 0)));
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

    #[test]
    fn transaction_history_cost_scales_with_changed_cells_not_workbook_size() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        {
            let mut workbook = state.workbook.lock().unwrap();
            let sheet = workbook.sheet_mut(0).unwrap();
            for row in 0..100 {
                for col in 0..100 {
                    sheet.set_cell_value(row, col, format!("{row}:{col}"));
                }
            }
        }
        begin_transaction(serde_json::json!({}), &state, &comments).unwrap();
        set_cell_value_in_workbook(&state, 0, 50, 50, "changed".into()).unwrap();
        commit_transaction(serde_json::json!({}), &state, &comments).unwrap();

        let history = state.history.lock().unwrap();
        let transaction = history.undo.last().unwrap();
        assert_eq!(transaction_coordinate_count(transaction), 1);
        assert!(matches!(
            transaction.operations.as_slice(),
            [WorkbookDelta::Cells(values)] if values.len() == 1
        ));
        assert!(transaction.serialized_bytes < 4096);
    }

    #[test]
    fn one_transaction_undoes_and_redoes_cell_format_and_metadata_together() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        let stable_id = state.workbook.lock().unwrap().sheet(0).unwrap().stable_id();
        begin_transaction(
            serde_json::json!({"sheet_states": {stable_id.to_string(): {"frozenRowCount": 0}}}),
            &state,
            &comments,
        )
        .unwrap();
        set_cell_value_in_workbook(&state, 0, 0, 0, "42".into()).unwrap();
        let format = CellFormat {
            bold: Some(true),
            ..CellFormat::default()
        };
        with_pending_mutation(&state, |pending| {
            pending
                .workbook
                .sheet_mut(0)
                .unwrap()
                .set_format(0, 0, format);
            Ok(())
        })
        .unwrap();
        commit_transaction(
            serde_json::json!({"sheet_states": {stable_id.to_string(): {"frozenRowCount": 2}}}),
            &state,
            &comments,
        )
        .unwrap();

        let undone = restore_history_transaction(&state, &comments, false).unwrap();
        assert_eq!(
            undone.metadata["sheet_states"][stable_id.to_string()]["frozenRowCount"],
            0
        );
        {
            let workbook = state.workbook.lock().unwrap();
            assert_eq!(workbook.sheet(0).unwrap().cell_value(0, 0), None);
            assert_eq!(workbook.sheet(0).unwrap().get_format(0, 0), None);
        }

        let redone = restore_history_transaction(&state, &comments, true).unwrap();
        assert_eq!(
            redone.metadata["sheet_states"][stable_id.to_string()]["frozenRowCount"],
            2
        );
        let workbook = state.workbook.lock().unwrap();
        assert_eq!(
            workbook.sheet(0).unwrap().cell_value(0, 0),
            Some("42".into())
        );
        assert_eq!(
            workbook.sheet(0).unwrap().get_format(0, 0).unwrap().bold,
            Some(true)
        );
    }

    #[test]
    fn view_and_print_metadata_roundtrip_through_undo_and_redo() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        let stable_id = state.workbook.lock().unwrap().sheet(0).unwrap().stable_id();
        let before = serde_json::json!({
            "sheet_states": {
                stable_id.to_string(): {
                    "showGridlines": true,
                    "printPageSize": "Letter",
                    "printOrientation": "Portrait"
                }
            }
        });
        let after = serde_json::json!({
            "sheet_states": {
                stable_id.to_string(): {
                    "showGridlines": false,
                    "printPageSize": "A4",
                    "printOrientation": "Landscape"
                }
            }
        });

        begin_transaction(before.clone(), &state, &comments).unwrap();
        commit_transaction(after.clone(), &state, &comments).unwrap();

        let undone = restore_history_transaction(&state, &comments, false).unwrap();
        assert_eq!(undone.metadata, before);

        let redone = restore_history_transaction(&state, &comments, true).unwrap();
        assert_eq!(redone.metadata, after);
    }

    #[test]
    fn native_metadata_scopes_backend_features_by_stable_sheet_id() {
        let mut workbook = Workbook::new();
        workbook.add_sheet("Second").unwrap();
        let second_id = workbook.sheet(1).unwrap().stable_id().to_string();
        let mut comments = SheetComments::default();
        comments.comments.insert(
            (1, 2, 3),
            CellComment {
                row: 2,
                col: 3,
                text: "second-sheet note".into(),
                author: "author".into(),
                visible: false,
            },
        );
        let protections = vec![SheetProtection::default(), SheetProtection::default()];
        let mut second_locks = CellLockManager::new();
        second_locks.set_locked(2, 3, false);
        let locks = vec![CellLockManager::new(), second_locks];

        let metadata = metadata_with_backend_sheet_states(
            &workbook,
            serde_json::json!({"sheet_states": {}}),
            &comments,
            &protections,
            &locks,
        )
        .unwrap();

        assert!(metadata.get("comments").is_none());
        assert_eq!(
            metadata["sheet_states"][&second_id]["comments"][0]["text"],
            "second-sheet note"
        );
        assert_eq!(
            metadata["sheet_states"][&second_id]["cell_locks"][0],
            serde_json::json!([2, 3, false])
        );
    }

    #[test]
    fn feature_only_transaction_restores_comments_on_undo_and_redo() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        begin_transaction(serde_json::json!({}), &state, &comments).unwrap();
        with_pending_mutation(&state, |pending| {
            pending.comments.comments.insert(
                (0, 4, 5),
                CellComment {
                    row: 4,
                    col: 5,
                    text: "transactional note".into(),
                    author: "author".into(),
                    visible: false,
                },
            );
            Ok(())
        })
        .unwrap();
        commit_transaction(serde_json::json!({}), &state, &comments).unwrap();

        restore_history_transaction(&state, &comments, false).unwrap();
        assert!(comments.lock().unwrap().comments.is_empty());

        restore_history_transaction(&state, &comments, true).unwrap();
        assert_eq!(
            comments.lock().unwrap().comments[&(0, 4, 5)].text,
            "transactional note"
        );
    }

    #[test]
    fn mutation_without_transaction_is_rejected_and_live_state_is_unchanged() {
        let state = test_state();
        let result = set_cell_value_in_workbook(&state, 0, 0, 0, "blocked".into());
        assert!(result.is_err());
        assert_eq!(
            state
                .workbook
                .lock()
                .unwrap()
                .sheet(0)
                .unwrap()
                .cell_value(0, 0),
            None
        );
    }

    #[test]
    fn budget_overflow_keeps_live_state_and_history_unchanged() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        begin_transaction(serde_json::json!({}), &state, &comments).unwrap();
        set_cell_value_in_workbook(&state, 0, 0, 0, "x".repeat(MAX_TRANSACTION_BYTES + 1)).unwrap();

        assert!(commit_transaction(serde_json::json!({}), &state, &comments).is_err());
        assert_eq!(
            state
                .workbook
                .lock()
                .unwrap()
                .sheet(0)
                .unwrap()
                .cell_value(0, 0),
            None
        );
        assert!(state.history.lock().unwrap().undo.is_empty());
        assert!(state.history.lock().unwrap().pending.is_some());
        abort_transaction(&state).unwrap();
    }

    #[test]
    fn serialized_history_size_counts_large_cell_format_name_and_sheet_payloads() {
        let mut sheet = Sheet::with_stable_id(99, format!("Sheet-{}", "n".repeat(128 * 1024)));
        sheet.set_cell_value(0, 0, "v".repeat(512 * 1024));
        sheet.set_format(
            0,
            0,
            CellFormat {
                font_name: Some("f".repeat(256 * 1024)),
                number_format: Some("0".repeat(256 * 1024)),
                ..CellFormat::default()
            },
        );
        let mut transaction = WorkbookTransaction {
            operations: vec![WorkbookDelta::AddedSheet { index: 1, sheet }],
            metadata_before: serde_json::json!({"large": "m".repeat(128 * 1024)}),
            metadata_after: serde_json::json!({}),
            backend_state_before: serde_json::json!({}),
            backend_state_after: serde_json::json!({}),
            serialized_bytes: 0,
        };
        transaction.serialized_bytes = transaction_serialized_size(&transaction).unwrap();
        assert!(transaction.serialized_bytes > 1_200_000);
    }

    #[test]
    fn aggregate_history_eviction_obeys_count_and_byte_budgets() {
        let make = |bytes| WorkbookTransaction {
            operations: Vec::new(),
            metadata_before: serde_json::json!({}),
            metadata_after: serde_json::json!({}),
            backend_state_before: serde_json::json!({}),
            backend_state_after: serde_json::json!({}),
            serialized_bytes: bytes,
        };
        let mut history = vec![
            make(24 * 1024 * 1024),
            make(24 * 1024 * 1024),
            make(24 * 1024 * 1024),
        ];
        evict_history_to_budget(&mut history);
        assert_eq!(history.len(), 2);
        assert!(
            history
                .iter()
                .map(|item| item.serialized_bytes)
                .sum::<usize>()
                <= MAX_HISTORY_BYTES
        );

        history.extend((0..MAX_TRANSACTION_HISTORY + 5).map(|_| make(1)));
        evict_history_to_budget(&mut history);
        assert_eq!(history.len(), MAX_TRANSACTION_HISTORY);
    }

    #[test]
    fn failed_multi_delta_restore_does_not_partially_mutate_or_move_history() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        let stable_id = state.workbook.lock().unwrap().sheet(0).unwrap().stable_id();
        let mut value_sheet = Sheet::new("value");
        value_sheet.set_cell_value(0, 0, "42".into());
        let transaction = WorkbookTransaction {
            operations: vec![
                WorkbookDelta::Cells(vec![CellDelta {
                    sheet_stable_id: stable_id,
                    row: 0,
                    col: 0,
                    before: None,
                    after: value_sheet.cell(0, 0).cloned(),
                }]),
                WorkbookDelta::Formats(vec![FormatDelta {
                    sheet_stable_id: u64::MAX,
                    row: 0,
                    col: 0,
                    before: None,
                    after: Some(CellFormat::default().bold(true)),
                }]),
            ],
            metadata_before: serde_json::json!({}),
            metadata_after: serde_json::json!({}),
            backend_state_before: serde_json::json!({"sheet_states": {}}),
            backend_state_after: serde_json::json!({"sheet_states": {}}),
            serialized_bytes: 1024,
        };
        state.history.lock().unwrap().redo.push(transaction);

        assert!(restore_history_transaction(&state, &comments, true).is_err());
        assert_eq!(
            state
                .workbook
                .lock()
                .unwrap()
                .sheet(0)
                .unwrap()
                .cell_value(0, 0),
            None
        );
        let history = state.history.lock().unwrap();
        assert_eq!(history.redo.len(), 1);
        assert!(history.undo.is_empty());
    }

    #[test]
    fn csv_import_is_undoable_as_one_transaction() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        begin_transaction(serde_json::json!({}), &state, &comments).unwrap();
        import_csv_data_inner("name,value\nalpha,42", ',', Some(0), &state).unwrap();
        commit_transaction(serde_json::json!({}), &state, &comments).unwrap();
        assert_eq!(
            state
                .workbook
                .lock()
                .unwrap()
                .sheet(0)
                .unwrap()
                .cell_value(1, 1),
            Some("42".into())
        );

        restore_history_transaction(&state, &comments, false).unwrap();
        assert_eq!(
            state
                .workbook
                .lock()
                .unwrap()
                .sheet(0)
                .unwrap()
                .cell_value(1, 1),
            None
        );
    }

    #[test]
    fn deleted_sheet_metadata_stays_filtered_through_undo_redo_and_reopen() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        state.workbook.lock().unwrap().add_sheet("Second").unwrap();
        let ids: Vec<String> = state
            .workbook
            .lock()
            .unwrap()
            .sheets()
            .iter()
            .map(|sheet| sheet.stable_id().to_string())
            .collect();
        let before = serde_json::json!({"sheet_states": {
            ids[0].clone(): {"frozenRowCount": 0},
            ids[1].clone(): {"frozenRowCount": 2}
        }});
        begin_transaction(before.clone(), &state, &comments).unwrap();
        with_pending_mutation(&state, |pending| {
            pending.workbook.delete_sheet(1);
            pending.dep_graph = dependency_graphs_for_workbook(&pending.workbook)?;
            Ok(())
        })
        .unwrap();
        let after = serde_json::json!({"sheet_states": {ids[0].clone(): {"frozenRowCount": 0}}});
        commit_transaction(after.clone(), &state, &comments).unwrap();
        let undone = restore_history_transaction(&state, &comments, false).unwrap();
        assert!(undone.metadata["sheet_states"].get(&ids[1]).is_some());
        let redone = restore_history_transaction(&state, &comments, true).unwrap();
        assert!(redone.metadata["sheet_states"].get(&ids[1]).is_none());

        let workbook = state.workbook.lock().unwrap();
        let metadata = metadata_with_backend_sheet_states(
            &workbook,
            serde_json::json!({"sheet_states": {
                ids[0].clone(): {},
                ids[1].clone(): {},
                "999999": {}
            }}),
            &comments.lock().unwrap(),
            &state.protections.lock().unwrap(),
            &state.cell_locks.lock().unwrap(),
        )
        .unwrap();
        assert!(metadata["sheet_states"].get(&ids[1]).is_none());
        assert!(metadata["sheet_states"].get("999999").is_none());
        let encoded =
            sheets_json::export_native_workbook_with_metadata(&workbook, metadata).unwrap();
        let (_, reopened_metadata) =
            sheets_json::import_native_workbook_with_metadata(&encoded).unwrap();
        assert!(reopened_metadata["sheet_states"].get(&ids[1]).is_none());
    }

    #[test]
    fn recovery_store_discovers_restores_discards_and_never_touches_source() {
        let root = std::env::temp_dir().join(format!(
            "900sheets-recovery-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source = root.with_extension("900sheets");
        std::fs::write(&source, "source remains unchanged").unwrap();
        let store = RecoveryStore::new(root.clone());
        store.write("session_1", b"first recovery").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(store.path("session_1").unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        assert_eq!(store.list().unwrap()[0].id, "session_1");
        assert_eq!(store.read("session_1").unwrap(), "first recovery");
        assert_eq!(
            std::fs::read_to_string(&source).unwrap(),
            "source remains unchanged"
        );
        store.write("session_1", b"replacement recovery").unwrap();
        assert_eq!(store.read("session_1").unwrap(), "replacement recovery");
        store.discard("session_1").unwrap();
        assert!(store.list().unwrap().is_empty());

        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_file(source);
    }

    #[test]
    fn failed_recovery_write_retains_last_good_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "900sheets-recovery-failure-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let store = RecoveryStore::new(root.clone());
        store.write("active", b"last good").unwrap();
        store.fail_before_rename.store(true, Ordering::SeqCst);

        assert!(store.write("active", b"incomplete").is_err());
        assert_eq!(store.read("active").unwrap(), "last good");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn failed_cleanup_retires_snapshot_from_discovery_and_is_retryable() {
        let root = std::env::temp_dir().join(format!(
            "900sheets-recovery-cleanup-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let store = RecoveryStore::new(root.clone());
        store.write("active", b"stale workbook").unwrap();
        store
            .fail_cleanup_after_retire
            .store(true, Ordering::SeqCst);

        assert!(store.discard("active").is_err());
        assert!(store.list().unwrap().is_empty());
        assert!(!store.path("active").unwrap().exists());
        assert!(store.cleanup_path("active").unwrap().exists());

        store.discard("active").unwrap();
        assert!(!store.cleanup_path("active").unwrap().exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn overlapping_recovery_writes_are_complete_and_leave_no_temp_files() {
        let root = std::env::temp_dir().join(format!(
            "900sheets-recovery-overlap-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let store = RecoveryStore::new(root.clone());
        let first = store.clone();
        let second = store.clone();
        let one = std::thread::spawn(move || first.write("active", &vec![b'a'; 64 * 1024]));
        let two = std::thread::spawn(move || second.write("active", &vec![b'b'; 64 * 1024]));
        one.join().unwrap().unwrap();
        two.join().unwrap().unwrap();

        let contents = store.read("active").unwrap();
        assert!(
            contents.as_bytes() == vec![b'a'; 64 * 1024]
                || contents.as_bytes() == vec![b'b'; 64 * 1024]
        );
        assert!(std::fs::read_dir(&root).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn recovery_store_rejects_symlink_root_and_target() {
        use std::os::unix::fs::symlink;
        let base = std::env::temp_dir().join(format!(
            "900sheets-recovery-symlink-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir(&base).unwrap();
        let real_root = base.join("real");
        std::fs::create_dir(&real_root).unwrap();
        let linked_root = base.join("linked");
        symlink(&real_root, &linked_root).unwrap();
        assert!(RecoveryStore::new(linked_root)
            .write("active", b"data")
            .is_err());

        let outside = base.join("outside");
        std::fs::write(&outside, "outside").unwrap();
        let store = RecoveryStore::new(real_root);
        symlink(&outside, store.path("active").unwrap()).unwrap();
        assert!(store.write("active", b"replacement").is_err());
        assert_eq!(std::fs::read_to_string(outside).unwrap(), "outside");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn corrupt_recovery_is_quarantined_from_discovery() {
        let root = std::env::temp_dir().join(format!(
            "900sheets-recovery-corrupt-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let store = RecoveryStore::new(root.clone());
        store.write("active", b"not a native workbook").unwrap();
        let contents = store.read("active").unwrap();
        assert!(sheets_json::import_native_workbook_with_metadata(&contents).is_err());
        store.quarantine("active").unwrap();
        assert!(store.list().unwrap().is_empty());
        assert!(std::fs::read_dir(&root).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".corrupt.")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn atomic_native_write_replaces_existing_file_and_cleans_unique_temporary_files() {
        let root = std::env::temp_dir().join(format!(
            "900sheets-atomic-native-test-{}-{}",
            std::process::id(),
            PRIVATE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let target = root.join("Workbook.900sheets");
        write_atomic(&target, b"first").unwrap();
        write_atomic(&target, b"second").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"second");
        assert!(std::fs::read_dir(&root).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn interrupted_native_write_retains_last_good_file() {
        let root = std::env::temp_dir().join(format!(
            "900sheets-atomic-interruption-test-{}-{}",
            std::process::id(),
            PRIVATE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let target = root.join("Workbook.900sheets");
        write_atomic(&target, b"last good").unwrap();
        assert!(write_atomic_with_interruption(&target, b"incomplete", true).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"last good");
        assert!(std::fs::read_dir(&root).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn interrupted_exchange_export_write_retains_last_good_file() {
        let root = std::env::temp_dir().join(format!(
            "900sheets-exchange-atomic-interruption-test-{}-{}",
            std::process::id(),
            PRIVATE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let target = root.join("Workbook.xlsx");
        write_atomic(&target, b"last good exchange export").unwrap();
        assert!(write_atomic_with_interruption(&target, b"incomplete", true).is_err());
        assert_eq!(
            std::fs::read(&target).unwrap(),
            b"last good exchange export"
        );
        assert!(std::fs::read_dir(&root).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn post_replace_directory_sync_failure_is_a_warning_not_a_false_write_failure() {
        let root = std::env::temp_dir().join(format!(
            "900sheets-post-replace-warning-test-{}-{}",
            std::process::id(),
            PRIVATE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let target = root.join("Workbook.900sheets");
        write_atomic(&target, b"old").unwrap();

        let warning = write_atomic_with_faults(&target, b"new", false, true)
            .unwrap()
            .expect("post-replace sync failure should be reported as a warning");
        assert!(warning.contains("file was replaced"));
        assert_eq!(std::fs::read(&target).unwrap(), b"new");

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn atomic_native_write_rejects_symlink_target() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!(
            "900sheets-atomic-symlink-test-{}-{}",
            std::process::id(),
            PRIVATE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let outside = root.join("outside");
        let target = root.join("Workbook.900sheets");
        std::fs::write(&outside, "outside").unwrap();
        symlink(&outside, &target).unwrap();
        assert!(write_atomic(&target, b"replacement").is_err());
        assert_eq!(std::fs::read_to_string(outside).unwrap(), "outside");
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn permission_failure_does_not_replace_last_good_native_file() {
        use std::os::unix::fs::PermissionsExt;
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        let root = std::env::temp_dir().join(format!(
            "900sheets-atomic-permission-test-{}-{}",
            std::process::id(),
            PRIVATE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let target = root.join("Workbook.900sheets");
        write_atomic(&target, b"last good").unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o500)).unwrap();
        assert!(write_atomic(&target, b"not written").is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"last good");
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_backup_store_rotates_per_document_and_keeps_private_files() {
        let root = std::env::temp_dir().join(format!(
            "900sheets-native-backup-test-{}-{}",
            std::process::id(),
            PRIVATE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let store = NativeBackupStore::new(root.clone());
        let source = std::env::temp_dir().join("Budget.900sheets");
        for index in 0..7 {
            store
                .write_for_path(&source, &native_test_data(&index.to_string()))
                .unwrap();
        }
        let entries = store.list().unwrap();
        assert_eq!(entries.len(), MAX_NATIVE_BACKUPS_PER_DOCUMENT);
        assert!(entries
            .iter()
            .all(|entry| entry.document_name == "Budget.900sheets"));
        let newest = store.read(&entries[0].id).unwrap();
        assert!(newest.native_workbook.contains('6'));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
                0o700
            );
            for entry in &entries {
                assert_eq!(
                    std::fs::metadata(store.path(&entry.id).unwrap())
                        .unwrap()
                        .permissions()
                        .mode()
                        & 0o777,
                    0o600
                );
            }
        }
        store.delete(&entries[0].id).unwrap();
        assert_eq!(
            store.list().unwrap().len(),
            MAX_NATIVE_BACKUPS_PER_DOCUMENT - 1
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn failed_backup_write_and_rotation_never_remove_last_good_backup() {
        let root = std::env::temp_dir().join(format!(
            "900sheets-native-backup-failure-test-{}-{}",
            std::process::id(),
            PRIVATE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let store = NativeBackupStore::new(root.clone());
        let source = std::env::temp_dir().join("Failure.900sheets");
        let first = store
            .write_for_path(&source, &native_test_data("last good"))
            .unwrap();
        store.fail_before_replace.store(true, Ordering::SeqCst);
        assert!(store
            .write_for_path(&source, &native_test_data("interrupted"))
            .is_err());
        assert_eq!(store.list().unwrap().len(), 1);
        assert_eq!(store.list().unwrap()[0].id, first.entry.id);

        for index in 0..MAX_NATIVE_BACKUPS_PER_DOCUMENT {
            store
                .write_for_path(&source, &native_test_data(&format!("next-{index}")))
                .unwrap();
        }
        store.fail_rotation_cleanup.store(true, Ordering::SeqCst);
        let result = store
            .write_for_path(&source, &native_test_data("newest survives"))
            .unwrap();
        assert!(result.rotation_warning.is_some());
        assert!(store.list().unwrap().len() > MAX_NATIVE_BACKUPS_PER_DOCUMENT);
        assert!(store.read(&result.entry.id).is_ok());
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn native_backup_store_rejects_symlink_root_and_target() {
        use std::os::unix::fs::symlink;
        let base = std::env::temp_dir().join(format!(
            "900sheets-native-backup-symlink-test-{}-{}",
            std::process::id(),
            PRIVATE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&base).unwrap();
        let real_root = base.join("real");
        std::fs::create_dir(&real_root).unwrap();
        let linked_root = base.join("linked");
        symlink(&real_root, &linked_root).unwrap();
        let source = std::env::temp_dir().join("Linked.900sheets");
        assert!(NativeBackupStore::new(linked_root)
            .write_for_path(&source, &native_test_data("data"))
            .is_err());

        let store = NativeBackupStore::new(real_root);
        let id = "0123456789abcdef_1_00000000000000000001";
        let outside = base.join("outside");
        std::fs::write(&outside, "outside").unwrap();
        symlink(&outside, store.path(id).unwrap()).unwrap();
        assert!(write_atomic(&store.path(id).unwrap(), b"replacement").is_err());
        assert_eq!(std::fs::read_to_string(outside).unwrap(), "outside");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn invalid_native_restore_leaves_live_workbook_unchanged() {
        let state = test_state();
        let comments = Mutex::new(SheetComments::default());
        state
            .workbook
            .lock()
            .unwrap()
            .sheet_mut(0)
            .unwrap()
            .set_cell_value(0, 0, "live value".into());
        assert!(import_native_contents("not a native workbook", &state, &comments).is_err());
        assert_eq!(
            state
                .workbook
                .lock()
                .unwrap()
                .sheet(0)
                .unwrap()
                .cell_value(0, 0),
            Some("live value".into())
        );
    }
}
