use crate::model::BoardKey;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardConfig {
    pub owner: String,
    pub number: u32,
    pub name: String,
}

impl BoardConfig {
    pub fn key(&self) -> BoardKey {
        BoardKey { owner: self.owner.clone(), number: self.number }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Explicit output folder from the file; `None` falls back to the default.
    pub output: Option<PathBuf>,
    /// Board order here is the tie-break order when a note shows several boards.
    pub boards: Vec<BoardConfig>,
}

#[derive(Deserialize)]
struct RawBoard {
    owner: String,
    number: u32,
    name: String,
}

#[derive(Deserialize)]
struct RawConfig {
    output: Option<String>,
    boards: Vec<RawBoard>,
}

/// Expands a leading `~` with `$HOME`.
pub fn expand_home(p: &str) -> PathBuf {
    match p.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => {
            let home = std::env::var("HOME").unwrap_or_default();
            PathBuf::from(format!("{home}{rest}"))
        }
        _ => PathBuf::from(p),
    }
}

pub fn default_out() -> PathBuf {
    expand_home("~/Documents/Citadel/Boards")
}

pub fn load(path: &Path) -> Result<Config, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let raw: RawConfig = serde_json::from_str(&text).map_err(|e| format!("bad config {}: {e}", path.display()))?;
    Ok(Config {
        output: raw.output.as_deref().map(expand_home),
        boards: raw
            .boards
            .into_iter()
            .map(|b| BoardConfig { owner: b.owner, number: b.number, name: b.name })
            .collect(),
    })
}
