use crate::model::{BoardData, BoardKey, Item, Managed, NoteKey, STATUSES};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What the last sync saw: the note's status and each board's status for the item.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub status: Option<String>,
    /// Keyed by `owner/number`.
    pub boards: BTreeMap<String, BoardEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BoardEntry {
    pub item_id: String,
    pub status: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Write { key: NoteKey, managed: Managed },
    Push { board: BoardKey, item_id: String, status: String },
    Trash { key: NoteKey },
    Conflict { key: NoteKey, vault: String, github: String, board: BoardKey },
    Warn(String),
}

fn snapshot_from_github(present: &[(&BoardData, &Item)]) -> Snapshot {
    Snapshot {
        status: present[0].1.status.clone(),
        boards: present
            .iter()
            .map(|(bd, it)| (bd.key.to_string(), BoardEntry { item_id: it.item_id.clone(), status: it.status.clone() }))
            .collect(),
    }
}

/// Three-way merge of one item for one note.
///
/// `base` is the last-synced snapshot, `vault_status` is the note's current status,
/// `present` lists the boards holding the item in config order, and `note_exists`
/// says whether the note is on disk. Returns the actions and the new snapshot
/// (`None` when the note is trashed).
pub fn plan(
    key: &NoteKey,
    base: Option<&Snapshot>,
    vault_status: Option<&str>,
    present: &[(&BoardData, &Item)],
    note_exists: bool,
) -> (Vec<Action>, Option<Snapshot>) {
    if present.is_empty() {
        let acts = if note_exists { vec![Action::Trash { key: key.clone() }] } else { Vec::new() };
        return (acts, None);
    }
    let (first_board, first_item) = present[0];
    let gh_status = first_item.status.clone();
    let gh_str = gh_status.clone().unwrap_or_default();
    let gh_changed = match base {
        None => true,
        Some(b) => present.iter().any(|(bd, it)| match b.boards.get(&bd.key.to_string()) {
            None => true,
            Some(e) => e.status != it.status,
        }),
    };
    // A missing, empty, `~` or `null` status line is not a vault edit: GitHub's value is restored.
    // With no base snapshot (state file lost), any valid vault value counts as an edit,
    // so a differing GitHub status surfaces as a CONFLICT instead of a silent revert.
    let v_opt = vault_status.filter(|s| !matches!(*s, "" | "~" | "null"));
    let vault_changed = note_exists && v_opt.is_some() && base.map_or(true, |b| v_opt != b.status.as_deref());

    let mut acts = Vec::new();
    let mut snap = snapshot_from_github(present);
    let mut write_status = gh_str.clone();
    let mut pushed = false;

    if vault_changed {
        let v = v_opt.unwrap_or("");
        if !STATUSES.contains(&v) {
            acts.push(Action::Warn(format!(
                "{}: status '{}' is not Planned/In PR/Review/Done; left alone",
                key.to_string_key(),
                v
            )));
            write_status = v.to_string();
            // Keep the last-synced snapshot, so a GitHub move made while the typo sits
            // is still seen as a GitHub change once the note is fixed.
            snap = base.cloned().unwrap_or(snap);
        } else if !gh_changed {
            for (bd, it) in present {
                if it.status.as_deref() != Some(v) {
                    acts.push(Action::Push {
                        board: bd.key.clone(),
                        item_id: it.item_id.clone(),
                        status: v.to_string(),
                    });
                }
            }
            pushed = true;
            write_status = v.to_string();
            snap = Snapshot {
                status: Some(v.to_string()),
                boards: present
                    .iter()
                    .map(|(bd, it)| {
                        (bd.key.to_string(), BoardEntry { item_id: it.item_id.clone(), status: Some(v.to_string()) })
                    })
                    .collect(),
            };
        } else if gh_status.as_deref() != Some(v) {
            acts.push(Action::Conflict {
                key: key.clone(),
                vault: v.to_string(),
                github: gh_str.clone(),
                board: first_board.key.clone(),
            });
        }
    }

    if !pushed {
        let statuses: Vec<&Option<String>> = present.iter().map(|(_, it)| &it.status).collect();
        if statuses.iter().any(|s| *s != statuses[0]) {
            let list: Vec<String> = present
                .iter()
                .map(|(bd, it)| format!("{}={}", bd.key, it.status.as_deref().unwrap_or("")))
                .collect();
            acts.push(Action::Warn(format!(
                "{}: status differs across boards: {}; note shows the first",
                key.to_string_key(),
                list.join(", ")
            )));
        }
    }

    let mut boards: Vec<BoardKey> = present.iter().map(|(bd, _)| bd.key.clone()).collect();
    boards.sort();
    let (title, kind, state, repo, number, url, updated) = {
        let c = &first_item.content;
        (c.title.clone(), c.kind.clone(), c.state.clone(), c.repo.clone(), c.number, c.url.clone(), c.updated.clone())
    };
    let draft_id = match key {
        NoteKey::Draft { id } => Some(id.clone()),
        _ => None,
    };
    acts.push(Action::Write {
        key: key.clone(),
        managed: Managed {
            title,
            status: write_status,
            repo,
            number,
            kind,
            state,
            url,
            boards,
            updated,
            draft_id,
        },
    });
    (acts, Some(snap))
}
