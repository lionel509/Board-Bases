use board_bases::base::render;
use board_bases::config::BoardConfig;
use yaml_rust2::{Yaml, YamlLoader};

fn board() -> BoardConfig {
    BoardConfig { owner: "lionel509".into(), number: 4, name: "Obsidian Tooling".into() }
}

#[test]
fn render_matches_fixture() {
    let expected = "filters:
  and:
    - 'boards.contains(\"lionel509/4\")'
views:
  - type: kanban
    name: Board
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
      - note.status
      - note.repo
      - note.number
      - note.type
      - note.state
      - note.updated
  - type: table
    name: Table
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
      - note.status
      - note.repo
      - note.number
      - note.type
      - note.state
      - note.updated
";
    assert_eq!(render(&board()), expected);
}

#[test]
fn parsed_views_are_kanban_then_table_grouped_by_status() {
    let doc = YamlLoader::load_from_str(&render(&board())).expect("generated .base is valid YAML");
    let views: &Vec<Yaml> = doc[0]["views"].as_vec().expect("views is a list");
    assert_eq!(views.len(), 2, "kanban then table");

    assert_eq!(views[0]["type"].as_str(), Some("kanban"));
    assert_eq!(views[0]["name"].as_str(), Some("Board"));
    assert_eq!(views[1]["type"].as_str(), Some("table"));
    assert_eq!(views[1]["name"].as_str(), Some("Table"));

    for view in views {
        assert!(view["groupBy"].as_hash().is_some(), "groupBy must be an object, not a string");
        assert_eq!(view["groupBy"]["property"].as_str(), Some("note.status"));
        assert_eq!(view["groupBy"]["direction"].as_str(), Some("ASC"));
        let order: Vec<&str> = view["groupOrder"]
            .as_vec()
            .expect("groupOrder is a list")
            .iter()
            .map(|y| y.as_str().expect("groupOrder item is a string"))
            .collect();
        assert_eq!(order, ["Planned", "In PR", "Review", "Done"]);
    }
}
