// SPDX-License-Identifier: LicenseRef-Dioptra-Lab
//! Files on disk for batch analysis: running one input file into an output folder, and a
//! polling watcher that reports new files once their size has stopped changing.
//!
//! Callers check the license (`Session::require_lab("Batch analysis")`) before using these.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::batch::{self, BatchItem, BatchOptions};

/// The subfolder a watched folder's results go to.
pub const OUTPUT_DIR: &str = "analysed";

/// Extensions read as data files.
pub const DATA_EXTENSIONS: [&str; 9] = [
    "csv", "tsv", "tab", "txt", "xlsx", "xls", "ods", "pzfx", "prism",
];

/// Whether a path looks like a data file (not hidden, not an Office lock file).
pub fn is_data_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.starts_with('.') || name.starts_with("~$") {
        return false;
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    DATA_EXTENSIONS.contains(&ext.as_str())
}

/// The data files directly inside `dir`, sorted by name.
pub fn data_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_data_file(p))
        .collect();
    v.sort();
    Ok(v)
}

/// Write an item's files into `out_dir` (created if needed), recording the paths in
/// `written` and dropping the bytes.
pub fn write_outputs(out_dir: &Path, item: &mut BatchItem) -> Result<(), String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    for f in item.files.drain(..) {
        let p = out_dir.join(&f.name);
        std::fs::write(&p, &f.base64).map_err(|e| format!("{}: {e}", p.display()))?;
        item.written.push(p.to_string_lossy().into_owned());
    }
    Ok(())
}

/// Read one input file, apply the template, write the outputs into `out_dir`.
pub fn run_file(
    template: &[u8],
    table: &str,
    input: &Path,
    out_dir: &Path,
    opts: &BatchOptions,
) -> BatchItem {
    let name = input
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut item = match std::fs::read(input) {
        Ok(data) => batch::run_one(template, table, &name, &data, opts),
        Err(e) => BatchItem {
            name: name.clone(),
            error: Some(format!("cannot read {}: {e}", input.display())),
            ..Default::default()
        },
    };
    if let Err(e) = write_outputs(out_dir, &mut item) {
        item.error.get_or_insert(e);
    }
    item
}

/// Write `summary.csv` for the items into `out_dir`.
pub fn write_summary(out_dir: &Path, items: &[BatchItem]) -> Result<PathBuf, String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let p = out_dir.join("summary.csv");
    std::fs::write(&p, batch::summary_csv(&batch::summarise(items)))
        .map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(p)
}

/// Polls a folder for new data files. Files present when watching starts are left alone; a
/// new file is ready once two polls in a row see the same size.
#[derive(Debug)]
pub struct Watcher {
    dir: PathBuf,
    sizes: BTreeMap<PathBuf, u64>,
    done: BTreeSet<PathBuf>,
}

impl Watcher {
    /// Start watching `dir`.
    pub fn new(dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        let done = data_files(&dir)?.into_iter().collect();
        Ok(Watcher {
            dir,
            sizes: BTreeMap::new(),
            done,
        })
    }

    /// The watched folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// One poll: the files that became ready since the last one.
    pub fn poll(&mut self) -> std::io::Result<Vec<PathBuf>> {
        let mut ready = Vec::new();
        for p in data_files(&self.dir)? {
            if self.done.contains(&p) {
                continue;
            }
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            if size > 0 && self.sizes.get(&p) == Some(&size) {
                self.sizes.remove(&p);
                self.done.insert(p.clone());
                ready.push(p);
            } else {
                self.sizes.insert(p, size);
            }
        }
        Ok(ready)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_files_are_ready_once_stable() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/test-tmp")
            .join(format!("watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("old.csv"), "x\n1\n").unwrap();
        let mut w = Watcher::new(&dir).unwrap();
        assert!(
            w.poll().unwrap().is_empty(),
            "files already there are left alone"
        );
        std::fs::write(dir.join("new.csv"), "x\n1\n").unwrap();
        std::fs::write(dir.join(".hidden.csv"), "x\n").unwrap();
        std::fs::write(dir.join("notes.md"), "x\n").unwrap();
        assert!(w.poll().unwrap().is_empty(), "first sight");
        std::fs::write(dir.join("new.csv"), "x\n1\n2\n").unwrap();
        assert!(w.poll().unwrap().is_empty(), "still growing");
        assert_eq!(w.poll().unwrap(), [dir.join("new.csv")]);
        assert!(w.poll().unwrap().is_empty(), "reported once");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
