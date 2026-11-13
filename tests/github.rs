mod support;

use board_bases::config::BoardConfig;
use board_bases::gh::FakeGh;
use board_bases::github::fetch_board;
use board_bases::model::{Kind, NoteKey, State};
use serde_json::json;
use support::*;

fn node(id: &str, archived: bool, status: Option<&str>, content: serde_json::Value) -> serde_json::Value {
    json!({
        "id": id,
        "updatedAt": "2026-10-02T00:00:00Z",
        "isArchived": archived,
        "fieldValueByName": status.map(|s| json!({"name": s})).unwrap_or(json!(null)),
        "content": content
    })
}

#[test]
fn parses_issue_pr_merged_and_draft_and_skips_archived() {
    let b = BoardConfig { owner: OWNER.into(), number: 1, name: "Alpha".into() };
    let issue = json!({"__typename": "Issue", "number": 1, "title": "Open issue", "url": "u1",
        "state": "OPEN", "updatedAt": "2026-10-02T00:00:00Z", "repository": {"nameWithOwner": REPO}});
    let merged = json!({"__typename": "PullRequest", "number": 2, "title": "Merged PR", "url": "u2",
        "state": "CLOSED", "merged": true, "updatedAt": "2026-10-02T00:00:00Z", "repository": {"nameWithOwner": REPO}});
    let pr = json!({"__typename": "PullRequest", "number": 3, "title": "Open PR", "url": "u3",
        "state": "OPEN", "merged": false, "updatedAt": "2026-10-02T00:00:00Z", "repository": {"nameWithOwner": REPO}});
    let draft = json!({"__typename": "DraftIssue", "id": "DI_1", "title": "Idea", "updatedAt": "2026-10-02T00:00:00Z"});
    let archived = json!({"__typename": "Issue", "number": 4, "title": "Old", "url": "u4",
        "state": "CLOSED", "updatedAt": "2026-10-02T00:00:00Z", "repository": {"nameWithOwner": REPO}});
    let page = project_page(&b, vec![
        node("I1", false, Some("Planned"), issue),
        node("I2", false, Some("Done"), merged),
        node("I3", false, None, pr),
        node("I4", false, Some("Review"), draft),
        node("I5", true, Some("Done"), archived),
    ]);
    let gh = FakeGh::with_pages(vec![page]);
    let data = fetch_board(&gh, &b).expect("fetch");
    assert_eq!(data.items.len(), 4, "archived item skipped");
    assert_eq!(data.items[0].content.kind, Kind::Issue);
    assert_eq!(data.items[0].content.state, State::Open);
    assert_eq!(data.items[1].content.state, State::Merged);
    assert_eq!(data.items[2].status, None);
    assert_eq!(data.items[2].content.kind, Kind::Pr);
    assert_eq!(data.items[3].content.kind, Kind::Draft);
    assert_eq!(data.items[3].content.key, NoteKey::Draft { id: "DI_1".into() });
}
