use board_bases::merge::{plan, Action, BoardEntry, Snapshot};
use board_bases::model::*;
use std::collections::BTreeMap;

fn key() -> NoteKey {
    NoteKey::Repo { repo: "acme/widgets".into(), number: 1 }
}

fn board(n: u32, status: Option<&str>) -> (BoardData, Item) {
    let content = Content {
        key: key(),
        title: "Item".into(),
        kind: Kind::Issue,
        state: State::Open,
        repo: "acme/widgets".into(),
        number: 1,
        url: "https://github.com/acme/widgets/issues/1".into(),
        updated: "2026-10-01T00:00:00Z".into(),
    };
    let item = Item { item_id: format!("I{n}"), status: status.map(String::from), content };
    let data = BoardData {
        key: BoardKey { owner: "acme-org".into(), number: n },
        name: format!("B{n}"),
        project_id: format!("PVT{n}"),
        status_field_id: "F".into(),
        option_ids: BTreeMap::new(),
        items: vec![item.clone()],
    };
    (data, item)
}

fn snap(status: &str, boards: &[(u32, &str)]) -> Snapshot {
    Snapshot {
        status: Some(status.into()),
        boards: boards
            .iter()
            .map(|(n, s)| {
                (format!("acme-org/{n}"), BoardEntry { item_id: format!("I{n}"), status: Some(s.to_string()) })
            })
            .collect(),
    }
}

fn pushes(acts: &[Action]) -> Vec<(u32, String)> {
    acts.iter()
        .filter_map(|a| match a {
            Action::Push { board, status, .. } => Some((board.number, status.clone())),
            _ => None,
        })
        .collect()
}

fn written_status(acts: &[Action]) -> String {
    acts.iter()
        .find_map(|a| match a {
            Action::Write { managed, .. } => Some(managed.status.clone()),
            _ => None,
        })
        .expect("a Write action")
}

#[test]
fn vault_only_change_pushes_to_every_board_holding_it() {
    let (a, ai) = board(1, Some("Planned"));
    let (b, bi) = board(2, Some("Planned"));
    let base = snap("Planned", &[(1, "Planned"), (2, "Planned")]);
    let (acts, new) = plan(&key(), Some(&base), Some("Review"), &[(&a, &ai), (&b, &bi)], true);
    assert_eq!(pushes(&acts), vec![(1, "Review".to_string()), (2, "Review".to_string())]);
    assert!(!acts.iter().any(|a| matches!(a, Action::Conflict { .. })));
    assert_eq!(written_status(&acts), "Review");
    assert_eq!(new.unwrap().status.as_deref(), Some("Review"));
}

#[test]
fn github_only_change_rewrites_note_and_pushes_nothing() {
    let (a, ai) = board(1, Some("Done"));
    let (b, bi) = board(2, Some("Done"));
    let base = snap("Planned", &[(1, "Planned"), (2, "Planned")]);
    let (acts, _) = plan(&key(), Some(&base), Some("Planned"), &[(&a, &ai), (&b, &bi)], true);
    assert!(pushes(&acts).is_empty());
    assert_eq!(written_status(&acts), "Done");
}

#[test]
fn both_changed_different_github_wins_and_reports_conflict() {
    let (a, ai) = board(1, Some("Done"));
    let base = snap("Planned", &[(1, "Planned")]);
    let (acts, _) = plan(&key(), Some(&base), Some("Review"), &[(&a, &ai)], true);
    assert!(acts.iter().any(|x| matches!(x, Action::Conflict { vault, github, .. } if vault == "Review" && github == "Done")));
    assert!(pushes(&acts).is_empty());
    assert_eq!(written_status(&acts), "Done");
}

#[test]
fn both_changed_same_value_no_push_no_conflict() {
    let (a, ai) = board(1, Some("Review"));
    let base = snap("Planned", &[(1, "Planned")]);
    let (acts, _) = plan(&key(), Some(&base), Some("Review"), &[(&a, &ai)], true);
    assert!(pushes(&acts).is_empty());
    assert!(!acts.iter().any(|x| matches!(x, Action::Conflict { .. })));
}

#[test]
fn removed_from_all_boards_trashes() {
    let base = snap("Planned", &[(1, "Planned")]);
    let (acts, new) = plan(&key(), Some(&base), Some("Planned"), &[], true);
    assert!(matches!(acts.as_slice(), [Action::Trash { .. }]));
    assert!(new.is_none());
}

#[test]
fn removed_from_one_board_keeps_note_and_pushes_to_the_rest() {
    let (a, ai) = board(1, Some("Planned"));
    let base = snap("Planned", &[(1, "Planned"), (2, "Planned")]);
    let (acts, _) = plan(&key(), Some(&base), Some("Review"), &[(&a, &ai)], true);
    assert_eq!(pushes(&acts), vec![(1, "Review".to_string())]);
    assert!(!acts.iter().any(|x| matches!(x, Action::Trash { .. })));
}

#[test]
fn invalid_vault_status_warns_and_does_not_push() {
    let (a, ai) = board(1, Some("Planned"));
    let base = snap("Planned", &[(1, "Planned")]);
    let (acts, _) = plan(&key(), Some(&base), Some("Blocked"), &[(&a, &ai)], true);
    assert!(acts.iter().any(|x| matches!(x, Action::Warn(_))));
    assert!(pushes(&acts).is_empty());
}

#[test]
fn first_sync_with_no_state_writes_and_never_pushes() {
    let (a, ai) = board(1, Some("Planned"));
    let (acts, new) = plan(&key(), None, None, &[(&a, &ai)], false);
    assert!(pushes(&acts).is_empty());
    assert_eq!(written_status(&acts), "Planned");
    assert!(new.is_some());
}
