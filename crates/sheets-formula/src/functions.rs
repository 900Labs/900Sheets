use crate::ast::Expr;
use crate::error::FormulaError;
use crate::evaluator::{CellProvider, SimpleProvider, Value};
use std::collections::HashSet;

// Per-function bounds keep a small formula from exhausting a low-memory computer.
pub(crate) const MAX_FUNCTION_TEXT_BYTES: usize = 1024 * 1024;
const MAX_FUNCTION_ITERATIONS: u64 = 10_000;
const MAX_FIXED_DECIMALS: i32 = 1_000;
// The current date model uses the Gregorian calendar without Excel's fictitious leap day.
const MAX_DATE_SERIAL: f64 = 2_958_464.0; // 9999-12-31

type FnHandler = fn(&[Value], &dyn CellProvider, &mut HashSet<(u32, u32)>, &[&Expr]) -> Value;

pub struct FunctionRegistry {
    functions: std::collections::HashMap<String, FnHandler>,
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FunctionRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            functions: std::collections::HashMap::new(),
        };
        reg.register_all();
        reg
    }

    pub fn call(
        &self,
        name: &str,
        args: &[Value],
        provider: &dyn CellProvider,
        visited: &mut HashSet<(u32, u32)>,
        exprs: &[&Expr],
    ) -> Value {
        let upper = name.to_uppercase();
        if let Some(handler) = self.functions.get(&upper) {
            ensure_finite_value(handler(args, provider, visited, exprs), &upper)
        } else {
            Value::Error(FormulaError::UnknownFunction(name.into()))
        }
    }

    fn register(&mut self, name: &str, handler: FnHandler) {
        self.functions.insert(name.to_uppercase(), handler);
    }

    fn register_all(&mut self) {
        // Math functions
        self.register("SUM", sum);
        self.register("AVERAGE", average);
        self.register("MIN", min);
        self.register("MAX", max);
        self.register("ABS", abs);
        self.register("ROUND", round);
        self.register("FLOOR", floor);
        self.register("CEILING", ceiling);
        self.register("POWER", power);
        self.register("SQRT", sqrt);
        self.register("MOD", modulo);
        self.register("INT", int_fn);
        self.register("EXP", exp_fn);
        self.register("LN", ln_fn);
        self.register("LOG10", log10_fn);
        self.register("PI", pi_fn);
        self.register("RAND", rand_fn);
        self.register("SIN", sin_fn);
        self.register("COS", cos_fn);
        self.register("TAN", tan_fn);
        self.register("ASIN", asin_fn);
        self.register("ACOS", acos_fn);
        self.register("ATAN", atan_fn);
        self.register("ATAN2", atan2_fn);
        self.register("DEGREES", degrees_fn);
        self.register("RADIANS", radians_fn);
        self.register("COUNT", count_fn);
        self.register("COUNTA", counta_fn);
        self.register("PRODUCT", product_fn);
        self.register("FACT", fact_fn);
        self.register("SIGN", sign_fn);

        // Logical functions
        self.register("IF", if_fn);
        self.register("AND", and_fn);
        self.register("OR", or_fn);
        self.register("NOT", not_fn);
        self.register("TRUE", true_fn);
        self.register("FALSE", false_fn);
        self.register("IFERROR", iferror_fn);
        self.register("IFNA", ifna_fn);
        self.register("XOR", xor_fn);

        // Text functions
        self.register("LEN", len_fn);
        self.register("UPPER", upper_fn);
        self.register("LOWER", lower_fn);
        self.register("PROPER", proper_fn);
        self.register("TRIM", trim_fn);
        self.register("LEFT", left_fn);
        self.register("RIGHT", right_fn);
        self.register("MID", mid_fn);
        self.register("CONCATENATE", concatenate_fn);
        self.register("SUBSTITUTE", substitute_fn);
        self.register("REPT", rept_fn);
        self.register("FIND", find_fn);
        self.register("SEARCH", search_fn);
        self.register("REPLACE", replace_fn);
        self.register("TEXT", text_fn);
        self.register("VALUE", value_fn);
        self.register("MIDB", mid_fn);

        // Info functions
        self.register("ISNUMBER", isnumber_fn);
        self.register("ISTEXT", istext_fn);
        self.register("ISLOGICAL", islogical_fn);
        self.register("ISERROR", iserror_fn);
        self.register("ISEMPTY", isempty_fn);
        self.register("ISBLANK", isempty_fn);
        self.register("ISNONTEXT", isnontext_fn);
        self.register("ISODD", isodd_fn);
        self.register("ISEVEN", iseven_fn);
        self.register("NA", na_fn);
        self.register("TYPE", type_fn);

        // Advanced math functions
        self.register("ROUNDUP", roundup_fn);
        self.register("ROUNDDOWN", rounddown_fn);
        self.register("TRUNC", trunc_fn);
        self.register("MROUND", mround_fn);
        self.register("QUOTIENT", quotient_fn);
        self.register("GCD", gcd_fn);
        self.register("LCM", lcm_fn);
        self.register("COMBIN", combin_fn);
        self.register("PERMUT", permut_fn);
        self.register("RANDBETWEEN", randbetween_fn);
        self.register("LOG", log_fn);
        self.register("LOG2", log2_fn);
        self.register("SINH", sinh_fn);
        self.register("COSH", cosh_fn);
        self.register("TANH", tanh_fn);
        self.register("ASINH", asinh_fn);
        self.register("ACOSH", acosh_fn);
        self.register("ATANH", atanh_fn);
        self.register("FACTDOUBLE", factdouble_fn);
        self.register("SQRTPI", sqrtpi_fn);
        self.register("GESTEP", gestep_fn);
        self.register("DELTA", delta_fn);
        self.register("MULTINOMIAL", multinomial_fn);
        self.register("COMBINA", combina_fn);
        self.register("PERMUTA", permuta_fn);
        self.register("PERMUTATIONA", permuta_fn);
        self.register("SERIESSUM", seriessum_fn);

        // Statistical functions
        self.register("MEDIAN", median_fn);
        self.register("MODE", mode_fn);
        self.register("STDEV", stdev_fn);
        self.register("VAR", var_fn);
        self.register("LARGE", large_fn);
        self.register("SMALL", small_fn);
        self.register("RANK", rank_fn);
        self.register("PERCENTILE", percentile_fn);
        self.register("QUARTILE", quartile_fn);
        self.register("PERCENTRANK", percentrank_fn);
        self.register("FORECAST", forecast_fn);
        self.register("SLOPE", slope_fn);
        self.register("INTERCEPT", intercept_fn);
        self.register("CORREL", correl_fn);
        self.register("COVAR", covar_fn);
        self.register("STDEVP", stdevp_fn);
        self.register("VARP", varp_fn);
        self.register("AVERAGEIF", averageif_fn);

        // Advanced text functions
        self.register("CHAR", char_fn);
        self.register("CODE", code_fn);
        self.register("CLEAN", clean_fn);
        self.register("FIXED", fixed_fn);
        self.register("TEXTJOIN", textjoin_fn);
        self.register("CONCAT", concat_fn);
        self.register("T", t_fn);
        self.register("N", n_fn);
        self.register("EXACT", exact_fn);
        self.register("UNICODE", unicode_fn);
        self.register("TEXTBEFORE", textbefore_fn);
        self.register("TEXTAFTER", textafter_fn);

        // Date/time functions
        self.register("DATE", date_fn);
        self.register("TIME", time_fn);
        self.register("NOW", now_fn);
        self.register("TODAY", today_fn);
        self.register("YEAR", year_fn);
        self.register("MONTH", month_fn);
        self.register("DAY", day_fn);
        self.register("HOUR", hour_fn);
        self.register("MINUTE", minute_fn);
        self.register("SECOND", second_fn);
        self.register("WEEKDAY", weekday_fn);
        self.register("WEEKNUM", weeknum_fn);
        self.register("DATEVALUE", datevalue_fn);
        self.register("TIMEVALUE", timevalue_fn);
        self.register("EDATE", edate_fn);
        self.register("EOMONTH", eomonth_fn);
        self.register("DATEDIF", datedif_fn);
        self.register("DAYS", days_fn);
        self.register("ISOWEEKNUM", isoweeknum_fn);

        // Lookup functions
        self.register("VLOOKUP", vlookup_fn);
        self.register("HLOOKUP", hlookup_fn);
        self.register("INDEX", index_fn);
        self.register("MATCH", match_fn);
        self.register("CHOOSE", choose_fn);
        self.register("ROW", row_fn);
        self.register("COLUMN", column_fn);
        self.register("ROWS", rows_fn);
        self.register("COLUMNS", columns_fn);
        self.register("ADDRESS", address_fn);
        self.register("LOOKUP", lookup_fn);

        // Financial functions
        self.register("PMT", pmt_fn);
        self.register("PV", pv_fn);
        self.register("FV", fv_fn);
        self.register("NPV", npv_fn);
        self.register("RATE", rate_fn);
        self.register("NPER", nper_fn);
        self.register("SLN", sln_fn);
        self.register("SYD", syd_fn);
        self.register("DDB", ddb_fn);

        // Engineering functions
        self.register("BIN2DEC", bin2dec_fn);
        self.register("DEC2BIN", dec2bin_fn);
        self.register("HEX2DEC", hex2dec_fn);
        self.register("DEC2HEX", dec2hex_fn);
        self.register("OCT2DEC", oct2dec_fn);
        self.register("DEC2OCT", dec2oct_fn);
        self.register("BITAND", bitand_fn);
        self.register("BITOR", bitor_fn);
        self.register("BITXOR", bitxor_fn);
        self.register("BITLSHIFT", bitlshift_fn);
        self.register("BITRSHIFT", bitrshift_fn);
    }
}

// --- Math functions ---

fn ensure_finite_value(value: Value, context: &str) -> Value {
    match value {
        Value::Number(n) if !n.is_finite() => Value::Error(FormulaError::NumError(format!(
            "{} produced a non-finite number",
            context
        ))),
        Value::String(text) if text.len() > MAX_FUNCTION_TEXT_BYTES => {
            Value::Error(text_limit_error())
        }
        Value::Array(values) => {
            let mut checked = Vec::with_capacity(values.len());
            for value in values {
                let value = ensure_finite_value(value, context);
                if value.is_error() {
                    return value;
                }
                checked.push(value);
            }
            Value::Array(checked)
        }
        other => other,
    }
}

fn text_limit_error() -> FormulaError {
    FormulaError::ValueError(format!(
        "function text exceeds the {MAX_FUNCTION_TEXT_BYTES}-byte limit"
    ))
}

fn checked_text_size(bytes: Option<usize>) -> Result<usize, FormulaError> {
    bytes
        .filter(|&size| size <= MAX_FUNCTION_TEXT_BYTES)
        .ok_or_else(text_limit_error)
}

fn push_bounded_text(result: &mut String, text: &str) -> Result<(), FormulaError> {
    checked_text_size(result.len().checked_add(text.len()))?;
    result.push_str(text);
    Ok(())
}

fn nonnegative_integer(number: f64, context: &str) -> Result<u64, FormulaError> {
    // The strict upper bound avoids a saturating float-to-integer conversion.
    if !number.is_finite() || number < 0.0 || number >= u64::MAX as f64 {
        return Err(FormulaError::NumError(format!(
            "{context} requires a finite nonnegative integer below 2^64"
        )));
    }
    Ok(number.trunc() as u64)
}

fn check_iterations(iterations: u64, context: &str) -> Result<(), FormulaError> {
    if iterations > MAX_FUNCTION_ITERATIONS {
        return Err(FormulaError::NumError(format!(
            "{context} exceeds the {MAX_FUNCTION_ITERATIONS}-iteration limit"
        )));
    }
    Ok(())
}

fn combination(n: u64, k: u64, context: &str) -> Result<f64, FormulaError> {
    if k > n {
        return Ok(0.0);
    }
    let k = k.min(n - k);
    check_iterations(k, context)?;
    let mut result = 1.0_f64;
    for i in 0..k {
        result *= (n - i) as f64 / (i + 1) as f64;
        if !result.is_finite() {
            return Err(FormulaError::NumError(format!("{context} overflow")));
        }
    }
    Ok(result)
}

macro_rules! function_try {
    ($result:expr) => {
        match $result {
            Ok(value) => value,
            Err(error) => return Value::Error(error),
        }
    };
}

macro_rules! collect_numbers_or_error {
    ($args:expr) => {
        match collect_numbers($args) {
            Ok(nums) => nums,
            Err(e) => return Value::Error(e),
        }
    };
}

fn collect_numbers(args: &[Value]) -> Result<Vec<f64>, FormulaError> {
    let mut nums = Vec::new();
    for v in args {
        collect_nums_from_value(v, &mut nums)?;
    }
    Ok(nums)
}

fn collect_nums_from_value(v: &Value, nums: &mut Vec<f64>) -> Result<(), FormulaError> {
    match v {
        Value::Number(n) if n.is_finite() => nums.push(*n),
        Value::Number(_) => {
            return Err(FormulaError::NumError("non-finite number".into()));
        }
        Value::Boolean(b) => nums.push(if *b { 1.0 } else { 0.0 }),
        Value::String(s) => {
            if let Ok(n) = s.parse::<f64>() {
                if n.is_finite() {
                    nums.push(n);
                } else {
                    return Err(FormulaError::NumError("non-finite number".into()));
                }
            }
        }
        Value::Empty => {}
        Value::Error(_) => {}
        Value::Array(arr) => {
            for v in arr {
                collect_nums_from_value(v, nums)?;
            }
        }
    }
    Ok(())
}

fn sum(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    let nums = collect_numbers_or_error!(args);
    Value::Number(nums.iter().sum())
}

fn average(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    let nums = collect_numbers_or_error!(args);
    if nums.is_empty() {
        return Value::Error(FormulaError::DivisionByZero);
    }
    Value::Number(nums.iter().sum::<f64>() / nums.len() as f64)
}

fn min(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    let nums = collect_numbers_or_error!(args);
    if nums.is_empty() {
        return Value::Number(0.0);
    }
    Value::Number(nums.iter().cloned().fold(f64::INFINITY, f64::min))
}

fn max(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    let nums = collect_numbers_or_error!(args);
    if nums.is_empty() {
        return Value::Number(0.0);
    }
    Value::Number(nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
}

fn abs(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "ABS requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Number(n.abs()),
        Err(e) => Value::Error(e),
    }
}

fn round(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "ROUND requires at least 1 argument".into(),
        ));
    }
    let n = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let digits = if args.len() >= 2 {
        match args[1].as_number() {
            Ok(d) => d as i32,
            Err(e) => return Value::Error(e),
        }
    } else {
        0
    };
    let factor = 10f64.powi(digits);
    Value::Number((n * factor).round() / factor)
}

fn floor(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "FLOOR requires arguments".into(),
        ));
    }
    let n = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let sig = if args.len() >= 2 {
        match args[1].as_number() {
            Ok(s) => s,
            Err(e) => return Value::Error(e),
        }
    } else {
        1.0
    };
    if sig == 0.0 {
        return Value::Error(FormulaError::DivisionByZero);
    }
    Value::Number((n / sig).floor() * sig)
}

fn ceiling(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "CEILING requires arguments".into(),
        ));
    }
    let n = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let sig = if args.len() >= 2 {
        match args[1].as_number() {
            Ok(s) => s,
            Err(e) => return Value::Error(e),
        }
    } else {
        1.0
    };
    if sig == 0.0 {
        return Value::Error(FormulaError::DivisionByZero);
    }
    Value::Number((n / sig).ceil() * sig)
}

fn power(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "POWER requires 2 arguments".into(),
        ));
    }
    let base = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let exp = match args[1].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    Value::Number(base.powf(exp))
}

fn sqrt(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "SQRT requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => {
            if n < 0.0 {
                Value::Error(FormulaError::NumError("SQRT of negative number".into()))
            } else {
                Value::Number(n.sqrt())
            }
        }
        Err(e) => Value::Error(e),
    }
}

fn modulo(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "MOD requires 2 arguments".into(),
        ));
    }
    let a = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let b = match args[1].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    if b == 0.0 {
        return Value::Error(FormulaError::DivisionByZero);
    }
    Value::Number(a % b)
}

fn int_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "INT requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Number(n.floor()),
        Err(e) => Value::Error(e),
    }
}

fn exp_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "EXP requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Number(n.exp()),
        Err(e) => Value::Error(e),
    }
}

fn ln_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "LN requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => {
            if n <= 0.0 {
                Value::Error(FormulaError::NumError("LN of non-positive number".into()))
            } else {
                Value::Number(n.ln())
            }
        }
        Err(e) => Value::Error(e),
    }
}

fn log10_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "LOG10 requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => {
            if n <= 0.0 {
                Value::Error(FormulaError::NumError(
                    "LOG10 of non-positive number".into(),
                ))
            } else {
                Value::Number(n.log10())
            }
        }
        Err(e) => Value::Error(e),
    }
}

fn pi_fn(_: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    Value::Number(std::f64::consts::PI)
}

fn rand_fn(_: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    Value::Number(rand_value())
}

fn rand_value() -> f64 {
    use std::cell::Cell;
    use std::time::Instant;

    thread_local! {
        static SEED: Cell<u64> = Cell::new({
            let now = Instant::now();
            now.elapsed().as_nanos() as u64 ^ std::process::id() as u64
        });
    }

    SEED.with(|s| {
        let mut x = s.get();
        // xorshift64
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        (x as f64) / (u64::MAX as f64)
    })
}

fn sin_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "SIN requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Number(n.sin()),
        Err(e) => Value::Error(e),
    }
}

fn cos_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "COS requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Number(n.cos()),
        Err(e) => Value::Error(e),
    }
}

fn tan_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "TAN requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Number(n.tan()),
        Err(e) => Value::Error(e),
    }
}

fn asin_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "ASIN requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Number(n.asin()),
        Err(e) => Value::Error(e),
    }
}

fn acos_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "ACOS requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Number(n.acos()),
        Err(e) => Value::Error(e),
    }
}

fn atan_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "ATAN requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Number(n.atan()),
        Err(e) => Value::Error(e),
    }
}

fn atan2_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "ATAN2 requires 2 arguments".into(),
        ));
    }
    let y = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let x = match args[1].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    Value::Number(y.atan2(x))
}

fn degrees_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "DEGREES requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Number(n.to_degrees()),
        Err(e) => Value::Error(e),
    }
}

fn radians_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "RADIANS requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Number(n.to_radians()),
        Err(e) => Value::Error(e),
    }
}

fn count_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    let nums = collect_numbers_or_error!(args);
    Value::Number(nums.len() as f64)
}

fn counta_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    let mut count = 0usize;
    for v in args {
        match v {
            Value::Array(arr) => {
                for v in arr {
                    if !v.is_empty() {
                        count += 1;
                    }
                }
            }
            Value::Empty => {}
            _ => count += 1,
        }
    }
    Value::Number(count as f64)
}

fn product_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    let nums = collect_numbers_or_error!(args);
    if nums.is_empty() {
        return Value::Number(0.0);
    }
    Value::Number(nums.iter().product())
}

fn fact_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "FACT requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => {
            let n_int = function_try!(nonnegative_integer(n, "FACT"));
            if n_int > 170 {
                return Value::Error(FormulaError::NumError("FACT overflow above 170".into()));
            }
            let mut result: f64 = 1.0;
            for i in 1..=n_int {
                result *= i as f64;
            }
            Value::Number(result)
        }
        Err(e) => Value::Error(e),
    }
}

fn sign_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "SIGN requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Number(n.signum()),
        Err(e) => Value::Error(e),
    }
}

// --- Logical functions ---

fn if_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _exprs: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "IF requires at least 1 argument".into(),
        ));
    }
    // For IF, we need to evaluate based on the first arg value
    // args[0] is already evaluated
    match args[0].as_boolean() {
        Ok(true) => {
            if args.len() > 1 {
                args[1].clone()
            } else {
                Value::Boolean(true)
            }
        }
        Ok(false) => {
            if args.len() > 2 {
                args[2].clone()
            } else {
                Value::Boolean(false)
            }
        }
        Err(e) => Value::Error(e),
    }
}

fn and_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    for v in args {
        match v.as_boolean() {
            Ok(b) => {
                if !b {
                    return Value::Boolean(false);
                }
            }
            Err(e) => return Value::Error(e),
        }
    }
    Value::Boolean(true)
}

fn or_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    for v in args {
        match v.as_boolean() {
            Ok(true) => return Value::Boolean(true),
            Ok(false) => {}
            Err(e) => return Value::Error(e),
        }
    }
    Value::Boolean(false)
}

fn not_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "NOT requires 1 argument".into(),
        ));
    }
    match args[0].as_boolean() {
        Ok(b) => Value::Boolean(!b),
        Err(e) => Value::Error(e),
    }
}

fn true_fn(_: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    Value::Boolean(true)
}

fn false_fn(_: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    Value::Boolean(false)
}

fn iferror_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "IFERROR requires 2 arguments".into(),
        ));
    }
    if args[0].is_error() {
        args[1].clone()
    } else {
        args[0].clone()
    }
}

fn ifna_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "IFNA requires 2 arguments".into(),
        ));
    }
    match &args[0] {
        Value::Error(FormulaError::NotAvailable(_)) => args[1].clone(),
        _ => args[0].clone(),
    }
}

fn xor_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    let mut count = 0;
    for v in args {
        match v.as_boolean() {
            Ok(true) => count += 1,
            Ok(false) => {}
            Err(e) => return Value::Error(e),
        }
    }
    Value::Boolean(count % 2 == 1)
}

// --- Text functions ---

fn len_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "LEN requires 1 argument".into(),
        ));
    }
    match args[0].as_string() {
        Ok(s) => Value::Number(s.chars().count() as f64),
        Err(e) => Value::Error(e),
    }
}

fn upper_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "UPPER requires 1 argument".into(),
        ));
    }
    match args[0].as_string() {
        Ok(s) => Value::String(s.to_uppercase()),
        Err(e) => Value::Error(e),
    }
}

fn lower_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "LOWER requires 1 argument".into(),
        ));
    }
    match args[0].as_string() {
        Ok(s) => Value::String(s.to_lowercase()),
        Err(e) => Value::Error(e),
    }
}

fn proper_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "PROPER requires 1 argument".into(),
        ));
    }
    match args[0].as_string() {
        Ok(s) => {
            let mut result = String::new();
            let mut capitalize = true;
            for ch in s.chars() {
                if capitalize && ch.is_alphabetic() {
                    result.extend(ch.to_uppercase());
                    capitalize = false;
                } else {
                    result.push(ch.to_lowercase().next().unwrap_or(ch));
                    if ch.is_whitespace() {
                        capitalize = true;
                    }
                }
            }
            Value::String(result)
        }
        Err(e) => Value::Error(e),
    }
}

fn trim_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "TRIM requires 1 argument".into(),
        ));
    }
    match args[0].as_string() {
        Ok(s) => {
            let trimmed: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
            Value::String(trimmed)
        }
        Err(e) => Value::Error(e),
    }
}

fn left_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "LEFT requires at least 1 argument".into(),
        ));
    }
    let s = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    let n = if args.len() >= 2 {
        match args[1].as_number() {
            Ok(n) => n as usize,
            Err(e) => return Value::Error(e),
        }
    } else {
        1
    };
    let result: String = s.chars().take(n).collect();
    Value::String(result)
}

fn right_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "RIGHT requires at least 1 argument".into(),
        ));
    }
    let s = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    let n = if args.len() >= 2 {
        match args[1].as_number() {
            Ok(n) => n as usize,
            Err(e) => return Value::Error(e),
        }
    } else {
        1
    };
    let chars: Vec<char> = s.chars().collect();
    let start = chars.len().saturating_sub(n);
    Value::String(chars[start..].iter().collect())
}

fn mid_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.len() < 3 {
        return Value::Error(FormulaError::InvalidArguments(
            "MID requires 3 arguments".into(),
        ));
    }
    let s = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    let start = match args[1].as_number() {
        Ok(n) => n as usize,
        Err(e) => return Value::Error(e),
    };
    let len = match args[2].as_number() {
        Ok(n) => n as usize,
        Err(e) => return Value::Error(e),
    };
    if start == 0 {
        return Value::Error(FormulaError::ValueError("MID start must be >= 1".into()));
    }
    let chars: Vec<char> = s.chars().collect();
    let start_idx = start - 1;
    let end_idx = start_idx.saturating_add(len).min(chars.len());
    if start_idx >= chars.len() {
        return Value::String(String::new());
    }
    Value::String(chars[start_idx..end_idx].iter().collect())
}

fn concatenate_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    let mut result = String::new();
    for v in args {
        match v.as_string() {
            Ok(s) => function_try!(push_bounded_text(&mut result, &s)),
            Err(e) => return Value::Error(e),
        }
    }
    Value::String(result)
}

fn substitute_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 3 {
        return Value::Error(FormulaError::InvalidArguments(
            "SUBSTITUTE requires at least 3 arguments".into(),
        ));
    }
    let text = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    let old = match args[1].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    let new = match args[2].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    if old.is_empty() {
        return Value::String(text);
    }
    let instance = if args.len() >= 4 {
        Some(function_try!(args[3].as_number()) as usize)
    } else {
        None
    };
    // Count matches before allocating: replacement text may be much larger than the input.
    let matches = text.matches(&old).count();
    let replacements = match instance {
        Some(index) => usize::from(index > 0 && index <= matches),
        None => matches,
    };
    let retained = text.len() - replacements * old.len();
    let size = function_try!(checked_text_size(
        replacements
            .checked_mul(new.len())
            .and_then(|bytes| retained.checked_add(bytes))
    ));
    let mut result = String::with_capacity(size);
    let mut previous = 0;
    for (index, (position, matched)) in text.match_indices(&old).enumerate() {
        result.push_str(&text[previous..position]);
        if instance.is_none() || instance == Some(index + 1) {
            result.push_str(&new);
        } else {
            result.push_str(matched);
        }
        previous = position + old.len();
    }
    result.push_str(&text[previous..]);
    Value::String(result)
}

fn rept_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "REPT requires 2 arguments".into(),
        ));
    }
    let s = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    let n = function_try!(nonnegative_integer(
        function_try!(args[1].as_number()),
        "REPT"
    ));
    if s.is_empty() || n == 0 {
        return Value::String(String::new());
    }
    let n = match usize::try_from(n) {
        Ok(n) => n,
        Err(_) => return Value::Error(text_limit_error()),
    };
    function_try!(checked_text_size(s.len().checked_mul(n)));
    Value::String(s.repeat(n))
}

fn find_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "FIND requires at least 2 arguments".into(),
        ));
    }
    let needle = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    let haystack = match args[1].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    let start = if args.len() >= 3 {
        match args[2].as_number() {
            Ok(n) => n as usize,
            Err(e) => return Value::Error(e),
        }
    } else {
        1
    };
    if start == 0 || start > haystack.chars().count() + 1 {
        return Value::Error(FormulaError::ValueError(
            "FIND start position invalid".into(),
        ));
    }
    let start_byte = haystack
        .char_indices()
        .nth(start - 1)
        .map(|(i, _)| i)
        .unwrap_or(haystack.len());
    if let Some(pos) = haystack[start_byte..].find(&needle) {
        let byte_pos = start_byte + pos;
        let char_pos = haystack[..byte_pos].chars().count() + 1;
        Value::Number(char_pos as f64)
    } else {
        Value::Error(FormulaError::ValueError("FIND: substring not found".into()))
    }
}

fn search_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "SEARCH requires at least 2 arguments".into(),
        ));
    }
    let needle = match args[0].as_string() {
        Ok(s) => s.to_lowercase(),
        Err(e) => return Value::Error(e),
    };
    let haystack = match args[1].as_string() {
        Ok(s) => s.to_lowercase(),
        Err(e) => return Value::Error(e),
    };
    let start = if args.len() >= 3 {
        match args[2].as_number() {
            Ok(n) => n as usize,
            Err(e) => return Value::Error(e),
        }
    } else {
        1
    };
    if start == 0 || start > haystack.chars().count() + 1 {
        return Value::Error(FormulaError::ValueError(
            "SEARCH start position invalid".into(),
        ));
    }
    let start_byte = haystack
        .char_indices()
        .nth(start - 1)
        .map(|(i, _)| i)
        .unwrap_or(haystack.len());
    if let Some(pos) = haystack[start_byte..].find(&needle) {
        let byte_pos = start_byte + pos;
        let char_pos = haystack[..byte_pos].chars().count() + 1;
        Value::Number(char_pos as f64)
    } else {
        Value::Error(FormulaError::ValueError(
            "SEARCH: substring not found".into(),
        ))
    }
}

fn replace_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 4 {
        return Value::Error(FormulaError::InvalidArguments(
            "REPLACE requires 4 arguments".into(),
        ));
    }
    let text = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    let start = match args[1].as_number() {
        Ok(n) => n as usize,
        Err(e) => return Value::Error(e),
    };
    let len = match args[2].as_number() {
        Ok(n) => n as usize,
        Err(e) => return Value::Error(e),
    };
    let replacement = match args[3].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    if start == 0 {
        return Value::Error(FormulaError::ValueError(
            "REPLACE start must be >= 1".into(),
        ));
    }
    let start_idx = start - 1;
    let end_idx = start_idx.saturating_add(len);
    let start_byte = text
        .char_indices()
        .nth(start_idx)
        .map_or(text.len(), |(i, _)| i);
    let end_byte = text
        .char_indices()
        .nth(end_idx)
        .map_or(text.len(), |(i, _)| i);
    let retained = text.len() - (end_byte - start_byte);
    let size = function_try!(checked_text_size(retained.checked_add(replacement.len())));
    let mut result = String::with_capacity(size);
    result.push_str(&text[..start_byte]);
    result.push_str(&replacement);
    result.push_str(&text[end_byte..]);
    Value::String(result)
}

fn text_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "TEXT requires 2 arguments".into(),
        ));
    }
    let format_str = match args[1].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    let number_format = sheets_core::number_format::NumberFormat::from_pattern(&format_str);
    match args[0] {
        Value::Number(n) => {
            if matches!(
                number_format,
                sheets_core::number_format::NumberFormat::Date
                    | sheets_core::number_format::NumberFormat::DateTime
            ) {
                function_try!(serial_to_date(n));
            }
            Value::String(number_format.format(n))
        }
        _ => match args[0].as_string() {
            Ok(s) => Value::String(s),
            Err(e) => Value::Error(e),
        },
    }
}

fn value_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "VALUE requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Number(n),
        Err(e) => Value::Error(e),
    }
}

// --- Info functions ---

fn isnumber_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Boolean(false);
    }
    Value::Boolean(matches!(args[0], Value::Number(_)))
}

fn istext_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Boolean(false);
    }
    Value::Boolean(matches!(args[0], Value::String(_)))
}

fn islogical_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Boolean(false);
    }
    Value::Boolean(matches!(args[0], Value::Boolean(_)))
}

fn iserror_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Boolean(false);
    }
    Value::Boolean(args[0].is_error())
}

fn isempty_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Boolean(true);
    }
    Value::Boolean(args[0].is_empty())
}

fn isnontext_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Boolean(true);
    }
    Value::Boolean(!matches!(args[0], Value::String(_)))
}

fn isodd_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "ISODD requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Boolean((n as i64) % 2 != 0),
        Err(e) => Value::Error(e),
    }
}

fn iseven_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "ISEVEN requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Boolean((n as i64) % 2 == 0),
        Err(e) => Value::Error(e),
    }
}

fn na_fn(_: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    Value::Error(FormulaError::NotAvailable("N/A".into()))
}

fn type_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Number(1.0);
    }
    Value::Number(match args[0] {
        Value::Number(_) => 1.0,
        Value::String(_) => 2.0,
        Value::Boolean(_) => 4.0,
        Value::Error(_) => 16.0,
        Value::Empty => 1.0,
        Value::Array(_) => 64.0,
    })
}

// --- Advanced math functions ---

fn roundup_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "ROUNDUP requires 1 argument".into(),
        ));
    }
    let n = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let digits = if args.len() >= 2 {
        match args[1].as_number() {
            Ok(d) => d as i32,
            Err(e) => return Value::Error(e),
        }
    } else {
        0
    };
    let factor = 10f64.powi(digits);
    Value::Number((n.abs() * factor).ceil() / factor * n.signum())
}

fn rounddown_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "ROUNDDOWN requires 1 argument".into(),
        ));
    }
    let n = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let digits = if args.len() >= 2 {
        match args[1].as_number() {
            Ok(d) => d as i32,
            Err(e) => return Value::Error(e),
        }
    } else {
        0
    };
    let factor = 10f64.powi(digits);
    Value::Number((n.abs() * factor).floor() / factor * n.signum())
}

fn trunc_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "TRUNC requires 1 argument".into(),
        ));
    }
    let n = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let digits = if args.len() >= 2 {
        match args[1].as_number() {
            Ok(d) => d as i32,
            Err(e) => return Value::Error(e),
        }
    } else {
        0
    };
    let factor = 10f64.powi(digits);
    Value::Number((n * factor).trunc() / factor)
}

fn mround_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "MROUND requires 2 arguments".into(),
        ));
    }
    let n = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let multiple = match args[1].as_number() {
        Ok(m) => m,
        Err(e) => return Value::Error(e),
    };
    if multiple == 0.0 {
        return Value::Error(FormulaError::DivisionByZero);
    }
    Value::Number((n / multiple).round() * multiple)
}

fn quotient_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "QUOTIENT requires 2 arguments".into(),
        ));
    }
    let a = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let b = match args[1].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    if b == 0.0 {
        return Value::Error(FormulaError::DivisionByZero);
    }
    Value::Number((a / b).trunc())
}

fn gcd_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    let nums = collect_numbers_or_error!(args);
    let mut result = 0_u64;
    for number in nums {
        let number = function_try!(nonnegative_integer(number, "GCD"));
        result = gcd_u64(result, number);
    }
    Value::Number(result as f64)
}

fn gcd_u64(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

fn lcm_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    let nums = collect_numbers_or_error!(args);
    if nums.is_empty() {
        return Value::Number(0.0);
    }
    let mut result = 1_u64;
    for number in nums {
        let number = function_try!(nonnegative_integer(number, "LCM"));
        if number == 0 || result == 0 {
            result = 0;
            continue;
        }
        result = match (result / gcd_u64(result, number)).checked_mul(number) {
            Some(result) => result,
            None => return Value::Error(FormulaError::NumError("LCM integer overflow".into())),
        };
    }
    Value::Number(result as f64)
}

fn combin_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "COMBIN requires 2 arguments".into(),
        ));
    }
    let n = function_try!(nonnegative_integer(
        function_try!(args[0].as_number()),
        "COMBIN"
    ));
    let k = function_try!(nonnegative_integer(
        function_try!(args[1].as_number()),
        "COMBIN"
    ));
    Value::Number(function_try!(combination(n, k, "COMBIN")))
}

fn permut_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "PERMUT requires 2 arguments".into(),
        ));
    }
    let n = function_try!(nonnegative_integer(
        function_try!(args[0].as_number()),
        "PERMUT"
    ));
    let k = function_try!(nonnegative_integer(
        function_try!(args[1].as_number()),
        "PERMUT"
    ));
    if k > n {
        return Value::Number(0.0);
    }
    function_try!(check_iterations(k, "PERMUT"));
    let mut result = 1.0_f64;
    for i in 0..k {
        result *= (n - i) as f64;
        if !result.is_finite() {
            return Value::Error(FormulaError::NumError("PERMUT overflow".into()));
        }
    }
    Value::Number(result)
}

fn randbetween_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "RANDBETWEEN requires 2 arguments".into(),
        ));
    }
    let lo = match args[0].as_number() {
        Ok(n) if n.is_finite() && n >= i64::MIN as f64 && n < i64::MAX as f64 => n as i64,
        Ok(_) => {
            return Value::Error(FormulaError::NumError(
                "RANDBETWEEN requires finite bounds in the signed 64-bit range".into(),
            ))
        }
        Err(e) => return Value::Error(e),
    };
    let hi = match args[1].as_number() {
        Ok(n) if n.is_finite() && n >= i64::MIN as f64 && n < i64::MAX as f64 => n as i64,
        Ok(_) => {
            return Value::Error(FormulaError::NumError(
                "RANDBETWEEN requires finite bounds in the signed 64-bit range".into(),
            ))
        }
        Err(e) => return Value::Error(e),
    };
    if lo > hi {
        return Value::Error(FormulaError::NumError("RANDBETWEEN: bottom > top".into()));
    }
    // Subtract as floats so even the full signed range cannot overflow an integer.
    let range = hi as f64 - lo as f64 + 1.0;
    Value::Number((lo as f64 + (rand_value() * range).floor()).min(hi as f64))
}

fn log_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "LOG requires at least 1 argument".into(),
        ));
    }
    let n = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    if n <= 0.0 {
        return Value::Error(FormulaError::NumError("LOG of non-positive number".into()));
    }
    let base = if args.len() >= 2 {
        match args[1].as_number() {
            Ok(b) => b,
            Err(e) => return Value::Error(e),
        }
    } else {
        10.0
    };
    if base <= 0.0 || base == 1.0 {
        return Value::Error(FormulaError::NumError("LOG invalid base".into()));
    }
    Value::Number(n.log(base))
}

fn log2_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "LOG2 requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => {
            if n <= 0.0 {
                Value::Error(FormulaError::NumError("LOG2 of non-positive number".into()))
            } else {
                Value::Number(n.log2())
            }
        }
        Err(e) => Value::Error(e),
    }
}

fn sinh_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "SINH requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Number(n.sinh()),
        Err(e) => Value::Error(e),
    }
}
fn cosh_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "COSH requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Number(n.cosh()),
        Err(e) => Value::Error(e),
    }
}
fn tanh_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "TANH requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Number(n.tanh()),
        Err(e) => Value::Error(e),
    }
}
fn asinh_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "ASINH requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => Value::Number(n.asinh()),
        Err(e) => Value::Error(e),
    }
}
fn acosh_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "ACOSH requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => {
            if n < 1.0 {
                Value::Error(FormulaError::NumError("ACOSH requires n >= 1".into()))
            } else {
                Value::Number(n.acosh())
            }
        }
        Err(e) => Value::Error(e),
    }
}
fn atanh_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "ATANH requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => {
            if n.abs() >= 1.0 {
                Value::Error(FormulaError::NumError("ATANH requires |n| < 1".into()))
            } else {
                Value::Number(n.atanh())
            }
        }
        Err(e) => Value::Error(e),
    }
}

fn factdouble_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "FACTDOUBLE requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => {
            let n_int = function_try!(nonnegative_integer(n, "FACTDOUBLE"));
            if n_int > 300 {
                return Value::Error(FormulaError::NumError(
                    "FACTDOUBLE overflow above 300".into(),
                ));
            }
            let mut result: f64 = 1.0;
            let mut i = if n_int.is_multiple_of(2) { 2 } else { 1 };
            while i <= n_int {
                result *= i as f64;
                i += 2;
            }
            Value::Number(result)
        }
        Err(e) => Value::Error(e),
    }
}

fn sqrtpi_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "SQRTPI requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => {
            if n < 0.0 {
                Value::Error(FormulaError::NumError("SQRTPI of negative".into()))
            } else {
                Value::Number((n * std::f64::consts::PI).sqrt())
            }
        }
        Err(e) => Value::Error(e),
    }
}

fn gestep_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "GESTEP requires at least 1 argument".into(),
        ));
    }
    let n = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let step = if args.len() >= 2 {
        match args[1].as_number() {
            Ok(s) => s,
            Err(e) => return Value::Error(e),
        }
    } else {
        0.0
    };
    Value::Number(if n >= step { 1.0 } else { 0.0 })
}

fn delta_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "DELTA requires at least 1 argument".into(),
        ));
    }
    let n = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let m = if args.len() >= 2 {
        match args[1].as_number() {
            Ok(m) => m,
            Err(e) => return Value::Error(e),
        }
    } else {
        0.0
    };
    Value::Number(if n == m { 1.0 } else { 0.0 })
}

fn multinomial_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    let nums = collect_numbers_or_error!(args);
    if nums.is_empty() {
        return Value::Number(1.0);
    }
    let mut total = 0_u64;
    let mut result = 1.0_f64;
    let mut work = 0_u64;
    for n in nums {
        let n = function_try!(nonnegative_integer(n, "MULTINOMIAL"));
        // Sequential binomial coefficients avoid looping over a huge single category.
        work = match work.checked_add(n.min(total)) {
            Some(work) => work,
            None => {
                return Value::Error(FormulaError::NumError("MULTINOMIAL input overflow".into()))
            }
        };
        function_try!(check_iterations(work, "MULTINOMIAL"));
        total = match total.checked_add(n) {
            Some(total) => total,
            None => {
                return Value::Error(FormulaError::NumError("MULTINOMIAL input overflow".into()))
            }
        };
        result *= function_try!(combination(total, n, "MULTINOMIAL"));
        if !result.is_finite() {
            return Value::Error(FormulaError::NumError("MULTINOMIAL overflow".into()));
        }
    }
    Value::Number(result)
}

fn combina_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "COMBINA requires 2 arguments".into(),
        ));
    }
    let n = function_try!(nonnegative_integer(
        function_try!(args[0].as_number()),
        "COMBINA"
    ));
    let k = function_try!(nonnegative_integer(
        function_try!(args[1].as_number()),
        "COMBINA"
    ));
    if k == 0 {
        return Value::Number(1.0);
    }
    if n == 0 {
        return Value::Number(0.0);
    }
    let total = match n.checked_add(k - 1) {
        Some(total) => total,
        None => return Value::Error(FormulaError::NumError("COMBINA input overflow".into())),
    };
    Value::Number(function_try!(combination(total, k, "COMBINA")))
}

fn permuta_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "PERMUTA requires 2 arguments".into(),
        ));
    }
    let n = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let k = match args[1].as_number() {
        Ok(k) => k,
        Err(e) => return Value::Error(e),
    };
    let n = function_try!(nonnegative_integer(n, "PERMUTA")) as f64;
    let k = function_try!(nonnegative_integer(k, "PERMUTA"));
    function_try!(check_iterations(k, "PERMUTA"));
    if n == 0.0 && k > 0 {
        return Value::Error(FormulaError::NumError(
            "PERMUTATIONA requires objects when selecting any".into(),
        ));
    }
    // Permutations with repetition are n^k, with both arguments truncated.
    // https://support.microsoft.com/en-us/excel/permutationa-function
    Value::Number(n.powi(k as i32))
}

fn seriessum_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 4 {
        return Value::Error(FormulaError::InvalidArguments(
            "SERIESSUM requires 4 arguments".into(),
        ));
    }
    let x = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let n = match args[1].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let m = match args[2].as_number() {
        Ok(m) => m,
        Err(e) => return Value::Error(e),
    };
    let coeffs = match &args[3] {
        Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v.as_number().ok())
            .collect::<Vec<_>>(),
        v => vec![v.as_number().unwrap_or(0.0)],
    };
    let mut sum = 0.0_f64;
    for (i, &c) in coeffs.iter().enumerate() {
        sum += c * x.powf(n + m * i as f64);
    }
    Value::Number(sum)
}

// --- Statistical functions ---

fn median_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    let mut nums = collect_numbers_or_error!(args);
    if nums.is_empty() {
        return Value::Error(FormulaError::NumError("MEDIAN: no numbers".into()));
    }
    nums.sort_by(|a, b| a.total_cmp(b));
    let len = nums.len();
    Value::Number(if len.is_multiple_of(2) {
        (nums[len / 2 - 1] + nums[len / 2]) / 2.0
    } else {
        nums[len / 2]
    })
}

fn mode_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    let nums = collect_numbers_or_error!(args);
    if nums.is_empty() {
        return Value::Error(FormulaError::NumError("MODE: no numbers".into()));
    }
    let mut counts: std::collections::HashMap<u64, (f64, usize)> = std::collections::HashMap::new();
    for &n in &nums {
        let key = n.to_bits();
        let entry = counts.entry(key).or_insert((n, 0));
        entry.1 += 1;
    }
    counts
        .into_values()
        .max_by_key(|(_, c)| *c)
        .map(|(n, _)| Value::Number(n))
        .unwrap_or(Value::Error(FormulaError::NumError("MODE: no mode".into())))
}

fn stdev_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    let nums = collect_numbers_or_error!(args);
    if nums.len() < 2 {
        return Value::Error(FormulaError::NumError("STDEV: needs >= 2 values".into()));
    }
    let mean = nums.iter().sum::<f64>() / nums.len() as f64;
    let variance = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (nums.len() - 1) as f64;
    Value::Number(variance.sqrt())
}

fn var_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    let nums = collect_numbers_or_error!(args);
    if nums.len() < 2 {
        return Value::Error(FormulaError::NumError("VAR: needs >= 2 values".into()));
    }
    let mean = nums.iter().sum::<f64>() / nums.len() as f64;
    let variance = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (nums.len() - 1) as f64;
    Value::Number(variance)
}

fn stdevp_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    let nums = collect_numbers_or_error!(args);
    if nums.is_empty() {
        return Value::Error(FormulaError::NumError("STDEVP: no numbers".into()));
    }
    let mean = nums.iter().sum::<f64>() / nums.len() as f64;
    let variance = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / nums.len() as f64;
    Value::Number(variance.sqrt())
}

fn varp_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    let nums = collect_numbers_or_error!(args);
    if nums.is_empty() {
        return Value::Error(FormulaError::NumError("VARP: no numbers".into()));
    }
    let mean = nums.iter().sum::<f64>() / nums.len() as f64;
    let variance = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / nums.len() as f64;
    Value::Number(variance)
}

fn large_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "LARGE requires 2 arguments".into(),
        ));
    }
    let mut nums = collect_numbers_or_error!(&args[0..1]);
    let k = match args[1].as_number() {
        Ok(k) => k as usize,
        Err(e) => return Value::Error(e),
    };
    if k == 0 || k > nums.len() {
        return Value::Error(FormulaError::NumError("LARGE: k out of range".into()));
    }
    nums.sort_by(|a, b| b.total_cmp(a));
    Value::Number(nums[k - 1])
}

fn small_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "SMALL requires 2 arguments".into(),
        ));
    }
    let mut nums = collect_numbers_or_error!(&args[0..1]);
    let k = match args[1].as_number() {
        Ok(k) => k as usize,
        Err(e) => return Value::Error(e),
    };
    if k == 0 || k > nums.len() {
        return Value::Error(FormulaError::NumError("SMALL: k out of range".into()));
    }
    nums.sort_by(|a, b| a.total_cmp(b));
    Value::Number(nums[k - 1])
}

fn rank_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "RANK requires 2 arguments".into(),
        ));
    }
    let target = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let nums = collect_numbers_or_error!(&args[1..2]);
    let ascending = if args.len() >= 3 {
        match args[2].as_number() {
            Ok(n) => n != 0.0,
            Err(_) => false,
        }
    } else {
        false
    };
    let mut sorted: Vec<f64> = nums.to_vec();
    if ascending {
        sorted.sort_by(|a, b| a.total_cmp(b));
    } else {
        sorted.sort_by(|a, b| b.total_cmp(a));
    }
    let rank = sorted
        .iter()
        .position(|&x| x == target)
        .map(|p| p + 1)
        .unwrap_or(0);
    Value::Number(rank as f64)
}

fn percentile_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "PERCENTILE requires 2 arguments".into(),
        ));
    }
    let mut nums = collect_numbers_or_error!(&args[0..1]);
    let p = match args[1].as_number() {
        Ok(p) => p,
        Err(e) => return Value::Error(e),
    };
    if !(0.0..=1.0).contains(&p) || nums.is_empty() {
        return Value::Error(FormulaError::NumError("PERCENTILE invalid".into()));
    }
    nums.sort_by(|a, b| a.total_cmp(b));
    let idx = p * (nums.len() - 1) as f64;
    let lo = idx.floor() as usize;
    let hi = idx.ceil() as usize;
    if lo == hi {
        Value::Number(nums[lo])
    } else {
        Value::Number(nums[lo] + (nums[hi] - nums[lo]) * (idx - lo as f64))
    }
}

fn quartile_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "QUARTILE requires 2 arguments".into(),
        ));
    }
    let q = match args[1].as_number() {
        Ok(q) => q as i32,
        Err(e) => return Value::Error(e),
    };
    if !(0..=4).contains(&q) {
        return Value::Error(FormulaError::NumError("QUARTILE: quart must be 0-4".into()));
    }
    let p = match q {
        0 => 0.0,
        1 => 0.25,
        2 => 0.5,
        3 => 0.75,
        4 => 1.0,
        _ => 0.0,
    };
    let new_args = vec![args[0].clone(), Value::Number(p)];
    percentile_fn(&new_args, &SimpleProvider::new(), &mut HashSet::new(), &[])
}

fn percentrank_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "PERCENTRANK requires 2 arguments".into(),
        ));
    }
    let mut nums = collect_numbers_or_error!(&args[0..1]);
    let x = match args[1].as_number() {
        Ok(x) => x,
        Err(e) => return Value::Error(e),
    };
    if nums.is_empty() {
        return Value::Error(FormulaError::NumError("PERCENTRANK: empty array".into()));
    }
    nums.sort_by(|a, b| a.total_cmp(b));
    if x < nums[0] || x > *nums.last().unwrap() {
        return Value::Error(FormulaError::NumError("PERCENTRANK: x out of range".into()));
    }
    let n = nums.len();
    let rank = nums.iter().filter(|&&v| v <= x).count();
    Value::Number(if n == 1 {
        1.0
    } else {
        (rank as f64 - 1.0) / (n as f64 - 1.0)
    })
}

fn forecast_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 3 {
        return Value::Error(FormulaError::InvalidArguments(
            "FORECAST requires 3 arguments".into(),
        ));
    }
    let x = match args[0].as_number() {
        Ok(x) => x,
        Err(e) => return Value::Error(e),
    };
    let ys = collect_numbers_or_error!(&args[1..2]);
    let xs = collect_numbers_or_error!(&args[2..3]);
    if xs.len() != ys.len() || xs.is_empty() {
        return Value::Error(FormulaError::NumError("FORECAST: array mismatch".into()));
    }
    let n = xs.len() as f64;
    let mean_x = xs.iter().sum::<f64>() / n;
    let mean_y = ys.iter().sum::<f64>() / n;
    let slope = xs
        .iter()
        .zip(ys.iter())
        .map(|(xi, yi)| (xi - mean_x) * (yi - mean_y))
        .sum::<f64>()
        / xs.iter().map(|xi| (xi - mean_x).powi(2)).sum::<f64>();
    let intercept = mean_y - slope * mean_x;
    Value::Number(intercept + slope * x)
}

fn slope_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "SLOPE requires 2 arguments".into(),
        ));
    }
    let ys = collect_numbers_or_error!(&args[0..1]);
    let xs = collect_numbers_or_error!(&args[1..2]);
    if xs.len() != ys.len() || xs.is_empty() {
        return Value::Error(FormulaError::NumError("SLOPE: array mismatch".into()));
    }
    let n = xs.len() as f64;
    let mean_x = xs.iter().sum::<f64>() / n;
    let mean_y = ys.iter().sum::<f64>() / n;
    let slope = xs
        .iter()
        .zip(ys.iter())
        .map(|(xi, yi)| (xi - mean_x) * (yi - mean_y))
        .sum::<f64>()
        / xs.iter().map(|xi| (xi - mean_x).powi(2)).sum::<f64>();
    Value::Number(slope)
}

fn intercept_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "INTERCEPT requires 2 arguments".into(),
        ));
    }
    let ys = collect_numbers_or_error!(&args[0..1]);
    let xs = collect_numbers_or_error!(&args[1..2]);
    if xs.len() != ys.len() || xs.is_empty() {
        return Value::Error(FormulaError::NumError("INTERCEPT: array mismatch".into()));
    }
    let n = xs.len() as f64;
    let mean_x = xs.iter().sum::<f64>() / n;
    let mean_y = ys.iter().sum::<f64>() / n;
    let slope = xs
        .iter()
        .zip(ys.iter())
        .map(|(xi, yi)| (xi - mean_x) * (yi - mean_y))
        .sum::<f64>()
        / xs.iter().map(|xi| (xi - mean_x).powi(2)).sum::<f64>();
    Value::Number(mean_y - slope * mean_x)
}

fn correl_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "CORREL requires 2 arguments".into(),
        ));
    }
    let xs = collect_numbers_or_error!(&args[0..1]);
    let ys = collect_numbers_or_error!(&args[1..2]);
    if xs.len() != ys.len() || xs.len() < 2 {
        return Value::Error(FormulaError::NumError("CORREL: array mismatch".into()));
    }
    let n = xs.len() as f64;
    let mean_x = xs.iter().sum::<f64>() / n;
    let mean_y = ys.iter().sum::<f64>() / n;
    let cov = xs
        .iter()
        .zip(ys.iter())
        .map(|(xi, yi)| (xi - mean_x) * (yi - mean_y))
        .sum::<f64>();
    let std_x = xs
        .iter()
        .map(|xi| (xi - mean_x).powi(2))
        .sum::<f64>()
        .sqrt();
    let std_y = ys
        .iter()
        .map(|yi| (yi - mean_y).powi(2))
        .sum::<f64>()
        .sqrt();
    if std_x == 0.0 || std_y == 0.0 {
        return Value::Error(FormulaError::NumError("CORREL: zero variance".into()));
    }
    Value::Number(cov / (std_x * std_y))
}

fn covar_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "COVAR requires 2 arguments".into(),
        ));
    }
    let xs = collect_numbers_or_error!(&args[0..1]);
    let ys = collect_numbers_or_error!(&args[1..2]);
    if xs.len() != ys.len() || xs.is_empty() {
        return Value::Error(FormulaError::NumError("COVAR: array mismatch".into()));
    }
    let n = xs.len() as f64;
    let mean_x = xs.iter().sum::<f64>() / n;
    let mean_y = ys.iter().sum::<f64>() / n;
    let cov = xs
        .iter()
        .zip(ys.iter())
        .map(|(xi, yi)| (xi - mean_x) * (yi - mean_y))
        .sum::<f64>()
        / n;
    Value::Number(cov)
}

fn averageif_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "AVERAGEIF requires 2 arguments".into(),
        ));
    }
    let nums = collect_numbers_or_error!(&args[0..1]);
    let criterion = match args[1].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    let target_nums = if args.len() >= 3 {
        collect_numbers_or_error!(&args[2..3])
    } else {
        nums.clone()
    };
    let mut sum = 0.0_f64;
    let mut count = 0usize;
    for (i, &n) in nums.iter().enumerate() {
        let check_val = target_nums.get(i).copied().unwrap_or(n);
        if match_criterion(&criterion, check_val) {
            sum += n;
            count += 1;
        }
    }
    if count == 0 {
        return Value::Error(FormulaError::DivisionByZero);
    }
    Value::Number(sum / count as f64)
}

fn match_criterion(criterion: &str, val: f64) -> bool {
    if criterion.eq_ignore_ascii_case("true") {
        return val != 0.0;
    }
    if criterion.eq_ignore_ascii_case("false") {
        return val == 0.0;
    }
    for op in [">=", "<=", "<>", ">", "<", "="] {
        if let Some(rest) = criterion.strip_prefix(op) {
            if let Ok(threshold) = rest.parse::<f64>() {
                return match op {
                    ">=" => val >= threshold,
                    "<=" => val <= threshold,
                    "<>" => val != threshold,
                    ">" => val > threshold,
                    "<" => val < threshold,
                    "=" => val == threshold,
                    _ => false,
                };
            }
        }
    }
    criterion.parse::<f64>().map(|t| val == t).unwrap_or(false)
}

// --- Advanced text functions ---

fn char_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "CHAR requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(n) => {
            if let Some(ch) = char::from_u32(n as u32) {
                Value::String(ch.to_string())
            } else {
                Value::Error(FormulaError::NumError("CHAR: invalid code".into()))
            }
        }
        Err(e) => Value::Error(e),
    }
}

fn code_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "CODE requires 1 argument".into(),
        ));
    }
    match args[0].as_string() {
        Ok(s) => {
            if let Some(ch) = s.chars().next() {
                Value::Number(ch as u32 as f64)
            } else {
                Value::Number(0.0)
            }
        }
        Err(e) => Value::Error(e),
    }
}

fn clean_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "CLEAN requires 1 argument".into(),
        ));
    }
    match args[0].as_string() {
        Ok(s) => Value::String(s.chars().filter(|c| !c.is_control()).collect()),
        Err(e) => Value::Error(e),
    }
}

fn fixed_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "FIXED requires 1 argument".into(),
        ));
    }
    let n = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let decimals = if args.len() >= 2 {
        match args[1].as_number() {
            Ok(d) if d.is_finite() => d as i32,
            Ok(_) => {
                return Value::Error(FormulaError::NumError(
                    "FIXED requires finite decimals".into(),
                ))
            }
            Err(e) => return Value::Error(e),
        }
    } else {
        2
    };
    if decimals > MAX_FIXED_DECIMALS {
        return Value::Error(FormulaError::NumError(format!(
            "FIXED exceeds the {MAX_FIXED_DECIMALS}-decimal limit"
        )));
    }
    Value::String(format!("{:.*}", decimals.max(0) as usize, n))
}

fn textjoin_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 3 {
        return Value::Error(FormulaError::InvalidArguments(
            "TEXTJOIN requires 3 arguments".into(),
        ));
    }
    let delim = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    let ignore_empty = match args[1].as_boolean() {
        Ok(b) => b,
        Err(e) => return Value::Error(e),
    };
    let mut result = String::new();
    let mut first = true;
    let mut append = |value: &Value| -> Result<(), FormulaError> {
        if value.is_empty() && ignore_empty {
            return Ok(());
        }
        if let Ok(text) = value.as_string() {
            if !first {
                push_bounded_text(&mut result, &delim)?;
            }
            push_bounded_text(&mut result, &text)?;
            first = false;
        }
        Ok(())
    };
    for value in &args[2..] {
        match value {
            Value::Array(values) => {
                for value in values {
                    function_try!(append(value));
                }
            }
            value => function_try!(append(value)),
        }
    }
    Value::String(result)
}

fn concat_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    let mut result = String::new();
    for v in args {
        match v {
            Value::Array(arr) => {
                for item in arr {
                    if let Ok(s) = item.as_string() {
                        function_try!(push_bounded_text(&mut result, &s));
                    }
                }
            }
            v => {
                if let Ok(s) = v.as_string() {
                    function_try!(push_bounded_text(&mut result, &s));
                }
            }
        }
    }
    Value::String(result)
}

fn t_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.is_empty() {
        return Value::String(String::new());
    }
    match args[0] {
        Value::String(_) => args[0].clone(),
        _ => Value::String(String::new()),
    }
}

fn n_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.is_empty() {
        return Value::Number(0.0);
    }
    match &args[0] {
        Value::Number(n) => Value::Number(*n),
        Value::Boolean(b) => Value::Number(if *b { 1.0 } else { 0.0 }),
        Value::String(_) => Value::Number(0.0),
        Value::Error(e) => Value::Error(e.clone()),
        Value::Empty => Value::Number(0.0),
        Value::Array(arr) => {
            if let Some(first) = arr.first() {
                match first {
                    Value::Number(n) => Value::Number(*n),
                    _ => Value::Number(0.0),
                }
            } else {
                Value::Number(0.0)
            }
        }
    }
}

fn exact_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "EXACT requires 2 arguments".into(),
        ));
    }
    let a = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    let b = match args[1].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    Value::Boolean(a == b)
}

fn unicode_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "UNICODE requires 1 argument".into(),
        ));
    }
    match args[0].as_string() {
        Ok(s) => {
            if let Some(ch) = s.chars().next() {
                Value::Number(ch as u32 as f64)
            } else {
                Value::Number(0.0)
            }
        }
        Err(e) => Value::Error(e),
    }
}

fn textbefore_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "TEXTBEFORE requires 2 arguments".into(),
        ));
    }
    let text = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    let delim = match args[1].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    if let Some(pos) = text.find(&delim) {
        Value::String(text[..pos].to_string())
    } else {
        Value::String(String::new())
    }
}

fn textafter_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "TEXTAFTER requires 2 arguments".into(),
        ));
    }
    let text = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    let delim = match args[1].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    if let Some(pos) = text.find(&delim) {
        Value::String(text[pos + delim.len()..].to_string())
    } else {
        Value::String(String::new())
    }
}

// --- Date/time functions ---
// Excel epoch: Jan 1, 1900 = 1. We use a simplified model.

fn date_to_serial(year: i32, month: u32, day: u32) -> Result<f64, FormulaError> {
    // Days from 1900-01-01 (Excel epoch, day 1)
    let mut y = i64::from(year);
    let mut m = i64::from(month);
    if m < 1 {
        m = 1;
    }
    if m > 12 {
        y += (m - 1) / 12;
        m = ((m - 1) % 12) + 1;
    }
    if !(1900..=9999).contains(&y) {
        return Err(FormulaError::NumError(
            "date year must be between 1900 and 9999".into(),
        ));
    }
    let y = y as i32;
    // At most 8,099 iterations after validating the normalized year.
    let mut days: f64 = 0.0;
    for yr in 1900..y {
        days += if is_leap_year(yr) { 366.0 } else { 365.0 };
    }
    let month_days = [
        31,
        if is_leap_year(y) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    for &dim in month_days.iter().take(m as usize - 1) {
        days += dim as f64;
    }
    days += day as f64;
    if !(0.0..=MAX_DATE_SERIAL).contains(&days) {
        return Err(FormulaError::NumError("date exceeds year 9999".into()));
    }
    Ok(days)
}

fn is_leap_year(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

fn serial_to_date(serial: f64) -> Result<(i32, u32, u32), FormulaError> {
    if !serial.is_finite() || !(0.0..MAX_DATE_SERIAL + 1.0).contains(&serial) {
        return Err(FormulaError::NumError(
            "date serial must be between 0 and year 9999".into(),
        ));
    }
    let serial = serial.floor() as i64;
    let mut remaining = serial;
    let mut year = 1900;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining <= days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }
    let month_days = [
        31,
        if is_leap_year(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u32;
    for &dim in &month_days {
        if remaining <= dim {
            break;
        }
        remaining -= dim;
        month += 1;
    }
    Ok((year, month, remaining as u32))
}

fn date_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 3 {
        return Value::Error(FormulaError::InvalidArguments(
            "DATE requires 3 arguments".into(),
        ));
    }
    let y = match args[0].as_number() {
        Ok(n) => n as i32,
        Err(e) => return Value::Error(e),
    };
    let m = match args[1].as_number() {
        Ok(n) => n as u32,
        Err(e) => return Value::Error(e),
    };
    let d = match args[2].as_number() {
        Ok(n) => n as u32,
        Err(e) => return Value::Error(e),
    };
    Value::Number(function_try!(date_to_serial(y, m, d)))
}

fn time_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 3 {
        return Value::Error(FormulaError::InvalidArguments(
            "TIME requires 3 arguments".into(),
        ));
    }
    let h = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let m = match args[1].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let s = match args[2].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    Value::Number((h + m / 60.0 + s / 3600.0) / 24.0)
}

fn now_fn(_: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    Value::Number(days as f64 + 25569.0) // Unix epoch to Excel epoch
}

fn today_fn(_: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    Value::Number(days as f64 + 25569.0)
}

fn year_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "YEAR requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(serial) => {
            let (y, _, _) = function_try!(serial_to_date(serial));
            Value::Number(y as f64)
        }
        Err(e) => Value::Error(e),
    }
}
fn month_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "MONTH requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(serial) => {
            let (_, m, _) = function_try!(serial_to_date(serial));
            Value::Number(m as f64)
        }
        Err(e) => Value::Error(e),
    }
}
fn day_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "DAY requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(serial) => {
            let (_, _, d) = function_try!(serial_to_date(serial));
            Value::Number(d as f64)
        }
        Err(e) => Value::Error(e),
    }
}
fn hour_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "HOUR requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(serial) => {
            let frac = serial - serial.floor();
            Value::Number((frac * 24.0).floor())
        }
        Err(e) => Value::Error(e),
    }
}
fn minute_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "MINUTE requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(serial) => {
            let frac = serial - serial.floor();
            Value::Number(((frac * 24.0 - (frac * 24.0).floor()) * 60.0).floor())
        }
        Err(e) => Value::Error(e),
    }
}
fn second_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "SECOND requires 1 argument".into(),
        ));
    }
    match args[0].as_number() {
        Ok(serial) => {
            let frac = serial - serial.floor();
            let total_secs = frac * 86400.0;
            Value::Number(total_secs % 60.0)
        }
        Err(e) => Value::Error(e),
    }
}

fn weekday_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "WEEKDAY requires 1 argument".into(),
        ));
    }
    let serial = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let return_type = if args.len() >= 2 {
        match args[1].as_number() {
            Ok(n) => n as i32,
            Err(e) => return Value::Error(e),
        }
    } else {
        1
    };
    // Excel epoch day 1 = Jan 1, 1900 which was a Monday (actually Sunday in Excel's flawed calendar)
    // We use: day 1 = Sunday (matching Excel's convention)
    let day_of_week = ((serial as i64) % 7) as i32; // 0=Sunday for serial=7,14,...
    let day_of_week = if day_of_week == 0 { 7 } else { day_of_week }; // 1-7, Sunday=1
    let result = match return_type {
        1 => day_of_week, // 1=Sunday, 7=Saturday
        2 => {
            if day_of_week == 1 {
                7
            } else {
                day_of_week - 1
            }
        } // 1=Monday, 7=Sunday
        3 => {
            if day_of_week == 1 {
                6
            } else {
                day_of_week - 2
            }
        } // 0=Monday, 6=Sunday
        _ => day_of_week,
    };
    Value::Number(result as f64)
}

fn weeknum_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "WEEKNUM requires 1 argument".into(),
        ));
    }
    let serial = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let (year, _, _) = function_try!(serial_to_date(serial));
    let jan1_serial = function_try!(date_to_serial(year, 1, 1));
    let diff = serial - jan1_serial;
    Value::Number((diff / 7.0).ceil() + 1.0)
}

fn datevalue_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "DATEVALUE requires 1 argument".into(),
        ));
    }
    let s = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    // Try parsing "YYYY-MM-DD" or "MM/DD/YYYY"
    let parts: Vec<&str> = s.split(['-', '/']).collect();
    if parts.len() == 3 {
        if let (Ok(y), Ok(m), Ok(d)) = (
            parts[0].parse::<i32>(),
            parts[1].parse::<u32>(),
            parts[2].parse::<u32>(),
        ) {
            return Value::Number(function_try!(date_to_serial(y, m, d)));
        }
    }
    Value::Error(FormulaError::ValueError(
        "DATEVALUE: cannot parse date".into(),
    ))
}

fn timevalue_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "TIMEVALUE requires 1 argument".into(),
        ));
    }
    let s = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() >= 2 {
        if let (Ok(h), Ok(m)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
            let sec = if parts.len() >= 3 {
                parts[2].parse::<f64>().unwrap_or(0.0)
            } else {
                0.0
            };
            return Value::Number((h + m / 60.0 + sec / 3600.0) / 24.0);
        }
    }
    Value::Error(FormulaError::ValueError(
        "TIMEVALUE: cannot parse time".into(),
    ))
}

fn edate_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "EDATE requires 2 arguments".into(),
        ));
    }
    let serial = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let months = match args[1].as_number() {
        Ok(n) => n as i32,
        Err(e) => return Value::Error(e),
    };
    let (y, m, d) = function_try!(serial_to_date(serial));
    let total_months = match (y * 12 + m as i32 - 1).checked_add(months) {
        Some(total) => total,
        None => return Value::Error(FormulaError::NumError("month offset overflow".into())),
    };
    let new_year = total_months.div_euclid(12);
    let new_month = total_months.rem_euclid(12) as u32 + 1;
    let month_days = [
        31,
        if is_leap_year(new_year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let new_day = d.min(month_days[new_month as usize - 1]);
    Value::Number(function_try!(date_to_serial(new_year, new_month, new_day)))
}

fn eomonth_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "EOMONTH requires 2 arguments".into(),
        ));
    }
    let serial = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let months = match args[1].as_number() {
        Ok(n) => n as i32,
        Err(e) => return Value::Error(e),
    };
    let (y, m, _) = function_try!(serial_to_date(serial));
    let total_months = match (y * 12 + m as i32 - 1).checked_add(months) {
        Some(total) => total,
        None => return Value::Error(FormulaError::NumError("month offset overflow".into())),
    };
    let new_year = total_months.div_euclid(12);
    let new_month = total_months.rem_euclid(12) as u32 + 1;
    let month_days = [
        31,
        if is_leap_year(new_year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    Value::Number(function_try!(date_to_serial(
        new_year,
        new_month,
        month_days[new_month as usize - 1],
    )))
}

fn datedif_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 3 {
        return Value::Error(FormulaError::InvalidArguments(
            "DATEDIF requires 3 arguments".into(),
        ));
    }
    let start = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let end = match args[1].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let unit = match args[2].as_string() {
        Ok(s) => s.to_uppercase(),
        Err(e) => return Value::Error(e),
    };
    let (sy, sm, sd) = function_try!(serial_to_date(start));
    let (ey, em, ed) = function_try!(serial_to_date(end));
    match unit.as_str() {
        "D" => Value::Number(end - start),
        "M" => {
            let months = (ey - sy) * 12 + em as i32 - sm as i32;
            let adjusted = if ed < sd { months - 1 } else { months };
            Value::Number(adjusted as f64)
        }
        "Y" => {
            let mut years = ey - sy;
            if em < sm || (em == sm && ed < sd) {
                years -= 1;
            }
            Value::Number(years as f64)
        }
        "MD" => {
            let day_diff = ed as i32 - sd as i32;
            Value::Number(if day_diff < 0 {
                day_diff + 30
            } else {
                day_diff
            } as f64)
        }
        "YM" => {
            let months = em as i32 - sm as i32;
            Value::Number(if months < 0 { months + 12 } else { months } as f64)
        }
        "YD" => Value::Number((end - start).min(365.0)),
        _ => Value::Error(FormulaError::ValueError("DATEDIF: invalid unit".into())),
    }
}

fn days_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "DAYS requires 2 arguments".into(),
        ));
    }
    let end = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let start = match args[1].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    Value::Number(end - start)
}

fn isoweeknum_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "ISOWEEKNUM requires 1 argument".into(),
        ));
    }
    let serial = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let (year, _, _) = function_try!(serial_to_date(serial));
    let jan1_serial = function_try!(date_to_serial(year, 1, 1));
    let diff = serial - jan1_serial;
    Value::Number((diff / 7.0).ceil() + 1.0)
}

// --- Lookup functions ---

fn vlookup_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 3 {
        return Value::Error(FormulaError::InvalidArguments(
            "VLOOKUP requires 3 arguments".into(),
        ));
    }
    let target = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    let col_index = match args[1].as_number() {
        Ok(n) => n as usize,
        Err(e) => return Value::Error(e),
    };
    let exact = if args.len() >= 4 {
        match args[3].as_number() {
            Ok(n) => n == 0.0,
            Err(_) => false,
        }
    } else {
        true
    };
    if let Value::Array(arr) = &args[1] {
        // Not a proper table; return error
        let _ = arr;
    }
    // VLOOKUP with array values: args[1] should be a 2D range, but our evaluator passes arrays
    // For now, search through args[1] as array of rows
    if let Value::Array(table) = &args[1] {
        for row in table.chunks(col_index) {
            if let Some(first) = row.first() {
                if let Ok(val) = first.as_string() {
                    if exact && val == target {
                        return row.get(col_index - 1).cloned().unwrap_or(Value::Error(
                            FormulaError::NotAvailable("VLOOKUP: column out of range".into()),
                        ));
                    }
                }
            }
        }
    }
    Value::Error(FormulaError::NotAvailable(
        "VLOOKUP: value not found".into(),
    ))
}

fn hlookup_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 3 {
        return Value::Error(FormulaError::InvalidArguments(
            "HLOOKUP requires 3 arguments".into(),
        ));
    }
    Value::Error(FormulaError::NotAvailable(
        "HLOOKUP: requires range reference".into(),
    ))
}

fn index_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "INDEX requires 2 arguments".into(),
        ));
    }
    let row_num = match args[1].as_number() {
        Ok(n) => n as usize,
        Err(e) => return Value::Error(e),
    };
    if let Value::Array(arr) = &args[0] {
        if row_num > 0 && row_num <= arr.len() {
            return arr[row_num - 1].clone();
        }
    }
    Value::Error(FormulaError::NotAvailable("INDEX: out of range".into()))
}

fn match_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "MATCH requires 2 arguments".into(),
        ));
    }
    let target = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    let match_type = if args.len() >= 3 {
        match args[2].as_number() {
            Ok(n) => n as i32,
            Err(_) => 1,
        }
    } else {
        1
    };
    if let Value::Array(arr) = &args[1] {
        for (i, v) in arr.iter().enumerate() {
            if let Ok(s) = v.as_string() {
                if match_type == 0 && s == target {
                    return Value::Number((i + 1) as f64);
                }
            }
        }
    }
    Value::Error(FormulaError::NotAvailable("MATCH: not found".into()))
}

fn choose_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "CHOOSE requires at least 2 arguments".into(),
        ));
    }
    let index = match args[0].as_number() {
        Ok(n) => n as usize,
        Err(e) => return Value::Error(e),
    };
    if index == 0 || index >= args.len() {
        return Value::Error(FormulaError::NumError("CHOOSE: index out of range".into()));
    }
    args[index].clone()
}

fn row_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.is_empty() {
        return Value::Number(1.0);
    }
    // Without cell context, return 1
    Value::Number(1.0)
}

fn column_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Number(1.0);
    }
    Value::Number(1.0)
}

fn rows_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Number(1.0);
    }
    if let Value::Array(arr) = &args[0] {
        Value::Number(arr.len() as f64)
    } else {
        Value::Number(1.0)
    }
}

fn columns_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Number(1.0);
    }
    Value::Number(1.0)
}

fn address_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "ADDRESS requires 2 arguments".into(),
        ));
    }
    let row = match args[0].as_number() {
        Ok(n) => n as u32,
        Err(e) => return Value::Error(e),
    };
    let col = match args[1].as_number() {
        Ok(n) => n as u32,
        Err(e) => return Value::Error(e),
    };
    let col_str = col_to_letter(col);
    Value::String(format!("{}{}", col_str, row))
}

fn col_to_letter(col: u32) -> String {
    let mut result = String::new();
    let mut c = col;
    while c > 0 {
        c -= 1;
        result.insert(0, (b'A' + (c % 26) as u8) as char);
        c /= 26;
    }
    result
}

fn lookup_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "LOOKUP requires 2 arguments".into(),
        ));
    }
    let target = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    if let Value::Array(arr) = &args[1] {
        for v in arr {
            if let Ok(s) = v.as_string() {
                if s == target {
                    return v.clone();
                }
            }
        }
    }
    Value::Error(FormulaError::NotAvailable("LOOKUP: not found".into()))
}

// --- Financial functions ---

fn pmt_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.len() < 3 {
        return Value::Error(FormulaError::InvalidArguments(
            "PMT requires 3 arguments".into(),
        ));
    }
    let rate = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let nper = match args[1].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let pv = match args[2].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    if rate == 0.0 {
        return Value::Number(-pv / nper);
    }
    Value::Number(-pv * rate * (1.0 + rate).powf(nper) / ((1.0 + rate).powf(nper) - 1.0))
}

fn pv_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.len() < 3 {
        return Value::Error(FormulaError::InvalidArguments(
            "PV requires 3 arguments".into(),
        ));
    }
    let rate = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let nper = match args[1].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let pmt = match args[2].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    if rate == 0.0 {
        return Value::Number(-pmt * nper);
    }
    Value::Number(-pmt * (1.0 - (1.0 + rate).powf(-nper)) / rate)
}

fn fv_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.len() < 3 {
        return Value::Error(FormulaError::InvalidArguments(
            "FV requires 3 arguments".into(),
        ));
    }
    let rate = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let nper = match args[1].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let pmt = match args[2].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    if rate == 0.0 {
        return Value::Number(-pmt * nper);
    }
    Value::Number(-pmt * ((1.0 + rate).powf(nper) - 1.0) / rate)
}

fn npv_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "NPV requires arguments".into(),
        ));
    }
    let rate = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let cashflows = collect_numbers_or_error!(&args[1..]);
    let mut total = 0.0_f64;
    for (i, &cf) in cashflows.iter().enumerate() {
        total += cf / (1.0 + rate).powf((i + 1) as f64);
    }
    Value::Number(total)
}

fn rate_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 3 {
        return Value::Error(FormulaError::InvalidArguments(
            "RATE requires 3 arguments".into(),
        ));
    }
    let nper = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let pmt = match args[1].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let pv = match args[2].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    // Newton-Raphson approximation
    let mut rate = 0.1_f64;
    for _ in 0..100 {
        let f = pv * rate * (1.0 + rate).powf(nper) / ((1.0 + rate).powf(nper) - 1.0) + pmt;
        let fprime = pv * ((1.0 + rate).powf(nper - 1.0) * (nper * rate - 1.0) + 1.0)
            / ((1.0 + rate).powf(nper) - 1.0).powi(2);
        if fprime.abs() < 1e-12 {
            break;
        }
        let new_rate = rate - f / fprime;
        if (new_rate - rate).abs() < 1e-10 {
            break;
        }
        rate = new_rate;
    }
    Value::Number(rate)
}

fn nper_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 3 {
        return Value::Error(FormulaError::InvalidArguments(
            "NPER requires 3 arguments".into(),
        ));
    }
    let rate = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let pmt = match args[1].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let pv = match args[2].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    if rate == 0.0 {
        return Value::Number(-pv / pmt);
    }
    Value::Number((-pmt / (pmt + pv * rate)).ln() / (1.0 + rate).ln())
}

fn sln_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.len() < 3 {
        return Value::Error(FormulaError::InvalidArguments(
            "SLN requires 3 arguments".into(),
        ));
    }
    let cost = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let salvage = match args[1].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let life = match args[2].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    if life == 0.0 {
        return Value::Error(FormulaError::DivisionByZero);
    }
    Value::Number((cost - salvage) / life)
}

fn syd_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.len() < 4 {
        return Value::Error(FormulaError::InvalidArguments(
            "SYD requires 4 arguments".into(),
        ));
    }
    let cost = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let salvage = match args[1].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let life = match args[2].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let period = match args[3].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let sum = life * (life + 1.0) / 2.0;
    Value::Number((cost - salvage) * (life - period + 1.0) / sum)
}

fn ddb_fn(args: &[Value], _: &dyn CellProvider, _: &mut HashSet<(u32, u32)>, _: &[&Expr]) -> Value {
    if args.len() < 4 {
        return Value::Error(FormulaError::InvalidArguments(
            "DDB requires 4 arguments".into(),
        ));
    }
    let cost = match args[0].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let salvage = match args[1].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let life = match args[2].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let period = match args[3].as_number() {
        Ok(n) => n,
        Err(e) => return Value::Error(e),
    };
    let factor = if args.len() >= 5 {
        match args[4].as_number() {
            Ok(n) => n,
            Err(e) => return Value::Error(e),
        }
    } else {
        2.0
    };
    if !period.is_finite() || period < 0.0 || period > MAX_FUNCTION_ITERATIONS as f64 {
        return Value::Error(FormulaError::NumError(format!(
            "DDB period must be between 0 and {MAX_FUNCTION_ITERATIONS}"
        )));
    }
    let rate = factor / life;
    let mut book = cost;
    for _ in 1..period as i32 {
        book -= book * rate;
        if book < salvage {
            book = salvage;
            break;
        }
    }
    let dep = book * rate;
    Value::Number(if book - dep < salvage {
        book - salvage
    } else {
        dep
    })
}

// --- Engineering functions ---

fn bin2dec_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "BIN2DEC requires 1 argument".into(),
        ));
    }
    let s = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    match i64::from_str_radix(&s, 2) {
        Ok(n) => Value::Number(n as f64),
        Err(_) => Value::Error(FormulaError::NumError("BIN2DEC: invalid binary".into())),
    }
}

fn dec2bin_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "DEC2BIN requires 1 argument".into(),
        ));
    }
    let n = match args[0].as_number() {
        Ok(n) => n as i64,
        Err(e) => return Value::Error(e),
    };
    Value::String(format!("{:b}", n))
}

fn hex2dec_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "HEX2DEC requires 1 argument".into(),
        ));
    }
    let s = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    match i64::from_str_radix(&s, 16) {
        Ok(n) => Value::Number(n as f64),
        Err(_) => Value::Error(FormulaError::NumError("HEX2DEC: invalid hex".into())),
    }
}

fn dec2hex_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "DEC2HEX requires 1 argument".into(),
        ));
    }
    let n = match args[0].as_number() {
        Ok(n) => n as i64,
        Err(e) => return Value::Error(e),
    };
    Value::String(format!("{:X}", n))
}

fn oct2dec_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "OCT2DEC requires 1 argument".into(),
        ));
    }
    let s = match args[0].as_string() {
        Ok(s) => s,
        Err(e) => return Value::Error(e),
    };
    match i64::from_str_radix(&s, 8) {
        Ok(n) => Value::Number(n as f64),
        Err(_) => Value::Error(FormulaError::NumError("OCT2DEC: invalid octal".into())),
    }
}

fn dec2oct_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.is_empty() {
        return Value::Error(FormulaError::InvalidArguments(
            "DEC2OCT requires 1 argument".into(),
        ));
    }
    let n = match args[0].as_number() {
        Ok(n) => n as i64,
        Err(e) => return Value::Error(e),
    };
    Value::String(format!("{:o}", n))
}

fn bitand_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "BITAND requires 2 arguments".into(),
        ));
    }
    let a = match args[0].as_number() {
        Ok(n) => n as u64,
        Err(e) => return Value::Error(e),
    };
    let b = match args[1].as_number() {
        Ok(n) => n as u64,
        Err(e) => return Value::Error(e),
    };
    Value::Number((a & b) as f64)
}

fn bitor_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "BITOR requires 2 arguments".into(),
        ));
    }
    let a = match args[0].as_number() {
        Ok(n) => n as u64,
        Err(e) => return Value::Error(e),
    };
    let b = match args[1].as_number() {
        Ok(n) => n as u64,
        Err(e) => return Value::Error(e),
    };
    Value::Number((a | b) as f64)
}

fn bitxor_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "BITXOR requires 2 arguments".into(),
        ));
    }
    let a = match args[0].as_number() {
        Ok(n) => n as u64,
        Err(e) => return Value::Error(e),
    };
    let b = match args[1].as_number() {
        Ok(n) => n as u64,
        Err(e) => return Value::Error(e),
    };
    Value::Number((a ^ b) as f64)
}

fn bitlshift_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "BITLSHIFT requires 2 arguments".into(),
        ));
    }
    let a = match args[0].as_number() {
        Ok(n) => n as u64,
        Err(e) => return Value::Error(e),
    };
    let s = match args[1].as_number() {
        Ok(n) => n as u32,
        Err(e) => return Value::Error(e),
    };
    Value::Number((a << s.min(63)) as f64)
}

fn bitrshift_fn(
    args: &[Value],
    _: &dyn CellProvider,
    _: &mut HashSet<(u32, u32)>,
    _: &[&Expr],
) -> Value {
    if args.len() < 2 {
        return Value::Error(FormulaError::InvalidArguments(
            "BITRSHIFT requires 2 arguments".into(),
        ));
    }
    let a = match args[0].as_number() {
        Ok(n) => n as u64,
        Err(e) => return Value::Error(e),
    };
    let s = match args[1].as_number() {
        Ok(n) => n as u32,
        Err(e) => return Value::Error(e),
    };
    Value::Number((a >> s.min(63)) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluator::{Evaluator, SimpleProvider};
    use crate::parser::Parser;

    fn eval(formula: &str, provider: &dyn CellProvider) -> Value {
        let evaluator = Evaluator::new();
        let expr = Parser::parse_formula(formula).unwrap();
        evaluator.evaluate(&expr, provider)
    }

    #[test]
    fn integer_function_extremes_return_errors_without_panics() {
        let provider = SimpleProvider::new();
        for formula in [
            "GCD(-1e20,1)",
            "GCD(1e20,1)",
            "LCM(-1e20,1)",
            "LCM(1000000000000,1000000000001)",
            "RANDBETWEEN(-1e20,1e20)",
        ] {
            assert!(
                matches!(
                    eval(formula, &provider),
                    Value::Error(FormulaError::NumError(_))
                ),
                "{formula}"
            );
        }
        assert_eq!(eval("GCD(0,12,18)", &provider), Value::Number(6.0));
        assert_eq!(eval("LCM(0,12,18)", &provider), Value::Number(0.0));
        assert_eq!(eval("LCM(12,18)", &provider), Value::Number(36.0));
        assert_eq!(eval("RANDBETWEEN(2,2)", &provider), Value::Number(2.0));
        assert!(
            matches!(eval("RANDBETWEEN(-9000000000000000000,9000000000000000000)", &provider), Value::Number(n) if n.is_finite() && (-9e18..=9e18).contains(&n))
        );
    }

    #[test]
    fn oversized_text_functions_reject_before_allocating_output() {
        let provider = SimpleProvider::new();
        for formula in [
            "REPT(\"x\",1e12)",
            "REPT(\"xx\",9e18)",
            "REPT(\"é\",524289)",
            "SUBSTITUTE(REPT(\"x\",1024),\"x\",REPT(\"y\",2048))",
            "CONCAT(REPT(\"x\",1048576),\"y\")",
            "CONCATENATE(REPT(\"x\",1048576),\"y\")",
            "TEXTJOIN(\"--\",FALSE,REPT(\"x\",1048575),\"y\")",
            "REPLACE(REPT(\"x\",1048576),1,0,\"y\")",
        ] {
            assert!(
                matches!(
                    eval(formula, &provider),
                    Value::Error(FormulaError::ValueError(_))
                ),
                "{formula}"
            );
        }
    }

    #[test]
    fn text_function_boundaries_preserve_unicode_and_empty_results() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("LEN(REPT(\"é\",524288))", &provider),
            Value::Number(524288.0)
        );
        assert_eq!(
            eval("REPT(\"\",1e12)", &provider),
            Value::String(String::new())
        );
        assert_eq!(
            eval("REPT(\"x\",0)", &provider),
            Value::String(String::new())
        );
        assert_eq!(
            eval("REPT(\"x\",2.9)", &provider),
            Value::String("xx".into())
        );
        assert_eq!(
            eval("SUBSTITUTE(\"ééé\",\"é\",\"猫\",2)", &provider),
            Value::String("é猫é".into())
        );
        assert_eq!(
            eval("SUBSTITUTE(\"aaa\",\"a\",\"xx\",4)", &provider),
            Value::String("aaa".into())
        );
        assert_eq!(
            eval("REPLACE(\"a猫c\",2,1,\"é\")", &provider),
            Value::String("aéc".into())
        );
        assert_eq!(
            eval("MID(\"abc\",2,1e308)", &provider),
            Value::String("bc".into())
        );
        assert_eq!(
            eval("MID(\"abc\",1e308,1e308)", &provider),
            Value::String(String::new())
        );
        assert_eq!(
            eval("REPLACE(\"abc\",1e308,1e308,\"z\")", &provider),
            Value::String("abcz".into())
        );
    }

    #[test]
    fn numeric_work_limits_reject_huge_inputs() {
        let provider = SimpleProvider::new();
        for formula in [
            "FACT(1e12)",
            "FACTDOUBLE(1e12)",
            "COMBIN(1e12,5e11)",
            "PERMUT(1e12,1e12)",
            "MULTINOMIAL(1e12,1e12)",
            "COMBINA(1e12,1e12)",
            "PERMUTA(1,1e12)",
            "PERMUTATIONA(1,10001)",
            "DDB(100,0,1e12,1e12)",
            "FIXED(1,1e12)",
            "FIXED(1,1001)",
            "FACT(-1)",
            "COMBIN(-1,1)",
            "MULTINOMIAL(-1,2)",
            "PERMUTA(0,1)",
            "COMBINA(1e308,1)",
        ] {
            assert!(
                matches!(
                    eval(formula, &provider),
                    Value::Error(FormulaError::NumError(_))
                ),
                "{formula}"
            );
        }
    }

    #[test]
    fn factorial_and_fixed_normal_boundaries_remain_supported() {
        let provider = SimpleProvider::new();
        assert!(
            matches!(eval("FACT(170.9)", &provider), Value::Number(n) if n.is_finite() && n > 7e306)
        );
        assert!(matches!(
            eval("FACT(171)", &provider),
            Value::Error(FormulaError::NumError(_))
        ));
        assert!(matches!(eval("FACTDOUBLE(300)", &provider), Value::Number(n) if n.is_finite()));
        assert!(matches!(
            eval("FACTDOUBLE(301)", &provider),
            Value::Error(FormulaError::NumError(_))
        ));
        assert_eq!(eval("LEN(FIXED(1,1000))", &provider), Value::Number(1002.0));
        assert!(
            matches!(eval("DDB(100,0,1e12,10000)", &provider), Value::Number(n) if n.is_finite() && n > 0.0)
        );
    }

    #[test]
    fn combinatorial_fast_paths_and_repetition_counts_are_correct() {
        let provider = SimpleProvider::new();
        for (formula, expected) in [
            ("COMBIN(1e12,1e12)", 1.0),
            ("COMBIN(1e12,1)", 1e12),
            ("MULTINOMIAL(1e12)", 1.0),
            ("MULTINOMIAL(1e12,1)", 1e12 + 1.0),
            ("MULTINOMIAL(2,3,4)", 1260.0),
            ("COMBINA(1,1e12)", 1.0),
            ("COMBINA(4,3)", 20.0),
            ("PERMUTA(3,2)", 9.0),
            ("PERMUTATIONA(3,2)", 9.0),
            ("PERMUTATIONA(2,2)", 4.0),
            ("PERMUTATIONA(3.9,2.9)", 9.0),
            ("PERMUTATIONA(1,10000)", 1.0),
        ] {
            assert_eq!(
                eval(formula, &provider),
                Value::Number(expected),
                "{formula}"
            );
        }
    }

    #[test]
    fn date_resource_bounds_cover_all_conversion_entry_points() {
        let provider = SimpleProvider::new();
        for formula in [
            "DATE(1e12,1,1)",
            "DATE(9999,13,1)",
            "DATE(9999,12,32)",
            "DATE(2024,1e12,1)",
            "DATEVALUE(\"2147483647-1-1\")",
            "YEAR(1e12)",
            "MONTH(1e12)",
            "DAY(1e12)",
            "WEEKNUM(1e12)",
            "ISOWEEKNUM(1e12)",
            "EDATE(1e12,1)",
            "EOMONTH(1e12,1)",
            "EDATE(45000,1e12)",
            "EOMONTH(45000,1e12)",
            "DATEDIF(1,1e12,\"Y\")",
            "TEXT(1e308,\"yyyy-mm-dd\")",
        ] {
            assert!(
                matches!(
                    eval(formula, &provider),
                    Value::Error(FormulaError::NumError(_))
                ),
                "{formula}"
            );
        }
        assert_eq!(
            eval("DATE(9999,12,31)", &provider),
            Value::Number(MAX_DATE_SERIAL)
        );
        assert_eq!(
            eval("YEAR(DATE(9999,12,31))", &provider),
            Value::Number(9999.0)
        );
        assert_eq!(
            eval("DAY(DATE(9999,12,31))", &provider),
            Value::Number(31.0)
        );
        assert_eq!(
            eval("YEAR(DATE(1900,1,1))", &provider),
            Value::Number(1900.0)
        );
    }

    #[test]
    fn test_sum() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("SUM(1,2,3)", &provider), Value::Number(6.0));
    }

    #[test]
    fn test_average() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("AVERAGE(1,2,3)", &provider), Value::Number(2.0));
    }

    #[test]
    fn test_min_max() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("MIN(3,1,2)", &provider), Value::Number(1.0));
        assert_eq!(eval("MAX(3,1,2)", &provider), Value::Number(3.0));
    }

    #[test]
    fn test_abs() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("ABS(-5)", &provider), Value::Number(5.0));
    }

    #[test]
    fn test_round() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("ROUND(3.14159,2)", &provider),
            Value::Number(314.0 / 100.0)
        );
    }

    #[test]
    fn test_if_true() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("IF(1>0,\"yes\",\"no\")", &provider),
            Value::String("yes".into())
        );
    }

    #[test]
    fn test_if_false() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("IF(0>1,\"yes\",\"no\")", &provider),
            Value::String("no".into())
        );
    }

    #[test]
    fn test_and_or() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("AND(TRUE,TRUE)", &provider), Value::Boolean(true));
        assert_eq!(eval("AND(TRUE,FALSE)", &provider), Value::Boolean(false));
        assert_eq!(eval("OR(FALSE,TRUE)", &provider), Value::Boolean(true));
    }

    #[test]
    fn test_not() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("NOT(TRUE)", &provider), Value::Boolean(false));
    }

    #[test]
    fn test_len() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("LEN(\"hello\")", &provider), Value::Number(5.0));
    }

    #[test]
    fn test_upper_lower() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("UPPER(\"hello\")", &provider),
            Value::String("HELLO".into())
        );
        assert_eq!(
            eval("LOWER(\"HELLO\")", &provider),
            Value::String("hello".into())
        );
    }

    #[test]
    fn test_left_right_mid() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("LEFT(\"hello\",2)", &provider),
            Value::String("he".into())
        );
        assert_eq!(
            eval("RIGHT(\"hello\",2)", &provider),
            Value::String("lo".into())
        );
        assert_eq!(
            eval("MID(\"hello\",2,3)", &provider),
            Value::String("ell".into())
        );
    }

    #[test]
    fn test_concatenate() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("CONCATENATE(\"a\",\"b\",\"c\")", &provider),
            Value::String("abc".into())
        );
    }

    #[test]
    fn test_iferror() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("IFERROR(1/0,\"error\")", &provider),
            Value::String("error".into())
        );
        assert_eq!(eval("IFERROR(5,\"error\")", &provider), Value::Number(5.0));
    }

    #[test]
    fn test_sqrt() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("SQRT(9)", &provider), Value::Number(3.0));
    }

    #[test]
    fn test_power() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("POWER(2,3)", &provider), Value::Number(8.0));
    }

    #[test]
    fn test_mod() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("MOD(10,3)", &provider), Value::Number(1.0));
    }

    #[test]
    fn test_fact() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("FACT(5)", &provider), Value::Number(120.0));
    }

    #[test]
    fn test_isnumber() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("ISNUMBER(42)", &provider), Value::Boolean(true));
        assert_eq!(eval("ISNUMBER(\"hi\")", &provider), Value::Boolean(false));
    }

    #[test]
    fn test_pi() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("PI()", &provider), Value::Number(std::f64::consts::PI));
    }

    #[test]
    fn test_count() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("COUNT(1,2,\"three\",4)", &provider),
            Value::Number(3.0)
        );
    }

    #[test]
    fn test_trim() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("TRIM(\"  hello  world  \")", &provider),
            Value::String("hello world".into())
        );
    }

    #[test]
    fn test_proper() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("PROPER(\"hello world\")", &provider),
            Value::String("Hello World".into())
        );
    }

    #[test]
    fn test_substitute() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("SUBSTITUTE(\"hello\",\"l\",\"L\")", &provider),
            Value::String("heLLo".into())
        );
    }

    #[test]
    fn test_unknown_function() {
        let provider = SimpleProvider::new();
        let result = eval("FOO(1)", &provider);
        assert!(result.is_error());
    }

    #[test]
    fn test_roundup() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("ROUNDUP(3.1)", &provider), Value::Number(4.0));
        assert_eq!(eval("ROUNDUP(3.14,1)", &provider), Value::Number(3.2));
    }

    #[test]
    fn test_rounddown() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("ROUNDDOWN(3.9)", &provider), Value::Number(3.0));
        assert_eq!(eval("ROUNDDOWN(3.99,1)", &provider), Value::Number(3.9));
    }

    #[test]
    fn test_trunc() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("TRUNC(3.99)", &provider), Value::Number(3.0));
        assert_eq!(eval("TRUNC(3.99,1)", &provider), Value::Number(3.9));
    }

    #[test]
    fn test_gcd_lcm() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("GCD(12,18)", &provider), Value::Number(6.0));
        assert_eq!(eval("LCM(4,6)", &provider), Value::Number(12.0));
    }

    #[test]
    fn test_combin() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("COMBIN(5,2)", &provider), Value::Number(10.0));
    }

    #[test]
    fn test_permut() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("PERMUT(5,2)", &provider), Value::Number(20.0));
    }

    #[test]
    fn test_log() {
        let provider = SimpleProvider::new();
        let result = eval("LOG(100,10)", &provider);
        assert_eq!(result, Value::Number(2.0));
    }

    #[test]
    fn test_log2() {
        let provider = SimpleProvider::new();
        let result = eval("LOG2(8)", &provider);
        assert_eq!(result, Value::Number(3.0));
    }

    #[test]
    fn test_hyp_functions() {
        let provider = SimpleProvider::new();
        let sinh = eval("SINH(0)", &provider);
        assert_eq!(sinh, Value::Number(0.0));
        let cosh = eval("COSH(0)", &provider);
        assert_eq!(cosh, Value::Number(1.0));
    }

    #[test]
    fn test_median() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("MEDIAN(1,2,3)", &provider), Value::Number(2.0));
        assert_eq!(eval("MEDIAN(1,2,3,4)", &provider), Value::Number(2.5));
    }

    #[test]
    fn test_non_finite_formula_results_are_num_errors() {
        let provider = SimpleProvider::new();
        assert!(matches!(
            eval("POWER(-1,0.5)", &provider),
            Value::Error(FormulaError::NumError(_))
        ));
        assert!(matches!(
            eval("MEDIAN(POWER(-1,0.5),1)", &provider),
            Value::Error(FormulaError::NumError(_))
        ));
        assert!(matches!(
            eval("LARGE(POWER(-1,0.5),1)", &provider),
            Value::Error(FormulaError::NumError(_))
        ));
        assert!(matches!(
            eval("PERCENTILE(POWER(-1,0.5),0.5)", &provider),
            Value::Error(FormulaError::NumError(_))
        ));
    }

    #[test]
    fn test_stdev_var() {
        let provider = SimpleProvider::new();
        let result = eval("STDEV(1,2,3,4,5)", &provider);
        assert!((result.as_number().unwrap() - 1.5811).abs() < 0.01);
    }

    #[test]
    fn test_char_code() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("CHAR(65)", &provider), Value::String("A".into()));
        assert_eq!(eval("CODE(\"A\")", &provider), Value::Number(65.0));
    }

    #[test]
    fn test_concat() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("CONCAT(\"a\",\"b\",\"c\")", &provider),
            Value::String("abc".into())
        );
    }

    #[test]
    fn test_textjoin() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("TEXTJOIN(\",\",TRUE,\"a\",\"b\")", &provider),
            Value::String("a,b".into())
        );
    }

    #[test]
    fn test_exact() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("EXACT(\"abc\",\"abc\")", &provider),
            Value::Boolean(true)
        );
        assert_eq!(
            eval("EXACT(\"abc\",\"ABC\")", &provider),
            Value::Boolean(false)
        );
    }

    #[test]
    fn test_date() {
        let provider = SimpleProvider::new();
        let result = eval("DATE(2024,1,1)", &provider);
        assert!(matches!(result, Value::Number(_)));
    }

    #[test]
    fn test_year_month_day() {
        let provider = SimpleProvider::new();
        let _serial = eval("DATE(2024,3,15)", &provider);
        assert_eq!(
            eval("YEAR(DATE(2024,3,15))", &provider),
            Value::Number(2024.0)
        );
        assert_eq!(
            eval("MONTH(DATE(2024,3,15))", &provider),
            Value::Number(3.0)
        );
        assert_eq!(eval("DAY(DATE(2024,3,15))", &provider), Value::Number(15.0));
    }

    #[test]
    fn test_choose() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("CHOOSE(2,\"a\",\"b\",\"c\")", &provider),
            Value::String("b".into())
        );
    }

    #[test]
    fn test_address() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("ADDRESS(1,1)", &provider), Value::String("A1".into()));
        assert_eq!(
            eval("ADDRESS(3,27)", &provider),
            Value::String("AA3".into())
        );
    }

    #[test]
    fn test_pmt() {
        let provider = SimpleProvider::new();
        let result = eval("PMT(0.05/12,60,10000)", &provider);
        assert!(matches!(result, Value::Number(_)));
    }

    #[test]
    fn test_npv() {
        let provider = SimpleProvider::new();
        let result = eval("NPV(0.1,-100,30,40,50)", &provider);
        assert!(matches!(result, Value::Number(_)));
    }

    #[test]
    fn test_bin2dec_dec2bin() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("BIN2DEC(\"1010\")", &provider), Value::Number(10.0));
        assert_eq!(eval("DEC2BIN(10)", &provider), Value::String("1010".into()));
    }

    #[test]
    fn test_hex2dec_dec2hex() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("HEX2DEC(\"FF\")", &provider), Value::Number(255.0));
        assert_eq!(eval("DEC2HEX(255)", &provider), Value::String("FF".into()));
    }

    #[test]
    fn test_bitand_bitor() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("BITAND(12,10)", &provider), Value::Number(8.0));
        assert_eq!(eval("BITOR(12,10)", &provider), Value::Number(14.0));
    }

    #[test]
    fn test_t_n() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("T(\"hello\")", &provider),
            Value::String("hello".into())
        );
        assert_eq!(eval("T(42)", &provider), Value::String(String::new()));
        assert_eq!(eval("N(42)", &provider), Value::Number(42.0));
        assert_eq!(eval("N(\"hello\")", &provider), Value::Number(0.0));
    }

    #[test]
    fn test_unicode() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("UNICODE(\"A\")", &provider), Value::Number(65.0));
    }

    #[test]
    fn test_textbefore_textafter() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("TEXTBEFORE(\"hello,world\",\",\")", &provider),
            Value::String("hello".into())
        );
        assert_eq!(
            eval("TEXTAFTER(\"hello,world\",\",\")", &provider),
            Value::String("world".into())
        );
    }

    #[test]
    fn test_clean() {
        let provider = SimpleProvider::new();
        let result = eval("CLEAN(\"hello\\nworld\")", &provider);
        assert!(matches!(result, Value::String(_)));
    }

    #[test]
    fn test_fixed() {
        let provider = SimpleProvider::new();
        assert_eq!(
            eval("FIXED(3.14159,2)", &provider),
            Value::String("3.14".into())
        );
    }

    #[test]
    fn test_factdouble() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("FACTDOUBLE(6)", &provider), Value::Number(48.0));
    }

    #[test]
    fn test_delta() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("DELTA(5,5)", &provider), Value::Number(1.0));
        assert_eq!(eval("DELTA(5,3)", &provider), Value::Number(0.0));
    }

    #[test]
    fn test_sln() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("SLN(10000,1000,5)", &provider), Value::Number(1800.0));
    }

    #[test]
    fn test_sqrtpi() {
        let provider = SimpleProvider::new();
        let result = eval("SQRTPI(1)", &provider);
        assert!((result.as_number().unwrap() - std::f64::consts::PI.sqrt()).abs() < 0.001);
    }

    #[test]
    fn test_large_small() {
        let provider = SimpleProvider::new();
        let large = eval("LARGE(5,1)", &provider);
        assert_eq!(large, Value::Number(5.0));
        let small = eval("SMALL(5,1)", &provider);
        assert_eq!(small, Value::Number(5.0));
    }

    #[test]
    fn test_mround() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("MROUND(10,3)", &provider), Value::Number(9.0));
    }

    #[test]
    fn test_quotient() {
        let provider = SimpleProvider::new();
        assert_eq!(eval("QUOTIENT(10,3)", &provider), Value::Number(3.0));
    }
}
