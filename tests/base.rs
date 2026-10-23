use board_bases::base::render;
use board_bases::config::BoardConfig;

#[test]
fn render_matches_fixture() {
    let b = BoardConfig { owner: "lionel509".into(), number: 4, name: "Obsidian Tooling".into() };
    let expected = "filters:
  and:
    - 'boards.contains(\"lionel509/4\")'
views:
  - type: kanban
    name: Obsidian Tooling
    groupBy:
      property: note.status
      direction: ASC
    groupOrder:
      - Planned
      - In PR
      - Review
      - Done
    order:
      - note.title
      - note.repo
      - note.number
      - note.type
      - note.state
  - type: table
    name: Table
    groupBy:
      property: note.status
      direction: ASC
    order:
      - note.title
      - note.status
      - note.repo
      - note.number
      - note.type
      - note.state
      - note.updated
";
    assert_eq!(render(&b), expected);
}
