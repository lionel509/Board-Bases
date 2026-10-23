use crate::merge::Snapshot;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Last-synced snapshots, keyed by the note key's string form.
pub type StateFile = BTreeMap<String, Snapshot>;

fn path(out: &Path) -> PathBuf {
    out.join(".board-bases").join("state.json")
}

/// A missing file is an empty state, so the first sync treats everything as new.
pub fn load(out: &Path) -> Result<StateFile, String> {
    match fs::read_to_string(path(out)) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!("bad state file: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(StateFile::new()),
        Err(e) => Err(format!("cannot read state file: {e}")),
    }
}

/// Writes the state only when its bytes differ from the file on disk.
pub fn save(out: &Path, st: &StateFile) -> Result<bool, String> {
    let mut text = serde_json::to_string_pretty(st).map_err(|e| e.to_string())?;
    text.push('\n');
    let p = path(out);
    if fs::read_to_string(&p).ok().as_deref() == Some(text.as_str()) {
        return Ok(false);
    }
    fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(&p, text).map_err(|e| e.to_string())?;
    Ok(true)
}
