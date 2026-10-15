//! `dioptra` command line: inspect project files, recompute and print results sheets,
//! import data, run one analysis on a file, render graphs, export tables, measure the
//! engine, and list the equation library. With a Dioptra Lab license, apply a template project
//! to many data files (`batch`).
//!
//! Every subcommand drives the same `Session` the desktop app and the browser build use,
//! through the same JSON commands, so what the CLI prints is what the app shows.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use dioptra_core::api::Session;
use serde_json::{json, Value};

/// `println!` that ends quietly when stdout is closed (`dioptra equations | head`).
macro_rules! outln {
    () => { outln!("") };
    ($($t:tt)*) => {{
        use std::io::Write;
        if let Err(e) = writeln!(std::io::stdout(), $($t)*) {
            if e.kind() == std::io::ErrorKind::BrokenPipe {
                std::process::exit(0);
            }
        }
    }};
}

/// `print!` that ends quietly when stdout is closed.
macro_rules! out {
    ($($t:tt)*) => {{
        use std::io::Write;
        if let Err(e) = write!(std::io::stdout(), $($t)*) {
            if e.kind() == std::io::ErrorKind::BrokenPipe {
                std::process::exit(0);
            }
        }
    }};
}

#[derive(Parser)]
#[command(
    name = "dioptra",
    version,
    about = "Dioptra from the command line: inspect projects, run analyses, render graphs",
    long_about = "Dioptra from the command line. Inspect a .dioptra project, recompute its analyses \
and print the results sheets, import data into a new project, run one analysis on a data file, \
render a graph to SVG, PDF or PNG, export a table, measure the engine, or list the equations \
nonlinear regression can fit.",
    after_help = "Examples:\n  dioptra info study.dioptra\n  dioptra run study.dioptra --format markdown\n  \
dioptra analyze plasma.csv --kind column --test one_way_anova\n  \
dioptra render study.dioptra --graph \"Dose response\" --out fig1.pdf\n  \
dioptra import plate.xlsx --kind xy --out plate.dioptra\n  \
dioptra batch assay.dioptra --table Plate exports/ --out results --format png"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show the objects in a project: tables, their analyses and graphs, layouts.
    Info {
        /// Project file (.dioptra).
        file: PathBuf,
    },
    /// Recompute a project and print its results sheets.
    Run {
        /// Project file (.dioptra).
        file: PathBuf,
        /// Only this analysis (id or title).
        #[arg(long)]
        analysis: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = SheetFormat::Tsv)]
        format: SheetFormat,
    },
    /// Import a data file (csv, tsv, txt, xlsx, xls, ods, pzfx, prism) into a new project file.
    Import {
        /// Data file.
        data: PathBuf,
        /// Table kind for delimited text and spreadsheets.
        #[arg(long, value_enum)]
        kind: Option<Kind>,
        /// Project file to write (default: the data file's name with .dioptra, beside it).
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Import a data file, run one analysis with its default options (plus --options) and
    /// print the results sheet.
    Analyze {
        /// Data file.
        data: PathBuf,
        /// Table kind of the data.
        #[arg(long, value_enum)]
        kind: Kind,
        /// Analysis kind, e.g. t_test, one_way_anova, survival, nonlinear_regression.
        #[arg(long)]
        test: String,
        /// Options as a JSON object merged into the defaults, e.g. '{"post_hoc": {"method": {"name": "holm"}}}'.
        #[arg(long)]
        options: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = SheetFormat::Tsv)]
        format: SheetFormat,
        /// Also save the project (data and analysis) to this file.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Render a graph (or a layout) to SVG, PDF or PNG; the format follows the extension.
    Render {
        /// Project file (.dioptra).
        file: PathBuf,
        /// Graph or layout (id or title).
        #[arg(long)]
        graph: String,
        /// Output file: .svg, .pdf or .png.
        #[arg(long)]
        out: PathBuf,
        /// Resolution for PNG.
        #[arg(long, default_value_t = 300.0)]
        dpi: f64,
        /// Width in millimetres (default: the graph's own).
        #[arg(long)]
        width_mm: Option<f64>,
        /// Height in millimetres (default: the graph's own).
        #[arg(long)]
        height_mm: Option<f64>,
    },
    /// Export a table to CSV, TSV or XLSX; the format follows the extension.
    Export {
        /// Project file (.dioptra).
        file: PathBuf,
        /// Table (id or title).
        #[arg(long)]
        table: String,
        /// Output file: .csv, .tsv or .xlsx.
        #[arg(long)]
        out: PathBuf,
    },
    /// Measure the engine: paste, formula recompute, graph rendering, export.
    Bench {
        /// Print the table as Markdown.
        #[arg(long)]
        markdown: bool,
    },
    /// List the built-in equations of nonlinear regression with their parameters.
    Equations {
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Apply a template project to many data files (Dioptra Lab): one recomputed project,
    /// the graphs and a row of summary.csv per file.
    Batch {
        /// Template project (.dioptra).
        template: PathBuf,
        /// The table that receives each file's data (title or id).
        #[arg(long)]
        table: String,
        /// Data files, or folders of data files.
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        /// Output folder.
        #[arg(long)]
        out: PathBuf,
        /// Figure format.
        #[arg(long, value_enum, default_value_t = FigureFormat::Pdf)]
        format: FigureFormat,
        /// Resolution for PNG.
        #[arg(long, default_value_t = 300.0)]
        dpi: f64,
        /// Plate map for plate-reader files (a grid or a well,label list).
        #[arg(long)]
        plate_map: Option<PathBuf>,
    },
    /// Convert Prism files to Dioptra projects, or Dioptra projects to Prism files
    /// (Dioptra Lab): .pzfx and .prism become .dioptra; .dioptra becomes .pzfx.
    Convert {
        /// Files, or folders of files.
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        /// Output folder.
        #[arg(long)]
        out: PathBuf,
    },
    /// Run the operational qualification (Dioptra Lab): every NIST StRD and reference-library
    /// check on this installation. Exits 0 when all pass.
    Oq {
        /// Write the report here: .pdf or .csv.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Show, set or remove the Dioptra Lab license key (shared with the desktop app;
    /// $DIOPTRA_LICENSE takes precedence).
    License {
        /// Store this key.
        #[arg(long, conflicts_with = "remove")]
        set: Option<String>,
        /// Remove the stored key.
        #[arg(long)]
        remove: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum FigureFormat {
    Pdf,
    Png,
    Svg,
}

#[derive(Clone, Copy, ValueEnum)]
enum SheetFormat {
    Tsv,
    Markdown,
    Json,
}

#[derive(Clone, Copy, ValueEnum)]
enum Kind {
    Xy,
    Column,
    Grouped,
    Contingency,
    Survival,
    Parts,
    Variables,
    Nested,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Xy => "xy",
            Kind::Column => "column",
            Kind::Grouped => "grouped",
            Kind::Contingency => "contingency",
            Kind::Survival => "survival",
            Kind::Parts => "parts",
            Kind::Variables => "variables",
            Kind::Nested => "nested",
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("dioptra: error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Info { file } => info(&file),
        Cmd::Run {
            file,
            analysis,
            format,
        } => run_project(&file, analysis.as_deref(), format),
        Cmd::Import { data, kind, out } => import(&data, kind, out),
        Cmd::Analyze {
            data,
            kind,
            test,
            options,
            format,
            out,
        } => analyze(&data, kind, &test, options.as_deref(), format, out),
        Cmd::Render {
            file,
            graph,
            out,
            dpi,
            width_mm,
            height_mm,
        } => render(&file, &graph, &out, dpi, width_mm, height_mm),
        Cmd::Export { file, table, out } => export(&file, &table, &out),
        Cmd::Bench { markdown } => bench(markdown),
        Cmd::Equations { json } => equations(json),
        Cmd::Batch {
            template,
            table,
            inputs,
            out,
            format,
            dpi,
            plate_map,
        } => batch(
            &template,
            &table,
            &inputs,
            &out,
            format,
            dpi,
            plate_map.as_deref(),
        ),
        Cmd::Convert { inputs, out } => convert(&inputs, &out),
        Cmd::Oq { out } => oq(out.as_deref()),
        Cmd::License { set, remove } => license(set.as_deref(), remove),
    }
}

// ---- the engine ---------------------------------------------------------------------------

/// One project open in a session.
struct Engine {
    s: Session,
}

/// An object of the project tree.
#[derive(Clone)]
struct Item {
    id: String,
    kind: String,
    title: String,
    derived_from: Option<String>,
    subtype: Option<String>,
}

fn abs(p: &Path) -> Result<String> {
    let p = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .context("cannot read the current directory")?
            .join(p)
    };
    Ok(p.to_string_lossy().into_owned())
}

impl Engine {
    fn new() -> Self {
        Engine { s: Session::new() }
    }

    /// Open a project file.
    fn open(file: &Path) -> Result<Self> {
        if !file.is_file() {
            bail!("no such project file: {}", file.display());
        }
        let mut e = Engine::new();
        e.call(json!({"type": "open_project_path", "path": abs(file)?}))
            .with_context(|| format!("cannot open {}", file.display()))?;
        Ok(e)
    }

    /// Send one command; an engine error becomes an `Err`.
    fn call(&mut self, cmd: Value) -> Result<Value> {
        let out = dioptra_lab::dispatch_json(&mut self.s, &cmd.to_string());
        let v: Value = serde_json::from_str(&out).context("the engine answered with bad JSON")?;
        if v["type"] == "error" {
            bail!(
                "{}",
                v["message"].as_str().unwrap_or("unknown engine error")
            );
        }
        Ok(v)
    }

    fn items(&mut self) -> Result<Vec<Item>> {
        let p = self.call(json!({"type": "get_project"}))?;
        Ok(p["items"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|i| Item {
                        id: i["id"].as_str().unwrap_or_default().into(),
                        kind: i["kind"].as_str().unwrap_or_default().into(),
                        title: i["title"].as_str().unwrap_or_default().into(),
                        derived_from: i["derived_from"].as_str().map(String::from),
                        subtype: i["subtype"].as_str().map(String::from),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Find an object of one of `kinds` by id, exact title, or case-insensitive title.
    fn find(&mut self, kinds: &[&str], key: &str) -> Result<Item> {
        let items: Vec<Item> = self
            .items()?
            .into_iter()
            .filter(|i| kinds.contains(&i.kind.as_str()))
            .collect();
        if let Some(i) = items.iter().find(|i| i.id == key || i.title == key) {
            return Ok(i.clone());
        }
        let loose: Vec<&Item> = items
            .iter()
            .filter(|i| i.title.eq_ignore_ascii_case(key.trim()))
            .collect();
        if loose.len() == 1 {
            return Ok(loose[0].clone());
        }
        let what = kinds.join(" or ");
        if items.is_empty() {
            bail!("there is no {what} called \"{key}\": the project has none");
        }
        let names: Vec<String> = items.iter().map(|i| format!("\"{}\"", i.title)).collect();
        bail!(
            "there is no {what} called \"{key}\"; the project has {}",
            names.join(", ")
        )
    }
}

fn strip_type(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.remove("type");
    }
    v
}

// ---- info ---------------------------------------------------------------------------------

fn info(file: &Path) -> Result<()> {
    let mut e = Engine::open(file)?;
    let p = e.call(json!({"type": "get_project"}))?;
    let items = e.items()?;
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let title = p["title"]
        .as_str()
        .filter(|t| !t.is_empty())
        .unwrap_or(&stem);
    outln!("{title}  ({})", file.display());
    let count = |k: &str| items.iter().filter(|i| i.kind == k).count();
    outln!(
        "{} tables, {} analyses, {} graphs, {} layouts; p values in {} style",
        count("table"),
        count("analysis"),
        count("graph"),
        count("layout"),
        p["p_value_style"].as_str().unwrap_or("apa").to_uppercase()
    );
    let roots: Vec<&Item> = items
        .iter()
        .filter(|i| i.kind == "table" && i.derived_from.is_none())
        .collect();
    let mut lines = Vec::new();
    for t in roots {
        tree(&mut e, &items, t, 0, &mut lines)?;
    }
    // Graphs and derived objects whose source is gone stay visible.
    let shown: Vec<String> = lines.iter().map(|(id, _)| id.clone()).collect();
    for i in items.iter().filter(|i| i.kind != "layout") {
        if !shown.contains(&i.id) {
            tree(&mut e, &items, i, 0, &mut lines)?;
        }
    }
    for l in items.iter().filter(|i| i.kind == "layout") {
        let v = e.call(json!({"type": "get_layout", "id": l.id}))?;
        let graphs: Vec<String> = v["graphs"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|g| g.as_str())
                    .map(|g| {
                        items
                            .iter()
                            .find(|i| i.id == g)
                            .map_or(g.to_string(), |i| i.title.clone())
                    })
                    .collect()
            })
            .unwrap_or_default();
        lines.push((
            l.id.clone(),
            format!(
                "layout    {}  [{} panels, {} columns: {}]  {}",
                l.title,
                graphs.len(),
                v["columns"],
                graphs.join(", "),
                l.id
            ),
        ));
    }
    for (_, l) in lines {
        outln!("{l}");
    }
    Ok(())
}

fn tree(
    e: &mut Engine,
    items: &[Item],
    it: &Item,
    depth: usize,
    out: &mut Vec<(String, String)>,
) -> Result<()> {
    let pad = "  ".repeat(depth);
    let sub = it.subtype.as_deref().unwrap_or("");
    let detail = if it.kind == "table" {
        let m = e.call(json!({"type": "get_table_meta", "table": it.id}))?;
        let ds = m["datasets"].as_array().map_or(0, Vec::len);
        let derived = if it.derived_from.is_some() {
            ", derived"
        } else {
            ""
        };
        format!(
            "[{sub}, {ds} data set{}, {} rows{derived}]",
            if ds == 1 { "" } else { "s" },
            m["rows"]
        )
    } else {
        format!("[{sub}]")
    };
    out.push((
        it.id.clone(),
        format!("{pad}{:<9} {}  {detail}  {}", it.kind, it.title, it.id),
    ));
    for c in items
        .iter()
        .filter(|c| c.derived_from.as_deref() == Some(it.id.as_str()))
    {
        tree(e, items, c, depth + 1, out)?;
    }
    Ok(())
}

// ---- run ----------------------------------------------------------------------------------

fn print_sheet(e: &mut Engine, id: &str, format: SheetFormat) -> Result<bool> {
    let sheet = e.call(json!({"type": "get_results", "id": id}))?;
    let failed =
        sheet["message"].is_string() && sheet["sections"].as_array().is_none_or(|s| s.is_empty());
    match format {
        SheetFormat::Json => {}
        SheetFormat::Tsv | SheetFormat::Markdown => {
            let f = if matches!(format, SheetFormat::Tsv) {
                "tsv"
            } else {
                "markdown"
            };
            let t = e.call(json!({"type": "export_results", "id": id, "format": f}))?;
            out!("{}", t["text"].as_str().unwrap_or_default());
            if !t["text"].as_str().unwrap_or_default().ends_with('\n') {
                outln!();
            }
        }
    }
    if failed {
        eprintln!(
            "dioptra: \"{}\" could not run: {}",
            sheet["title"].as_str().unwrap_or("analysis"),
            sheet["message"].as_str().unwrap_or_default()
        );
    }
    Ok(!failed)
}

fn run_project(file: &Path, analysis: Option<&str>, format: SheetFormat) -> Result<()> {
    let mut e = Engine::open(file)?;
    if let Err(err) = e.call(json!({"type": "recompute_all"})) {
        eprintln!("dioptra: warning: {err}");
    }
    let targets: Vec<Item> = match analysis {
        Some(key) => vec![e.find(&["analysis"], key)?],
        None => e
            .items()?
            .into_iter()
            .filter(|i| i.kind == "analysis")
            .collect(),
    };
    if targets.is_empty() {
        bail!("{} has no analyses to run", file.display());
    }
    let mut ok = true;
    let mut sheets = Vec::new();
    for (n, a) in targets.iter().enumerate() {
        if matches!(format, SheetFormat::Json) {
            let s = e.call(json!({"type": "get_results", "id": a.id}))?;
            ok &= !(s["message"].is_string()
                && s["sections"].as_array().is_none_or(|x| x.is_empty()));
            let mut s = strip_type(s);
            s["id"] = json!(a.id);
            sheets.push(s);
        } else {
            if n > 0 {
                outln!();
            }
            ok &= print_sheet(&mut e, &a.id, format)?;
        }
    }
    if matches!(format, SheetFormat::Json) {
        outln!("{}", serde_json::to_string_pretty(&sheets)?);
    }
    if !ok {
        bail!("one or more analyses could not run");
    }
    Ok(())
}

// ---- import / analyze ---------------------------------------------------------------------

fn import_into(e: &mut Engine, data: &Path, kind: Option<Kind>) -> Result<(String, Value)> {
    if !data.is_file() {
        bail!("no such data file: {}", data.display());
    }
    let r = e
        .call(json!({
            "type": "import_path",
            "path": abs(data)?,
            "kind": kind.map(Kind::name),
        }))
        .with_context(|| format!("cannot import {}", data.display()))?;
    let id = r["changed"]["created"]
        .as_str()
        .ok_or_else(|| anyhow!("{} held no data to import", data.display()))?
        .to_string();
    Ok((id, r))
}

fn report_line(r: &Value) -> String {
    let mut s = format!(
        "{} rows × {} columns, {} numbers, {} text cells, {} missing",
        r["rows"], r["cols"], r["numeric_cells"], r["text_cells"], r["missing_cells"]
    );
    if let Some(notes) = r["notes"].as_array() {
        for n in notes.iter().filter_map(|n| n.as_str()) {
            s.push_str("\n  ");
            s.push_str(n);
        }
    }
    s
}

fn import(data: &Path, kind: Option<Kind>, out: Option<PathBuf>) -> Result<()> {
    let mut e = Engine::new();
    let (_, report) = import_into(&mut e, data, kind)?;
    let out = out.unwrap_or_else(|| data.with_extension("dioptra"));
    if out.extension().and_then(|x| x.to_str()) != Some("dioptra") {
        bail!("the project file must end in .dioptra: {}", out.display());
    }
    e.call(json!({"type": "save_project_path", "path": abs(&out)?}))?;
    let tables = e.items()?.into_iter().filter(|i| i.kind == "table").count();
    eprintln!("Imported {}: {}", data.display(), report_line(&report));
    outln!(
        "Wrote {} ({tables} table{}).",
        out.display(),
        if tables == 1 { "" } else { "s" }
    );
    Ok(())
}

fn analyze(
    data: &Path,
    kind: Kind,
    test: &str,
    options: Option<&str>,
    format: SheetFormat,
    out: Option<PathBuf>,
) -> Result<()> {
    let mut e = Engine::new();
    let (table, report) = import_into(&mut e, data, Some(kind))?;
    eprintln!("Imported {}: {}", data.display(), report_line(&report));
    let fits = e.call(json!({"type": "list_analyses", "table": table, "range": null}))?;
    if let Some(a) = fits["analyses"]
        .as_array()
        .and_then(|a| a.iter().find(|a| a["kind"] == test))
    {
        if a["fits"] == false {
            bail!(
                "{} does not fit this table: {}",
                a["name"].as_str().unwrap_or(test),
                a["reason"].as_str().unwrap_or("no reason given")
            );
        }
    } else {
        let names: Vec<&str> = fits["analyses"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x["kind"].as_str()).collect())
            .unwrap_or_default();
        bail!(
            "unknown analysis \"{test}\"; the kinds are: {}",
            names.join(", ")
        );
    }
    let mut spec =
        strip_type(e.call(json!({"type": "default_analysis_spec", "table": table, "kind": test}))?);
    let known: Vec<String> = spec
        .as_object()
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    if let Some(o) = options {
        let patch: Value = serde_json::from_str(o).context("--options is not valid JSON")?;
        if !patch.is_object() {
            bail!("--options must be a JSON object, e.g. '{{\"ci_level\": 0.9}}'");
        }
        dioptra_core::merge_patch(&mut spec, &patch);
        spec["kind"] = json!(test);
    }
    let a = e
        .call(json!({"type": "create_analysis", "table": table, "spec": spec}))
        .context("the options were not accepted")?["changed"]["created"]
        .as_str()
        .ok_or_else(|| anyhow!("the analysis was not created"))?
        .to_string();
    // Unknown option names would be dropped silently: name them instead.
    if let Some(o) = options.and_then(|o| serde_json::from_str::<Value>(o).ok()) {
        let stored = e.call(json!({"type": "get_analysis_spec", "id": a}))?;
        for k in o.as_object().into_iter().flat_map(|m| m.keys()) {
            if k != "kind" && stored.get(k).is_none() && !known.contains(k) {
                bail!(
                    "unknown option \"{k}\" for {test}; the options are: {}",
                    known
                        .iter()
                        .filter(|x| *x != "kind")
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
    }
    let ok = if matches!(format, SheetFormat::Json) {
        let s = e.call(json!({"type": "get_results", "id": a}))?;
        let ok =
            !(s["message"].is_string() && s["sections"].as_array().is_none_or(|x| x.is_empty()));
        outln!("{}", serde_json::to_string_pretty(&strip_type(s))?);
        ok
    } else {
        print_sheet(&mut e, &a, format)?
    };
    if let Some(out) = out {
        e.call(json!({"type": "save_project_path", "path": abs(&out)?}))?;
        eprintln!("Wrote {}.", out.display());
    }
    if !ok {
        bail!("the analysis could not run");
    }
    Ok(())
}

// ---- render / export ----------------------------------------------------------------------

fn extension(p: &Path) -> String {
    p.extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn render(
    file: &Path,
    key: &str,
    out: &Path,
    dpi: f64,
    width_mm: Option<f64>,
    height_mm: Option<f64>,
) -> Result<()> {
    let format = match extension(out).as_str() {
        "svg" => json!({"format": "svg"}),
        "pdf" => json!({"format": "pdf"}),
        "png" => {
            if !(dpi.is_finite() && (10.0..=2400.0).contains(&dpi)) {
                bail!("--dpi must be between 10 and 2400 (got {dpi})");
            }
            json!({"format": "png", "dpi": dpi})
        }
        other => bail!("cannot tell the format from \".{other}\": use .svg, .pdf or .png"),
    };
    for v in [width_mm, height_mm].into_iter().flatten() {
        if !(v.is_finite() && (5.0..=2000.0).contains(&v)) {
            bail!("sizes must be between 5 and 2000 mm (got {v})");
        }
    }
    let mut e = Engine::open(file)?;
    let it = e.find(&["graph", "layout"], key)?;
    let path = abs(out)?;
    if it.kind == "layout" {
        if width_mm.is_some() || height_mm.is_some() {
            bail!("a layout takes its size from its graphs; --width-mm and --height-mm apply to graphs");
        }
        e.call(json!({"type": "export_layout_path", "id": it.id, "format": format, "path": path}))?;
    } else {
        let size = if width_mm.is_some() || height_mm.is_some() {
            let spec = e.call(json!({"type": "get_graph_spec", "id": it.id}))?;
            let s = &spec["style"]["size_mm"];
            json!({
                "width": width_mm.or(s["width"].as_f64()).unwrap_or(85.0),
                "height": height_mm.or(s["height"].as_f64()).unwrap_or(70.0),
            })
        } else {
            Value::Null
        };
        e.call(json!({"type": "export_graph_path", "id": it.id, "format": format, "size": size, "path": path}))?;
    }
    let bytes = std::fs::metadata(out).map(|m| m.len()).unwrap_or(0);
    outln!("Wrote {} ({}, {bytes} bytes).", out.display(), it.title);
    Ok(())
}

fn export(file: &Path, key: &str, out: &Path) -> Result<()> {
    let format = match extension(out).as_str() {
        "csv" => "csv",
        "tsv" | "tab" | "txt" => "tsv",
        "xlsx" => "xlsx",
        other => bail!("cannot tell the format from \".{other}\": use .csv, .tsv or .xlsx"),
    };
    let mut e = Engine::open(file)?;
    let t = e.find(&["table"], key)?;
    e.call(
        json!({"type": "export_table_path", "table": t.id, "format": format, "path": abs(out)?}),
    )?;
    outln!("Wrote {} (table \"{}\").", out.display(), t.title);
    Ok(())
}

// ---- bench --------------------------------------------------------------------------------

fn grid_text(rows: usize, cols: usize) -> String {
    let mut s = String::with_capacity(rows * cols * 9);
    for r in 0..rows {
        for c in 0..cols {
            if c > 0 {
                s.push('\t');
            }
            let v = (r * cols + c) as f64 * 0.731 + 0.5;
            s.push_str(&format!("{v:.3}"));
        }
        s.push('\n');
    }
    s
}

/// Time `f` `runs` times and return the median in milliseconds.
fn median_ms(runs: usize, mut f: impl FnMut() -> Result<()>) -> Result<f64> {
    let mut t = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t0 = Instant::now();
        f()?;
        t.push(t0.elapsed().as_secs_f64() * 1e3);
    }
    t.sort_by(f64::total_cmp);
    Ok(t[t.len() / 2])
}

fn created(v: &Value) -> Result<String> {
    v["changed"]["created"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow!("nothing was created"))
}

fn kb(v: &Value) -> f64 {
    // Base64 carries 3 bytes in 4 characters.
    v["base64"]
        .as_str()
        .map_or(0.0, |s| s.len() as f64 * 0.75 / 1e3)
}

fn machine() -> String {
    let cpu = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split_once(':'))
                .map(|(_, v)| v.trim().to_string())
        })
        .unwrap_or_else(|| std::env::consts::ARCH.to_string());
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    format!(
        "{cpu}, {threads} threads, {} build, {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        std::env::consts::OS
    )
}

fn bench(markdown: bool) -> Result<()> {
    let mut rows: Vec<(String, String, String)> = Vec::new();
    let mut add = |op: &str, size: &str, time: String| {
        rows.push((op.to_string(), size.to_string(), time));
    };
    let ms = |v: f64| {
        if v >= 100.0 {
            format!("{v:.0} ms")
        } else if v >= 10.0 {
            format!("{v:.1} ms")
        } else {
            format!("{v:.2} ms")
        }
    };

    // 1. Paste 1,000,000 cells (10 columns × 100,000 rows) into a fresh table.
    let text = grid_text(100_000, 10);
    let mut e = Engine::new();
    let mut table = String::new();
    let paste = median_ms(3, || {
        e = Engine::new();
        table = created(&e.call(json!({
            "type": "create_table", "kind": "column", "title": "Bench",
            "datasets": 1, "format": {"kind": "values", "n": 1}, "rows": 0
        }))?)?;
        let r = e.call(json!({
            "type": "paste", "table": table, "row0": 0, "col0": 0, "text": text, "options": {}
        }))?;
        if r["numeric_cells"] != 1_000_000 {
            bail!("the paste stored {} numbers", r["numeric_cells"]);
        }
        Ok(())
    })?;
    add(
        "Paste (parse, detect, store)",
        &format!(
            "1,000,000 cells, 10 × 100,000 ({:.1} MB of text)",
            text.len() as f64 / 1e6
        ),
        ms(paste),
    );

    // 2. Export the pasted table.
    let csv = median_ms(3, || {
        e.call(json!({"type": "export_table", "table": table, "format": "csv"}))?;
        Ok(())
    })?;
    add("Export table to CSV", "1,000,000 cells", ms(csv));
    let xlsx = median_ms(3, || {
        e.call(json!({"type": "export_table", "table": table, "format": "xlsx"}))?;
        Ok(())
    })?;
    add("Export table to XLSX", "1,000,000 cells", ms(xlsx));
    let save = median_ms(3, || {
        e.call(json!({"type": "save_project_bytes"}))?;
        Ok(())
    })?;
    add("Save the project (.dioptra)", "1,000,000 cells", ms(save));

    // 3. A column formula over 1,000,000 rows, recomputed when it is read.
    let mut f = Engine::new();
    let ft = created(&f.call(json!({
        "type": "create_table", "kind": "column", "title": "Formula",
        "datasets": 2, "format": {"kind": "values", "n": 1}, "rows": 0
    }))?)?;
    let col: String = (0..1_000_000)
        .map(|i| format!("{}\n", (i % 977) as f64 + 0.25))
        .collect();
    f.call(json!({
        "type": "paste", "table": ft, "row0": 0, "col0": 0, "text": col, "options": {"header": false}
    }))?;
    let mut flip = 0usize;
    let formula = median_ms(5, || {
        flip += 1;
        let expr = if flip.is_multiple_of(2) {
            "A * 2 + 1"
        } else {
            "A * 2 + 1.5"
        };
        f.call(json!({"type": "set_column_formula", "table": ft, "col": 1, "expr": expr}))?;
        f.call(json!({"type": "get_cells", "table": ft, "range": {"row0": 0, "row1": 3, "col0": 0, "col1": 2}}))?;
        Ok(())
    })?;
    add(
        "Column formula = A * 2 + 1 (set, recompute on read)",
        "1,000,000 rows",
        ms(formula),
    );
    let mut row = 0usize;
    let edit = median_ms(5, || {
        row += 1;
        f.call(json!({"type": "set_cell", "table": ft, "row": row, "col": 0, "input": format!("{}", row as f64 + 0.5)}))?;
        f.call(json!({"type": "get_cells", "table": ft, "range": {"row0": 0, "row1": 3, "col0": 0, "col1": 2}}))?;
        Ok(())
    })?;
    add(
        "Edit one cell, formula column recomputes",
        "1,000,000 rows",
        ms(edit),
    );

    // 4. A 10,000-point scatter: render for display, then export.
    let mut g = Engine::new();
    let xt = created(&g.call(json!({
        "type": "create_table", "kind": "xy", "title": "Scatter",
        "datasets": 1, "format": {"kind": "values", "n": 1}, "rows": 0
    }))?)?;
    let mut pts = String::with_capacity(10_000 * 16);
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    for i in 0..10_000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let noise = (seed % 10_000) as f64 / 10_000.0 - 0.5;
        let x = i as f64 / 100.0;
        pts.push_str(&format!("{x:.2}\t{:.3}\n", 2.0 + 0.4 * x + 6.0 * noise));
    }
    g.call(json!({"type": "paste", "table": xt, "row0": 0, "col0": 0, "text": pts, "options": {"header": false}}))?;
    let graph = created(&g.call(
        json!({"type": "create_graph", "source": xt, "family": "scatter", "title": "Bench"}),
    )?)?;
    let mut svg_kb = 0.0;
    let render = median_ms(9, || {
        let r = g.call(json!({"type": "render_graph", "id": graph}))?;
        svg_kb = r["svg"].as_str().map_or(0.0, |s| s.len() as f64 / 1e3);
        Ok(())
    })?;
    add(
        "Scatter → SVG for the editor (with hit-test targets)",
        &format!("10,000 points ({svg_kb:.0} kB)"),
        ms(render),
    );
    for (label, format) in [
        ("Scatter → SVG file", json!({"format": "svg"})),
        ("Scatter → PDF (fonts embedded)", json!({"format": "pdf"})),
        (
            "Scatter → PNG at 300 dpi",
            json!({"format": "png", "dpi": 300.0}),
        ),
    ] {
        let mut size = 0.0;
        let t = median_ms(5, || {
            let r = g.call(
                json!({"type": "export_graph", "id": graph, "format": format, "size": null}),
            )?;
            size = kb(&r);
            Ok(())
        })?;
        add(label, &format!("10,000 points ({size:.0} kB)"), ms(t));
    }

    // Print.
    let header = ("Operation", "Size", "Time (median)");
    outln!(
        "dioptra bench {} on {}",
        env!("CARGO_PKG_VERSION"),
        machine()
    );
    outln!(
        "Times are wall clock for one engine command (JSON in, JSON out), median of 3 to 9 runs."
    );
    outln!();
    if markdown {
        outln!("| {} | {} | {} |", header.0, header.1, header.2);
        outln!("|---|---|---|");
        for (a, b, c) in &rows {
            outln!("| {a} | {b} | {c} |");
        }
    } else {
        let w0 = rows
            .iter()
            .map(|r| r.0.chars().count())
            .max()
            .unwrap_or(0)
            .max(header.0.len());
        let w1 = rows
            .iter()
            .map(|r| r.1.chars().count())
            .max()
            .unwrap_or(0)
            .max(header.1.len());
        let pad =
            |s: &str, w: usize| format!("{s}{}", " ".repeat(w.saturating_sub(s.chars().count())));
        outln!("{}  {}  {}", pad(header.0, w0), pad(header.1, w1), header.2);
        outln!(
            "{}  {}  {}",
            "-".repeat(w0),
            "-".repeat(w1),
            "-".repeat(header.2.len())
        );
        for (a, b, c) in &rows {
            outln!("{}  {}  {c}", pad(a, w0), pad(b, w1));
        }
    }
    if cfg!(debug_assertions) {
        eprintln!(
            "dioptra: note: this is a debug build; build with --release for representative numbers"
        );
    }
    Ok(())
}

// ---- equations ----------------------------------------------------------------------------

fn equations(as_json: bool) -> Result<()> {
    use dioptra_core::compute::{builtin_equation, EQUATION_KEYS};
    let mut list = Vec::new();
    let mut names = Vec::new();
    for key in EQUATION_KEYS {
        let eq = builtin_equation(key).map_err(|e| anyhow!(e))?;
        names.push(eq.name.clone());
        list.push((key.to_string(), eq));
    }
    // Library equations without a snake_case name are reachable by their full name.
    for eq in dioptra_stats::equations::library() {
        if !names.contains(&eq.name) {
            names.push(eq.name.clone());
            list.push((eq.name.clone(), eq));
        }
    }
    // Group by family, families in order of first appearance.
    let mut order: Vec<String> = Vec::new();
    for (_, e) in &list {
        if !order.contains(&e.family) {
            order.push(e.family.clone());
        }
    }
    list.sort_by_key(|(_, e)| order.iter().position(|f| *f == e.family));
    if as_json {
        let v: Vec<Value> = list
            .iter()
            .map(|(k, e)| {
                json!({
                    "key": k,
                    "name": e.name,
                    "family": e.family,
                    "formula": e.formula,
                    "parameters": e.params.iter().filter(|p| !p.constant).map(|p| p.name.clone()).collect::<Vec<_>>(),
                    "constants": e.params.iter().filter(|p| p.constant).map(|p| p.name.clone()).collect::<Vec<_>>(),
                    "derived": e.derived.iter().map(|d| d.name.clone()).collect::<Vec<_>>(),
                })
            })
            .collect();
        outln!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    outln!(
        "{} built-in equations. Name one in a nonlinear regression as equation.name (the key), \
or write your own as equation.formula.",
        list.len()
    );
    let mut family = String::new();
    for (k, e) in &list {
        if e.family != family {
            family = e.family.clone();
            outln!("\n{family}");
        }
        let params: Vec<&str> = e
            .params
            .iter()
            .filter(|p| !p.constant)
            .map(|p| p.name.as_str())
            .collect();
        let consts: Vec<&str> = e
            .params
            .iter()
            .filter(|p| p.constant)
            .map(|p| p.name.as_str())
            .collect();
        outln!("  {k}");
        outln!("      {}", e.name);
        if e.formula.contains('=') {
            outln!("      {}", e.formula);
        } else {
            outln!("      Y = {}", e.formula);
        }
        let mut p = format!("      parameters: {}", params.join(", "));
        if !consts.is_empty() {
            p.push_str(&format!("; constants per data set: {}", consts.join(", ")));
        }
        if !e.derived.is_empty() {
            let d: Vec<&str> = e.derived.iter().map(|d| d.name.as_str()).collect();
            p.push_str(&format!("; derived: {}", d.join(", ")));
        }
        outln!("{p}");
    }
    Ok(())
}

// ---- Dioptra Lab ------------------------------------------------------------------------

/// The desktop app's license file: `license.key` in the config directory of
/// `io.github.zahakj.dioptra`.
fn license_path() -> Result<PathBuf> {
    const ID: &str = "io.github.zahakj.dioptra";
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = if cfg!(windows) {
        env("APPDATA").ok_or_else(|| anyhow!("%APPDATA% is not set"))?
    } else if cfg!(target_os = "macos") {
        env("HOME")
            .ok_or_else(|| anyhow!("$HOME is not set"))?
            .join("Library/Application Support")
    } else {
        match env("XDG_CONFIG_HOME") {
            Some(p) => p,
            None => env("HOME")
                .ok_or_else(|| anyhow!("$HOME is not set"))?
                .join(".config"),
        }
    };
    Ok(base.join(ID).join("license.key"))
}

/// Today in UTC, `YYYY-MM-DD`.
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = dioptra_core::util::civil_from_days(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// The key from $DIOPTRA_LICENSE or the license file.
fn stored_key() -> Result<Option<String>> {
    if let Some(k) = std::env::var("DIOPTRA_LICENSE")
        .ok()
        .filter(|k| !k.trim().is_empty())
    {
        return Ok(Some(k));
    }
    match std::fs::read_to_string(license_path()?) {
        Ok(k) => Ok(Some(k)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).context("cannot read the license file"),
    }
}

/// A session with the stored license applied.
fn licensed_engine() -> Result<Engine> {
    let mut e = Engine::new();
    let key = stored_key()?;
    e.call(json!({"type": "set_license", "key": key, "today": today()}))?;
    Ok(e)
}

fn license(set: Option<&str>, remove: bool) -> Result<()> {
    let path = license_path()?;
    if remove {
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(e).with_context(|| format!("cannot remove {}", path.display()))
            }
            _ => {}
        }
        outln!("Removed the license key.");
        return Ok(());
    }
    let mut e = Engine::new();
    let key = match set {
        Some(k) => Some(k.trim().to_string()),
        None => stored_key()?,
    };
    let v = e.call(json!({"type": "set_license", "key": key, "today": today()}))?;
    if let Some(err) = v["error"].as_str() {
        bail!("the key was refused: {err}");
    }
    if let Some(k) = set {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("cannot create {}", dir.display()))?;
        }
        std::fs::write(&path, k.trim())
            .with_context(|| format!("cannot write {}", path.display()))?;
        outln!("Stored the key in {}.", path.display());
    }
    let l = &v["license"];
    if l.is_null() {
        outln!("No license key. Lab features are off.");
        return Ok(());
    }
    outln!("Licensed to  {}", l["licensee"].as_str().unwrap_or(""));
    outln!("Plan         {}", v["plan_label"].as_str().unwrap_or(""));
    outln!(
        "Valid        {} to {}",
        l["issued"].as_str().unwrap_or(""),
        l["expires"].as_str().unwrap_or("")
    );
    outln!(
        "Lab          {}",
        if v["lab_enabled"].as_bool() == Some(true) {
            "on"
        } else {
            "off"
        }
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn batch(
    template: &Path,
    table: &str,
    inputs: &[PathBuf],
    out: &Path,
    format: FigureFormat,
    dpi: f64,
    plate_map: Option<&Path>,
) -> Result<()> {
    let e = licensed_engine()?;
    e.s.require_lab("Batch analysis").map_err(|m| anyhow!(m))?;
    let tpl =
        std::fs::read(template).with_context(|| format!("cannot read {}", template.display()))?;
    let plate_map = plate_map
        .map(|p| std::fs::read_to_string(p).with_context(|| format!("cannot read {}", p.display())))
        .transpose()?;
    let opts = dioptra_lab::batch::BatchOptions {
        format: match format {
            FigureFormat::Pdf => dioptra_lab::batch::FigureFormat::Pdf,
            FigureFormat::Png => dioptra_lab::batch::FigureFormat::Png,
            FigureFormat::Svg => dioptra_lab::batch::FigureFormat::Svg,
        },
        dpi,
        plate_map,
        ..Default::default()
    };
    let mut files = Vec::new();
    for p in inputs {
        if p.is_dir() {
            files.extend(
                dioptra_lab::watch::data_files(p)
                    .with_context(|| format!("cannot list {}", p.display()))?,
            );
        } else if p.is_file() {
            files.push(p.clone());
        } else {
            bail!("no such file or folder: {}", p.display());
        }
    }
    if files.is_empty() {
        bail!("no data files to analyse");
    }
    let mut items = Vec::new();
    for f in &files {
        let item = dioptra_lab::watch::run_file(&tpl, table, f, out, &opts);
        match &item.error {
            Some(err) => outln!("{}: error: {err}", item.name),
            None => outln!(
                "{}: {} file{} written",
                item.name,
                item.written.len(),
                if item.written.len() == 1 { "" } else { "s" }
            ),
        }
        items.push(item);
    }
    let summary = dioptra_lab::watch::write_summary(out, &items).map_err(|m| anyhow!(m))?;
    outln!("Summary: {}", summary.display());
    let failed = items.iter().filter(|i| i.error.is_some()).count();
    if failed > 0 {
        bail!("{failed} of {} files could not be analysed", items.len());
    }
    Ok(())
}

fn convert(inputs: &[PathBuf], out: &Path) -> Result<()> {
    licensed_engine()?
        .s
        .require_lab("Batch conversion")
        .map_err(|m| anyhow!(m))?;
    let ext = |p: &Path| {
        p.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default()
    };
    let wanted = |p: &Path| matches!(ext(p).as_str(), "pzfx" | "prism" | "dioptra");
    let mut files = Vec::new();
    for p in inputs {
        if p.is_dir() {
            let mut found: Vec<PathBuf> = std::fs::read_dir(p)
                .with_context(|| format!("cannot list {}", p.display()))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|f| f.is_file() && wanted(f))
                .collect();
            found.sort();
            files.extend(found);
        } else if p.is_file() {
            files.push(p.clone());
        } else {
            bail!("no such file or folder: {}", p.display());
        }
    }
    if files.is_empty() {
        bail!("no .pzfx, .prism or .dioptra files to convert");
    }
    std::fs::create_dir_all(out).with_context(|| format!("cannot create {}", out.display()))?;
    let mut failed = 0;
    for f in &files {
        let stem = f
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let result = (|| -> Result<(PathBuf, Vec<String>)> {
            let mut e = licensed_engine()?;
            if ext(f) == "dioptra" {
                e.call(json!({"type": "open_project_path", "path": abs(f)?}))?;
                let r = e.call(json!({"type": "lab_export_pzfx"}))?;
                let target = out.join(format!("{stem}.pzfx"));
                use base64::Engine as _;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(r["base64"].as_str().unwrap_or_default())
                    .context("bad file from the engine")?;
                std::fs::write(&target, bytes)
                    .with_context(|| format!("cannot write {}", target.display()))?;
                let notes = r["notes"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|n| n.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                Ok((target, notes))
            } else {
                let r = e.call(json!({"type": "import_path", "path": abs(f)?, "kind": null}))?;
                let target = out.join(format!("{stem}.dioptra"));
                e.call(json!({"type": "set_project_title", "title": stem}))?;
                e.call(json!({"type": "save_project_path", "path": abs(&target)?}))?;
                let notes = r["notes"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|n| n.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                Ok((target, notes))
            }
        })();
        match result {
            Ok((target, notes)) => {
                outln!("{} → {}", f.display(), target.display());
                for n in notes {
                    outln!("  {n}");
                }
            }
            Err(err) => {
                failed += 1;
                outln!("{}: error: {err:#}", f.display());
            }
        }
    }
    if failed > 0 {
        bail!("{failed} of {} files could not be converted", files.len());
    }
    Ok(())
}

fn oq(out: Option<&Path>) -> Result<()> {
    licensed_engine()?
        .s
        .require_lab("Validation pack")
        .map_err(|m| anyhow!(m))?;
    let r = dioptra_lab::oq::run_with(|i, n, title| {
        eprintln!("[{}/{n}] {title}", i + 1);
    });
    outln!(
        "Dioptra {} ({}) on {}: {} of {} checks passed in {:.1} s",
        r.version,
        r.target,
        r.os,
        r.totals.passed,
        r.totals.checks,
        r.seconds
    );
    outln!("Fixtures SHA-256 {}", r.fixtures_sha256);
    outln!("Report SHA-256   {}", r.report_sha256);
    for c in r.checks.iter().filter(|c| !c.pass) {
        outln!("FAIL {} {}", c.id, c.reference);
    }
    if let Some(path) = out {
        let bytes = match path.extension().and_then(|e| e.to_str()) {
            Some("pdf") => dioptra_lab::oq::pdf(&r, None).map_err(|m| anyhow!(m))?,
            Some("csv") => dioptra_lab::oq::csv(&r).into_bytes(),
            _ => bail!("the report must end in .pdf or .csv"),
        };
        std::fs::write(path, bytes).with_context(|| format!("cannot write {}", path.display()))?;
        outln!("Wrote {}", path.display());
    }
    if !r.pass {
        bail!("the OQ failed");
    }
    Ok(())
}
