use crate::config::BoardConfig;

/// The `.base` file for one board: a kanban grouped by status, plus a table fallback
/// for Obsidian builds that do not know the kanban view.
pub fn render(board: &BoardConfig) -> String {
    let key = format!("{}/{}", board.owner, board.number);
    let name = &board.name;
    format!(
        "filters:
  and:
    - 'boards.contains(\"{key}\")'
views:
  - type: kanban
    name: {name}
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
"
    )
}
