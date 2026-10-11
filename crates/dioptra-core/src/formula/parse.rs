//! Lexer and Pratt parser for the formula language (DESIGN §4.2, docs/FORMULAS.md).

use super::ast::{prec, BinOp, ColRef, ColTarget, Expr, UnOp};

/// A token with its byte offset.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    Backtick(String),
    /// `.N` subcolumn suffix glued to an identifier or backtick name.
    Sub(usize),
    Op(&'static str),
    End,
}

/// A parse error with a position (byte offset into the input).
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    /// Byte offset.
    pub pos: usize,
    /// Message.
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (at character {})", self.message, self.pos + 1)
    }
}

const OPS: [&str; 22] = [
    "==", "!=", "<>", "<=", ">=", "&&", "||", "+", "-", "*", "/", "^", "%", "(", ")", "[", "]",
    ",", ":", "<", ">", "!",
];

pub(crate) fn lex(src: &str) -> Result<Vec<(Tok, usize)>, ParseError> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out: Vec<(Tok, usize)> = Vec::new();
    let err = |pos: usize, m: String| ParseError { pos, message: m };
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        // Subcolumn suffix glued to a name: `A.2`, `` `Dose`.3 ``.
        if c == b'.'
            && i + 1 < b.len()
            && b[i + 1].is_ascii_digit()
            && matches!(out.last(), Some((Tok::Ident(_) | Tok::Backtick(_), p)) if {
                // must be glued (no whitespace) to the previous token
                let prev_end = *p + tok_len(src, *p);
                prev_end == i
            })
        {
            i += 1;
            let s = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let n: usize = src[s..i]
                .parse()
                .map_err(|_| err(start, "bad subcolumn number".into()))?;
            out.push((Tok::Sub(n), start));
            continue;
        }
        if c.is_ascii_digit() || (c == b'.' && i + 1 < b.len() && b[i + 1].is_ascii_digit()) {
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
                let mut j = i + 1;
                if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
                    j += 1;
                }
                if j < b.len() && b[j].is_ascii_digit() {
                    i = j;
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                }
            }
            let text = &src[start..i];
            let v: f64 = text
                .parse()
                .map_err(|_| err(start, format!("bad number '{text}'")))?;
            out.push((Tok::Num(v), start));
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push((Tok::Ident(src[start..i].to_string()), start));
            continue;
        }
        if c == b'`' {
            let mut s = String::new();
            i += 1;
            loop {
                if i >= b.len() {
                    return Err(err(start, "unclosed backtick name".into()));
                }
                if b[i] == b'`' {
                    if i + 1 < b.len() && b[i + 1] == b'`' {
                        s.push('`');
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                let ch = src[i..].chars().next().expect("char");
                s.push(ch);
                i += ch.len_utf8();
            }
            out.push((Tok::Backtick(s), start));
            continue;
        }
        if c == b'"' {
            let mut s = String::new();
            i += 1;
            loop {
                if i >= b.len() {
                    return Err(err(start, "unclosed string".into()));
                }
                if b[i] == b'\\' && i + 1 < b.len() {
                    let ch = src[i + 1..].chars().next().expect("char");
                    s.push(ch);
                    i += 1 + ch.len_utf8();
                    continue;
                }
                if b[i] == b'"' {
                    i += 1;
                    break;
                }
                let ch = src[i..].chars().next().expect("char");
                s.push(ch);
                i += ch.len_utf8();
            }
            out.push((Tok::Str(s), start));
            continue;
        }
        if c == b'=' {
            // `=` alone is equality inside a formula; `==` too.
            i += if b.get(i + 1) == Some(&b'=') { 2 } else { 1 };
            out.push((Tok::Op("=="), start));
            continue;
        }
        if let Some(op) = OPS.iter().find(|op| src[i..].starts_with(**op)) {
            i += op.len();
            out.push((Tok::Op(if *op == "<>" { "!=" } else { op }), start));
            continue;
        }
        let ch = src[i..].chars().next().expect("char");
        return Err(err(start, format!("unexpected character '{ch}'")));
    }
    out.push((Tok::End, src.len()));
    Ok(out)
}

fn tok_len(src: &str, pos: usize) -> usize {
    // Length of an identifier or backtick token starting at `pos`.
    let b = src.as_bytes();
    if b[pos] == b'`' {
        let mut i = pos + 1;
        while i < b.len() {
            if b[i] == b'`' {
                if i + 1 < b.len() && b[i + 1] == b'`' {
                    i += 2;
                    continue;
                }
                return i + 1 - pos;
            }
            i += 1;
        }
        b.len() - pos
    } else {
        let mut i = pos;
        while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
            i += 1;
        }
        i - pos
    }
}

struct Parser {
    toks: Vec<(Tok, usize)>,
    i: usize,
}

/// Parse a formula. A leading `=` is allowed and ignored.
pub fn parse(src: &str) -> Result<Expr, ParseError> {
    let body = src.trim_start();
    let (body, offset) = match body.strip_prefix('=') {
        Some(rest) => (rest, src.len() - rest.len()),
        None => (body, src.len() - body.len()),
    };
    let toks = lex(body).map_err(|e| ParseError {
        pos: e.pos + offset,
        ..e
    })?;
    let mut p = Parser { toks, i: 0 };
    if matches!(p.peek(), Tok::End) {
        return Err(ParseError {
            pos: offset,
            message: "empty formula".into(),
        });
    }
    let e = p.expr(0).map_err(|e| ParseError {
        pos: e.pos + offset,
        ..e
    })?;
    if !matches!(p.peek(), Tok::End) {
        let pos = p.pos();
        return Err(ParseError {
            pos: pos + offset,
            message: format!("unexpected {}", p.describe()),
        });
    }
    Ok(e)
}

/// Split `ABC12` into ("ABC", 12) when it is uppercase letters followed by digits.
fn split_cell(name: &str) -> Option<(&str, usize)> {
    let k = name.find(|c: char| c.is_ascii_digit())?;
    let (l, d) = name.split_at(k);
    if l.is_empty()
        || !l.bytes().all(|c| c.is_ascii_uppercase())
        || !d.bytes().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let r: usize = d.parse().ok()?;
    (r >= 1).then_some((l, r))
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.i].0
    }
    fn pos(&self) -> usize {
        self.toks[self.i].1
    }
    fn bump(&mut self) -> Tok {
        let t = self.toks[self.i].0.clone();
        if self.i + 1 < self.toks.len() {
            self.i += 1;
        }
        t
    }
    fn describe(&self) -> String {
        match self.peek() {
            Tok::Num(v) => format!("number {v}"),
            Tok::Str(s) => format!("text \"{s}\""),
            Tok::Ident(s) => format!("'{s}'"),
            Tok::Backtick(s) => format!("`{s}`"),
            Tok::Sub(n) => format!("'.{n}'"),
            Tok::Op(o) => format!("'{o}'"),
            Tok::End => "end of formula".into(),
        }
    }
    fn err<T>(&self, m: impl Into<String>) -> Result<T, ParseError> {
        Err(ParseError {
            pos: self.pos(),
            message: m.into(),
        })
    }
    fn expect(&mut self, op: &str) -> Result<(), ParseError> {
        if *self.peek() == Tok::Op(op_static(op)) {
            self.bump();
            Ok(())
        } else {
            self.err(format!("expected '{op}' but found {}", self.describe()))
        }
    }

    fn infix(&self) -> Option<(BinOp, u8, u8)> {
        let op = match self.peek() {
            Tok::Op(o) => match *o {
                "+" => BinOp::Add,
                "-" => BinOp::Sub,
                "*" => BinOp::Mul,
                "/" => BinOp::Div,
                "%" => BinOp::Mod,
                "^" => BinOp::Pow,
                "==" => BinOp::Eq,
                "!=" => BinOp::Ne,
                "<" => BinOp::Lt,
                "<=" => BinOp::Le,
                ">" => BinOp::Gt,
                ">=" => BinOp::Ge,
                "&&" => BinOp::And,
                "||" => BinOp::Or,
                _ => return None,
            },
            Tok::Ident(s) if s.eq_ignore_ascii_case("and") => BinOp::And,
            Tok::Ident(s) if s.eq_ignore_ascii_case("or") => BinOp::Or,
            _ => return None,
        };
        let p = op.prec();
        // (left binding power, right binding power); `^` is right-associative.
        Some(if op == BinOp::Pow {
            (op, p, p)
        } else {
            (op, p, p + 1)
        })
    }

    fn expr(&mut self, min: u8) -> Result<Expr, ParseError> {
        let mut lhs = self.prefix()?;
        loop {
            // Postfix indexing.
            if *self.peek() == Tok::Op("[") {
                if prec::POSTFIX < min {
                    break;
                }
                lhs = self.postfix(lhs)?;
                continue;
            }
            let Some((op, lbp, rbp)) = self.infix() else {
                break;
            };
            if lbp < min {
                break;
            }
            self.bump();
            let rhs = self.expr(rbp)?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn postfix(&mut self, base: Expr) -> Result<Expr, ParseError> {
        self.expect("[")?;
        let lo = if *self.peek() == Tok::Op(":") {
            None
        } else {
            Some(Box::new(self.expr(0)?))
        };
        if *self.peek() == Tok::Op(":") {
            self.bump();
            let hi = if *self.peek() == Tok::Op("]") {
                None
            } else {
                Some(Box::new(self.expr(0)?))
            };
            self.expect("]")?;
            return Ok(Expr::Slice(Box::new(base), lo, hi));
        }
        self.expect("]")?;
        let idx = lo.expect("index present");
        // `A[3]` on a plain column is the same cell as `A3`.
        if let (Expr::Column(c), Expr::Number(n)) = (&base, idx.as_ref()) {
            if *n >= 1.0 && n.fract() == 0.0 {
                return Ok(Expr::Cell(c.clone(), *n as usize));
            }
        }
        Ok(Expr::Index(Box::new(base), idx))
    }

    fn sub_suffix(&mut self) -> Option<usize> {
        if let Tok::Sub(n) = self.peek() {
            let n = *n;
            self.bump();
            Some(n)
        } else {
            None
        }
    }

    fn prefix(&mut self) -> Result<Expr, ParseError> {
        let pos = self.pos();
        match self.bump() {
            Tok::Num(v) => Ok(Expr::Number(v)),
            Tok::Str(s) => Ok(Expr::Str(s)),
            Tok::Backtick(name) => {
                let sub = self.sub_suffix();
                Ok(Expr::Column(ColRef {
                    target: ColTarget::Name(name),
                    sub,
                }))
            }
            Tok::Op("(") => {
                let e = self.expr(0)?;
                self.expect(")")?;
                Ok(e)
            }
            Tok::Op("[") => {
                let mut items = Vec::new();
                if *self.peek() != Tok::Op("]") {
                    loop {
                        items.push(self.expr(0)?);
                        if *self.peek() == Tok::Op(",") {
                            self.bump();
                            continue;
                        }
                        break;
                    }
                }
                self.expect("]")?;
                Ok(Expr::Vector(items))
            }
            Tok::Op("-") => Ok(Expr::Unary(UnOp::Neg, Box::new(self.expr(prec::NEG)?))),
            Tok::Op("+") => Ok(Expr::Unary(UnOp::Plus, Box::new(self.expr(prec::NEG)?))),
            Tok::Op("!") => Ok(Expr::Unary(UnOp::Not, Box::new(self.expr(prec::NOT)?))),
            Tok::Ident(name) => self.ident(name, pos),
            Tok::Sub(_) => Err(ParseError {
                pos,
                message: "a '.N' subcolumn needs a column before it".into(),
            }),
            Tok::End => Err(ParseError {
                pos,
                message: "the formula ends too early".into(),
            }),
            Tok::Op(o) => Err(ParseError {
                pos,
                message: format!("unexpected '{o}'"),
            }),
        }
    }

    fn ident(&mut self, name: String, pos: usize) -> Result<Expr, ParseError> {
        if *self.peek() == Tok::Op("(") {
            self.bump();
            let mut args = Vec::new();
            if *self.peek() != Tok::Op(")") {
                loop {
                    args.push(self.expr(0)?);
                    if *self.peek() == Tok::Op(",") {
                        self.bump();
                        continue;
                    }
                    break;
                }
            }
            self.expect(")")?;
            return Ok(Expr::Call(name.to_ascii_lowercase(), args));
        }
        let lower = name.to_ascii_lowercase();
        match lower.as_str() {
            "true" => return Ok(Expr::Bool(true)),
            "false" => return Ok(Expr::Bool(false)),
            "not" => return Ok(Expr::Unary(UnOp::Not, Box::new(self.expr(prec::NOT)?))),
            _ => {}
        }
        if name == "NA" {
            return Ok(Expr::Missing);
        }
        if name == "X" {
            let sub = self.sub_suffix();
            return Ok(Expr::Column(ColRef {
                target: ColTarget::X,
                sub,
            }));
        }
        if name.bytes().all(|c| c.is_ascii_uppercase()) {
            let sub = self.sub_suffix();
            return Ok(Expr::Column(ColRef {
                target: ColTarget::Letter(name),
                sub,
            }));
        }
        if let Some((l, r)) = split_cell(&name) {
            let target = if l == "X" {
                ColTarget::X
            } else {
                ColTarget::Letter(l.to_string())
            };
            return Ok(Expr::Cell(ColRef { target, sub: None }, r));
        }
        Err(ParseError {
            pos,
            message: format!(
                "unknown name '{name}': columns are capital letters (A, B, X) or `names in backticks`; functions need parentheses"
            ),
        })
    }
}

fn op_static(op: &str) -> &'static str {
    OPS.iter().find(|o| **o == op).copied().unwrap_or("?")
}
