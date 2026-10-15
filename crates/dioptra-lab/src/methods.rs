// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! The methods paragraph: each analysis's methods line (test, sidedness, correction for
//! multiple comparisons, post tests, CI level, data form), the software and version, and
//! the references to cite. References are the ones recorded in docs/METHODS.md for the
//! methods Dioptra implements, matched on the names the methods lines use.

use dioptra_core::api::{Command, Response, Session};
use serde::{Deserialize, Serialize};

use crate::journal::{ok, Targets};

/// Where the software can be found.
pub const SOFTWARE_URL: &str = "https://github.com/ZahakJ/dioptra";

/// Phrases in a methods line and the references they call for, as in docs/METHODS.md.
const REFERENCES: &[(&str, &[&str])] = &[
    ("t test", &["Student (1908)"]),
    (
        "Welch's unpaired t test",
        &["Welch (1947)", "Satterthwaite (1946)"],
    ),
    ("Welch's ANOVA", &["Welch (1951), doi:10.2307/2332579"]),
    (
        "Brown–Forsythe",
        &["Brown & Forsythe (1974), doi:10.2307/1267501"],
    ),
    ("one-way ANOVA", &["Fisher (1925)"]),
    ("Two-way ANOVA", &["Searle (1987)"]),
    (
        "Greenhouse–Geisser",
        &["Greenhouse & Geisser (1959), doi:10.1007/BF02289823"],
    ),
    (
        "Tukey",
        &["Tukey (1953)", "Kramer (1956), doi:10.2307/3001469"],
    ),
    (
        "Dunnett",
        &["Dunnett (1955), doi:10.1080/01621459.1955.10501294"],
    ),
    ("Holm", &["Holm (1979)"]),
    ("Šidák", &["Šidák (1967)"]),
    ("Hochberg", &["Hochberg (1988)"]),
    ("Benjamini", &["Benjamini & Hochberg (1995)"]),
    ("LSD", &["Fisher (1935)"]),
    ("Mann–Whitney", &["Mann & Whitney (1947)"]),
    ("Hodges–Lehmann", &["Hodges & Lehmann (1963)"]),
    ("Wilcoxon", &["Wilcoxon (1945)"]),
    (
        "Kruskal–Wallis",
        &["Kruskal & Wallis (1952), doi:10.1080/01621459.1952.10483441"],
    ),
    (
        "Friedman",
        &["Friedman (1937), doi:10.1080/01621459.1937.10503522"],
    ),
    (
        "Dunn's",
        &["Dunn (1964), doi:10.1080/00401706.1964.10490181"],
    ),
    ("Chi-square test", &["Pearson (1900)"]),
    ("Yates", &["Yates (1934)"]),
    ("Fisher's exact", &["Fisher (1935)", "Cornfield (1956)"]),
    (
        "Fisher–Freeman–Halton",
        &[
            "Freeman & Halton (1951)",
            "Mehta & Patel (1983), doi:10.1080/01621459.1983.10477989",
        ],
    ),
    ("Pearson correlation", &["Pearson (1896)", "Fisher (1921)"]),
    ("Spearman", &["Spearman (1904)"]),
    ("Kendall", &["Kendall (1945)"]),
    ("linear regression", &["Draper & Smith (1998)"]),
    ("runs test", &["Wald & Wolfowitz (1940)"]),
    (
        "Levenberg–Marquardt",
        &["Moré (1978), doi:10.1007/BFb0067700"],
    ),
    (
        "ROUT",
        &["Motulsky & Brown (2006), BMC Bioinformatics 7:123"],
    ),
    ("Grubbs", &["Grubbs (1969)"]),
    ("Shapiro–Wilk", &["Royston (1995), AS R94"]),
    ("D'Agostino", &["D'Agostino, Belanger & D'Agostino (1990)"]),
    ("Anderson–Darling", &["Anderson & Darling (1954)"]),
    (
        "Kaplan–Meier",
        &["Kaplan & Meier (1958)", "Greenwood (1926)"],
    ),
    ("log-rank", &["Mantel (1966)"]),
    ("Cox", &["Cox (1972)"]),
    (
        "ROC curve",
        &["DeLong, DeLong & Clarke-Pearson (1988), doi:10.2307/2531595"],
    ),
    ("Youden", &["Youden (1950)"]),
    ("Wilson", &["Wilson (1927)"]),
    ("Bland–Altman", &["Bland & Altman (1986), Lancet 327:307"]),
    ("Principal component", &["Jolliffe (2002)"]),
    (
        "Deming",
        &[
            "Deming (1943)",
            "Linnet (1993), doi:10.1093/clinchem/39.3.424",
        ],
    ),
];

/// A methods paragraph and what it cites.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Methods {
    /// The paragraph, ready to paste.
    pub text: String,
    /// References, in order of first mention.
    pub references: Vec<String>,
}

/// References a methods line calls for, in order of the table.
pub fn references_for(line: &str) -> Vec<&'static str> {
    let mut out = Vec::new();
    for (phrase, refs) in REFERENCES {
        if line.contains(phrase) {
            for r in *refs {
                if !out.contains(r) {
                    out.push(*r);
                }
            }
        }
    }
    out
}

/// A methods line without its closing "Dioptra x.y.z." and final full stop.
fn strip_version(line: &str) -> String {
    let l = line.trim();
    let l = match l.rfind(". Dioptra ") {
        Some(i) => &l[..i],
        None => l,
    };
    l.trim_end_matches('.').to_string()
}

/// The analyses in a scope, in project order. Graph ids stand for the analysis they draw.
pub fn analyses_in(session: &Session, scope: &Targets) -> Result<Vec<String>, String> {
    let p = session.project();
    let mut out: Vec<String> = Vec::new();
    match scope {
        Targets::Named(_) => {
            out.extend(
                p.order
                    .iter()
                    .filter(|id| p.analyses.contains_key(*id))
                    .cloned(),
            );
        }
        Targets::Ids(ids) => {
            for id in ids {
                let a = if p.analyses.contains_key(id) {
                    id.clone()
                } else if let Some(g) = p.graphs.get(id) {
                    if !p.analyses.contains_key(&g.spec.source) {
                        continue;
                    }
                    g.spec.source.clone()
                } else {
                    return Err(format!("no analysis or graph with id {id}"));
                };
                if !out.contains(&a) {
                    out.push(a);
                }
            }
        }
    }
    Ok(out)
}

/// Write the methods paragraph for a scope.
pub fn methods(session: &mut Session, scope: &Targets) -> Result<Methods, String> {
    session.require_lab("Journal figures")?;
    let ids = analyses_in(session, scope)?;
    if ids.is_empty() {
        return Err("There are no analyses to describe.".into());
    }
    let mut sentences = Vec::new();
    let mut references: Vec<String> = Vec::new();
    for id in &ids {
        let Response::Results(sheet) =
            ok(session.dispatch(Command::GetResults { id: id.clone() }))?
        else {
            continue;
        };
        let Some(line) = sheet.methods.as_deref() else {
            continue;
        };
        let p = session.project();
        let table = p
            .analyses
            .get(id)
            .and_then(|a| p.tables.get(&a.table))
            .map(|t| t.title.clone());
        let body = strip_version(line);
        sentences.push(format!("{}: {body}.", table.unwrap_or(sheet.title)));
        for r in references_for(line) {
            if !references.iter().any(|x| x == r) {
                references.push(r.to_string());
            }
        }
    }
    if sentences.is_empty() {
        return Err("None of the analyses has results to describe.".into());
    }
    let version = env!("CARGO_PKG_VERSION");
    let mut text = sentences.join(" ");
    text.push_str(&format!(
        " Analyses were performed in Dioptra {version} ({SOFTWARE_URL})."
    ));
    if !references.is_empty() {
        text.push_str("\n\nReferences: ");
        text.push_str(&references.join("; "));
        text.push('.');
    }
    Ok(Methods { text, references })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal_testkit::{column_table, licensed, ok as run};
    use serde_json::json;

    fn analysis(s: &mut Session, t: &str, kind: &str, patch: serde_json::Value) -> String {
        let mut spec = run(
            s,
            json!({"type": "default_analysis_spec", "table": t, "kind": kind}),
        );
        spec.as_object_mut().unwrap().remove("type");
        dioptra_core::merge_patch(&mut spec, &patch);
        run(
            s,
            json!({"type": "create_analysis", "table": t, "spec": spec}),
        )["changed"]["created"]
            .as_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn t_test_and_anova_with_tukey() {
        let mut s = Session::new();
        let t = column_table(&mut s);
        let tt = analysis(&mut s, &t, "t_test", json!({}));
        assert!(methods(&mut s, &Targets::default())
            .unwrap_err()
            .contains("Dioptra Lab"));
        licensed(&mut s, true);
        let m = methods(&mut s, &Targets::Ids(vec![tt.clone()])).unwrap();
        assert!(
            m.text
                .contains("Welch's unpaired t test, two-sided, 95% CI"),
            "{}",
            m.text
        );
        assert!(m.text.starts_with("Plasma IL-6: Welch"), "{}", m.text);
        assert!(m
            .text
            .contains(&format!("Dioptra {}", env!("CARGO_PKG_VERSION"))));
        assert_eq!(
            m.references,
            vec!["Student (1908)", "Welch (1947)", "Satterthwaite (1946)"]
        );

        let anova = analysis(
            &mut s,
            &t,
            "one_way_anova",
            json!({"post_hoc": {"kind": "tukey"}}),
        );
        let m = methods(&mut s, &Targets::Ids(vec![anova])).unwrap();
        assert!(m.text.contains("Ordinary one-way ANOVA"), "{}", m.text);
        assert!(
            m.text.contains("Tukey's multiple comparisons"),
            "{}",
            m.text
        );
        assert!(m.text.contains("family-wise α = 0.05"), "{}", m.text);
        assert_eq!(
            m.references,
            vec![
                "Fisher (1925)",
                "Tukey (1953)",
                "Kramer (1956), doi:10.2307/3001469"
            ]
        );
        // The whole project: both, in order, each reference once.
        let m = methods(&mut s, &Targets::default()).unwrap();
        assert!(m.text.find("Welch").unwrap() < m.text.find("ANOVA").unwrap());
        assert_eq!(m.references.len(), 6);
        licensed(&mut s, false);
        assert!(methods(&mut s, &Targets::default()).is_err());
    }

    #[test]
    fn references_follow_the_names() {
        assert!(references_for(
            "Kruskal–Wallis test, Dunn's multiple comparisons with Holm adjustment"
        )
        .contains(&"Holm (1979)"));
        assert!(references_for("Descriptive statistics").is_empty());
        assert_eq!(
            strip_version("Paired t test, two-sided. Dioptra 0.3.0."),
            "Paired t test, two-sided"
        );
    }
}
