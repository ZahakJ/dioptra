//! Evaluation: an expression is *compiled* against a table into a tree whose whole-column
//! parts (aggregates, series operations, slices) are already computed, then evaluated row
//! by row. A column formula over a million rows is one pass of a small tree per row.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use super::ast::{BinOp, ColRef, ColTarget, Expr, UnOp};
use super::units::{conversion, Conversion};
use crate::table::{Column, Table};
use crate::util::letters_index;

/// A formula value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A number.
    Number(f64),
    /// Text.
    Text(String),
    /// A boolean.
    Bool(bool),
    /// Missing (empty, excluded, or `NA`).
    Missing,
    /// A vector of values.
    Vector(Vec<Value>),
    /// An error, with a message for the cell.
    Error(String),
}

impl Value {
    fn err(m: impl Into<String>) -> Value {
        Value::Error(m.into())
    }

    /// The number in this value, if it is one (booleans count as 1 and 0).
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(v) => Some(*v),
            Value::Bool(b) => Some(f64::from(u8::from(*b))),
            _ => None,
        }
    }

    /// Truthiness: `Some(bool)` for numbers and booleans, `None` for missing.
    fn truth(&self) -> Result<Option<bool>, Value> {
        match self {
            Value::Bool(b) => Ok(Some(*b)),
            Value::Number(v) => Ok(Some(*v != 0.0)),
            Value::Missing => Ok(None),
            Value::Error(_) => Err(self.clone()),
            Value::Text(t) => Err(Value::err(format!("\"{t}\" is text, not true or false"))),
            Value::Vector(_) => Err(Value::err("a list of values is not true or false")),
        }
    }
}

fn num(x: f64) -> Value {
    if x.is_finite() {
        Value::Number(x)
    } else if x.is_nan() {
        Value::err("result is not a real number")
    } else {
        Value::err("result is too large")
    }
}

/// Element-wise functions.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Func {
    Abs,
    Sqrt,
    Exp,
    Ln,
    Log,
    Log10,
    Log2,
    Pow,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Sinh,
    Cosh,
    Tanh,
    Floor,
    Ceil,
    Round,
    Trunc,
    Sign,
    If,
    IsNa,
    Coalesce,
    Clamp,
    Pct,
    Normalize,
    Convert(Conversion),
}

/// A compiled node.
#[derive(Debug)]
enum Node<'a> {
    Const(Value),
    Col(&'a Column),
    RowVec(Vec<Value>),
    RowNum,
    Unary(UnOp, Box<Node<'a>>),
    Binary(BinOp, Box<Node<'a>>, Box<Node<'a>>),
    Call(Func, Vec<Node<'a>>),
}

impl Node<'_> {
    fn varying(&self) -> bool {
        match self {
            Node::Const(_) => false,
            Node::Col(_) | Node::RowVec(_) | Node::RowNum => true,
            Node::Unary(_, a) => a.varying(),
            Node::Binary(_, a, b) => a.varying() || b.varying(),
            Node::Call(_, args) => args.iter().any(Node::varying),
        }
    }
}

/// Read one cell as a formula value: excluded and empty cells are missing.
#[inline]
pub fn cell_value(col: &Column, row: usize) -> Value {
    let Some(&v) = col.values.get(row) else {
        return Value::Missing;
    };
    if !col.excluded.is_empty() && col.excluded.contains(&row) {
        return Value::Missing;
    }
    if !v.is_nan() {
        return Value::Number(v);
    }
    if !col.errors.is_empty() {
        if let Some(e) = col.errors.get(&row) {
            return Value::Error(e.clone());
        }
    }
    if let Some(e) = &col.formula_error {
        return Value::Error(e.clone());
    }
    if !col.text.is_empty() {
        if let Some(t) = col.text.get(&row) {
            return Value::Text(t.clone());
        }
    }
    Value::Missing
}

/// Resolve a column reference to a flat column index.
pub fn resolve(table: &Table, r: &ColRef) -> Result<usize, String> {
    let ds = match &r.target {
        ColTarget::X => {
            return if table.x.is_some() && r.sub.unwrap_or(1) == 1 {
                Ok(0)
            } else {
                Err("this table has no X column".into())
            };
        }
        ColTarget::Letter(l) => {
            let d = letters_index(l).ok_or_else(|| format!("'{l}' is not a column letter"))?;
            if d >= table.datasets.len() {
                return Err(format!(
                    "there is no data set {l}: this table has {} data set{}",
                    table.datasets.len(),
                    if table.datasets.len() == 1 { "" } else { "s" }
                ));
            }
            d
        }
        ColTarget::Name(n) => {
            if let Some(d) = table.datasets.iter().position(|d| d.name == *n) {
                d
            } else if let Some(d) = table
                .datasets
                .iter()
                .position(|d| d.name.eq_ignore_ascii_case(n))
            {
                d
            } else if table
                .x
                .as_ref()
                .is_some_and(|x| x.title.eq_ignore_ascii_case(n))
            {
                return Ok(0);
            } else {
                return Err(format!("no column named `{n}`"));
            }
        }
    };
    let sub = r.sub.unwrap_or(1);
    if sub == 0 {
        return Err("subcolumns are numbered from 1".into());
    }
    table.flat_index(ds, sub - 1).ok_or_else(|| {
        format!(
            "{r} does not exist: data set has {} subcolumns",
            table.datasets[ds].subcolumns.len()
        )
    })
}

/// The compiler/evaluator context.
pub struct Ctx<'a> {
    table: &'a Table,
    rows: usize,
}

/// A compiled formula ready to evaluate row by row.
pub struct Compiled<'a> {
    root: Node<'a>,
    rows: usize,
}

impl<'a> Compiled<'a> {
    /// Value at one row (column formulas, and cell formulas by implicit intersection).
    pub fn at(&self, row: usize) -> Value {
        eval(&self.root, row)
    }

    /// Whether the result changes from row to row.
    pub fn varying(&self) -> bool {
        self.root.varying()
    }

    /// If the result is a fixed vector, it (column formulas may produce one of row length).
    pub fn const_vector(&self) -> Option<&[Value]> {
        match &self.root {
            Node::Const(Value::Vector(v)) => Some(v),
            _ => None,
        }
    }

    /// Row count this was compiled against.
    pub fn rows(&self) -> usize {
        self.rows
    }
}

/// Compile an expression against a table.
pub fn compile<'a>(expr: &Expr, table: &'a Table) -> Result<Compiled<'a>, String> {
    let ctx = Ctx {
        table,
        rows: table.rows,
    };
    let root = ctx.compile(expr)?;
    Ok(Compiled {
        root,
        rows: table.rows,
    })
}

/// Evaluate an expression to a value (a whole column is a vector).
pub fn evaluate(expr: &Expr, table: &Table) -> Result<Value, String> {
    let ctx = Ctx {
        table,
        rows: table.rows,
    };
    ctx.full(expr)
}

/// The result of evaluating a column formula.
#[derive(Debug, Default)]
pub struct ColumnResult {
    /// Values (NaN where empty, text or error).
    pub values: Vec<f64>,
    /// Text results.
    pub text: BTreeMap<usize, String>,
    /// Per-cell errors.
    pub errors: BTreeMap<usize, String>,
}

impl ColumnResult {
    /// Store one value at a row.
    pub fn put(&mut self, row: usize, v: Value) {
        match v {
            Value::Number(x) => self.values[row] = x,
            Value::Bool(b) => self.values[row] = f64::from(u8::from(b)),
            Value::Missing => {}
            Value::Text(t) => {
                self.text.insert(row, t);
            }
            Value::Error(e) => {
                self.errors.insert(row, e);
            }
            Value::Vector(v) => {
                self.errors
                    .insert(row, format!("gives {} values; a cell holds one", v.len()));
            }
        }
    }
}

/// Evaluate a column formula for every row of the table.
pub fn eval_column(expr: &Expr, table: &Table) -> Result<ColumnResult, String> {
    let c = compile(expr, table)?;
    let rows = table.rows;
    let mut out = ColumnResult {
        values: vec![f64::NAN; rows],
        ..Default::default()
    };
    if let Some(v) = c.const_vector() {
        if v.len() != rows {
            return Err(format!(
                "the formula gives {} values but the table has {rows} rows",
                v.len()
            ));
        }
        for (i, x) in v.iter().enumerate() {
            out.put(i, x.clone());
        }
        return Ok(out);
    }
    if !c.varying() {
        let v = c.at(0);
        for i in 0..rows {
            out.put(i, v.clone());
        }
        return Ok(out);
    }
    for i in 0..rows {
        // Fast path for plain numbers avoids the match in `put`.
        match eval(&c.root, i) {
            Value::Number(x) => out.values[i] = x,
            v => out.put(i, v),
        }
    }
    Ok(out)
}

/// Evaluate a cell formula at a row.
pub fn eval_cell(expr: &Expr, table: &Table, row: usize) -> Value {
    match compile(expr, table) {
        Ok(c) => match c.const_vector() {
            Some(v) => Value::err(format!("gives {} values; a cell holds one", v.len())),
            None => c.at(row),
        },
        Err(e) => Value::Error(e),
    }
}

// ---------------------------------------------------------------------------
// Row evaluation
// ---------------------------------------------------------------------------

fn eval(node: &Node<'_>, row: usize) -> Value {
    match node {
        Node::Const(v) => v.clone(),
        Node::Col(c) => cell_value(c, row),
        Node::RowVec(v) => v.get(row).cloned().unwrap_or(Value::Missing),
        Node::RowNum => Value::Number((row + 1) as f64),
        Node::Unary(op, a) => unary(*op, eval(a, row)),
        Node::Binary(BinOp::And, a, b) => logic(true, eval(a, row), || eval(b, row)),
        Node::Binary(BinOp::Or, a, b) => logic(false, eval(a, row), || eval(b, row)),
        Node::Binary(op, a, b) => binary(*op, eval(a, row), eval(b, row)),
        Node::Call(f, args) => {
            let vals: Vec<Value> = args.iter().map(|a| eval(a, row)).collect();
            apply(*f, vals)
        }
    }
}

fn logic(is_and: bool, a: Value, b: impl FnOnce() -> Value) -> Value {
    // Short-circuit on a scalar left side that decides the result.
    if !matches!(a, Value::Vector(_)) && matches!(a.truth(), Ok(Some(t)) if t != is_and) {
        return Value::Bool(!is_and);
    }
    logic_strict(is_and, a, b())
}

fn logic_strict(is_and: bool, a: Value, b: Value) -> Value {
    if matches!(a, Value::Vector(_)) || matches!(b, Value::Vector(_)) {
        return broadcast2(a, b, &|x, y| logic_strict(is_and, x, y));
    }
    let ta = match a.truth() {
        Ok(t) => t,
        Err(e) => return e,
    };
    if ta == Some(!is_and) {
        return Value::Bool(!is_and);
    }
    let tb = match b.truth() {
        Ok(t) => t,
        Err(e) => return e,
    };
    match (ta, tb) {
        (_, Some(x)) if x == !is_and => Value::Bool(!is_and),
        (Some(_), Some(x)) => Value::Bool(x),
        _ => Value::Missing,
    }
}

fn broadcast1(a: Value, f: &dyn Fn(Value) -> Value) -> Value {
    match a {
        Value::Vector(v) => Value::Vector(v.into_iter().map(|x| broadcast1(x, f)).collect()),
        x => f(x),
    }
}

fn broadcast2(a: Value, b: Value, f: &dyn Fn(Value, Value) -> Value) -> Value {
    match (a, b) {
        (Value::Vector(x), Value::Vector(y)) => {
            if x.len() != y.len() {
                return Value::err(format!(
                    "lists of different lengths ({} and {})",
                    x.len(),
                    y.len()
                ));
            }
            Value::Vector(
                x.into_iter()
                    .zip(y)
                    .map(|(p, q)| broadcast2(p, q, f))
                    .collect(),
            )
        }
        (Value::Vector(x), s) => {
            Value::Vector(x.into_iter().map(|p| broadcast2(p, s.clone(), f)).collect())
        }
        (s, Value::Vector(y)) => {
            Value::Vector(y.into_iter().map(|q| broadcast2(s.clone(), q, f)).collect())
        }
        (p, q) => f(p, q),
    }
}

fn broadcast_n(args: Vec<Value>, f: &dyn Fn(Vec<Value>) -> Value) -> Value {
    let len = args.iter().find_map(|a| {
        if let Value::Vector(v) = a {
            Some(v.len())
        } else {
            None
        }
    });
    let Some(len) = len else { return f(args) };
    for a in &args {
        if let Value::Vector(v) = a {
            if v.len() != len {
                return Value::err(format!(
                    "lists of different lengths ({} and {})",
                    len,
                    v.len()
                ));
            }
        }
    }
    Value::Vector(
        (0..len)
            .map(|i| {
                let row: Vec<Value> = args
                    .iter()
                    .map(|a| {
                        if let Value::Vector(v) = a {
                            v[i].clone()
                        } else {
                            a.clone()
                        }
                    })
                    .collect();
                broadcast_n(row, f)
            })
            .collect(),
    )
}

fn unary(op: UnOp, a: Value) -> Value {
    match a {
        Value::Vector(_) => broadcast1(a, &|x| unary(op, x)),
        Value::Error(_) | Value::Missing => a,
        _ => match op {
            UnOp::Neg | UnOp::Plus => match a.as_number() {
                Some(x) => Value::Number(if op == UnOp::Neg { -x } else { x }),
                None => text_arith(&a),
            },
            UnOp::Not => match a.truth() {
                Ok(Some(t)) => Value::Bool(!t),
                Ok(None) => Value::Missing,
                Err(e) => e,
            },
        },
    }
}

fn text_arith(v: &Value) -> Value {
    match v {
        Value::Text(t) => Value::err(format!("\"{t}\" is text, not a number")),
        Value::Error(_) => v.clone(),
        _ => Value::err("not a number"),
    }
}

#[inline]
fn binary(op: BinOp, a: Value, b: Value) -> Value {
    // Fast path: two numbers.
    if let (Value::Number(x), Value::Number(y)) = (&a, &b) {
        return arith(op, *x, *y);
    }
    if matches!(a, Value::Vector(_)) || matches!(b, Value::Vector(_)) {
        return broadcast2(a, b, &|x, y| binary(op, x, y));
    }
    if let Value::Error(_) = a {
        return a;
    }
    if let Value::Error(_) = b {
        return b;
    }
    if matches!(a, Value::Missing) || matches!(b, Value::Missing) {
        return Value::Missing;
    }
    let cmp = matches!(
        op,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
    );
    if cmp {
        let ord = match (&a, &b) {
            (Value::Text(x), Value::Text(y)) => Some(x.cmp(y)),
            _ => match (a.as_number(), b.as_number()) {
                (Some(x), Some(y)) => x.partial_cmp(&y),
                _ => None,
            },
        };
        return match (op, ord) {
            (BinOp::Eq, o) => Value::Bool(o == Some(Ordering::Equal)),
            (BinOp::Ne, o) => Value::Bool(o != Some(Ordering::Equal)),
            (_, None) => Value::err("cannot compare text with a number"),
            (BinOp::Lt, Some(o)) => Value::Bool(o == Ordering::Less),
            (BinOp::Le, Some(o)) => Value::Bool(o != Ordering::Greater),
            (BinOp::Gt, Some(o)) => Value::Bool(o == Ordering::Greater),
            (_, Some(o)) => Value::Bool(o != Ordering::Less),
        };
    }
    match (a.as_number(), b.as_number()) {
        (Some(x), Some(y)) => arith(op, x, y),
        (None, _) => text_arith(&a),
        (_, None) => text_arith(&b),
    }
}

#[inline]
fn arith(op: BinOp, x: f64, y: f64) -> Value {
    match op {
        BinOp::Add => num(x + y),
        BinOp::Sub => num(x - y),
        BinOp::Mul => num(x * y),
        BinOp::Div => {
            if y == 0.0 {
                Value::err("division by zero")
            } else {
                num(x / y)
            }
        }
        BinOp::Pow => num(x.powf(y)),
        BinOp::Mod => {
            if y == 0.0 {
                Value::err("modulo by zero")
            } else {
                num(x - y * (x / y).floor())
            }
        }
        BinOp::Eq => Value::Bool(x == y),
        BinOp::Ne => Value::Bool(x != y),
        BinOp::Lt => Value::Bool(x < y),
        BinOp::Le => Value::Bool(x <= y),
        BinOp::Gt => Value::Bool(x > y),
        BinOp::Ge => Value::Bool(x >= y),
        BinOp::And | BinOp::Or => {
            logic_strict(op == BinOp::And, Value::Number(x), Value::Number(y))
        }
    }
}

fn math1(a: &Value, name: &str, f: impl Fn(f64) -> Result<f64, &'static str>) -> Value {
    match a {
        Value::Missing | Value::Error(_) => a.clone(),
        _ => match a.as_number() {
            Some(x) => match f(x) {
                Ok(y) => num(y),
                Err(m) => Value::err(format!("{name}: {m}")),
            },
            None => text_arith(a),
        },
    }
}

fn first_bad(args: &[Value]) -> Option<Value> {
    args.iter()
        .find(|a| matches!(a, Value::Error(_) | Value::Missing))
        .cloned()
}

fn nums(args: &[Value]) -> Result<Vec<f64>, Value> {
    if let Some(b) = first_bad(args) {
        return Err(b);
    }
    args.iter()
        .map(|a| a.as_number().ok_or_else(|| text_arith(a)))
        .collect()
}

fn apply(f: Func, args: Vec<Value>) -> Value {
    if args.iter().any(|a| matches!(a, Value::Vector(_))) {
        return broadcast_n(args, &|row| apply(f, row));
    }
    let a0 = args.first().cloned().unwrap_or(Value::Missing);
    let ok = |x: f64| -> Result<f64, &'static str> { Ok(x) };
    match f {
        Func::Abs => math1(&a0, "abs", |x| ok(x.abs())),
        Func::Sqrt => math1(&a0, "sqrt", |x| {
            if x < 0.0 {
                Err("negative number")
            } else {
                Ok(x.sqrt())
            }
        }),
        Func::Exp => math1(&a0, "exp", |x| ok(x.exp())),
        Func::Ln => math1(&a0, "ln", log_of(f64::ln)),
        Func::Log10 => math1(&a0, "log10", log_of(f64::log10)),
        Func::Log2 => math1(&a0, "log2", log_of(f64::log2)),
        Func::Log => {
            if args.len() == 2 {
                match nums(&args) {
                    Ok(v) if v[0] > 0.0 && v[1] > 0.0 && v[1] != 1.0 => num(v[0].ln() / v[1].ln()),
                    Ok(_) => {
                        Value::err("log: needs a positive number and a positive base other than 1")
                    }
                    Err(e) => e,
                }
            } else {
                math1(&a0, "log", log_of(f64::ln))
            }
        }
        Func::Pow => match nums(&args) {
            Ok(v) => num(v[0].powf(v[1])),
            Err(e) => e,
        },
        Func::Sin => math1(&a0, "sin", |x| ok(x.sin())),
        Func::Cos => math1(&a0, "cos", |x| ok(x.cos())),
        Func::Tan => math1(&a0, "tan", |x| ok(x.tan())),
        Func::Asin => math1(&a0, "asin", |x| {
            if x.abs() > 1.0 {
                Err("outside -1 to 1")
            } else {
                Ok(x.asin())
            }
        }),
        Func::Acos => math1(&a0, "acos", |x| {
            if x.abs() > 1.0 {
                Err("outside -1 to 1")
            } else {
                Ok(x.acos())
            }
        }),
        Func::Atan => math1(&a0, "atan", |x| ok(x.atan())),
        Func::Sinh => math1(&a0, "sinh", |x| ok(x.sinh())),
        Func::Cosh => math1(&a0, "cosh", |x| ok(x.cosh())),
        Func::Tanh => math1(&a0, "tanh", |x| ok(x.tanh())),
        Func::Floor => math1(&a0, "floor", |x| ok(x.floor())),
        Func::Ceil => math1(&a0, "ceil", |x| ok(x.ceil())),
        Func::Trunc => math1(&a0, "trunc", |x| ok(x.trunc())),
        Func::Sign => math1(&a0, "sign", |x| {
            ok(if x > 0.0 {
                1.0
            } else if x < 0.0 {
                -1.0
            } else {
                0.0
            })
        }),
        Func::Round => {
            let n = if args.len() > 1 {
                match nums(&args[1..]) {
                    Ok(v) => v[0] as i32,
                    Err(e) => return e,
                }
            } else {
                0
            };
            math1(&a0, "round", move |x| Ok(round_half_away(x, n)))
        }
        Func::If => match a0.truth() {
            Ok(Some(true)) => args.get(1).cloned().unwrap_or(Value::Missing),
            Ok(Some(false)) => args.get(2).cloned().unwrap_or(Value::Missing),
            Ok(None) => Value::Missing,
            Err(e) => e,
        },
        Func::IsNa => Value::Bool(matches!(a0, Value::Missing)),
        Func::Coalesce => args
            .into_iter()
            .find(|a| !matches!(a, Value::Missing))
            .unwrap_or(Value::Missing),
        Func::Clamp => match nums(&args) {
            Ok(v) => {
                if v[1] > v[2] {
                    Value::err("clamp: lower limit is above upper limit")
                } else {
                    num(v[0].clamp(v[1], v[2]))
                }
            }
            Err(e) => e,
        },
        Func::Pct => match nums(&args) {
            Ok(v) if v[1] == 0.0 => Value::err("pct: reference is zero"),
            Ok(v) => num(100.0 * v[0] / v[1]),
            Err(e) => e,
        },
        Func::Normalize => match nums(&args) {
            Ok(v) if v[2] == v[1] => Value::err("normalize: low and high are equal"),
            Ok(v) => num((v[0] - v[1]) / (v[2] - v[1])),
            Err(e) => e,
        },
        Func::Convert(c) => math1(&a0, "convert", |x| ok(x * c.factor + c.offset)),
    }
}

/// Round to `n` decimals, half away from zero, treating values within floating-point noise
/// of a tie as ties (so 2.675 rounds to 2.68 as a spreadsheet does).
pub fn round_half_away(x: f64, n: i32) -> f64 {
    let p = 10f64.powi(n);
    let s = x * p;
    let t = s.trunc();
    let frac = (s - t).abs();
    let r = if (frac - 0.5).abs() <= 1e-9 * s.abs().max(1.0) {
        t + s.signum()
    } else {
        s.round()
    };
    r / p
}

fn log_of(f: fn(f64) -> f64) -> impl Fn(f64) -> Result<f64, &'static str> {
    move |x| {
        if x <= 0.0 {
            Err("zero or negative number")
        } else {
            Ok(f(x))
        }
    }
}

// ---------------------------------------------------------------------------
// Aggregates and series
// ---------------------------------------------------------------------------

/// Flatten values into usable numbers: missing and text are skipped; an error aborts.
fn collect_numbers(v: &Value, out: &mut Vec<f64>) -> Result<(), String> {
    match v {
        Value::Number(x) => out.push(*x),
        Value::Bool(b) => out.push(f64::from(u8::from(*b))),
        Value::Vector(items) => {
            for i in items {
                collect_numbers(i, out)?;
            }
        }
        Value::Error(e) => return Err(e.clone()),
        Value::Missing | Value::Text(_) => {}
    }
    Ok(())
}

/// Sample mean.
pub fn mean(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len() as f64
}

/// Sample variance (n - 1), two-pass.
pub fn variance(x: &[f64]) -> f64 {
    let m = mean(x);
    x.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / (x.len() as f64 - 1.0)
}

/// Quantile by linear interpolation between order statistics (Hyndman–Fan type 7, as
/// spreadsheets' PERCENTILE.INC). `sorted` must be sorted ascending.
pub fn quantile7(sorted: &[f64], q: f64) -> f64 {
    let n = sorted.len();
    if n == 1 {
        return sorted[0];
    }
    let h = (n - 1) as f64 * q;
    let lo = h.floor() as usize;
    let hi = (lo + 1).min(n - 1);
    sorted[lo] + (h - lo as f64) * (sorted[hi] - sorted[lo])
}

fn sorted(mut x: Vec<f64>) -> Vec<f64> {
    x.sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
    x
}

fn aggregate(name: &str, x: Vec<f64>, extra: Option<f64>) -> Result<Value, String> {
    let n = x.len();
    let need = |k: usize| -> Result<(), String> {
        if n < k {
            Err(format!(
                "{name} needs at least {k} value{} (has {n})",
                if k == 1 { "" } else { "s" }
            ))
        } else {
            Ok(())
        }
    };
    let v = match name {
        "count" => n as f64,
        "sum" => x.iter().sum(),
        "mean" => {
            need(1)?;
            mean(&x)
        }
        "median" => {
            need(1)?;
            quantile7(&sorted(x), 0.5)
        }
        "quantile" => {
            need(1)?;
            let q = extra.expect("q");
            if !(0.0..=1.0).contains(&q) {
                return Err("quantile: q must be between 0 and 1".into());
            }
            quantile7(&sorted(x), q)
        }
        "var" => {
            need(2)?;
            variance(&x)
        }
        "sd" => {
            need(2)?;
            variance(&x).sqrt()
        }
        "sem" => {
            need(2)?;
            (variance(&x) / n as f64).sqrt()
        }
        "cv" => {
            need(2)?;
            let m = mean(&x);
            if m == 0.0 {
                return Err("cv: the mean is zero".into());
            }
            100.0 * variance(&x).sqrt() / m
        }
        "min" => {
            need(1)?;
            x.iter().copied().fold(f64::INFINITY, f64::min)
        }
        "max" => {
            need(1)?;
            x.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        }
        "range" => {
            need(1)?;
            x.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                - x.iter().copied().fold(f64::INFINITY, f64::min)
        }
        "geomean" => {
            need(1)?;
            if x.iter().any(|v| *v <= 0.0) {
                return Err("geomean: all values must be positive".into());
            }
            (x.iter().map(|v| v.ln()).sum::<f64>() / n as f64).exp()
        }
        "first" => {
            need(1)?;
            x[0]
        }
        "last" => {
            need(1)?;
            x[n - 1]
        }
        _ => unreachable!("aggregate {name}"),
    };
    Ok(num(v))
}

const AGGREGATES: [&str; 15] = [
    "mean", "median", "sd", "sem", "var", "sum", "count", "min", "max", "range", "quantile",
    "geomean", "cv", "first", "last",
];
const SERIES: [&str; 8] = [
    "lag",
    "lead",
    "diff",
    "cumsum",
    "rank",
    "zscore",
    "baseline",
    "normalize",
];

/// Names of every function, for the formula bar's completion list.
pub fn function_names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = vec![
        "abs", "sqrt", "exp", "ln", "log", "log10", "log2", "pow", "sin", "cos", "tan", "asin",
        "acos", "atan", "sinh", "cosh", "tanh", "floor", "ceil", "round", "trunc", "sign", "if",
        "isna", "na", "coalesce", "clamp", "pct", "convert", "row", "seq", "pi",
    ];
    v.extend(AGGREGATES);
    v.extend(SERIES);
    v.sort_unstable();
    v.dedup();
    v
}

fn ranks(x: &[Value]) -> Vec<Value> {
    let mut idx: Vec<(usize, f64)> = x
        .iter()
        .enumerate()
        .filter_map(|(i, v)| v.as_number().map(|n| (i, n)))
        .collect();
    idx.sort_by(|a, b| a.1.partial_cmp(&b.1).expect("finite"));
    let mut out = vec![Value::Missing; x.len()];
    let mut i = 0;
    while i < idx.len() {
        let mut j = i;
        while j + 1 < idx.len() && idx[j + 1].1 == idx[i].1 {
            j += 1;
        }
        let r = (i + j) as f64 / 2.0 + 1.0;
        for k in &idx[i..=j] {
            out[k.0] = Value::Number(r);
        }
        i = j + 1;
    }
    for (o, v) in out.iter_mut().zip(x) {
        if matches!(v, Value::Error(_)) {
            *o = v.clone();
        }
    }
    out
}

impl<'a> Ctx<'a> {
    fn col(&self, r: &ColRef) -> Result<&'a Column, String> {
        let flat = resolve(self.table, r)?;
        self.table
            .col(flat)
            .ok_or_else(|| format!("{r} does not exist"))
    }

    /// Evaluate fully: a varying node becomes a row-length vector.
    fn full(&self, e: &Expr) -> Result<Value, String> {
        let n = self.compile(e)?;
        Ok(self.materialise(n))
    }

    fn materialise(&self, n: Node<'_>) -> Value {
        match n {
            Node::Const(v) => v,
            Node::RowVec(v) => Value::Vector(v),
            Node::Col(c) => Value::Vector((0..self.rows).map(|i| cell_value(c, i)).collect()),
            n => Value::Vector((0..self.rows).map(|i| eval(&n, i)).collect()),
        }
    }

    /// Numbers of an aggregate argument (fast path for a plain column).
    fn numbers(&self, e: &Expr, out: &mut Vec<f64>) -> Result<(), String> {
        if let Expr::Column(r) = e {
            let c = self.col(r)?;
            if let Some(err) = &c.formula_error {
                return Err(err.clone());
            }
            if let Some((_, err)) = c.errors.iter().next() {
                return Err(err.clone());
            }
            out.extend(c.usable());
            return Ok(());
        }
        let v = self.full(e)?;
        collect_numbers(&v, out)
    }

    /// The vector of an argument (series operations).
    fn vector(&self, e: &Expr) -> Result<Vec<Value>, String> {
        match self.full(e)? {
            Value::Vector(v) => Ok(v),
            s => Ok(vec![s; self.rows]),
        }
    }

    fn scalar(&self, e: &Expr, what: &str) -> Result<f64, String> {
        let n = self.compile(e)?;
        if n.varying() {
            return Err(format!("{what} must be a single number, not a column"));
        }
        match self.materialise(n) {
            Value::Number(x) => Ok(x),
            Value::Bool(b) => Ok(f64::from(u8::from(b))),
            Value::Error(m) => Err(m),
            other => Err(format!("{what} must be a number, not {}", describe(&other))),
        }
    }

    fn text_arg(&self, e: &Expr, what: &str) -> Result<String, String> {
        match e {
            Expr::Str(s) => Ok(s.clone()),
            _ => Err(format!("{what} must be text in quotes, like \"mg/dL\"")),
        }
    }

    /// Turn a whole-column vector into a node usable alongside varying siblings.
    fn as_row_node(&self, v: Vec<Value>) -> Node<'a> {
        if v.len() == self.rows {
            Node::RowVec(v)
        } else {
            Node::Const(Value::Vector(v))
        }
    }

    /// When a node is combined element-wise with a varying sibling, a fixed vector must
    /// be the length of the table.
    fn align(&self, n: Node<'a>, varying: bool) -> Result<Node<'a>, String> {
        if !varying {
            return Ok(n);
        }
        match n {
            Node::Const(Value::Vector(v)) => {
                if v.len() == self.rows {
                    Ok(Node::RowVec(v))
                } else {
                    Err(format!(
                        "a list of {} values cannot be combined with columns of {} rows",
                        v.len(),
                        self.rows
                    ))
                }
            }
            n => Ok(n),
        }
    }

    fn fold(&self, n: Node<'a>) -> Node<'a> {
        if n.varying() || matches!(n, Node::Const(_)) {
            n
        } else {
            Node::Const(eval(&n, 0))
        }
    }

    fn compile(&self, e: &Expr) -> Result<Node<'a>, String> {
        Ok(match e {
            Expr::Number(v) => Node::Const(Value::Number(*v)),
            Expr::Str(s) => Node::Const(Value::Text(s.clone())),
            Expr::Bool(b) => Node::Const(Value::Bool(*b)),
            Expr::Missing => Node::Const(Value::Missing),
            Expr::Column(r) => Node::Col(self.col(r)?),
            Expr::Cell(r, row) => {
                let c = self.col(r)?;
                Node::Const(cell_value(c, row - 1))
            }
            Expr::Vector(items) => {
                let mut out = Vec::with_capacity(items.len());
                for i in items {
                    match self.full(i)? {
                        Value::Vector(v) => out.extend(v),
                        s => out.push(s),
                    }
                }
                Node::Const(Value::Vector(out))
            }
            Expr::Index(b, i) => {
                let i = self.scalar(i, "an index")?;
                let v = self.vector_of(b)?;
                Node::Const(index(&v, i)?)
            }
            Expr::Slice(b, lo, hi) => {
                let v = self.vector_of(b)?;
                let lo = match lo {
                    Some(e) => self.scalar(e, "a slice start")?,
                    None => 1.0,
                };
                let hi = match hi {
                    Some(e) => self.scalar(e, "a slice end")?,
                    None => v.len() as f64,
                };
                if lo < 1.0 || hi < lo - 1.0 || lo.fract() != 0.0 || hi.fract() != 0.0 {
                    return Err(format!("bad slice [{lo}:{hi}]: rows are numbered from 1"));
                }
                let (lo, hi) = (lo as usize - 1, (hi as usize).min(v.len()));
                Node::Const(Value::Vector(v.get(lo..hi.max(lo)).unwrap_or(&[]).to_vec()))
            }
            Expr::Unary(op, a) => self.fold(Node::Unary(*op, Box::new(self.compile(a)?))),
            Expr::Binary(op, a, b) => {
                let (a, b) = (self.compile(a)?, self.compile(b)?);
                let v = a.varying() || b.varying();
                let (a, b) = (self.align(a, v)?, self.align(b, v)?);
                self.fold(Node::Binary(*op, Box::new(a), Box::new(b)))
            }
            Expr::Call(name, args) => self.call(name, args)?,
        })
    }

    fn vector_of(&self, e: &Expr) -> Result<Vec<Value>, String> {
        match self.full(e)? {
            Value::Vector(v) => Ok(v),
            s => Ok(vec![s]),
        }
    }

    fn arity(&self, name: &str, args: &[Expr], min: usize, max: usize) -> Result<(), String> {
        if args.len() < min || args.len() > max {
            let want = if min == max {
                format!("{min}")
            } else if max == usize::MAX {
                format!("at least {min}")
            } else {
                format!("{min} to {max}")
            };
            return Err(format!(
                "{name}() takes {want} argument{}, got {}",
                if want == "1" { "" } else { "s" },
                args.len()
            ));
        }
        Ok(())
    }

    fn call(&self, name: &str, args: &[Expr]) -> Result<Node<'a>, String> {
        if AGGREGATES.contains(&name) {
            let (vals, extra) = if name == "quantile" {
                self.arity(name, args, 2, 2)?;
                (&args[..1], Some(self.scalar(&args[1], "quantile's q")?))
            } else {
                self.arity(name, args, 1, usize::MAX)?;
                (args, None)
            };
            let mut x = Vec::new();
            for a in vals {
                self.numbers(a, &mut x)?;
            }
            if name == "first" || name == "last" {
                // Order matters; numbers() keeps row order.
            }
            return Ok(Node::Const(
                aggregate(name, x, extra).unwrap_or_else(Value::Error),
            ));
        }
        let simple = |f: Func, n: usize| -> Result<Node<'a>, String> {
            self.arity(name, args, n, n)?;
            self.elementwise(f, args)
        };
        match name {
            "abs" => simple(Func::Abs, 1),
            "sqrt" => simple(Func::Sqrt, 1),
            "exp" => simple(Func::Exp, 1),
            "ln" => simple(Func::Ln, 1),
            "log" => {
                self.arity(name, args, 1, 2)?;
                self.elementwise(Func::Log, args)
            }
            "log10" => simple(Func::Log10, 1),
            "log2" => simple(Func::Log2, 1),
            "pow" => simple(Func::Pow, 2),
            "sin" => simple(Func::Sin, 1),
            "cos" => simple(Func::Cos, 1),
            "tan" => simple(Func::Tan, 1),
            "asin" => simple(Func::Asin, 1),
            "acos" => simple(Func::Acos, 1),
            "atan" => simple(Func::Atan, 1),
            "sinh" => simple(Func::Sinh, 1),
            "cosh" => simple(Func::Cosh, 1),
            "tanh" => simple(Func::Tanh, 1),
            "floor" => simple(Func::Floor, 1),
            "ceil" => simple(Func::Ceil, 1),
            "trunc" => simple(Func::Trunc, 1),
            "sign" => simple(Func::Sign, 1),
            "round" => {
                self.arity(name, args, 1, 2)?;
                self.elementwise(Func::Round, args)
            }
            "if" => simple(Func::If, 3),
            "isna" => simple(Func::IsNa, 1),
            "coalesce" => {
                self.arity(name, args, 1, usize::MAX)?;
                self.elementwise(Func::Coalesce, args)
            }
            "clamp" => simple(Func::Clamp, 3),
            "pct" => simple(Func::Pct, 2),
            "na" => {
                self.arity(name, args, 0, 0)?;
                Ok(Node::Const(Value::Missing))
            }
            "pi" => {
                self.arity(name, args, 0, 0)?;
                Ok(Node::Const(Value::Number(std::f64::consts::PI)))
            }
            "row" => {
                self.arity(name, args, 0, 0)?;
                Ok(Node::RowNum)
            }
            "seq" => {
                self.arity(name, args, 2, 3)?;
                let a = self.scalar(&args[0], "seq start")?;
                let b = self.scalar(&args[1], "seq end")?;
                let step = if args.len() == 3 {
                    self.scalar(&args[2], "seq step")?
                } else {
                    1.0
                };
                if step == 0.0 || ((b - a) / step) < 0.0 || ((b - a) / step) > 1e7 {
                    return Err("seq: bad step".into());
                }
                let n = ((b - a) / step + 1e-9).floor() as usize + 1;
                Ok(Node::Const(Value::Vector(
                    (0..n).map(|i| Value::Number(a + step * i as f64)).collect(),
                )))
            }
            "convert" => {
                self.arity(name, args, 3, 4)?;
                let from = self.text_arg(&args[1], "the unit to convert from")?;
                let to = self.text_arg(&args[2], "the unit to convert to")?;
                let sub = match args.get(3) {
                    Some(e) => Some(self.text_arg(e, "the substance")?),
                    None => None,
                };
                let c = conversion(&from, &to, sub.as_deref())?;
                self.elementwise(Func::Convert(c), &args[..1])
            }
            "normalize" if args.len() == 3 => self.elementwise(Func::Normalize, args),
            "lag" | "lead" => {
                self.arity(name, args, 1, 2)?;
                let k = if args.len() == 2 {
                    self.scalar(&args[1], "the shift")?
                } else {
                    1.0
                };
                if k.fract() != 0.0 {
                    return Err(format!("{name}: the shift must be a whole number"));
                }
                let k = if name == "lag" { k as i64 } else { -(k as i64) };
                let v = self.vector(&args[0])?;
                let n = v.len() as i64;
                let out = (0..n)
                    .map(|i| {
                        let j = i - k;
                        if (0..n).contains(&j) {
                            v[j as usize].clone()
                        } else {
                            Value::Missing
                        }
                    })
                    .collect();
                Ok(self.as_row_node(out))
            }
            "diff" => {
                self.arity(name, args, 1, 1)?;
                let v = self.vector(&args[0])?;
                let mut out = Vec::with_capacity(v.len());
                for i in 0..v.len() {
                    out.push(if i == 0 {
                        Value::Missing
                    } else {
                        binary(BinOp::Sub, v[i].clone(), v[i - 1].clone())
                    });
                }
                Ok(self.as_row_node(out))
            }
            "cumsum" => {
                self.arity(name, args, 1, 1)?;
                let v = self.vector(&args[0])?;
                let mut s = 0.0;
                let out = v
                    .into_iter()
                    .map(|x| match x {
                        Value::Missing => Value::Missing,
                        Value::Error(_) => x,
                        _ => match x.as_number() {
                            Some(n) => {
                                s += n;
                                Value::Number(s)
                            }
                            None => text_arith(&x),
                        },
                    })
                    .collect();
                Ok(self.as_row_node(out))
            }
            "rank" => {
                self.arity(name, args, 1, 1)?;
                let v = self.vector(&args[0])?;
                Ok(self.as_row_node(ranks(&v)))
            }
            "zscore" | "normalize" => {
                self.arity(name, args, 1, 1)?;
                let v = self.vector(&args[0])?;
                let mut x = Vec::new();
                collect_numbers(&Value::Vector(v.clone()), &mut x)?;
                let (a, b) = if name == "zscore" {
                    if x.len() < 2 {
                        return Err("zscore needs at least 2 values".into());
                    }
                    let m = mean(&x);
                    (m, variance(&x).sqrt())
                } else {
                    if x.is_empty() {
                        return Err("normalize needs at least 1 value".into());
                    }
                    let lo = x.iter().copied().fold(f64::INFINITY, f64::min);
                    let hi = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    (lo, hi - lo)
                };
                if b == 0.0 {
                    return Err(format!("{name}: all values are equal"));
                }
                let out = v
                    .into_iter()
                    .map(|x| {
                        binary(
                            BinOp::Div,
                            binary(BinOp::Sub, x, Value::Number(a)),
                            Value::Number(b),
                        )
                    })
                    .collect();
                Ok(self.as_row_node(out))
            }
            "baseline" => {
                self.arity(name, args, 2, usize::MAX)?;
                let v = self.vector(&args[0])?;
                let mut rows = Vec::new();
                for a in &args[1..] {
                    let n = self.compile(a)?;
                    if n.varying() {
                        return Err("baseline: give row numbers, like baseline(A, 1, 2, 3) or baseline(A, seq(1, 3))".into());
                    }
                    collect_numbers(&self.materialise(n), &mut rows)?;
                }
                let mut base = Vec::new();
                for r in rows {
                    if r < 1.0 || r.fract() != 0.0 || r as usize > v.len() {
                        return Err(format!("baseline: there is no row {r}"));
                    }
                    if let Some(x) = v[r as usize - 1].as_number() {
                        base.push(x);
                    }
                }
                if base.is_empty() {
                    return Err("baseline: the baseline rows are all empty".into());
                }
                let m = mean(&base);
                let out = v
                    .into_iter()
                    .map(|x| binary(BinOp::Sub, x, Value::Number(m)))
                    .collect();
                Ok(self.as_row_node(out))
            }
            _ => Err(format!("unknown function {name}()")),
        }
    }

    fn elementwise(&self, f: Func, args: &[Expr]) -> Result<Node<'a>, String> {
        let nodes: Vec<Node<'a>> = args
            .iter()
            .map(|a| self.compile(a))
            .collect::<Result<_, _>>()?;
        let v = nodes.iter().any(Node::varying);
        let nodes = nodes
            .into_iter()
            .map(|n| self.align(n, v))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self.fold(Node::Call(f, nodes)))
    }
}

fn index(v: &[Value], i: f64) -> Result<Value, String> {
    if i < 1.0 || i.fract() != 0.0 {
        return Err(format!("index {i}: rows are numbered from 1"));
    }
    Ok(v.get(i as usize - 1).cloned().unwrap_or(Value::Missing))
}

fn describe(v: &Value) -> &'static str {
    match v {
        Value::Number(_) => "a number",
        Value::Text(_) => "text",
        Value::Bool(_) => "true/false",
        Value::Missing => "missing",
        Value::Vector(_) => "a list",
        Value::Error(_) => "an error",
    }
}
