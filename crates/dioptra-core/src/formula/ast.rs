//! Formula syntax tree and its canonical pretty-printer.

use std::fmt;

/// What a column reference points at.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ColTarget {
    /// The X column.
    X,
    /// A dataset by letter (`A`, `B`, …, `AA`).
    Letter(String),
    /// A dataset (or the X column) by its header name, written in backticks.
    Name(String),
}

/// A column reference: a dataset and an optional 1-based subcolumn (`A.2`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColRef {
    /// Target.
    pub target: ColTarget,
    /// 1-based subcolumn, if written.
    pub sub: Option<usize>,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    /// `-x`
    Neg,
    /// `+x`
    Plus,
    /// `!x`, `not x`
    Not,
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `^`
    Pow,
    /// `%` (floored modulo)
    Mod,
    /// `==` or `=`
    Eq,
    /// `!=` or `<>`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `&&`, `and`
    And,
    /// `||`, `or`
    Or,
}

/// A formula expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Number literal.
    Number(f64),
    /// String literal.
    Str(String),
    /// `true` / `false`.
    Bool(bool),
    /// `NA`.
    Missing,
    /// A whole column.
    Column(ColRef),
    /// One cell, Excel style (`A3`, 1-based row).
    Cell(ColRef, usize),
    /// Vector literal `[1, 2, 3]`.
    Vector(Vec<Expr>),
    /// `x[i]` (1-based).
    Index(Box<Expr>, Box<Expr>),
    /// `x[a:b]` (1-based, inclusive; either end may be omitted).
    Slice(Box<Expr>, Option<Box<Expr>>, Option<Box<Expr>>),
    /// Unary operation.
    Unary(UnOp, Box<Expr>),
    /// Binary operation.
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// Function call (name lower-cased).
    Call(String, Vec<Expr>),
}

/// Precedence levels used by the parser and the printer (higher binds tighter).
pub(crate) mod prec {
    pub const OR: u8 = 2;
    pub const AND: u8 = 4;
    pub const NOT: u8 = 5;
    pub const CMP: u8 = 6;
    pub const ADD: u8 = 8;
    pub const MUL: u8 = 10;
    pub const NEG: u8 = 12;
    pub const POW: u8 = 14;
    pub const POSTFIX: u8 = 16;
    pub const ATOM: u8 = 18;
}

impl BinOp {
    /// Precedence level.
    pub fn prec(self) -> u8 {
        match self {
            BinOp::Or => prec::OR,
            BinOp::And => prec::AND,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => prec::CMP,
            BinOp::Add | BinOp::Sub => prec::ADD,
            BinOp::Mul | BinOp::Div | BinOp::Mod => prec::MUL,
            BinOp::Pow => prec::POW,
        }
    }

    /// Canonical spelling.
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Pow => "^",
            BinOp::Mod => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
        }
    }
}

impl Expr {
    fn prec(&self) -> u8 {
        match self {
            Expr::Binary(op, ..) => op.prec(),
            Expr::Unary(UnOp::Not, _) => prec::NOT,
            Expr::Unary(..) => prec::NEG,
            Expr::Number(v) if *v < 0.0 || (*v == 0.0 && v.is_sign_negative()) => prec::NEG,
            Expr::Index(..) | Expr::Slice(..) => prec::POSTFIX,
            _ => prec::ATOM,
        }
    }

    /// The same expression with every single-cell reference moved down by `delta` rows
    /// (fill-down of a cell formula, like a spreadsheet's relative references).
    pub fn shift_rows(&self, delta: isize) -> Expr {
        let b = |e: &Expr| Box::new(e.shift_rows(delta));
        match self {
            Expr::Cell(c, r) => Expr::Cell(c.clone(), (*r as isize + delta).max(1) as usize),
            Expr::Vector(v) => Expr::Vector(v.iter().map(|e| e.shift_rows(delta)).collect()),
            Expr::Call(n, v) => {
                Expr::Call(n.clone(), v.iter().map(|e| e.shift_rows(delta)).collect())
            }
            Expr::Index(a, i) => Expr::Index(b(a), b(i)),
            Expr::Slice(a, lo, hi) => Expr::Slice(b(a), lo.as_deref().map(b), hi.as_deref().map(b)),
            Expr::Unary(op, a) => Expr::Unary(*op, b(a)),
            Expr::Binary(op, x, y) => Expr::Binary(*op, b(x), b(y)),
            other => other.clone(),
        }
    }

    /// Every column this expression reads, with the 1-based row when only one cell of it
    /// is read (`A3`, `A[3]`).
    pub fn references(&self) -> Vec<(ColRef, Option<usize>)> {
        let mut out = Vec::new();
        self.collect_refs(&mut out);
        out
    }

    fn collect_refs(&self, out: &mut Vec<(ColRef, Option<usize>)>) {
        match self {
            Expr::Column(c) => out.push((c.clone(), None)),
            Expr::Cell(c, r) => out.push((c.clone(), Some(*r))),
            Expr::Index(base, idx) => {
                if let (Expr::Column(c), Expr::Number(n)) = (base.as_ref(), idx.as_ref()) {
                    if *n >= 1.0 && n.fract() == 0.0 {
                        out.push((c.clone(), Some(*n as usize)));
                        return;
                    }
                }
                base.collect_refs(out);
                idx.collect_refs(out);
            }
            Expr::Slice(b, lo, hi) => {
                b.collect_refs(out);
                for e in [lo, hi].into_iter().flatten() {
                    e.collect_refs(out);
                }
            }
            Expr::Vector(v) | Expr::Call(_, v) => v.iter().for_each(|e| e.collect_refs(out)),
            Expr::Unary(_, e) => e.collect_refs(out),
            Expr::Binary(_, a, b) => {
                a.collect_refs(out);
                b.collect_refs(out);
            }
            Expr::Number(_) | Expr::Str(_) | Expr::Bool(_) | Expr::Missing => {}
        }
    }
}

impl fmt::Display for ColRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.target {
            ColTarget::X => write!(f, "X")?,
            ColTarget::Letter(l) => write!(f, "{l}")?,
            ColTarget::Name(n) => write!(f, "`{}`", n.replace('`', "``"))?,
        }
        if let Some(s) = self.sub {
            write!(f, ".{s}")?;
        }
        Ok(())
    }
}

fn write_child(f: &mut fmt::Formatter<'_>, e: &Expr, min: u8) -> fmt::Result {
    if e.prec() < min {
        write!(f, "({e})")
    } else {
        write!(f, "{e}")
    }
}

/// Format a number literal so that it parses back to the same value.
pub fn number_literal(v: f64) -> String {
    let s = format!("{v}");
    if s.len() > 12 && (v.abs() >= 1e15 || (v != 0.0 && v.abs() < 1e-6)) {
        format!("{v:e}")
    } else {
        s
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Number(v) => write!(f, "{}", number_literal(*v)),
            Expr::Str(s) => write!(f, "\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            Expr::Bool(b) => write!(f, "{b}"),
            Expr::Missing => write!(f, "NA"),
            Expr::Column(c) => write!(f, "{c}"),
            Expr::Cell(c, r) => {
                if c.sub.is_none() && !matches!(c.target, ColTarget::Name(_)) {
                    write!(f, "{c}{r}")
                } else {
                    write!(f, "{c}[{r}]")
                }
            }
            Expr::Vector(items) => {
                write!(f, "[")?;
                for (i, e) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{e}")?;
                }
                write!(f, "]")
            }
            Expr::Index(b, i) => {
                write_child(f, b, prec::POSTFIX)?;
                write!(f, "[{i}]")
            }
            Expr::Slice(b, lo, hi) => {
                write_child(f, b, prec::POSTFIX)?;
                write!(f, "[")?;
                if let Some(lo) = lo {
                    write!(f, "{lo}")?;
                }
                write!(f, ":")?;
                if let Some(hi) = hi {
                    write!(f, "{hi}")?;
                }
                write!(f, "]")
            }
            Expr::Unary(op, e) => {
                let (sym, p) = match op {
                    UnOp::Neg => ("-", prec::NEG),
                    UnOp::Plus => ("+", prec::NEG),
                    UnOp::Not => ("not ", prec::NOT),
                };
                write!(f, "{sym}")?;
                // `- -x` must not print as `--x`; parenthesise nested signs.
                if matches!(op, UnOp::Neg | UnOp::Plus)
                    && (matches!(e.as_ref(), Expr::Unary(UnOp::Neg | UnOp::Plus, _))
                        || matches!(e.as_ref(), Expr::Number(v) if v.is_sign_negative()))
                {
                    return write!(f, "({e})");
                }
                write_child(f, e, p)
            }
            Expr::Binary(op, a, b) => {
                let p = op.prec();
                let (lmin, rmin) = if *op == BinOp::Pow {
                    (p + 1, p)
                } else {
                    (p, p + 1)
                };
                write_child(f, a, lmin)?;
                write!(f, " {} ", op.symbol())?;
                write_child(f, b, rmin)
            }
            Expr::Call(name, args) => {
                write!(f, "{name}(")?;
                for (i, e) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{e}")?;
                }
                write!(f, ")")
            }
        }
    }
}
