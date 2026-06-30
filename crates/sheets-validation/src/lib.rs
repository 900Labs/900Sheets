use serde::{Deserialize, Serialize};
use sheets_core::cell::CellType;
use sheets_core::format::CellFormat;
use sheets_core::sheet::Sheet;
use thiserror::Error;

// ============================================================================
// Data Validation
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationType {
    /// Whole number within constraints
    WholeNumber,
    /// Decimal number within constraints
    Decimal,
    /// Text length within constraints
    TextLength,
    /// Value must be in a list
    List,
    /// Date within constraints
    Date,
    /// Time within constraints
    Time,
    /// Custom formula must evaluate to true
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ValidationOperator {
    Between,
    NotBetween,
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationErrorStyle {
    /// Stop: show error dialog, reject input
    Stop,
    /// Warning: show warning, allow user to continue
    Warning,
    /// Information: show info, allow user to continue
    Information,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataValidation {
    pub validation_type: ValidationType,
    pub operator: ValidationOperator,
    /// First formula/value (e.g. min for "between")
    pub formula1: Option<String>,
    /// Second formula/value (e.g. max for "between")
    pub formula2: Option<String>,
    /// For List type: comma-separated values or a range reference
    pub source: Option<String>,
    /// Allow blank cells to pass validation
    pub allow_blank: bool,
    /// Show dropdown for list validation
    pub show_dropdown: bool,
    /// Error style
    pub error_style: ValidationErrorStyle,
    /// Error title
    pub error_title: Option<String>,
    /// Error message
    pub error_message: Option<String>,
    /// Input prompt title
    pub prompt_title: Option<String>,
    /// Input prompt message
    pub prompt_message: Option<String>,
}

impl Default for DataValidation {
    fn default() -> Self {
        Self {
            validation_type: ValidationType::WholeNumber,
            operator: ValidationOperator::Between,
            formula1: None,
            formula2: None,
            source: None,
            allow_blank: true,
            show_dropdown: true,
            error_style: ValidationErrorStyle::Stop,
            error_title: None,
            error_message: None,
            prompt_title: None,
            prompt_message: None,
        }
    }
}

impl DataValidation {
    /// Create a list validation from a comma-separated string
    pub fn list(values: &str) -> Self {
        Self {
            validation_type: ValidationType::List,
            source: Some(values.to_string()),
            allow_blank: true,
            show_dropdown: true,
            ..Default::default()
        }
    }

    /// Create a whole number validation with an operator
    pub fn whole_number(operator: ValidationOperator, min: &str, max: &str) -> Self {
        Self {
            validation_type: ValidationType::WholeNumber,
            operator,
            formula1: Some(min.to_string()),
            formula2: Some(max.to_string()),
            ..Default::default()
        }
    }

    /// Create a decimal validation with an operator
    pub fn decimal(operator: ValidationOperator, min: &str, max: &str) -> Self {
        Self {
            validation_type: ValidationType::Decimal,
            operator,
            formula1: Some(min.to_string()),
            formula2: Some(max.to_string()),
            ..Default::default()
        }
    }

    /// Create a date validation with an operator
    pub fn date(operator: ValidationOperator, min: &str, max: &str) -> Self {
        Self {
            validation_type: ValidationType::Date,
            operator,
            formula1: Some(min.to_string()),
            formula2: Some(max.to_string()),
            ..Default::default()
        }
    }

    /// Create a text length validation with an operator
    pub fn text_length(operator: ValidationOperator, min: &str, max: &str) -> Self {
        Self {
            validation_type: ValidationType::TextLength,
            operator,
            formula1: Some(min.to_string()),
            formula2: Some(max.to_string()),
            ..Default::default()
        }
    }

    /// Create a custom formula validation
    pub fn custom(formula: &str) -> Self {
        Self {
            validation_type: ValidationType::Custom,
            formula1: Some(formula.to_string()),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationRule {
    pub range: (u32, u32, u32, u32),
    pub validation: DataValidation,
}

#[derive(Debug, Error, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationError {
    #[error("Cell value is required")]
    Required,
    #[error("Value must be a whole number")]
    NotWholeNumber,
    #[error("Value must be a number")]
    NotNumber,
    #[error("Value must be a date")]
    NotDate,
    #[error("Value must be in the list: {0}")]
    NotInList(String),
    #[error("Value {0} is not {1} {2}")]
    ConstraintFailed(String, String, String),
    #[error("Text length {0} is not {1} {2}")]
    TextLengthFailed(String, String, String),
    #[error("Custom validation failed: {0}")]
    CustomFailed(String),
}

/// Validate a single cell value against a data validation rule
pub fn validate_cell(
    sheet: &Sheet,
    row: u32,
    col: u32,
    validation: &DataValidation,
) -> Result<(), ValidationError> {
    let cell_opt = sheet.cell(row, col);
    let value = cell_opt.map(|c| c.raw.as_str()).unwrap_or("");

    if value.is_empty() {
        if validation.allow_blank {
            return Ok(());
        }
        return Err(ValidationError::Required);
    }

    match validation.validation_type {
        ValidationType::List => validate_list(value, validation),
        ValidationType::WholeNumber => {
            let n = value
                .parse::<f64>()
                .map_err(|_| ValidationError::NotWholeNumber)?;
            if n.fract() != 0.0 {
                return Err(ValidationError::NotWholeNumber);
            }
            validate_numeric(n, validation)
        }
        ValidationType::Decimal => {
            let n = value
                .parse::<f64>()
                .map_err(|_| ValidationError::NotNumber)?;
            validate_numeric(n, validation)
        }
        ValidationType::Date => {
            let n = value.parse::<f64>().map_err(|_| ValidationError::NotDate)?;
            validate_numeric(n, validation)
        }
        ValidationType::Time => {
            let n = value.parse::<f64>().map_err(|_| ValidationError::NotDate)?;
            validate_numeric(n, validation)
        }
        ValidationType::TextLength => {
            let len = value.len() as f64;
            validate_text_length(len, validation)
        }
        ValidationType::Custom => {
            // Custom validation: formula1 is a formula that should evaluate to true
            // For simplicity, we check if the formula contains a comparison
            // In a full implementation, this would evaluate the formula
            Ok(())
        }
    }
}

fn validate_list(value: &str, validation: &DataValidation) -> Result<(), ValidationError> {
    let source = validation.source.as_deref().unwrap_or("");
    let allowed: Vec<&str> = source.split(',').map(|s| s.trim()).collect();
    if allowed.iter().any(|&a| a.eq_ignore_ascii_case(value)) {
        Ok(())
    } else {
        Err(ValidationError::NotInList(source.to_string()))
    }
}

fn validate_numeric(n: f64, validation: &DataValidation) -> Result<(), ValidationError> {
    let min = validation
        .formula1
        .as_deref()
        .and_then(|s| s.parse::<f64>().ok());
    let max = validation
        .formula2
        .as_deref()
        .and_then(|s| s.parse::<f64>().ok());

    let op_str = operator_str(validation.operator);

    let passes = match validation.operator {
        ValidationOperator::Between => {
            min.map(|m| n >= m).unwrap_or(true) && max.map(|m| n <= m).unwrap_or(true)
        }
        ValidationOperator::NotBetween => {
            !(min.map(|m| n >= m).unwrap_or(true) && max.map(|m| n <= m).unwrap_or(true))
        }
        ValidationOperator::Equal => min.map(|m| (n - m).abs() < f64::EPSILON).unwrap_or(true),
        ValidationOperator::NotEqual => min.map(|m| (n - m).abs() >= f64::EPSILON).unwrap_or(true),
        ValidationOperator::GreaterThan => min.map(|m| n > m).unwrap_or(true),
        ValidationOperator::LessThan => min.map(|m| n < m).unwrap_or(true),
        ValidationOperator::GreaterThanOrEqual => min.map(|m| n >= m).unwrap_or(true),
        ValidationOperator::LessThanOrEqual => min.map(|m| n <= m).unwrap_or(true),
    };

    if passes {
        Ok(())
    } else {
        Err(ValidationError::ConstraintFailed(
            format_number(n),
            op_str.to_string(),
            format!(
                "{}{}",
                min.map(format_number).unwrap_or_default(),
                max.map(|m| format!(" and {}", format_number(m)))
                    .unwrap_or_default()
            ),
        ))
    }
}

fn validate_text_length(len: f64, validation: &DataValidation) -> Result<(), ValidationError> {
    let min = validation
        .formula1
        .as_deref()
        .and_then(|s| s.parse::<f64>().ok());
    let max = validation
        .formula2
        .as_deref()
        .and_then(|s| s.parse::<f64>().ok());

    let op_str = operator_str(validation.operator);

    let passes = match validation.operator {
        ValidationOperator::Between => {
            min.map(|m| len >= m).unwrap_or(true) && max.map(|m| len <= m).unwrap_or(true)
        }
        ValidationOperator::NotBetween => {
            !(min.map(|m| len >= m).unwrap_or(true) && max.map(|m| len <= m).unwrap_or(true))
        }
        ValidationOperator::Equal => min.map(|m| (len - m).abs() < f64::EPSILON).unwrap_or(true),
        ValidationOperator::NotEqual => {
            min.map(|m| (len - m).abs() >= f64::EPSILON).unwrap_or(true)
        }
        ValidationOperator::GreaterThan => min.map(|m| len > m).unwrap_or(true),
        ValidationOperator::LessThan => min.map(|m| len < m).unwrap_or(true),
        ValidationOperator::GreaterThanOrEqual => min.map(|m| len >= m).unwrap_or(true),
        ValidationOperator::LessThanOrEqual => min.map(|m| len <= m).unwrap_or(true),
    };

    if passes {
        Ok(())
    } else {
        Err(ValidationError::TextLengthFailed(
            len.to_string(),
            op_str.to_string(),
            format!(
                "{}{}",
                min.map(|m| m.to_string()).unwrap_or_default(),
                max.map(|m| format!(" and {}", m)).unwrap_or_default()
            ),
        ))
    }
}

fn operator_str(op: ValidationOperator) -> &'static str {
    match op {
        ValidationOperator::Between => "between",
        ValidationOperator::NotBetween => "not between",
        ValidationOperator::Equal => "equal to",
        ValidationOperator::NotEqual => "not equal to",
        ValidationOperator::GreaterThan => "greater than",
        ValidationOperator::LessThan => "less than",
        ValidationOperator::GreaterThanOrEqual => "greater than or equal to",
        ValidationOperator::LessThanOrEqual => "less than or equal to",
    }
}

fn format_number(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

/// Validate all cells in a range against a validation rule
pub fn validate_range(sheet: &Sheet, rule: &ValidationRule) -> Vec<((u32, u32), ValidationError)> {
    let (start_row, start_col, end_row, end_col) = rule.range;
    let mut errors = Vec::new();

    for row in start_row..=end_row {
        for col in start_col..=end_col {
            if let Err(e) = validate_cell(sheet, row, col, &rule.validation) {
                errors.push(((row, col), e));
            }
        }
    }

    errors
}

/// Find all cells that would fail validation if a new value is set
pub fn check_new_value(
    validation: &DataValidation,
    new_value: &str,
) -> Result<(), ValidationError> {
    if new_value.is_empty() {
        if validation.allow_blank {
            return Ok(());
        }
        return Err(ValidationError::Required);
    }

    match validation.validation_type {
        ValidationType::List => validate_list(new_value, validation),
        ValidationType::WholeNumber => {
            let n = new_value
                .parse::<f64>()
                .map_err(|_| ValidationError::NotWholeNumber)?;
            if n.fract() != 0.0 {
                return Err(ValidationError::NotWholeNumber);
            }
            validate_numeric(n, validation)
        }
        ValidationType::Decimal => {
            let n = new_value
                .parse::<f64>()
                .map_err(|_| ValidationError::NotNumber)?;
            validate_numeric(n, validation)
        }
        ValidationType::Date => {
            let n = new_value
                .parse::<f64>()
                .map_err(|_| ValidationError::NotDate)?;
            validate_numeric(n, validation)
        }
        ValidationType::Time => {
            let n = new_value
                .parse::<f64>()
                .map_err(|_| ValidationError::NotDate)?;
            validate_numeric(n, validation)
        }
        ValidationType::TextLength => validate_text_length(new_value.len() as f64, validation),
        ValidationType::Custom => Ok(()),
    }
}

// ============================================================================
// Conditional Formatting
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConditionType {
    /// Cell value comparison
    CellValue,
    /// Formula-based condition
    Formula,
    /// Color scale (min to max gradient)
    ColorScale,
    /// Data bar (in-cell bar chart)
    DataBar,
    /// Icon set (traffic lights, arrows, etc.)
    IconSet,
    /// Top/Bottom N values
    TopBottom,
    /// Above/Below average
    Average,
    /// Duplicate/Unique values
    Duplicate,
    /// Text contains
    TextContains,
    /// Text not contains
    TextNotContains,
    /// Text begins with
    TextBeginsWith,
    /// Text ends with
    TextEndsWith,
    /// Blanks
    Blanks,
    /// No blanks
    NoBlanks,
    /// Errors
    Errors,
    /// No errors
    NoErrors,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ConditionOperator {
    Between,
    NotBetween,
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IconSetType {
    ThreeTrafficLights,
    ThreeArrows,
    ThreeSymbols,
    FourArrows,
    FourRating,
    FiveArrows,
    FiveRating,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ColorScaleType {
    /// Min to max gradient (2-color)
    MinMax,
    /// Percentile-based (3-color)
    Percentile,
    /// Number-based (3-color)
    Number,
    /// Formula-based
    Formula,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorScaleStop {
    pub value: Option<f64>,
    pub color: String,
    pub is_min: bool,
    pub is_max: bool,
    pub percentile: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConditionalFormat {
    pub id: String,
    pub condition_type: ConditionType,
    /// Cell range (start_row, start_col, end_row, end_col)
    pub range: (u32, u32, u32, u32),
    /// Operator for CellValue conditions
    pub operator: Option<ConditionOperator>,
    /// Comparison values
    pub value1: Option<String>,
    pub value2: Option<String>,
    /// Format to apply when condition is met
    pub format: CellFormat,
    /// For ColorScale: stops
    pub color_scale_stops: Vec<ColorScaleStop>,
    /// For DataBar: bar color
    pub bar_color: Option<String>,
    /// For DataBar: show value alongside bar
    pub show_bar_value: bool,
    /// For IconSet: icon set type
    pub icon_set_type: Option<IconSetType>,
    /// For TopBottom: is top (true) or bottom (false)
    pub is_top: bool,
    /// For TopBottom: rank (e.g. top 10)
    pub rank: u32,
    /// For Average: above (true) or below (false) average
    pub is_above_average: bool,
    /// For Average: standard deviation (0 = exact average, 1 = 1 std dev, etc.)
    pub std_dev: u32,
    /// Priority (lower = higher priority)
    pub priority: u32,
    /// Stop if true (don't evaluate lower-priority rules)
    pub stop_if_true: bool,
}

impl Default for ConditionalFormat {
    fn default() -> Self {
        Self {
            id: String::new(),
            condition_type: ConditionType::CellValue,
            range: (0, 0, 0, 0),
            operator: None,
            value1: None,
            value2: None,
            format: CellFormat::default(),
            color_scale_stops: Vec::new(),
            bar_color: None,
            show_bar_value: true,
            icon_set_type: None,
            is_top: true,
            rank: 10,
            is_above_average: true,
            std_dev: 0,
            priority: 0,
            stop_if_true: false,
        }
    }
}

impl ConditionalFormat {
    /// Create a cell value condition
    pub fn cell_value(
        operator: ConditionOperator,
        value: &str,
        format: CellFormat,
        range: (u32, u32, u32, u32),
    ) -> Self {
        Self {
            condition_type: ConditionType::CellValue,
            operator: Some(operator),
            value1: Some(value.to_string()),
            format,
            range,
            ..Default::default()
        }
    }

    /// Create a formula-based condition
    pub fn formula(formula: &str, format: CellFormat, range: (u32, u32, u32, u32)) -> Self {
        Self {
            condition_type: ConditionType::Formula,
            value1: Some(formula.to_string()),
            format,
            range,
            ..Default::default()
        }
    }

    /// Create a color scale (2-color min/max)
    pub fn color_scale(min_color: &str, max_color: &str, range: (u32, u32, u32, u32)) -> Self {
        Self {
            condition_type: ConditionType::ColorScale,
            color_scale_stops: vec![
                ColorScaleStop {
                    value: None,
                    color: min_color.to_string(),
                    is_min: true,
                    is_max: false,
                    percentile: None,
                },
                ColorScaleStop {
                    value: None,
                    color: max_color.to_string(),
                    is_min: false,
                    is_max: true,
                    percentile: None,
                },
            ],
            range,
            ..Default::default()
        }
    }

    /// Create a 3-color scale
    pub fn color_scale_3(
        min_color: &str,
        mid_color: &str,
        max_color: &str,
        mid_percentile: f64,
        range: (u32, u32, u32, u32),
    ) -> Self {
        Self {
            condition_type: ConditionType::ColorScale,
            color_scale_stops: vec![
                ColorScaleStop {
                    value: None,
                    color: min_color.to_string(),
                    is_min: true,
                    is_max: false,
                    percentile: None,
                },
                ColorScaleStop {
                    value: None,
                    color: mid_color.to_string(),
                    is_min: false,
                    is_max: false,
                    percentile: Some(mid_percentile),
                },
                ColorScaleStop {
                    value: None,
                    color: max_color.to_string(),
                    is_min: false,
                    is_max: true,
                    percentile: None,
                },
            ],
            range,
            ..Default::default()
        }
    }

    /// Create a data bar
    pub fn data_bar(color: &str, range: (u32, u32, u32, u32)) -> Self {
        Self {
            condition_type: ConditionType::DataBar,
            bar_color: Some(color.to_string()),
            show_bar_value: true,
            range,
            ..Default::default()
        }
    }

    /// Create an icon set
    pub fn icon_set(icon_type: IconSetType, range: (u32, u32, u32, u32)) -> Self {
        Self {
            condition_type: ConditionType::IconSet,
            icon_set_type: Some(icon_type),
            range,
            ..Default::default()
        }
    }

    /// Create a top N rule
    pub fn top_n(rank: u32, format: CellFormat, range: (u32, u32, u32, u32)) -> Self {
        Self {
            condition_type: ConditionType::TopBottom,
            is_top: true,
            rank,
            format,
            range,
            ..Default::default()
        }
    }

    /// Create a bottom N rule
    pub fn bottom_n(rank: u32, format: CellFormat, range: (u32, u32, u32, u32)) -> Self {
        Self {
            condition_type: ConditionType::TopBottom,
            is_top: false,
            rank,
            format,
            range,
            ..Default::default()
        }
    }

    /// Create an above average rule
    pub fn above_average(format: CellFormat, range: (u32, u32, u32, u32)) -> Self {
        Self {
            condition_type: ConditionType::Average,
            is_above_average: true,
            format,
            range,
            ..Default::default()
        }
    }

    /// Create a below average rule
    pub fn below_average(format: CellFormat, range: (u32, u32, u32, u32)) -> Self {
        Self {
            condition_type: ConditionType::Average,
            is_above_average: false,
            format,
            range,
            ..Default::default()
        }
    }

    /// Create a duplicates rule
    pub fn duplicates(format: CellFormat, range: (u32, u32, u32, u32)) -> Self {
        Self {
            condition_type: ConditionType::Duplicate,
            format,
            range,
            ..Default::default()
        }
    }

    /// Create a unique values rule
    pub fn unique_values(format: CellFormat, range: (u32, u32, u32, u32)) -> Self {
        Self {
            condition_type: ConditionType::Duplicate,
            format,
            range,
            ..Default::default()
        }
    }

    /// Create a text contains rule
    pub fn text_contains(text: &str, format: CellFormat, range: (u32, u32, u32, u32)) -> Self {
        Self {
            condition_type: ConditionType::TextContains,
            value1: Some(text.to_string()),
            format,
            range,
            ..Default::default()
        }
    }

    /// Create a blanks rule
    pub fn blanks(format: CellFormat, range: (u32, u32, u32, u32)) -> Self {
        Self {
            condition_type: ConditionType::Blanks,
            format,
            range,
            ..Default::default()
        }
    }
}

/// Result of evaluating a conditional format on a cell
#[derive(Debug, Clone, PartialEq)]
pub struct ConditionalFormatResult {
    /// Format to apply (merged with cell's existing format)
    pub format: CellFormat,
    /// Data bar value (0.0 to 1.0) if applicable
    pub data_bar_value: Option<f64>,
    /// Icon index if applicable
    pub icon_index: Option<usize>,
    /// Background color from color scale if applicable
    pub scale_color: Option<String>,
}

/// Evaluate a single conditional format rule against a cell
pub fn evaluate_condition(
    sheet: &Sheet,
    row: u32,
    col: u32,
    rule: &ConditionalFormat,
    all_values: &[f64],
) -> Option<ConditionalFormatResult> {
    let cell = sheet.cell(row, col);
    let value_str = cell.map(|c| c.raw.as_str()).unwrap_or("");
    let value_num = cell.and_then(|c| c.raw.parse::<f64>().ok());

    let matches = match &rule.condition_type {
        ConditionType::CellValue => {
            if let (Some(op), Some(v1)) = (&rule.operator, &rule.value1) {
                let n = value_num?;
                let threshold = v1.parse::<f64>().ok()?;
                let threshold2 = rule.value2.as_deref().and_then(|s| s.parse::<f64>().ok());

                match op {
                    ConditionOperator::Between => threshold2
                        .map(|t2| n >= threshold && n <= t2)
                        .unwrap_or(false),
                    ConditionOperator::NotBetween => threshold2
                        .map(|t2| !(n >= threshold && n <= t2))
                        .unwrap_or(false),
                    ConditionOperator::Equal => (n - threshold).abs() < f64::EPSILON,
                    ConditionOperator::NotEqual => (n - threshold).abs() >= f64::EPSILON,
                    ConditionOperator::GreaterThan => n > threshold,
                    ConditionOperator::LessThan => n < threshold,
                    ConditionOperator::GreaterThanOrEqual => n >= threshold,
                    ConditionOperator::LessThanOrEqual => n <= threshold,
                }
            } else {
                false
            }
        }

        ConditionType::Formula => {
            // Formula-based: in a full implementation, we'd evaluate the formula
            // For now, check if the cell has a value (simplified)
            !value_str.is_empty()
        }

        ConditionType::ColorScale => {
            // Color scale always "matches" - it applies a gradient
            return evaluate_color_scale(sheet, row, col, rule, all_values);
        }

        ConditionType::DataBar => {
            // Data bar always "matches" - it shows a proportional bar
            return evaluate_data_bar(sheet, row, col, rule, all_values);
        }

        ConditionType::IconSet => {
            // Icon set always "matches" - it assigns an icon
            return evaluate_icon_set(sheet, row, col, rule, all_values);
        }

        ConditionType::TopBottom => {
            if all_values.is_empty() {
                return None;
            }
            let n = value_num?;
            let mut sorted = all_values.to_vec();
            sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

            if rule.is_top {
                let threshold = sorted
                    .get((rule.rank as usize).saturating_sub(1))
                    .copied()
                    .unwrap_or(f64::MIN);
                n >= threshold
            } else {
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let threshold = sorted
                    .get((rule.rank as usize).saturating_sub(1))
                    .copied()
                    .unwrap_or(f64::MAX);
                n <= threshold
            }
        }

        ConditionType::Average => {
            if all_values.is_empty() {
                return None;
            }
            let n = value_num?;
            let avg = all_values.iter().sum::<f64>() / all_values.len() as f64;
            if rule.std_dev > 0 {
                let variance = all_values.iter().map(|v| (v - avg).powi(2)).sum::<f64>()
                    / all_values.len() as f64;
                let std_dev = variance.sqrt();
                let threshold = avg
                    + (rule.std_dev as f64)
                        * std_dev
                        * if rule.is_above_average { 1.0 } else { -1.0 };
                if rule.is_above_average {
                    n >= threshold
                } else {
                    n <= threshold
                }
            } else if rule.is_above_average {
                n > avg
            } else {
                n < avg
            }
        }

        ConditionType::Duplicate => {
            // Check if value appears more than once in the range
            let (sr, sc, er, ec) = rule.range;
            let mut count = 0;
            for r in sr..=er {
                for c in sc..=ec {
                    if let Some(cell) = sheet.cell(r, c) {
                        if cell.raw == value_str {
                            count += 1;
                        }
                    }
                }
            }
            count > 1
        }

        ConditionType::TextContains => {
            let search = rule.value1.as_deref().unwrap_or("");
            value_str.to_lowercase().contains(&search.to_lowercase())
        }

        ConditionType::TextNotContains => {
            let search = rule.value1.as_deref().unwrap_or("");
            !value_str.to_lowercase().contains(&search.to_lowercase())
        }

        ConditionType::TextBeginsWith => {
            let search = rule.value1.as_deref().unwrap_or("");
            value_str.to_lowercase().starts_with(&search.to_lowercase())
        }

        ConditionType::TextEndsWith => {
            let search = rule.value1.as_deref().unwrap_or("");
            value_str.to_lowercase().ends_with(&search.to_lowercase())
        }

        ConditionType::Blanks => value_str.is_empty(),

        ConditionType::NoBlanks => !value_str.is_empty(),

        ConditionType::Errors => cell
            .map(|c| c.cell_type == CellType::Error)
            .unwrap_or(false),

        ConditionType::NoErrors => !cell
            .map(|c| c.cell_type == CellType::Error)
            .unwrap_or(false),
    };

    if matches {
        Some(ConditionalFormatResult {
            format: rule.format.clone(),
            data_bar_value: None,
            icon_index: None,
            scale_color: None,
        })
    } else {
        None
    }
}

fn evaluate_color_scale(
    sheet: &Sheet,
    row: u32,
    col: u32,
    rule: &ConditionalFormat,
    all_values: &[f64],
) -> Option<ConditionalFormatResult> {
    let n = sheet
        .cell(row, col)
        .and_then(|c| c.raw.parse::<f64>().ok())?;
    if all_values.is_empty() {
        return None;
    }

    let min_val = all_values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = all_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    if (max_val - min_val).abs() < f64::EPSILON {
        // All values are the same - use the first stop color
        let color = rule.color_scale_stops.first().map(|s| s.color.clone());
        return Some(ConditionalFormatResult {
            format: CellFormat::default(),
            data_bar_value: None,
            icon_index: None,
            scale_color: color,
        });
    }

    let normalized = (n - min_val) / (max_val - min_val);

    let color = if rule.color_scale_stops.len() == 2 {
        // 2-color scale: interpolate between min and max
        let c1 = parse_color(&rule.color_scale_stops[0].color);
        let c2 = parse_color(&rule.color_scale_stops[1].color);
        interpolate_color(c1, c2, normalized)
    } else if rule.color_scale_stops.len() == 3 {
        // 3-color scale: interpolate through midpoint
        let mid_pct = rule.color_scale_stops[1].percentile.unwrap_or(50.0) / 100.0;
        let c1 = parse_color(&rule.color_scale_stops[0].color);
        let c2 = parse_color(&rule.color_scale_stops[1].color);
        let c3 = parse_color(&rule.color_scale_stops[2].color);
        if normalized <= mid_pct {
            let t = if mid_pct > 0.0 {
                normalized / mid_pct
            } else {
                0.0
            };
            interpolate_color(c1, c2, t)
        } else {
            let t = if (1.0 - mid_pct) > 0.0 {
                (normalized - mid_pct) / (1.0 - mid_pct)
            } else {
                0.0
            };
            interpolate_color(c2, c3, t)
        }
    } else {
        "#FFFFFF".to_string()
    };

    Some(ConditionalFormatResult {
        format: CellFormat::default(),
        data_bar_value: None,
        icon_index: None,
        scale_color: Some(color),
    })
}

fn evaluate_data_bar(
    sheet: &Sheet,
    row: u32,
    col: u32,
    rule: &ConditionalFormat,
    all_values: &[f64],
) -> Option<ConditionalFormatResult> {
    let n = sheet
        .cell(row, col)
        .and_then(|c| c.raw.parse::<f64>().ok())?;
    if all_values.is_empty() {
        return None;
    }

    let min_val = all_values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = all_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    let bar_value = if (max_val - min_val).abs() < f64::EPSILON {
        1.0
    } else {
        ((n - min_val) / (max_val - min_val)).clamp(0.0, 1.0)
    };

    Some(ConditionalFormatResult {
        format: CellFormat::default(),
        data_bar_value: Some(bar_value),
        icon_index: None,
        scale_color: rule.bar_color.clone(),
    })
}

fn evaluate_icon_set(
    sheet: &Sheet,
    row: u32,
    col: u32,
    rule: &ConditionalFormat,
    all_values: &[f64],
) -> Option<ConditionalFormatResult> {
    let n = sheet
        .cell(row, col)
        .and_then(|c| c.raw.parse::<f64>().ok())?;
    if all_values.is_empty() {
        return None;
    }

    let icon_type = rule.icon_set_type.as_ref()?;
    let num_icons = match icon_type {
        IconSetType::ThreeTrafficLights | IconSetType::ThreeArrows | IconSetType::ThreeSymbols => 3,
        IconSetType::FourArrows | IconSetType::FourRating => 4,
        IconSetType::FiveArrows | IconSetType::FiveRating => 5,
    };

    let min_val = all_values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_val = all_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    if (max_val - min_val).abs() < f64::EPSILON {
        return Some(ConditionalFormatResult {
            format: CellFormat::default(),
            data_bar_value: None,
            icon_index: Some(0),
            scale_color: None,
        });
    }

    let normalized = (n - min_val) / (max_val - min_val);
    let icon_index = ((normalized * num_icons as f64).floor() as usize).min(num_icons - 1);

    Some(ConditionalFormatResult {
        format: CellFormat::default(),
        data_bar_value: None,
        icon_index: Some(icon_index),
        scale_color: None,
    })
}

fn parse_color(hex: &str) -> (u8, u8, u8) {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
        (r, g, b)
    } else {
        (0, 0, 0)
    }
}

fn interpolate_color(c1: (u8, u8, u8), c2: (u8, u8, u8), t: f64) -> String {
    let t = t.clamp(0.0, 1.0);
    let r = (c1.0 as f64 + (c2.0 as f64 - c1.0 as f64) * t).round() as u8;
    let g = (c1.1 as f64 + (c2.1 as f64 - c1.1 as f64) * t).round() as u8;
    let b = (c1.2 as f64 + (c2.2 as f64 - c1.2 as f64) * t).round() as u8;
    format!("#{:02X}{:02X}{:02X}", r, g, b)
}

/// Collect all numeric values in a range (for color scale, data bar, icon set calculations)
pub fn collect_range_numbers(sheet: &Sheet, range: (u32, u32, u32, u32)) -> Vec<f64> {
    let (sr, sc, er, ec) = range;
    let mut values = Vec::new();
    for row in sr..=er {
        for col in sc..=ec {
            if let Some(cell) = sheet.cell(row, col) {
                if cell.cell_type == CellType::Number {
                    if let Ok(n) = cell.raw.parse::<f64>() {
                        values.push(n);
                    }
                }
            }
        }
    }
    values
}

/// Evaluate all conditional format rules for a given cell
/// Rules are evaluated in priority order (lowest priority value first)
/// Returns the merged format result
pub fn evaluate_all_conditions(
    sheet: &Sheet,
    row: u32,
    col: u32,
    rules: &[ConditionalFormat],
) -> Vec<ConditionalFormatResult> {
    let mut sorted_rules: Vec<&ConditionalFormat> = rules.iter().collect();
    sorted_rules.sort_by_key(|r| r.priority);

    let mut results = Vec::new();

    for rule in &sorted_rules {
        let (sr, sc, er, ec) = rule.range;
        if row < sr || row > er || col < sc || col > ec {
            continue;
        }

        let all_values = collect_range_numbers(sheet, rule.range);

        if let Some(result) = evaluate_condition(sheet, row, col, rule, &all_values) {
            results.push(result);
            if rule.stop_if_true {
                break;
            }
        }
    }

    results
}

/// Apply conditional formatting to a cell, merging with its existing format
pub fn apply_conditional_formatting(
    sheet: &Sheet,
    row: u32,
    col: u32,
    rules: &[ConditionalFormat],
    base_format: Option<&CellFormat>,
) -> CellFormat {
    let results = evaluate_all_conditions(sheet, row, col, rules);

    let mut merged = base_format.cloned().unwrap_or_default();

    for result in results {
        merged = merged.merge(&result.format);
        if let Some(color) = result.scale_color {
            merged.bg_color = Some(color);
        }
    }

    merged
}

/// Get all cells in a range that match a given conditional format rule
pub fn find_matching_cells(sheet: &Sheet, rule: &ConditionalFormat) -> Vec<(u32, u32)> {
    let (sr, sc, er, ec) = rule.range;
    let all_values = collect_range_numbers(sheet, rule.range);
    let mut matching = Vec::new();

    for row in sr..=er {
        for col in sc..=ec {
            if evaluate_condition(sheet, row, col, rule, &all_values).is_some() {
                matching.push((row, col));
            }
        }
    }

    matching
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sheet() -> Sheet {
        Sheet::new("Test")
    }

    // --- Data Validation Tests ---

    #[test]
    fn test_list_validation_pass() {
        let sheet = make_sheet();
        let validation = DataValidation::list("Apple,Banana,Cherry");
        assert!(validate_cell(&sheet, 0, 0, &validation).is_ok());
    }

    #[test]
    fn test_list_validation_with_value() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "Apple".into());
        let validation = DataValidation::list("Apple,Banana,Cherry");
        assert!(validate_cell(&sheet, 0, 0, &validation).is_ok());
    }

    #[test]
    fn test_list_validation_fail() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "Grape".into());
        let validation = DataValidation::list("Apple,Banana,Cherry");
        assert!(validate_cell(&sheet, 0, 0, &validation).is_err());
    }

    #[test]
    fn test_whole_number_between_pass() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "5".into());
        let validation = DataValidation::whole_number(ValidationOperator::Between, "1", "10");
        assert!(validate_cell(&sheet, 0, 0, &validation).is_ok());
    }

    #[test]
    fn test_whole_number_between_fail() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "15".into());
        let validation = DataValidation::whole_number(ValidationOperator::Between, "1", "10");
        assert!(validate_cell(&sheet, 0, 0, &validation).is_err());
    }

    #[test]
    fn test_whole_number_not_integer() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "3.5".into());
        let validation = DataValidation::whole_number(ValidationOperator::Between, "1", "10");
        let result = validate_cell(&sheet, 0, 0, &validation);
        assert!(matches!(result, Err(ValidationError::NotWholeNumber)));
    }

    #[test]
    fn test_decimal_greater_than_pass() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "5.5".into());
        let validation = DataValidation::decimal(ValidationOperator::GreaterThan, "5", "0");
        assert!(validate_cell(&sheet, 0, 0, &validation).is_ok());
    }

    #[test]
    fn test_decimal_greater_than_fail() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "4.5".into());
        let validation = DataValidation::decimal(ValidationOperator::GreaterThan, "5", "0");
        assert!(validate_cell(&sheet, 0, 0, &validation).is_err());
    }

    #[test]
    fn test_text_length_between() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "hello".into());
        let validation = DataValidation::text_length(ValidationOperator::Between, "3", "10");
        assert!(validate_cell(&sheet, 0, 0, &validation).is_ok());
    }

    #[test]
    fn test_text_length_too_short() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "hi".into());
        let validation = DataValidation::text_length(ValidationOperator::Between, "3", "10");
        assert!(validate_cell(&sheet, 0, 0, &validation).is_err());
    }

    #[test]
    fn test_blank_allowed() {
        let sheet = make_sheet();
        let validation = DataValidation::whole_number(ValidationOperator::Between, "1", "10");
        assert!(validate_cell(&sheet, 0, 0, &validation).is_ok());
    }

    #[test]
    fn test_blank_not_allowed() {
        let sheet = make_sheet();
        let validation = DataValidation {
            allow_blank: false,
            ..DataValidation::whole_number(ValidationOperator::Between, "1", "10")
        };
        assert!(validate_cell(&sheet, 0, 0, &validation).is_err());
    }

    #[test]
    fn test_check_new_value_list() {
        let validation = DataValidation::list("Yes,No");
        assert!(check_new_value(&validation, "Yes").is_ok());
        assert!(check_new_value(&validation, "No").is_ok());
        assert!(check_new_value(&validation, "Maybe").is_err());
    }

    #[test]
    fn test_check_new_value_whole_number() {
        let validation = DataValidation::whole_number(ValidationOperator::GreaterThan, "0", "0");
        assert!(check_new_value(&validation, "5").is_ok());
        assert!(check_new_value(&validation, "-1").is_err());
    }

    #[test]
    fn test_validate_range() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "5".into());
        sheet.set_cell_value(0, 1, "15".into());
        sheet.set_cell_value(0, 2, "3".into());

        let rule = ValidationRule {
            range: (0, 0, 0, 2),
            validation: DataValidation::whole_number(ValidationOperator::Between, "1", "10"),
        };

        let errors = validate_range(&sheet, &rule);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].0, (0, 1));
    }

    // --- Conditional Formatting Tests ---

    #[test]
    fn test_cell_value_condition_greater_than() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "15".into());
        sheet.set_cell_value(0, 1, "5".into());

        let rule = ConditionalFormat::cell_value(
            ConditionOperator::GreaterThan,
            "10",
            CellFormat::new().bold(true),
            (0, 0, 0, 1),
        );

        let all_values = collect_range_numbers(&sheet, rule.range);
        let result0 = evaluate_condition(&sheet, 0, 0, &rule, &all_values);
        let result1 = evaluate_condition(&sheet, 0, 1, &rule, &all_values);

        assert!(result0.is_some());
        assert!(result1.is_none());
    }

    #[test]
    fn test_cell_value_condition_between() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "5".into());
        sheet.set_cell_value(0, 1, "15".into());

        let mut rule = ConditionalFormat::cell_value(
            ConditionOperator::Between,
            "1",
            CellFormat::new().bg_color("#00FF00"),
            (0, 0, 0, 1),
        );
        rule.value2 = Some("10".to_string());

        let all_values = collect_range_numbers(&sheet, rule.range);
        let result0 = evaluate_condition(&sheet, 0, 0, &rule, &all_values);
        let result1 = evaluate_condition(&sheet, 0, 1, &rule, &all_values);

        assert!(result0.is_some());
        assert!(result1.is_none());
    }

    #[test]
    fn test_text_contains_condition() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "Hello World".into());
        sheet.set_cell_value(0, 1, "Goodbye".into());

        let rule =
            ConditionalFormat::text_contains("hello", CellFormat::new().bold(true), (0, 0, 0, 1));

        let all_values = collect_range_numbers(&sheet, rule.range);
        let result0 = evaluate_condition(&sheet, 0, 0, &rule, &all_values);
        let result1 = evaluate_condition(&sheet, 0, 1, &rule, &all_values);

        assert!(result0.is_some());
        assert!(result1.is_none());
    }

    #[test]
    fn test_blanks_condition() {
        let mut sheet = make_sheet();
        // Cell (0,0) is blank, cell (0,1) has a value
        sheet.set_cell_value(0, 1, "value".into());

        let rule = ConditionalFormat::blanks(CellFormat::new().bg_color("#FF0000"), (0, 0, 0, 1));

        let all_values = collect_range_numbers(&sheet, rule.range);
        let result0 = evaluate_condition(&sheet, 0, 0, &rule, &all_values);
        let result1 = evaluate_condition(&sheet, 0, 1, &rule, &all_values);

        assert!(result0.is_some());
        assert!(result1.is_none());
    }

    #[test]
    fn test_color_scale() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "1".into());
        sheet.set_cell_value(0, 1, "5".into());
        sheet.set_cell_value(0, 2, "10".into());

        let rule = ConditionalFormat::color_scale("#FF0000", "#00FF00", (0, 0, 0, 2));
        let all_values = collect_range_numbers(&sheet, rule.range);

        let result_min = evaluate_condition(&sheet, 0, 0, &rule, &all_values);
        let result_mid = evaluate_condition(&sheet, 0, 1, &rule, &all_values);
        let result_max = evaluate_condition(&sheet, 0, 2, &rule, &all_values);

        assert!(result_min.is_some());
        assert!(result_mid.is_some());
        assert!(result_max.is_some());

        // Min should be red-ish, max should be green-ish
        let min_color = result_min.unwrap().scale_color.unwrap();
        let max_color = result_max.unwrap().scale_color.unwrap();
        assert!(min_color.starts_with("#FF"));
        assert!(max_color.starts_with("#00FF"));
    }

    #[test]
    fn test_color_scale_3() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "1".into());
        sheet.set_cell_value(0, 1, "5".into());
        sheet.set_cell_value(0, 2, "10".into());

        let rule =
            ConditionalFormat::color_scale_3("#FF0000", "#FFFF00", "#00FF00", 50.0, (0, 0, 0, 2));
        let all_values = collect_range_numbers(&sheet, rule.range);

        let result_mid = evaluate_condition(&sheet, 0, 1, &rule, &all_values);
        assert!(result_mid.is_some());
        let mid_color = result_mid.unwrap().scale_color.unwrap();
        // Midpoint should be between red and yellow
        let (r, _g, b) = parse_color(&mid_color);
        assert_eq!(r, 255, "red should be 255 for midpoint: {}", mid_color);
        assert_eq!(b, 0, "blue should be 0 for midpoint: {}", mid_color);
    }

    #[test]
    fn test_data_bar() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "1".into());
        sheet.set_cell_value(0, 1, "5".into());
        sheet.set_cell_value(0, 2, "10".into());

        let rule = ConditionalFormat::data_bar("#0066CC", (0, 0, 0, 2));
        let all_values = collect_range_numbers(&sheet, rule.range);

        let result_min = evaluate_condition(&sheet, 0, 0, &rule, &all_values);
        let result_max = evaluate_condition(&sheet, 0, 2, &rule, &all_values);

        assert!(result_min.is_some());
        assert!(result_max.is_some());

        let bar_min = result_min.unwrap().data_bar_value.unwrap();
        let bar_max = result_max.unwrap().data_bar_value.unwrap();
        assert_eq!(bar_min, 0.0);
        assert_eq!(bar_max, 1.0);
    }

    #[test]
    fn test_icon_set() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "1".into());
        sheet.set_cell_value(0, 1, "5".into());
        sheet.set_cell_value(0, 2, "10".into());

        let rule = ConditionalFormat::icon_set(IconSetType::ThreeTrafficLights, (0, 0, 0, 2));
        let all_values = collect_range_numbers(&sheet, rule.range);

        let result_min = evaluate_condition(&sheet, 0, 0, &rule, &all_values);
        let result_max = evaluate_condition(&sheet, 0, 2, &rule, &all_values);

        assert!(result_min.is_some());
        assert!(result_max.is_some());

        let icon_min = result_min.unwrap().icon_index.unwrap();
        let icon_max = result_max.unwrap().icon_index.unwrap();
        assert_eq!(icon_min, 0);
        assert_eq!(icon_max, 2);
    }

    #[test]
    fn test_top_n() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "10".into());
        sheet.set_cell_value(0, 1, "5".into());
        sheet.set_cell_value(0, 2, "20".into());
        sheet.set_cell_value(0, 3, "1".into());

        let rule = ConditionalFormat::top_n(2, CellFormat::new().bold(true), (0, 0, 0, 3));
        let all_values = collect_range_numbers(&sheet, rule.range);

        let r0 = evaluate_condition(&sheet, 0, 0, &rule, &all_values);
        let r1 = evaluate_condition(&sheet, 0, 1, &rule, &all_values);
        let r2 = evaluate_condition(&sheet, 0, 2, &rule, &all_values);
        let r3 = evaluate_condition(&sheet, 0, 3, &rule, &all_values);

        assert!(r0.is_some()); // 10 is in top 2
        assert!(r1.is_none()); // 5 is not in top 2
        assert!(r2.is_some()); // 20 is in top 2
        assert!(r3.is_none()); // 1 is not in top 2
    }

    #[test]
    fn test_above_below_average() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "10".into());
        sheet.set_cell_value(0, 1, "2".into());
        sheet.set_cell_value(0, 2, "6".into()); // average is 6

        let rule_above =
            ConditionalFormat::above_average(CellFormat::new().bold(true), (0, 0, 0, 2));
        let all_values = collect_range_numbers(&sheet, rule_above.range);

        let r0 = evaluate_condition(&sheet, 0, 0, &rule_above, &all_values);
        let r1 = evaluate_condition(&sheet, 0, 1, &rule_above, &all_values);
        let r2 = evaluate_condition(&sheet, 0, 2, &rule_above, &all_values);

        assert!(r0.is_some()); // 10 > 6
        assert!(r1.is_none()); // 2 < 6
        assert!(r2.is_none()); // 6 == 6, not above
    }

    #[test]
    fn test_duplicates() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "apple".into());
        sheet.set_cell_value(0, 1, "apple".into());
        sheet.set_cell_value(0, 2, "banana".into());

        let rule = ConditionalFormat::duplicates(CellFormat::new().bold(true), (0, 0, 0, 2));
        let all_values = collect_range_numbers(&sheet, rule.range);

        let r0 = evaluate_condition(&sheet, 0, 0, &rule, &all_values);
        let r1 = evaluate_condition(&sheet, 0, 1, &rule, &all_values);
        let r2 = evaluate_condition(&sheet, 0, 2, &rule, &all_values);

        assert!(r0.is_some()); // apple appears twice
        assert!(r1.is_some()); // apple appears twice
        assert!(r2.is_none()); // banana appears once
    }

    #[test]
    fn test_evaluate_all_conditions_priority() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "15".into());

        let rule1 = ConditionalFormat {
            id: "r1".into(),
            condition_type: ConditionType::CellValue,
            operator: Some(ConditionOperator::GreaterThan),
            value1: Some("10".into()),
            format: CellFormat::new().bold(true),
            range: (0, 0, 0, 0),
            priority: 1,
            stop_if_true: false,
            ..Default::default()
        };

        let rule2 = ConditionalFormat {
            id: "r2".into(),
            condition_type: ConditionType::CellValue,
            operator: Some(ConditionOperator::GreaterThan),
            value1: Some("5".into()),
            format: CellFormat::new().italic(true),
            range: (0, 0, 0, 0),
            priority: 2,
            stop_if_true: false,
            ..Default::default()
        };

        let results = evaluate_all_conditions(&sheet, 0, 0, &[rule1, rule2]);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].format.bold, Some(true));
        assert_eq!(results[1].format.italic, Some(true));
    }

    #[test]
    fn test_stop_if_true() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "15".into());

        let rule1 = ConditionalFormat {
            id: "r1".into(),
            condition_type: ConditionType::CellValue,
            operator: Some(ConditionOperator::GreaterThan),
            value1: Some("10".into()),
            format: CellFormat::new().bold(true),
            range: (0, 0, 0, 0),
            priority: 1,
            stop_if_true: true,
            ..Default::default()
        };

        let rule2 = ConditionalFormat {
            id: "r2".into(),
            condition_type: ConditionType::CellValue,
            operator: Some(ConditionOperator::GreaterThan),
            value1: Some("5".into()),
            format: CellFormat::new().italic(true),
            range: (0, 0, 0, 0),
            priority: 2,
            stop_if_true: false,
            ..Default::default()
        };

        let results = evaluate_all_conditions(&sheet, 0, 0, &[rule1, rule2]);
        assert_eq!(results.len(), 1); // Only rule1 because stop_if_true
    }

    #[test]
    fn test_apply_conditional_formatting() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "15".into());
        sheet.set_format(0, 0, CellFormat::new().font_size(12.0));

        let rule = ConditionalFormat::cell_value(
            ConditionOperator::GreaterThan,
            "10",
            CellFormat::new().bold(true),
            (0, 0, 0, 0),
        );

        let base = sheet.get_format(0, 0);
        let result = apply_conditional_formatting(&sheet, 0, 0, &[rule], base);
        assert_eq!(result.bold, Some(true));
        assert_eq!(result.font_size, Some(12.0));
    }

    #[test]
    fn test_find_matching_cells() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "15".into());
        sheet.set_cell_value(0, 1, "5".into());
        sheet.set_cell_value(0, 2, "20".into());

        let rule = ConditionalFormat::cell_value(
            ConditionOperator::GreaterThan,
            "10",
            CellFormat::new().bold(true),
            (0, 0, 0, 2),
        );

        let matching = find_matching_cells(&sheet, &rule);
        assert_eq!(matching.len(), 2);
        assert!(matching.contains(&(0, 0)));
        assert!(matching.contains(&(0, 2)));
    }

    #[test]
    fn test_parse_color() {
        assert_eq!(parse_color("#FF0000"), (255, 0, 0));
        assert_eq!(parse_color("#00FF00"), (0, 255, 0));
        assert_eq!(parse_color("#0000FF"), (0, 0, 255));
    }

    #[test]
    fn test_interpolate_color() {
        let c1 = (0, 0, 0);
        let c2 = (255, 255, 255);
        assert_eq!(interpolate_color(c1, c2, 0.0), "#000000");
        assert_eq!(interpolate_color(c1, c2, 1.0), "#FFFFFF");
        assert_eq!(interpolate_color(c1, c2, 0.5), "#808080");
    }

    #[test]
    fn test_no_blanks_condition() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "value".into());
        // Cell (0,1) is blank

        let rule = ConditionalFormat {
            condition_type: ConditionType::NoBlanks,
            format: CellFormat::new().bold(true),
            range: (0, 0, 0, 1),
            ..Default::default()
        };

        let all_values = collect_range_numbers(&sheet, rule.range);
        let r0 = evaluate_condition(&sheet, 0, 0, &rule, &all_values);
        let r1 = evaluate_condition(&sheet, 0, 1, &rule, &all_values);

        assert!(r0.is_some());
        assert!(r1.is_none());
    }

    #[test]
    fn test_text_begins_with() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "Hello World".into());
        sheet.set_cell_value(0, 1, "World Hello".into());

        let rule = ConditionalFormat {
            condition_type: ConditionType::TextBeginsWith,
            value1: Some("hello".into()),
            format: CellFormat::new().bold(true),
            range: (0, 0, 0, 1),
            ..Default::default()
        };

        let all_values = collect_range_numbers(&sheet, rule.range);
        let r0 = evaluate_condition(&sheet, 0, 0, &rule, &all_values);
        let r1 = evaluate_condition(&sheet, 0, 1, &rule, &all_values);

        assert!(r0.is_some());
        assert!(r1.is_none());
    }

    #[test]
    fn test_text_ends_with() {
        let mut sheet = make_sheet();
        sheet.set_cell_value(0, 0, "Hello World".into());
        sheet.set_cell_value(0, 1, "World Hello".into());

        let rule = ConditionalFormat {
            condition_type: ConditionType::TextEndsWith,
            value1: Some("world".into()),
            format: CellFormat::new().bold(true),
            range: (0, 0, 0, 1),
            ..Default::default()
        };

        let all_values = collect_range_numbers(&sheet, rule.range);
        let r0 = evaluate_condition(&sheet, 0, 0, &rule, &all_values);
        let r1 = evaluate_condition(&sheet, 0, 1, &rule, &all_values);

        assert!(r0.is_some());
        assert!(r1.is_none());
    }
}
