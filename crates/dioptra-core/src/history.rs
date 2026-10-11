//! Undo, redo and the audit log (DESIGN §4.5).
//!
//! Every mutation records *snapshots* of exactly what it is about to change: a few cells,
//! one column, a whole table, one object, the display order or project settings. Undo swaps
//! each snapshot with the current state of the same thing, so the swapped-out state becomes
//! the redo entry. The audit log (`history.jsonl`) records every command with a timestamp.

use serde::{Deserialize, Serialize};

use crate::analysis::Analysis;
use crate::api::{Id, PValueStyle};
use crate::graph::{Graph, Layout};
use crate::project::Project;
use crate::table::{CellValue, Column, Table};

/// Maximum undo depth.
pub const UNDO_LIMIT: usize = 200;

/// One audit-log line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// When.
    pub at: String,
    /// What, in words.
    pub label: String,
    /// The command as JSON (large paste text is summarised).
    pub command: serde_json::Value,
}

/// State of one cell.
#[derive(Debug, Clone, PartialEq)]
pub struct CellState {
    /// Content (cell formulas as `=…` text).
    pub value: CellValue,
    /// Excluded flag.
    pub excluded: bool,
}

/// A saved piece of state that can be swapped back in.
#[derive(Debug, Clone)]
pub enum Snapshot {
    /// Some cells of a table (shape unchanged).
    Cells {
        /// Table.
        table: Id,
        /// (row, flat column, state).
        cells: Vec<(usize, usize, CellState)>,
    },
    /// The row count of a table.
    RowCount {
        /// Table.
        table: Id,
        /// Rows.
        rows: usize,
    },
    /// One whole column.
    Column {
        /// Table.
        table: Id,
        /// Flat index.
        flat: usize,
        /// Column.
        column: Box<Column>,
    },
    /// A whole table (`None`: absent).
    Table {
        /// Id.
        id: Id,
        /// Table.
        table: Option<Box<Table>>,
    },
    /// An analysis (`None`: absent).
    Analysis {
        /// Id.
        id: Id,
        /// Analysis.
        analysis: Option<Box<Analysis>>,
    },
    /// A graph (`None`: absent).
    Graph {
        /// Id.
        id: Id,
        /// Graph.
        graph: Option<Box<Graph>>,
    },
    /// A layout (`None`: absent).
    Layout {
        /// Id.
        id: Id,
        /// Layout.
        layout: Option<Layout>,
    },
    /// A column's hand-edit log.
    EditLog {
        /// Table.
        table: Id,
        /// Flat column.
        flat: usize,
        /// Edits.
        edits: Vec<crate::table::CellEdit>,
    },
    /// A column's display decimals.
    Decimals {
        /// Table.
        table: Id,
        /// Flat column.
        flat: usize,
        /// Decimals.
        decimals: Option<u8>,
    },
    /// A table's row titles.
    RowTitles {
        /// Table.
        table: Id,
        /// Titles.
        titles: Vec<String>,
    },
    /// An object's title.
    Title {
        /// Id.
        id: Id,
        /// Title.
        title: String,
    },
    /// Display order.
    Order(Vec<Id>),
    /// Project settings.
    Settings {
        /// Title.
        title: String,
        /// P-value style.
        p_value_style: PValueStyle,
    },
}

/// An undoable step.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Label.
    pub label: String,
    /// Snapshots, in capture order.
    pub snapshots: Vec<Snapshot>,
}

/// Undo and redo stacks plus the audit log.
#[derive(Debug, Clone, Default)]
pub struct History {
    /// Undo stack (newest last).
    pub undo: Vec<Entry>,
    /// Redo stack (newest last).
    pub redo: Vec<Entry>,
    /// Audit log (oldest first).
    pub log: Vec<HistoryEntry>,
}

/// Capture the current state of cells.
pub fn capture_cells(
    t: &Table,
    cells: impl Iterator<Item = (usize, usize)>,
) -> Vec<(usize, usize, CellState)> {
    cells
        .filter_map(|(r, c)| {
            let col = t.col(c)?;
            Some((
                r,
                c,
                CellState {
                    value: col.cell_value(r),
                    excluded: col.excluded.contains(&r),
                },
            ))
        })
        .collect()
}

/// What restoring a snapshot touched.
#[derive(Debug, Default)]
pub struct Touched {
    /// Tables whose data changed.
    pub tables: Vec<Id>,
    /// Other objects that changed.
    pub objects: Vec<Id>,
    /// Whether the tree changed.
    pub tree: bool,
}

/// The state that `snap` would overwrite, in the same shape.
pub fn counterpart(p: &Project, snap: &Snapshot) -> Snapshot {
    match snap {
        Snapshot::Cells { table, cells } => Snapshot::Cells {
            table: table.clone(),
            cells: p
                .tables
                .get(table)
                .map(|t| capture_cells(t, cells.iter().map(|(r, c, _)| (*r, *c))))
                .unwrap_or_default(),
        },
        Snapshot::RowCount { table, .. } => Snapshot::RowCount {
            table: table.clone(),
            rows: p.tables.get(table).map_or(0, |t| t.rows),
        },
        Snapshot::Column { table, flat, .. } => Snapshot::Column {
            table: table.clone(),
            flat: *flat,
            column: Box::new(
                p.tables
                    .get(table)
                    .and_then(|t| t.col(*flat))
                    .cloned()
                    .unwrap_or_default(),
            ),
        },
        Snapshot::EditLog { table, flat, .. } => Snapshot::EditLog {
            table: table.clone(),
            flat: *flat,
            edits: p
                .tables
                .get(table)
                .and_then(|t| t.col(*flat))
                .map(|c| c.edits.clone())
                .unwrap_or_default(),
        },
        Snapshot::Decimals { table, flat, .. } => Snapshot::Decimals {
            table: table.clone(),
            flat: *flat,
            decimals: p
                .tables
                .get(table)
                .and_then(|t| t.col(*flat))
                .and_then(|c| c.decimals),
        },
        Snapshot::RowTitles { table, .. } => Snapshot::RowTitles {
            table: table.clone(),
            titles: p
                .tables
                .get(table)
                .map(|t| t.row_titles.clone())
                .unwrap_or_default(),
        },
        Snapshot::Table { id, .. } => Snapshot::Table {
            id: id.clone(),
            table: p.tables.get(id).cloned().map(Box::new),
        },
        Snapshot::Analysis { id, .. } => Snapshot::Analysis {
            id: id.clone(),
            analysis: p.analyses.get(id).cloned().map(Box::new),
        },
        Snapshot::Graph { id, .. } => Snapshot::Graph {
            id: id.clone(),
            graph: p.graphs.get(id).cloned().map(Box::new),
        },
        Snapshot::Layout { id, .. } => Snapshot::Layout {
            id: id.clone(),
            layout: p.layouts.get(id).cloned(),
        },
        Snapshot::Title { id, .. } => Snapshot::Title {
            id: id.clone(),
            title: p.title_of(id).unwrap_or_default().to_string(),
        },
        Snapshot::Order(_) => Snapshot::Order(p.order.clone()),
        Snapshot::Settings { .. } => Snapshot::Settings {
            title: p.title.clone(),
            p_value_style: p.p_value_style,
        },
    }
}

/// Put a snapshot back.
pub fn restore(p: &mut Project, snap: Snapshot, touched: &mut Touched) {
    match snap {
        Snapshot::Cells { table, cells } => {
            if let Some(t) = p.tables.get_mut(&table) {
                for (r, c, s) in cells {
                    t.put_cell_value(r, c, &s.value);
                    if let Some(col) = t.col_mut(c) {
                        if s.excluded {
                            col.excluded.insert(r);
                        } else {
                            col.excluded.remove(&r);
                        }
                    }
                }
                touched.tables.push(table);
            }
        }
        Snapshot::RowCount { table, rows } => {
            if let Some(t) = p.tables.get_mut(&table) {
                t.set_rows(rows);
                touched.tables.push(table);
            }
        }
        Snapshot::Column {
            table,
            flat,
            column,
        } => {
            if let Some(t) = p.tables.get_mut(&table) {
                if let Some(c) = t.col_mut(flat) {
                    *c = *column;
                }
                touched.tables.push(table);
            }
        }
        Snapshot::EditLog { table, flat, edits } => {
            if let Some(c) = p.tables.get_mut(&table).and_then(|t| t.col_mut(flat)) {
                c.edits = edits;
            }
            touched.objects.push(table);
        }
        Snapshot::Decimals {
            table,
            flat,
            decimals,
        } => {
            if let Some(c) = p.tables.get_mut(&table).and_then(|t| t.col_mut(flat)) {
                c.decimals = decimals;
            }
            touched.objects.push(table);
        }
        Snapshot::RowTitles { table, titles } => {
            if let Some(t) = p.tables.get_mut(&table) {
                t.row_titles = titles;
            }
            touched.objects.push(table);
        }
        Snapshot::Table { id, table } => {
            match table {
                Some(t) => {
                    p.tables.insert(id.clone(), *t);
                    touched.tables.push(id);
                }
                None => {
                    p.tables.shift_remove(&id);
                    touched.objects.push(id);
                }
            }
            touched.tree = true;
        }
        Snapshot::Analysis { id, analysis } => {
            match analysis {
                Some(a) => {
                    let mut a = *a;
                    a.stale = true;
                    p.analyses.insert(id.clone(), a);
                }
                None => {
                    p.analyses.shift_remove(&id);
                }
            }
            touched.objects.push(id);
            touched.tree = true;
        }
        Snapshot::Graph { id, graph } => {
            match graph {
                Some(g) => {
                    p.graphs.insert(id.clone(), *g);
                }
                None => {
                    p.graphs.shift_remove(&id);
                }
            }
            touched.objects.push(id);
            touched.tree = true;
        }
        Snapshot::Layout { id, layout } => {
            match layout {
                Some(l) => {
                    p.layouts.insert(id.clone(), l);
                }
                None => {
                    p.layouts.shift_remove(&id);
                }
            }
            touched.objects.push(id);
            touched.tree = true;
        }
        Snapshot::Title { id, title } => {
            p.set_title(&id, title);
            touched.objects.push(id);
            touched.tree = true;
        }
        Snapshot::Order(o) => {
            p.order = o;
            touched.tree = true;
        }
        Snapshot::Settings {
            title,
            p_value_style,
        } => {
            p.title = title;
            p.p_value_style = p_value_style;
            touched.tree = true;
            for a in p.analyses.values_mut() {
                touched.objects.push(a.id.clone());
            }
        }
    }
}

impl History {
    /// Record a new undoable step (clears redo; caps the depth).
    pub fn push(&mut self, entry: Entry) {
        self.redo.clear();
        self.undo.push(entry);
        if self.undo.len() > UNDO_LIMIT {
            let extra = self.undo.len() - UNDO_LIMIT;
            self.undo.drain(..extra);
        }
    }

    /// Apply an entry's snapshots (last captured first) and return the counterpart entry.
    pub fn apply(p: &mut Project, entry: Entry, touched: &mut Touched) -> Entry {
        let mut back = Vec::with_capacity(entry.snapshots.len());
        for s in entry.snapshots.into_iter().rev() {
            back.push(counterpart(p, &s));
            restore(p, s, touched);
        }
        Entry {
            label: entry.label,
            snapshots: back,
        }
    }
}
