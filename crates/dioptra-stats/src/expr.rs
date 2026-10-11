//! Expression language for user-defined curve-fit equations.
//!
//! Grammar (Pratt parser, Pratt 1973 "Top down operator precedence"): numbers
//! (`1`, `2.5`, `1e-3`), identifiers, `+ - * /`, `^` (also `**`, right associative and
//! binding tighter than unary minus, so `-x^2 = -(x^2)`), parentheses and function calls.
//! Functions: `exp ln log log10 log2 sqrt abs sin cos tan asin acos atan sinh cosh tanh`
//! and `pow(a, b)`. Following Prism, `log` is the base-10 logarithm and `ln` the natural
//! one. Constants: `pi`, `e`. Identifiers are case-insensitive. The independent
//! variables are named by the caller (`X` by default); every other identifier is a
//! parameter, in order of first appearance unless an explicit order is given.
//!
//! Compiled expressions are evaluated on a postfix tape with forward-mode automatic
//! differentiation (dual numbers with a gradient over all parameters; Griewank & Walther
//! 2008, *Evaluating Derivatives*, ch. 3), so user equations get exact derivatives, not
//! finite differences.

use crate::{Result, StatsError};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Func {
    Exp,
    Ln,
    Log10,
    Log2,
    Sqrt,
    Abs,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Sinh,
    Cosh,
    Tanh,
}

impl Func {
    fn from_name(s: &str) -> Option<Func> {
        Some(match s {
            "exp" => Func::Exp,
            "ln" => Func::Ln,
            "log" | "log10" => Func::Log10,
            "log2" => Func::Log2,
            "sqrt" => Func::Sqrt,
            "abs" => Func::Abs,
            "sin" => Func::Sin,
            "cos" => Func::Cos,
            "tan" => Func::Tan,
            "asin" | "arcsin" => Func::Asin,
            "acos" | "arccos" => Func::Acos,
            "atan" | "arctan" => Func::Atan,
            "sinh" => Func::Sinh,
            "cosh" => Func::Cosh,
            "tanh" => Func::Tanh,
            _ => return None,
        })
    }

    /// Value and derivative with respect to the argument.
    fn eval(self, a: f64) -> (f64, f64) {
        match self {
            Func::Exp => {
                let v = a.exp();
                (v, v)
            }
            Func::Ln => (a.ln(), 1.0 / a),
            Func::Log10 => (a.log10(), 1.0 / (a * std::f64::consts::LN_10)),
            Func::Log2 => (a.log2(), 1.0 / (a * std::f64::consts::LN_2)),
            Func::Sqrt => {
                let v = a.sqrt();
                (v, 0.5 / v)
            }
            Func::Abs => (a.abs(), if a < 0.0 { -1.0 } else { 1.0 }),
            Func::Sin => (a.sin(), a.cos()),
            Func::Cos => (a.cos(), -a.sin()),
            Func::Tan => {
                let c = a.cos();
                (a.tan(), 1.0 / (c * c))
            }
            Func::Asin => (a.asin(), 1.0 / (1.0 - a * a).sqrt()),
            Func::Acos => (a.acos(), -1.0 / (1.0 - a * a).sqrt()),
            Func::Atan => (a.atan(), 1.0 / (1.0 + a * a)),
            Func::Sinh => (a.sinh(), a.cosh()),
            Func::Cosh => (a.cosh(), a.sinh()),
            Func::Tanh => {
                let t = a.tanh();
                (t, 1.0 - t * t)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Op(char),
    LParen,
    RParen,
    Comma,
}

fn err(msg: impl Into<String>) -> StatsError {
    StatsError::InvalidArgument(msg.into())
}

fn lex(s: &str) -> Result<Vec<(Tok, usize)>> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        if c.is_ascii_digit()
            || (c == '.' && i + 1 < b.len() && (b[i + 1] as char).is_ascii_digit())
        {
            while i < b.len() && ((b[i] as char).is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
                let mut j = i + 1;
                if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
                    j += 1;
                }
                if j < b.len() && (b[j] as char).is_ascii_digit() {
                    i = j;
                    while i < b.len() && (b[i] as char).is_ascii_digit() {
                        i += 1;
                    }
                }
            }
            let text = &s[start..i];
            let v: f64 = text
                .parse()
                .map_err(|_| err(format!("bad number '{text}' at position {start}")))?;
            out.push((Tok::Num(v), start));
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            while i < b.len() && ((b[i] as char).is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push((Tok::Ident(s[start..i].to_string()), start));
            continue;
        }
        match c {
            '+' | '-' | '/' | '^' => out.push((Tok::Op(c), start)),
            '*' => {
                if i + 1 < b.len() && b[i + 1] == b'*' {
                    out.push((Tok::Op('^'), start));
                    i += 1;
                } else {
                    out.push((Tok::Op('*'), start));
                }
            }
            '(' => out.push((Tok::LParen, start)),
            ')' => out.push((Tok::RParen, start)),
            ',' => out.push((Tok::Comma, start)),
            _ => {
                return Err(err(format!(
                    "unexpected character '{c}' at position {start}"
                )))
            }
        }
        i += 1;
    }
    Ok(out)
}

#[derive(Debug, Clone)]
enum Node {
    Const(f64),
    Var(usize),
    Param(usize),
    Neg(Box<Node>),
    Bin(char, Box<Node>, Box<Node>),
    Call(Func, Box<Node>),
}

struct Parser<'a> {
    toks: Vec<(Tok, usize)>,
    pos: usize,
    vars: &'a [String],
    params: Vec<String>,
    /// Parameters must come from this list when set (built-in equations).
    fixed_params: bool,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|t| &t.0)
    }
    fn at(&self) -> usize {
        self.toks.get(self.pos).map_or(usize::MAX, |t| t.1)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).map(|t| t.0.clone());
        self.pos += 1;
        t
    }

    fn expr(&mut self, min_bp: u8) -> Result<Node> {
        let at = self.at();
        let mut lhs = match self.next() {
            None => return Err(err("unexpected end of expression")),
            Some(Tok::Num(v)) => Node::Const(v),
            Some(Tok::Op('-')) => {
                let e = self.expr(5)?;
                match e {
                    Node::Const(v) => Node::Const(-v),
                    e => Node::Neg(Box::new(e)),
                }
            }
            Some(Tok::Op('+')) => self.expr(5)?,
            Some(Tok::LParen) => {
                let e = self.expr(0)?;
                if self.next() != Some(Tok::RParen) {
                    return Err(err(format!("missing ')' for '(' at position {at}")));
                }
                e
            }
            Some(Tok::Ident(name)) => self.ident(&name, at)?,
            Some(t) => return Err(err(format!("unexpected {t:?} at position {at}"))),
        };
        loop {
            let op = match self.peek() {
                Some(Tok::Op(c)) => *c,
                Some(Tok::RParen) | Some(Tok::Comma) | None => break,
                Some(t) => {
                    return Err(err(format!(
                        "expected an operator, found {t:?} at position {}",
                        self.at()
                    )))
                }
            };
            let (lbp, rbp) = match op {
                '+' | '-' => (1, 2),
                '*' | '/' => (3, 4),
                '^' => (8, 7),
                _ => unreachable!(),
            };
            if lbp < min_bp {
                break;
            }
            self.pos += 1;
            let rhs = self.expr(rbp)?;
            lhs = fold(op, lhs, rhs);
        }
        Ok(lhs)
    }

    fn ident(&mut self, name: &str, at: usize) -> Result<Node> {
        let lower = name.to_ascii_lowercase();
        if self.peek() == Some(&Tok::LParen) {
            self.pos += 1;
            let a = self.expr(0)?;
            if lower == "pow" {
                if self.next() != Some(Tok::Comma) {
                    return Err(err(format!("pow needs two arguments (position {at})")));
                }
                let b = self.expr(0)?;
                if self.next() != Some(Tok::RParen) {
                    return Err(err(format!("missing ')' after pow( at position {at}")));
                }
                return Ok(fold('^', a, b));
            }
            if self.next() != Some(Tok::RParen) {
                return Err(err(format!(
                    "function '{name}' takes one argument (position {at})"
                )));
            }
            let f = Func::from_name(&lower)
                .ok_or_else(|| err(format!("unknown function '{name}' at position {at}")))?;
            if let Node::Const(v) = a {
                return Ok(Node::Const(f.eval(v).0));
            }
            return Ok(Node::Call(f, Box::new(a)));
        }
        if let Some(i) = self.vars.iter().position(|v| v.eq_ignore_ascii_case(name)) {
            return Ok(Node::Var(i));
        }
        match lower.as_str() {
            "pi" => return Ok(Node::Const(std::f64::consts::PI)),
            "e" => return Ok(Node::Const(std::f64::consts::E)),
            _ => {}
        }
        if Func::from_name(&lower).is_some() || lower == "pow" {
            return Err(err(format!(
                "'{name}' is a function and needs parentheses (position {at})"
            )));
        }
        if let Some(i) = self
            .params
            .iter()
            .position(|p| p.eq_ignore_ascii_case(name))
        {
            return Ok(Node::Param(i));
        }
        if self.fixed_params {
            return Err(err(format!("unknown identifier '{name}' at position {at}")));
        }
        self.params.push(name.to_string());
        Ok(Node::Param(self.params.len() - 1))
    }
}

fn fold(op: char, a: Node, b: Node) -> Node {
    if let (Node::Const(x), Node::Const(y)) = (&a, &b) {
        let v = match op {
            '+' => x + y,
            '-' => x - y,
            '*' => x * y,
            '/' => x / y,
            _ => x.powf(*y),
        };
        return Node::Const(v);
    }
    Node::Bin(op, Box::new(a), Box::new(b))
}

#[derive(Debug, Clone, Copy)]
enum Op {
    Const(f64),
    Var(usize),
    Param(usize),
    Neg,
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    PowI(i32),
    PowC(f64),
    Call(Func),
}

/// A compiled expression: a postfix tape.
#[derive(Debug, Clone)]
pub(crate) struct Compiled {
    ops: Vec<Op>,
    depth: usize,
    pub n_params: usize,
    pub params: Vec<String>,
}

fn emit(n: &Node, out: &mut Vec<Op>) {
    match n {
        Node::Const(v) => out.push(Op::Const(*v)),
        Node::Var(i) => out.push(Op::Var(*i)),
        Node::Param(i) => out.push(Op::Param(*i)),
        Node::Neg(a) => {
            emit(a, out);
            out.push(Op::Neg);
        }
        Node::Bin(op, a, b) => {
            emit(a, out);
            if *op == '^' {
                if let Node::Const(c) = **b {
                    if c == c.round() && c.abs() <= 64.0 {
                        out.push(Op::PowI(c as i32));
                    } else {
                        out.push(Op::PowC(c));
                    }
                    return;
                }
            }
            emit(b, out);
            out.push(match op {
                '+' => Op::Add,
                '-' => Op::Sub,
                '*' => Op::Mul,
                '/' => Op::Div,
                _ => Op::Pow,
            });
        }
        Node::Call(f, a) => {
            emit(a, out);
            out.push(Op::Call(*f));
        }
    }
}

impl Compiled {
    /// Parse `src` with independent variables `vars`. When `params` is given, identifiers
    /// must be one of them and the order is theirs; otherwise parameters are discovered
    /// in order of first appearance.
    pub(crate) fn parse(src: &str, vars: &[String], params: Option<&[String]>) -> Result<Self> {
        let toks = lex(src)?;
        if toks.is_empty() {
            return Err(err("empty expression"));
        }
        let mut p = Parser {
            toks,
            pos: 0,
            vars,
            params: params.map(|p| p.to_vec()).unwrap_or_default(),
            fixed_params: params.is_some(),
        };
        let node = p.expr(0)?;
        if p.pos < p.toks.len() {
            return Err(err(format!(
                "unexpected {:?} at position {}",
                p.toks[p.pos].0, p.toks[p.pos].1
            )));
        }
        let mut ops = Vec::new();
        emit(&node, &mut ops);
        let mut d: usize = 0;
        let mut depth = 0;
        for op in &ops {
            match op {
                Op::Const(_) | Op::Var(_) | Op::Param(_) => d += 1,
                Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Pow => d -= 1,
                _ => {}
            }
            depth = depth.max(d);
        }
        Ok(Compiled {
            ops,
            depth,
            n_params: p.params.len(),
            params: p.params,
        })
    }

    /// Value only.
    pub(crate) fn eval(&self, x: &[f64], p: &[f64]) -> f64 {
        let mut st: Vec<f64> = Vec::with_capacity(self.depth);
        for op in &self.ops {
            match *op {
                Op::Const(v) => st.push(v),
                Op::Var(i) => st.push(x.get(i).copied().unwrap_or(f64::NAN)),
                Op::Param(i) => st.push(p[i]),
                Op::Neg => {
                    let a = st.last_mut().unwrap();
                    *a = -*a;
                }
                Op::PowI(k) => {
                    let a = st.last_mut().unwrap();
                    *a = a.powi(k);
                }
                Op::PowC(c) => {
                    let a = st.last_mut().unwrap();
                    *a = a.powf(c);
                }
                Op::Call(f) => {
                    let a = st.last_mut().unwrap();
                    *a = f.eval(*a).0;
                }
                Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Pow => {
                    let b = st.pop().unwrap();
                    let a = st.last_mut().unwrap();
                    *a = match *op {
                        Op::Add => *a + b,
                        Op::Sub => *a - b,
                        Op::Mul => *a * b,
                        Op::Div => *a / b,
                        _ => a.powf(b),
                    };
                }
            }
        }
        st.pop().unwrap_or(f64::NAN)
    }

    /// Value and gradient with respect to the parameters (forward-mode AD).
    pub(crate) fn eval_grad(&self, x: &[f64], p: &[f64], grad: &mut [f64]) -> f64 {
        let np = self.n_params;
        let mut val: Vec<f64> = Vec::with_capacity(self.depth);
        let mut live: Vec<bool> = Vec::with_capacity(self.depth);
        let mut g: Vec<f64> = vec![0.0; self.depth.max(1) * np];
        for op in &self.ops {
            let top = val.len();
            match *op {
                Op::Const(v) => {
                    val.push(v);
                    live.push(false);
                }
                Op::Var(i) => {
                    val.push(x.get(i).copied().unwrap_or(f64::NAN));
                    live.push(false);
                }
                Op::Param(i) => {
                    val.push(p[i]);
                    live.push(true);
                    let gs = &mut g[top * np..(top + 1) * np];
                    gs.fill(0.0);
                    gs[i] = 1.0;
                }
                Op::Neg => {
                    let k = top - 1;
                    val[k] = -val[k];
                    if live[k] {
                        for v in &mut g[k * np..(k + 1) * np] {
                            *v = -*v;
                        }
                    }
                }
                Op::PowI(_) | Op::PowC(_) | Op::Call(_) => {
                    let k = top - 1;
                    let a = val[k];
                    let (v, d) = match *op {
                        Op::PowI(n) => {
                            let v = a.powi(n);
                            let d = if n == 0 {
                                0.0
                            } else {
                                n as f64 * a.powi(n - 1)
                            };
                            (v, d)
                        }
                        Op::PowC(c) => (a.powf(c), c * a.powf(c - 1.0)),
                        Op::Call(f) => f.eval(a),
                        _ => unreachable!(),
                    };
                    val[k] = v;
                    if live[k] {
                        for gv in &mut g[k * np..(k + 1) * np] {
                            *gv *= d;
                        }
                    }
                }
                Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Pow => {
                    let kb = top - 1;
                    let ka = top - 2;
                    let (a, b) = (val[ka], val[kb]);
                    let (la, lb) = (live[ka], live[kb]);
                    let (v, da, db) = match *op {
                        Op::Add => (a + b, 1.0, 1.0),
                        Op::Sub => (a - b, 1.0, -1.0),
                        Op::Mul => (a * b, b, a),
                        Op::Div => (a / b, 1.0 / b, -a / (b * b)),
                        _ => {
                            let v = a.powf(b);
                            let da = if la { b * a.powf(b - 1.0) } else { 0.0 };
                            // d(a^b)/db = a^b ln a; taken as 0 where a^b = 0 (a = 0, b > 0).
                            let db = if lb && v != 0.0 { v * a.ln() } else { 0.0 };
                            (v, da, db)
                        }
                    };
                    let (lo, hi) = g.split_at_mut(kb * np);
                    let ga = &mut lo[ka * np..(ka + 1) * np];
                    let gb = &hi[..np];
                    match (la, lb) {
                        (true, true) => {
                            for (x, y) in ga.iter_mut().zip(gb) {
                                *x = da * *x + db * y;
                            }
                        }
                        (true, false) => {
                            for x in ga.iter_mut() {
                                *x *= da;
                            }
                        }
                        (false, true) => {
                            for (x, y) in ga.iter_mut().zip(gb) {
                                *x = db * y;
                            }
                        }
                        (false, false) => {}
                    }
                    val[ka] = v;
                    live[ka] = la || lb;
                    val.pop();
                    live.pop();
                }
            }
        }
        let v = val.pop().unwrap_or(f64::NAN);
        if live.pop().unwrap_or(false) {
            grad[..np].copy_from_slice(&g[..np]);
        } else {
            grad[..np].fill(0.0);
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(src: &str) -> Compiled {
        Compiled::parse(src, &["X".to_string()], None).unwrap()
    }

    #[test]
    fn precedence_and_unary() {
        assert_eq!(c("-2^2").eval(&[0.0], &[]), -4.0);
        assert_eq!(c("2^3^2").eval(&[0.0], &[]), 512.0);
        assert_eq!(c("2^-1*4").eval(&[0.0], &[]), 2.0);
        assert_eq!(c("1 - 2 - 3").eval(&[0.0], &[]), -4.0);
        assert_eq!(c("8/4/2").eval(&[0.0], &[]), 1.0);
        assert_eq!(c("2**3").eval(&[0.0], &[]), 8.0);
        assert!((c("log(100) + ln(e)").eval(&[0.0], &[]) - 3.0).abs() < 1e-15);
        assert!((c("x*pi").eval(&[2.0], &[]) - 2.0 * std::f64::consts::PI).abs() < 1e-15);
    }

    #[test]
    fn params_and_gradient() {
        let e = c("Bottom + (Top-Bottom)/(1+10^((LogEC50-X)*HillSlope))");
        assert_eq!(e.params, ["Bottom", "Top", "LogEC50", "HillSlope"]);
        let p = [1.0, 9.0, -6.0, 0.8];
        let x = [-6.5];
        let mut g = [0.0; 4];
        let v = e.eval_grad(&x, &p, &mut g);
        assert!((v - e.eval(&x, &p)).abs() < 1e-15);
        for j in 0..4 {
            let h = 1e-6 * p[j].abs().max(1.0);
            let mut pp = p;
            pp[j] += h;
            let mut pm = p;
            pm[j] -= h;
            let fd = (e.eval(&x, &pp) - e.eval(&x, &pm)) / (2.0 * h);
            assert!(
                (fd - g[j]).abs() < 1e-7 * (1.0 + g[j].abs()),
                "{j}: {fd} vs {}",
                g[j]
            );
        }
    }

    #[test]
    fn negative_base_integer_power_has_finite_gradient() {
        let e = c("a*(x-b)^2");
        let mut g = [0.0; 2];
        e.eval_grad(&[0.0], &[2.0, 3.0], &mut g);
        assert_eq!(g, [9.0, 12.0]);
    }

    #[test]
    fn errors() {
        assert!(Compiled::parse("1 +", &["X".into()], None).is_err());
        assert!(Compiled::parse("foo(1)", &["X".into()], None).is_err());
        assert!(Compiled::parse("(1", &["X".into()], None).is_err());
        assert!(Compiled::parse("1 $ 2", &["X".into()], None).is_err());
        assert!(Compiled::parse("a b", &["X".into()], None).is_err());
    }
}
