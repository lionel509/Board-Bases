use board_bases::model::{BoardKey, Kind, Managed, State};
use board_bases::note::parse;

fn managed(status: &str) -> Managed {
    Managed {
        title: "Loanword".into(),
        status: status.into(),
        repo: "lionel509/Loanword".into(),
        number: 4,
        kind: Kind::Issue,
        state: State::Open,
        url: "https://github.com/lionel509/Loanword/issues/4".into(),
        boards: vec![BoardKey { owner: "lionel509".into(), number: 4 }],
        updated: "2026-10-01T00:00:00Z".into(),
        draft_id: None,
    }
}

#[test]
fn parse_apply_render_preserves_extra_keys_and_body() {
    let text = "---\ntitle: \"Old\"\npriority: high\nstatus: Planned\nboards:\n  - lionel509/4\n---\nBody line [[Link]]\nmore\n";
    let mut n = parse(text);
    let m = managed("In PR");
    assert!(n.needs_write(&m));
    n.apply(&m);
    let out = n.render();
    assert!(out.contains("priority: high\n"));
    assert!(out.contains("status: In PR\n"));
    assert!(out.ends_with("Body line [[Link]]\nmore\n"));
    assert!(!n.needs_write(&m));
}

#[test]
fn needs_write_false_when_only_quoting_differs() {
    let m = managed("In PR");
    let mut n = parse("");
    n.apply(&m);
    let text = n.render().replace("status: In PR", "status: \"In PR\"");
    assert!(!parse(&text).needs_write(&m));
}

#[test]
fn boards_inline_and_block_lists_parse_equal() {
    let block = parse("---\nboards:\n  - lionel509/4\n  - Applied-Dynamics/1\n---\n");
    let inline = parse("---\nboards: [lionel509/4, \"Applied-Dynamics/1\"]\n---\n");
    assert_eq!(block.get_list("boards"), vec!["lionel509/4", "Applied-Dynamics/1"]);
    assert_eq!(inline.get_list("boards"), block.get_list("boards"));
}
