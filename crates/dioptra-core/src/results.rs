//! Results sheets: structured, typed output of an analysis (DESIGN §4.3).
//!
//! Numbers keep full precision and carry their display decimals; p values are stored raw
//! and formatted at display time by the project's [`PValueStyle`].

use serde::{Deserialize, Serialize};

use crate::api::PValueStyle;
use crate::util::{format_sig, nan_f64};

/// Status of one assumption check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    /// Checked and fine.
    Pass,
    /// Checked and in doubt.
    Caution,
    /// Checked and violated.
    Fail,
    /// Cannot be checked from the data; the user must judge.
    NotChecked,
}

impl CheckStatus {
    /// Plain label.
    pub fn label(self) -> &'static str {
        match self {
            CheckStatus::Pass => "pass",
            CheckStatus::Caution => "caution",
            CheckStatus::Fail => "fail",
            CheckStatus::NotChecked => "not checked",
        }
    }
}

/// One assumption and what was found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssumptionCheck {
    /// Name, e.g. "Normality of residuals".
    pub name: String,
    /// Status.
    pub status: CheckStatus,
    /// What was checked and what it means for this result.
    pub detail: String,
}

/// One cell of a results table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultCell {
    /// A number with optional fixed decimals (`None`: four significant digits).
    Number {
        /// Value (full precision).
        #[serde(with = "nan_f64")]
        v: f64,
        /// Display decimals.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        decimals: Option<u8>,
    },
    /// A p value, formatted by the project style.
    PValue {
        /// Value.
        #[serde(with = "nan_f64")]
        p: f64,
    },
    /// Text.
    Text(String),
    /// Nothing.
    Empty,
}

impl ResultCell {
    /// A number cell with automatic display.
    pub fn num(v: f64) -> Self {
        if v.is_finite() {
            ResultCell::Number { v, decimals: None }
        } else {
            ResultCell::Empty
        }
    }

    /// An integer-like count.
    pub fn count(n: usize) -> Self {
        ResultCell::Number {
            v: n as f64,
            decimals: Some(0),
        }
    }

    /// A text cell.
    pub fn text(s: impl Into<String>) -> Self {
        ResultCell::Text(s.into())
    }

    /// Display string under a p-value style.
    pub fn display(&self, style: PValueStyle) -> String {
        match self {
            ResultCell::Number { v, decimals } => format_sig(*v, *decimals, 4),
            ResultCell::PValue { p } => format_p(*p, style),
            ResultCell::Text(s) => s.clone(),
            ResultCell::Empty => String::new(),
        }
    }
}

/// A table of results.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ResultTable {
    /// Column headings.
    pub columns: Vec<String>,
    /// Rows.
    pub rows: Vec<Vec<ResultCell>>,
}

/// A titled section of a results sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    /// Title.
    pub title: String,
    /// Its table.
    pub table: ResultTable,
}

/// The output of an analysis.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ResultsSheet {
    /// Title.
    pub title: String,
    /// Analysis kind name (as in `ListAnalyses`).
    pub analysis_kind: String,
    /// A message instead of (or before) results, e.g. why the analysis could not run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Typed tables.
    pub sections: Vec<Section>,
    /// Assumption checks, shown first.
    pub assumptions: Vec<AssumptionCheck>,
    /// APA-style summary sentences.
    pub summary: Vec<String>,
    /// What this analysis can and cannot tell you.
    pub checklist: Vec<String>,
    /// Whether the data changed since this was computed.
    #[serde(default)]
    pub stale: bool,
}

/// GraphPad asterisks: `****` < .0001, `***` < .001, `**` < .01, `*` < .05, else `ns`.
/// Decided on the stored p, not the displayed one.
pub fn stars(p: f64) -> &'static str {
    if p < 0.0001 {
        "****"
    } else if p < 0.001 {
        "***"
    } else if p < 0.01 {
        "**"
    } else if p < 0.05 {
        "*"
    } else {
        "ns"
    }
}

fn strip_zero(s: String) -> String {
    s.strip_prefix("0.").map(|r| format!(".{r}")).unwrap_or(s)
}

/// A p value for a table cell.
///
/// - APA: `.005`, `< .001`, `> .999`
/// - NEJM: `0.005`, `<0.001`
/// - GP: `0.0049 (**)`, `<0.0001 (****)`, `0.2345 (ns)`
/// - Exact: four significant digits, `< 0.0001` below that
pub fn format_p(p: f64, style: PValueStyle) -> String {
    if !p.is_finite() {
        return String::new();
    }
    match style {
        PValueStyle::Apa => {
            if p < 0.001 {
                "< .001".into()
            } else if p > 0.999 {
                "> .999".into()
            } else {
                strip_zero(format!("{p:.3}"))
            }
        }
        PValueStyle::Nejm => {
            if p < 0.001 {
                "<0.001".into()
            } else if p > 0.999 {
                ">0.999".into()
            } else {
                format!("{p:.3}")
            }
        }
        PValueStyle::Gp => {
            let v = if p < 0.0001 {
                "<0.0001".to_string()
            } else if p > 0.9999 {
                ">0.9999".into()
            } else {
                format!("{p:.4}")
            };
            format!("{v} ({})", stars(p))
        }
        PValueStyle::Exact => {
            if p < 0.0001 {
                "< 0.0001".into()
            } else {
                let s = format!("{:.*e}", 3, p);
                let v: f64 = s.parse().unwrap_or(p);
                format_sig(v, None, 4)
            }
        }
    }
}

/// A p value inside a sentence: `p = .005`, `P=0.005`, `P = 0.0049`.
pub fn p_phrase(p: f64, style: PValueStyle) -> String {
    let cell = format_p(p, style);
    match style {
        PValueStyle::Apa => {
            if cell.starts_with(['<', '>']) {
                format!("p {cell}")
            } else {
                format!("p = {cell}")
            }
        }
        PValueStyle::Nejm => {
            if cell.starts_with(['<', '>']) {
                format!("P{cell}")
            } else {
                format!("P={cell}")
            }
        }
        PValueStyle::Gp => {
            let v = cell.split(' ').next().unwrap_or("").to_string();
            if v.starts_with(['<', '>']) {
                format!("P {v}")
            } else {
                format!("P = {v}")
            }
        }
        PValueStyle::Exact => {
            if cell.starts_with(['<', '>']) {
                format!("p {cell}")
            } else {
                format!("p = {cell}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p_styles() {
        assert_eq!(format_p(0.0049, PValueStyle::Apa), ".005");
        assert_eq!(p_phrase(0.0049, PValueStyle::Apa), "p = .005");
        assert_eq!(p_phrase(0.0004, PValueStyle::Apa), "p < .001");
        assert_eq!(format_p(0.0049, PValueStyle::Nejm), "0.005");
        assert_eq!(p_phrase(0.0004, PValueStyle::Nejm), "P<0.001");
        assert_eq!(p_phrase(0.0049, PValueStyle::Nejm), "P=0.005");
        assert_eq!(format_p(0.0049, PValueStyle::Gp), "0.0049 (**)");
        assert_eq!(format_p(0.00001, PValueStyle::Gp), "<0.0001 (****)");
        assert_eq!(format_p(0.05000001, PValueStyle::Gp), "0.0500 (ns)");
        assert_eq!(format_p(0.0004, PValueStyle::Gp), "0.0004 (***)");
        assert_eq!(format_p(0.049, PValueStyle::Gp), "0.0490 (*)");
        assert_eq!(format_p(0.004921, PValueStyle::Exact), "0.004921");
        assert_eq!(format_p(0.123456, PValueStyle::Exact), "0.1235");
        assert_eq!(format_p(0.00005, PValueStyle::Exact), "< 0.0001");
        assert_eq!(p_phrase(0.00005, PValueStyle::Exact), "p < 0.0001");
        assert_eq!(stars(0.05), "ns");
    }

    #[test]
    fn cells_serialise() {
        let c = vec![
            ResultCell::num(1.5),
            ResultCell::PValue { p: 0.01 },
            ResultCell::text("a"),
            ResultCell::Empty,
        ];
        let j = serde_json::to_string(&c).unwrap();
        assert_eq!(
            j,
            r#"[{"number":{"v":1.5}},{"p_value":{"p":0.01}},{"text":"a"},"empty"]"#
        );
        let back: Vec<ResultCell> = serde_json::from_str(&j).unwrap();
        assert_eq!(back, c);
    }
}
