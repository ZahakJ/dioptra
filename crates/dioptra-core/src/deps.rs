//! Dependencies between columns (formula references), derived tables, analyses and graphs.
//!
//! Inside a table, formula columns form a graph that is checked for cycles when a formula
//! is entered and evaluated in topological order. Across the project, a change to a table
//! marks its derived tables, analyses and graphs stale; nothing is recomputed until it is
//! asked for (pull-based), and then only what is dirty.

use std::collections::{BTreeMap, BTreeSet};

use crate::api::Id;
use crate::formula::{self, eval_cell, eval_column, Expr, Value};
use crate::project::Project;
use crate::table::{Column, Table};

/// The columns a column's formulas read. Single-cell references to the column itself
/// (cell formulas such as `B3 = B2 + 1`) are not edges; they are ordered within the column.
pub fn column_deps(table: &Table, flat: usize) -> Result<BTreeSet<usize>, String> {
    let Some(col) = table.col(flat) else {
        return Ok(BTreeSet::new());
    };
    let mut out = BTreeSet::new();
    let mut add = |expr: &Expr, cell: bool| -> Result<(), String> {
        for (r, row) in expr.references() {
            let target = formula::resolve(table, &r)?;
            if target == flat && cell && row.is_some() {
                continue;
            }
            out.insert(target);
        }
        Ok(())
    };
    if let Some(f) = &col.formula {
        add(&formula::parse(f).map_err(|e| e.to_string())?, false)?;
    }
    for f in col.cell_formulas.values() {
        add(&formula::parse(f).map_err(|e| e.to_string())?, true)?;
    }
    Ok(out)
}

/// Formula columns of a table with their dependencies; unparseable formulas have none.
fn graph(table: &Table) -> BTreeMap<usize, BTreeSet<usize>> {
    let mut g = BTreeMap::new();
    for (i, c) in table.columns().enumerate() {
        if c.has_formula() {
            g.insert(i, column_deps(table, i).unwrap_or_default());
        }
    }
    g
}

/// Topological order of formula columns, or the message naming a loop.
pub fn formula_order(table: &Table) -> Result<Vec<usize>, String> {
    topo(table, &graph(table))
}

fn topo(table: &Table, g: &BTreeMap<usize, BTreeSet<usize>>) -> Result<Vec<usize>, String> {
    // 0 = unvisited, 1 = on stack, 2 = done
    let mut state: BTreeMap<usize, u8> = BTreeMap::new();
    let mut order = Vec::new();
    fn visit(
        n: usize,
        g: &BTreeMap<usize, BTreeSet<usize>>,
        state: &mut BTreeMap<usize, u8>,
        stack: &mut Vec<usize>,
        order: &mut Vec<usize>,
    ) -> Result<(), Vec<usize>> {
        match state.get(&n) {
            Some(2) => return Ok(()),
            Some(1) => {
                let start = stack.iter().position(|&x| x == n).expect("on stack");
                let mut cyc = stack[start..].to_vec();
                cyc.push(n);
                return Err(cyc);
            }
            _ => {}
        }
        state.insert(n, 1);
        stack.push(n);
        if let Some(deps) = g.get(&n) {
            for &d in deps {
                if g.contains_key(&d) {
                    visit(d, g, state, stack, order)?;
                }
            }
        }
        stack.pop();
        state.insert(n, 2);
        order.push(n);
        Ok(())
    }
    for &n in g.keys() {
        let mut stack = Vec::new();
        if let Err(cyc) = visit(n, g, &mut state, &mut stack, &mut order) {
            // Dependencies point "reads from"; show the loop in the direction values flow.
            let names: Vec<String> = cyc.iter().rev().map(|&c| table.col_label(c)).collect();
            return Err(format!("Circular reference: {}", names.join(" → ")));
        }
    }
    Ok(order)
}

/// Check that the table's formulas (including a pending change) have no cycle and refer
/// only to columns that exist.
pub fn check(table: &Table) -> Result<(), String> {
    let mut g = BTreeMap::new();
    for (i, c) in table.columns().enumerate() {
        if c.has_formula() {
            g.insert(
                i,
                column_deps(table, i).map_err(|e| format!("{}: {e}", table.col_label(i)))?,
            );
        }
    }
    topo(table, &g)?;
    for (i, c) in table.columns().enumerate() {
        if !c.cell_formulas.is_empty() {
            cell_order(table, i, c)?;
        }
    }
    Ok(())
}

/// Order of cell formulas within one column by their references to the same column.
fn cell_order(table: &Table, flat: usize, col: &Column) -> Result<Vec<usize>, String> {
    let mut deps: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (&row, f) in &col.cell_formulas {
        let e = formula::parse(f).map_err(|e| e.to_string())?;
        let mut d = Vec::new();
        for (r, rr) in e.references() {
            if formula::resolve(table, &r).ok() == Some(flat) {
                if let Some(rr) = rr {
                    if col.cell_formulas.contains_key(&(rr - 1)) {
                        d.push(rr - 1);
                    }
                }
            }
        }
        deps.insert(row, d);
    }
    let mut state: BTreeMap<usize, u8> = BTreeMap::new();
    let mut order = Vec::new();
    fn visit(
        n: usize,
        deps: &BTreeMap<usize, Vec<usize>>,
        state: &mut BTreeMap<usize, u8>,
        stack: &mut Vec<usize>,
        order: &mut Vec<usize>,
    ) -> Result<(), Vec<usize>> {
        match state.get(&n) {
            Some(2) => return Ok(()),
            Some(1) => {
                let start = stack.iter().position(|&x| x == n).expect("on stack");
                let mut c = stack[start..].to_vec();
                c.push(n);
                return Err(c);
            }
            _ => {}
        }
        state.insert(n, 1);
        stack.push(n);
        for &d in deps.get(&n).map(Vec::as_slice).unwrap_or(&[]) {
            visit(d, deps, state, stack, order)?;
        }
        stack.pop();
        state.insert(n, 2);
        order.push(n);
        Ok(())
    }
    for &n in deps.keys() {
        let mut stack = Vec::new();
        if let Err(c) = visit(n, &deps, &mut state, &mut stack, &mut order) {
            let names: Vec<String> = c.iter().rev().map(|&r| table.address(r, flat)).collect();
            return Err(format!("Circular reference: {}", names.join(" → ")));
        }
    }
    Ok(order)
}

/// Formula columns that (transitively) read any of `changed`, plus the changed columns
/// themselves when they carry formulas.
pub fn downstream(table: &Table, changed: &BTreeSet<usize>) -> BTreeSet<usize> {
    let g = graph(table);
    let mut out: BTreeSet<usize> = changed
        .iter()
        .copied()
        .filter(|c| g.contains_key(c))
        .collect();
    let mut frontier: Vec<usize> = changed.iter().copied().collect();
    while let Some(c) = frontier.pop() {
        for (&n, deps) in &g {
            if deps.contains(&c) && out.insert(n) {
                frontier.push(n);
            }
        }
    }
    out
}

/// Mark the formula columns affected by a change to `changed` columns as dirty
/// (`None` = every formula column).
pub fn mark_columns(table: &mut Table, changed: Option<&BTreeSet<usize>>) {
    let d = match changed {
        Some(c) => downstream(table, c),
        None => graph(table).into_keys().collect(),
    };
    table.dirty.extend(d);
}

/// Recompute the table's dirty formula columns in dependency order.
pub fn recompute_table(table: &mut Table) {
    if table.dirty.is_empty() {
        return;
    }
    let order = match formula_order(table) {
        Ok(o) => o,
        Err(msg) => {
            // A loop slipped in (for example after a rename); flag every formula column.
            let dirty: Vec<usize> = std::mem::take(&mut table.dirty).into_iter().collect();
            for c in dirty {
                if let Some(col) = table.col_mut(c) {
                    col.formula_error = Some(msg.clone());
                }
            }
            return;
        }
    };
    let dirty = std::mem::take(&mut table.dirty);
    for c in order.into_iter().filter(|c| dirty.contains(c)) {
        recompute_column(table, c);
    }
}

/// Recompute one column's formulas.
pub fn recompute_column(table: &mut Table, flat: usize) {
    let Some(col) = table.col(flat) else { return };
    if let Some(f) = col.formula.clone() {
        let result = formula::parse(&f)
            .map_err(|e| e.to_string())
            .and_then(|e| eval_column(&e, table));
        let rows = table.rows;
        let col = table.col_mut(flat).expect("column");
        match result {
            Ok(r) => {
                col.values = r.values;
                col.text = r.text;
                col.errors = r.errors;
                col.formula_error = None;
            }
            Err(e) => {
                col.values = vec![f64::NAN; rows];
                col.text.clear();
                col.errors.clear();
                col.formula_error = Some(e);
            }
        }
        return;
    }
    if col.cell_formulas.is_empty() {
        return;
    }
    let order = match cell_order(table, flat, col) {
        Ok(o) => o,
        Err(msg) => {
            let col = table.col_mut(flat).expect("column");
            let rows: Vec<usize> = col.cell_formulas.keys().copied().collect();
            for r in rows {
                col.values[r] = f64::NAN;
                col.errors.insert(r, msg.clone());
            }
            return;
        }
    };
    for row in order {
        let f = table.col(flat).expect("column").cell_formulas[&row].clone();
        let v = match formula::parse(&f) {
            Ok(e) => eval_cell(&e, table, row),
            Err(e) => Value::Error(e.to_string()),
        };
        let col = table.col_mut(flat).expect("column");
        col.values[row] = f64::NAN;
        col.text.remove(&row);
        col.errors.remove(&row);
        match v {
            Value::Number(x) => col.values[row] = x,
            Value::Bool(b) => col.values[row] = f64::from(u8::from(b)),
            Value::Missing => {}
            Value::Text(t) => {
                col.text.insert(row, t);
            }
            Value::Error(e) => {
                col.errors.insert(row, e);
            }
            Value::Vector(v) => {
                col.errors
                    .insert(row, format!("gives {} values; a cell holds one", v.len()));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Project level
// ---------------------------------------------------------------------------

/// Objects that read `id` directly: derived tables, analyses on a table, graphs of a
/// table or analysis.
pub fn direct_dependents(project: &Project, id: &str) -> Vec<Id> {
    let mut out = Vec::new();
    for t in project.tables.values() {
        if t.derived_from.as_ref().is_some_and(|d| d.table == id) {
            out.push(t.id.clone());
        }
    }
    for a in project.analyses.values() {
        if a.table == id {
            out.push(a.id.clone());
        }
    }
    for g in project.graphs.values() {
        if g.spec.source == id {
            out.push(g.id.clone());
        }
    }
    out
}

/// Everything downstream of `id`, in breadth-first order, without `id` itself.
pub fn all_dependents(project: &Project, id: &str) -> Vec<Id> {
    let mut seen: BTreeSet<Id> = BTreeSet::new();
    let mut out = Vec::new();
    let mut queue = vec![id.to_string()];
    while let Some(cur) = queue.pop() {
        for d in direct_dependents(project, &cur) {
            if seen.insert(d.clone()) {
                out.push(d.clone());
                queue.push(d);
            }
        }
    }
    out
}

/// Record that a table's data changed: its affected formula columns become dirty and
/// everything downstream becomes stale. Returns the ids that changed (the table first).
pub fn table_changed(project: &mut Project, id: &str, cols: Option<&BTreeSet<usize>>) -> Vec<Id> {
    if let Some(t) = project.tables.get_mut(id) {
        mark_columns(t, cols);
    }
    let mut ids = vec![id.to_string()];
    for d in all_dependents(project, id) {
        if let Some(t) = project.tables.get_mut(&d) {
            t.stale = true;
        } else if let Some(a) = project.analyses.get_mut(&d) {
            a.stale = true;
        } else if let Some(g) = project.graphs.get_mut(&d) {
            g.stale = true;
        }
        ids.push(d);
    }
    project.dirty = true;
    ids
}

/// Bring a table up to date: replay its recipe if it is a stale derived table (after
/// refreshing its source), then recompute dirty formula columns.
pub fn refresh_table(project: &mut Project, id: &str) -> Result<(), String> {
    refresh_inner(project, id, 0)
}

fn refresh_inner(project: &mut Project, id: &str, depth: usize) -> Result<(), String> {
    if depth > 64 {
        return Err("derived tables are nested too deeply (or form a loop)".into());
    }
    let (stale, from) = match project.tables.get(id) {
        Some(t) => (t.stale, t.derived_from.clone()),
        None => return Err(format!("no table {id}")),
    };
    if let (true, Some(from)) = (stale, from) {
        refresh_inner(project, &from.table, depth + 1)?;
        let src = project
            .tables
            .get(&from.table)
            .ok_or("the source table was deleted")?;
        let rebuilt = crate::transform::apply(src, &from.op);
        let t = project.tables.get_mut(id).expect("table");
        match rebuilt {
            Ok(mut new) => {
                new.id = t.id.clone();
                new.title = t.title.clone();
                new.derived_from = t.derived_from.clone();
                // Keep the user's display decimals where the shape still matches.
                for (n, o) in new.columns_mut().zip(t.columns()) {
                    n.decimals = o.decimals;
                }
                *t = new;
            }
            Err(e) => {
                t.stale = false;
                return Err(e);
            }
        }
    }
    let t = project.tables.get_mut(id).expect("table");
    t.stale = false;
    recompute_table(t);
    Ok(())
}

/// Bring every table up to date without forcing a full recompute (before saving).
pub fn recompute_all_tables(project: &mut Project) {
    let ids: Vec<Id> = project.tables.keys().cloned().collect();
    for id in ids {
        let _ = refresh_table(project, &id);
    }
}

/// Recompute every stale table, column and analysis in the project.
pub fn recompute_all(project: &mut Project) -> Vec<String> {
    let mut errors = Vec::new();
    let ids: Vec<Id> = project.tables.keys().cloned().collect();
    for id in ids {
        if let Some(t) = project.tables.get_mut(&id) {
            if t.derived_from.is_none() {
                mark_columns(t, None);
            } else {
                t.stale = true;
            }
        }
    }
    let ids: Vec<Id> = project.tables.keys().cloned().collect();
    for id in &ids {
        if let Err(e) = refresh_table(project, id) {
            errors.push(format!("{}: {e}", project.tables[id].title));
        }
    }
    for a in project.analyses.values_mut() {
        a.stale = true;
    }
    for g in project.graphs.values_mut() {
        g.stale = true;
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{SubcolumnFormat, TableKind};

    fn t() -> Table {
        let mut t = Table::new(
            "t".into(),
            TableKind::Column,
            "T".into(),
            4,
            SubcolumnFormat::Values(1),
            3,
        );
        for (r, v) in ["1", "2", "3"].iter().enumerate() {
            t.set_cell_raw(r, 0, v).unwrap();
        }
        t
    }

    #[test]
    fn cycles_are_named() {
        let mut t = t();
        t.datasets[1].subcolumns[0].formula = Some("A * 2".into());
        t.datasets[2].subcolumns[0].formula = Some("B + D".into());
        t.datasets[3].subcolumns[0].formula = Some("C".into());
        let e = check(&t).unwrap_err();
        assert!(e.starts_with("Circular reference:"), "{e}");
        assert!(e.contains('C') && e.contains('D'), "{e}");
        t.datasets[3].subcolumns[0].formula = Some("B".into());
        check(&t).unwrap();
        t.datasets[1].subcolumns[0].formula = Some("B + 1".into());
        assert_eq!(check(&t).unwrap_err(), "Circular reference: B → B");
    }

    #[test]
    fn dirty_propagation_and_order() {
        let mut t = t();
        t.datasets[1].subcolumns[0].formula = Some("A * 2".into());
        t.datasets[2].subcolumns[0].formula = Some("B + 1".into());
        mark_columns(&mut t, None);
        recompute_table(&mut t);
        assert_eq!(t.datasets[2].subcolumns[0].values, vec![3.0, 5.0, 7.0]);
        t.set_cell_raw(0, 0, "10").unwrap();
        let changed: BTreeSet<usize> = [0].into();
        assert_eq!(downstream(&t, &changed), [1, 2].into());
        mark_columns(&mut t, Some(&changed));
        recompute_table(&mut t);
        assert_eq!(t.datasets[2].subcolumns[0].values[0], 21.0);
        assert!(t.dirty.is_empty());
    }

    #[test]
    fn cell_formulas_order_within_a_column() {
        let mut t = t();
        // B3 = B2 * 2, B2 = B1 + 1, B1 = A1: written out of order on purpose.
        t.set_cell_raw(2, 1, "=B2 * 2").unwrap();
        t.set_cell_raw(1, 1, "=B1 + 1").unwrap();
        t.set_cell_raw(0, 1, "=A1").unwrap();
        check(&t).unwrap();
        mark_columns(&mut t, None);
        recompute_table(&mut t);
        assert_eq!(t.datasets[1].subcolumns[0].values, vec![1.0, 2.0, 4.0]);
        t.set_cell_raw(0, 1, "=B3").unwrap();
        assert!(check(&t).unwrap_err().contains("B1"));
    }

    #[test]
    fn whole_column_errors_are_flagged() {
        let mut t = t();
        t.datasets[1].subcolumns[0].formula = Some("Q * 2".into());
        mark_columns(&mut t, None);
        recompute_table(&mut t);
        let cv = t.cell_view(0, &t.datasets[1].subcolumns[0]);
        assert!(cv.e.unwrap().contains("no data set Q"));
    }
}
