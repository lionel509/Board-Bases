mod support;

use board_bases::gh::FakeGh;
use board_bases::note;
use board_bases::sync;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;
use support::*;

fn run_sync(gh: &FakeGh, out: &Path) -> String {
    let mut log: Vec<u8> = Vec::new();
    sync::run(gh, &config(1), out, false, &mut log).unwrap();
    String::from_utf8(log).unwrap()
}

fn one_board(items: &[(u32, &str)]) -> FakeGh {
    FakeGh::with_pages(vec![page(&boards(1)[0], items)])
}

/// Break 1: a note whose `repo:` line the user deleted is treated as unknown,
/// so the sync writes a fresh note over the same path and drops the user's body.
#[test]
fn breaker_deleted_repo_line_keeps_user_body() {
    let out = tmp("breaker-repo-line");
    run_sync(&one_board(&[(1, "Planned")]), &out);
    let p = note_path(&out, 1);
    let text = fs::read_to_string(&p).unwrap();
    let edited = text.replace("repo: acme/widgets\n", "") + "\nMy own notes on this item.\n";
    fs::write(&p, &edited).unwrap();
    run_sync(&one_board(&[(1, "Planned")]), &out);
    let after = fs::read_to_string(&p).unwrap_or_default();
    assert!(after.contains("My own notes on this item."), "user body lost, file now:\n{after}");
}

/// Break 2: GitHub returns partial GraphQL errors (data present, one item's
/// content null because it is not visible). The item is dropped, so its note
/// is moved to .trash as if removed from the board.
#[test]
fn breaker_partial_graphql_errors_do_not_trash_notes() {
    let out = tmp("breaker-partial-errors");
    let b = boards(1)[0].clone();
    run_sync(&one_board(&[(1, "Planned"), (2, "Review")]), &out);
    assert!(note_path(&out, 2).exists());
    let inaccessible = json!({
        "id": item_id(&b, 2),
        "updatedAt": "2026-10-01T00:00:00Z",
        "isArchived": false,
        "fieldValueByName": {"name": "Review"},
        "content": null
    });
    let ok = page(&b, &[(1, "Planned")]);
    let mut resp: Value = ok.clone();
    resp["data"]["repositoryOwner"]["projectV2"]["items"]["nodes"] =
        json!([ok["data"]["repositoryOwner"]["projectV2"]["items"]["nodes"][0].clone(), inaccessible]);
    resp["errors"] = json!([{"type": "NOT_FOUND", "message": "Could not resolve to a node"}]);
    // Partial errors abort the whole fetch before any write, so nothing is trashed.
    let r = sync::run(&FakeGh::with_pages(vec![resp]), &config(1), &out, false, &mut Vec::new());
    assert!(r.is_err(), "partial GraphQL errors must abort the sync");
    assert!(note_path(&out, 2).exists(), "note 2 was trashed on a partial error");
}

/// Break 3: the user deletes the `status:` line. The sync treats the empty value
/// as a vault edit, warns, and keeps the note blank forever even though GitHub
/// still says Review.
#[test]
fn breaker_deleted_status_line_is_restored_from_github() {
    let out = tmp("breaker-status-line");
    run_sync(&one_board(&[(1, "Review")]), &out);
    let p = note_path(&out, 1);
    let text = fs::read_to_string(&p).unwrap();
    fs::write(&p, text.replace("status: Review\n", "")).unwrap();
    run_sync(&one_board(&[(1, "Review")]), &out);
    let st = note::parse(&fs::read_to_string(&p).unwrap()).get_scalar("status");
    assert_eq!(st.as_deref(), Some("Review"));
}

/// Break 4: a typo in the vault (`done`) freezes the note (plan: invalid values are
/// left alone, so the note keeps the typo while it sits). A GitHub move to Review
/// meanwhile must not be lost: fixing the typo to Done must be a CONFLICT, not a
/// silent push over Review.
#[test]
fn breaker_typo_then_fix_overwrites_github_move() {
    let out = tmp("breaker-typo");
    run_sync(&one_board(&[(1, "Planned")]), &out);
    let p = note_path(&out, 1);
    let text = fs::read_to_string(&p).unwrap();
    fs::write(&p, text.replace("status: Planned\n", "status: done\n")).unwrap();

    let gh = one_board(&[(1, "Review")]);
    run_sync(&gh, &out);
    let mid = note::parse(&fs::read_to_string(&p).unwrap()).get_scalar("status");
    assert_eq!(mid.as_deref(), Some("done"), "invalid vault value must be left alone");

    let text = fs::read_to_string(&p).unwrap();
    fs::write(&p, text.replace("status: done\n", "status: Done\n")).unwrap();
    let gh2 = one_board(&[(1, "Review")]);
    let log = run_sync(&gh2, &out);
    assert!(gh2.pushes.borrow().is_empty(), "Done pushed over GitHub's Review silently; log:\n{log}");
    assert!(log.contains("CONFLICT"), "expected CONFLICT for Done vs Review; log:\n{log}");
}

/// Break 5: the state file is lost (deleted, or the output folder is restored
/// from an older backup). With no base snapshot the vault drag is silently
/// reverted to GitHub's status, with no push and no CONFLICT line.
#[test]
fn breaker_lost_state_file_does_not_silently_revert_drag() {
    let out = tmp("breaker-state-lost");
    run_sync(&one_board(&[(1, "Planned")]), &out);
    let p = note_path(&out, 1);
    let text = fs::read_to_string(&p).unwrap();
    fs::write(&p, text.replace("status: Planned\n", "status: Done\n")).unwrap();
    fs::remove_file(out.join(".board-bases/state.json")).unwrap();
    let gh = one_board(&[(1, "Planned")]);
    let log = run_sync(&gh, &out);
    let pushed = gh.pushes.borrow().len();
    assert!(pushed == 1 || log.contains("CONFLICT"), "drag silently reverted; log:\n{log}");
}

/// Break 6: a title with quotes and backslashes must round-trip through the
/// frontmatter so the second sync writes nothing.
#[test]
fn breaker_tricky_titles_round_trip() {
    for title in ["Fix \"quoted\" thing", "path C:\\new\\x", "ends with backslash \\", "say \\\"hi\\\"", "it's 'x' ''y''"] {
        let out = tmp("breaker-title");
        let mut b = one_board(&[(1, "Planned")]);
        let mut page_v = page(&boards(1)[0], &[(1, "Planned")]);
        page_v["data"]["repositoryOwner"]["projectV2"]["items"]["nodes"][0]["content"]["title"] = json!(title);
        b.pages = std::cell::RefCell::new(vec![page_v.clone()].into());
        run_sync(&b, &out);
        let text = fs::read_to_string(note_path(&out, 1)).unwrap();
        let got = note::parse(&text).get_scalar("title");
        assert_eq!(got.as_deref(), Some(title), "title did not round-trip");
        let again = FakeGh::with_pages(vec![page_v]);
        let before = fs::metadata(note_path(&out, 1)).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let log = run_sync(&again, &out);
        again.pages.borrow_mut().clear();
        let after = fs::metadata(note_path(&out, 1)).unwrap().modified().unwrap();
        assert_eq!(before, after, "second sync rewrote note for {title:?}; log:\n{log}");
    }
}

/// Break 7: a note saved with CRLF line endings (a Windows editor or a sync
/// tool) does not parse as frontmatter, so it is treated as an unknown note and
/// the sync overwrites it. The user's text is lost.
#[test]
fn breaker_crlf_note_is_not_clobbered() {
    let out = tmp("breaker-crlf");
    run_sync(&one_board(&[(1, "Planned")]), &out);
    let p = note_path(&out, 1);
    let text = fs::read_to_string(&p).unwrap().replace('\n', "\r\n") + "my CRLF body\r\n";
    fs::write(&p, text).unwrap();
    run_sync(&one_board(&[(1, "Planned")]), &out);
    let after = fs::read_to_string(&p).unwrap_or_default();
    assert!(after.contains("my CRLF body"), "CRLF note overwritten, now:\n{after}");
}

/// Probe: pagination across two pages (more than 100 items on a board).
#[test]
fn breaker_pagination_merges_both_pages() {
    let out = tmp("breaker-pages");
    let b = boards(1)[0].clone();
    let mut p1 = page(&b, &[(1, "Planned")]);
    p1["data"]["repositoryOwner"]["projectV2"]["items"]["pageInfo"] = json!({"hasNextPage": true, "endCursor": "C1"});
    let p2 = page(&b, &[(2, "Done")]);
    let gh = FakeGh::with_pages(vec![p1, p2]);
    run_sync(&gh, &out);
    assert!(note_path(&out, 1).exists());
    assert!(note_path(&out, 2).exists(), "second page items missing");
}

/// Break 4b: same typo sequence, checking only the second half (the push).
#[test]
fn breaker_typo_fix_pushes_done_over_github_review() {
    let out = tmp("breaker-typo-b");
    run_sync(&one_board(&[(1, "Planned")]), &out);
    let p = note_path(&out, 1);
    let text = fs::read_to_string(&p).unwrap();
    fs::write(&p, text.replace("status: Planned\n", "status: done\n")).unwrap();
    run_sync(&one_board(&[(1, "Review")]), &out);
    let text = fs::read_to_string(&p).unwrap();
    fs::write(&p, text.replace("status: done\n", "status: Done\n")).unwrap();
    let gh2 = one_board(&[(1, "Review")]);
    let log = run_sync(&gh2, &out);
    assert!(gh2.pushes.borrow().is_empty(), "Done pushed over GitHub's Review silently; log:\n{log}");
}
