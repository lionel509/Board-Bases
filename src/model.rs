use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

pub const STATUSES: [&str; 4] = ["Planned", "In PR", "Review", "Done"];

/// A board as `owner/number`, e.g. `lionel509/4`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BoardKey {
    pub owner: String,
    pub number: u32,
}

impl fmt::Display for BoardKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.number)
    }
}

/// Identity of a note: an issue or PR (`repo` + `number`) or a draft item.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NoteKey {
    Repo { repo: String, number: u32 },
    Draft { id: String },
}

impl NoteKey {
    /// The string form used as the key in the state file.
    pub fn to_string_key(&self) -> String {
        match self {
            NoteKey::Repo { repo, number } => format!("{repo}#{number}"),
            NoteKey::Draft { id } => format!("draft:{id}"),
        }
    }

    pub fn parse(s: &str) -> Option<NoteKey> {
        if let Some(id) = s.strip_prefix("draft:") {
            return Some(NoteKey::Draft { id: id.to_string() });
        }
        let (repo, n) = s.rsplit_once('#')?;
        Some(NoteKey::Repo { repo: repo.to_string(), number: n.parse().ok()? })
    }

    /// Path of the note, relative to the output folder.
    pub fn rel_path(&self) -> PathBuf {
        match self {
            NoteKey::Repo { repo, number } => {
                let (owner, name) = repo.split_once('/').unwrap_or(("_", repo.as_str()));
                PathBuf::from(format!("notes/{owner}/{name} {number}.md"))
            }
            NoteKey::Draft { id } => PathBuf::from(format!("notes/drafts/{id}.md")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Issue,
    Pr,
    Draft,
}

impl Kind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Kind::Issue => "issue",
            Kind::Pr => "pr",
            Kind::Draft => "draft",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Open,
    Closed,
    Merged,
}

impl State {
    pub fn as_str(&self) -> &'static str {
        match self {
            State::Open => "open",
            State::Closed => "closed",
            State::Merged => "merged",
        }
    }
}

/// What a note shows about its item. Drafts have empty repo, number 0, empty url.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Content {
    pub key: NoteKey,
    pub title: String,
    pub kind: Kind,
    pub state: State,
    pub repo: String,
    pub number: u32,
    pub url: String,
    pub updated: String,
}

/// One item as it sits on one board.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub item_id: String,
    pub status: Option<String>,
    pub content: Content,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardData {
    pub key: BoardKey,
    pub name: String,
    pub project_id: String,
    pub status_field_id: String,
    pub option_ids: BTreeMap<String, String>,
    pub items: Vec<Item>,
}

/// The frontmatter fields the sync owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Managed {
    pub title: String,
    pub status: String,
    pub repo: String,
    pub number: u32,
    pub kind: Kind,
    pub state: State,
    pub url: String,
    pub boards: Vec<BoardKey>,
    pub updated: String,
    pub draft_id: Option<String>,
}
