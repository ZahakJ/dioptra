//! The project: tables, analyses, graphs and layouts in display order (DESIGN §4).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::analysis::Analysis;
use crate::api::{Id, PValueStyle};
use crate::graph::{Graph, Layout};
use crate::table::Table;

/// Kind of a project object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// Data table.
    Table,
    /// Analysis.
    Analysis,
    /// Graph.
    Graph,
    /// Layout.
    Layout,
}

/// One object in the project tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectItem {
    /// Id.
    pub id: Id,
    /// Kind.
    pub kind: ItemKind,
    /// Title.
    pub title: String,
    /// Object it was produced from (source table of an analysis, derived table or graph).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_from: Option<Id>,
    /// Whether it is out of date.
    pub stale: bool,
    /// Table kind, analysis kind or graph family.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtype: Option<String>,
}

/// The project tree for the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectView {
    /// Title.
    pub title: String,
    /// Path on disk, if saved there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Unsaved changes.
    pub dirty: bool,
    /// P-value style.
    pub p_value_style: PValueStyle,
    /// Objects in display order.
    pub items: Vec<ProjectItem>,
    /// Whether undo / redo are possible.
    pub can_undo: bool,
    /// Whether redo is possible.
    pub can_redo: bool,
}

/// An open project.
#[derive(Debug, Clone, Default)]
pub struct Project {
    /// Title.
    pub title: String,
    /// Display order of every object.
    pub order: Vec<Id>,
    /// Tables.
    pub tables: IndexMap<Id, Table>,
    /// Analyses.
    pub analyses: IndexMap<Id, Analysis>,
    /// Graphs.
    pub graphs: IndexMap<Id, Graph>,
    /// Layouts.
    pub layouts: IndexMap<Id, Layout>,
    /// P-value style for every results sheet and bracket.
    pub p_value_style: PValueStyle,
    /// Where it was opened from or saved to.
    pub path: Option<String>,
    /// Unsaved changes.
    pub dirty: bool,
    /// Creation time.
    pub created: String,
    /// UTF-8 text files kept for other components (Dioptra Lab's audit trail), saved as
    /// `extensions/<name>` and read back unchanged.
    pub extensions: std::collections::BTreeMap<String, Vec<u8>>,
}

impl Project {
    /// An empty project.
    pub fn new() -> Self {
        Project {
            title: "Untitled".into(),
            created: crate::util::now(),
            ..Default::default()
        }
    }

    /// A fresh object id.
    pub fn new_id() -> Id {
        uuid::Uuid::new_v4().to_string()
    }

    /// Kind of an object.
    pub fn kind_of(&self, id: &str) -> Option<ItemKind> {
        if self.tables.contains_key(id) {
            Some(ItemKind::Table)
        } else if self.analyses.contains_key(id) {
            Some(ItemKind::Analysis)
        } else if self.graphs.contains_key(id) {
            Some(ItemKind::Graph)
        } else if self.layouts.contains_key(id) {
            Some(ItemKind::Layout)
        } else {
            None
        }
    }

    /// Title of an object.
    pub fn title_of(&self, id: &str) -> Option<&str> {
        self.tables
            .get(id)
            .map(|t| t.title.as_str())
            .or_else(|| self.analyses.get(id).map(|a| a.title.as_str()))
            .or_else(|| self.graphs.get(id).map(|g| g.title.as_str()))
            .or_else(|| self.layouts.get(id).map(|l| l.title.as_str()))
    }

    /// Set an object's title.
    pub fn set_title(&mut self, id: &str, title: String) -> bool {
        if let Some(t) = self.tables.get_mut(id) {
            t.title = title;
        } else if let Some(a) = self.analyses.get_mut(id) {
            a.title = title;
        } else if let Some(g) = self.graphs.get_mut(id) {
            g.title = title;
        } else if let Some(l) = self.layouts.get_mut(id) {
            l.title = title;
        } else {
            return false;
        }
        true
    }

    /// A title not yet used, like "Table 3".
    pub fn unique_title(&self, base: &str) -> String {
        let used = |t: &str| self.title_of_any(t);
        if !used(base) {
            return base.to_string();
        }
        (2..)
            .map(|i| format!("{base} {i}"))
            .find(|t| !used(t))
            .expect("infinite")
    }

    fn title_of_any(&self, t: &str) -> bool {
        self.tables.values().any(|x| x.title == t)
            || self.analyses.values().any(|x| x.title == t)
            || self.graphs.values().any(|x| x.title == t)
            || self.layouts.values().any(|x| x.title == t)
    }

    /// The project tree.
    pub fn view(&self, can_undo: bool, can_redo: bool) -> ProjectView {
        let items = self
            .order
            .iter()
            .filter_map(|id| {
                if let Some(t) = self.tables.get(id) {
                    Some(ProjectItem {
                        id: id.clone(),
                        kind: ItemKind::Table,
                        title: t.title.clone(),
                        derived_from: t.derived_from.as_ref().map(|d| d.table.clone()),
                        stale: t.stale || !t.dirty.is_empty(),
                        subtype: Some(serde_json::to_value(t.kind).ok()?.as_str()?.to_string()),
                    })
                } else if let Some(a) = self.analyses.get(id) {
                    Some(ProjectItem {
                        id: id.clone(),
                        kind: ItemKind::Analysis,
                        title: a.title.clone(),
                        derived_from: Some(a.table.clone()),
                        stale: a.stale || a.results.is_none(),
                        subtype: Some(a.spec.kind().into()),
                    })
                } else if let Some(g) = self.graphs.get(id) {
                    Some(ProjectItem {
                        id: id.clone(),
                        kind: ItemKind::Graph,
                        title: g.title.clone(),
                        derived_from: Some(g.spec.source.clone()),
                        stale: g.stale,
                        subtype: Some(g.spec.family.clone()),
                    })
                } else {
                    self.layouts.get(id).map(|l| ProjectItem {
                        id: id.clone(),
                        kind: ItemKind::Layout,
                        title: l.title.clone(),
                        derived_from: None,
                        stale: false,
                        subtype: None,
                    })
                }
            })
            .collect();
        ProjectView {
            title: self.title.clone(),
            path: self.path.clone(),
            dirty: self.dirty,
            p_value_style: self.p_value_style,
            items,
            can_undo,
            can_redo,
        }
    }

    /// Whether two projects hold the same document (ignoring path, dirty flag and caches
    /// other than results).
    pub fn same_content(&self, o: &Project) -> bool {
        self.title == o.title
            && self.order == o.order
            && self.p_value_style == o.p_value_style
            && self.tables == o.tables
            && self.analyses.len() == o.analyses.len()
            && self
                .analyses
                .iter()
                .zip(&o.analyses)
                .all(|((ka, a), (kb, b))| ka == kb && a == b && a.results == b.results)
            && self.graphs == o.graphs
            && self.layouts == o.layouts
    }
}
