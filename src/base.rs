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
  - type: table
    name: {name}
    groupBy: note.status
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
