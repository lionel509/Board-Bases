use crate::config::BoardConfig;

/// The `.base` file for one board: a kanban view grouped by status (Obsidian 1.14.5+),
/// then a table view of the same items with the same grouping.
pub fn render(board: &BoardConfig) -> String {
    let key = format!("{}/{}", board.owner, board.number);
    format!(
        "filters:
  and:
    - 'boards.contains(\"{key}\")'
views:
{kanban}{table}",
        kanban = view("kanban", "Board"),
        table = view("table", "Table"),
    )
}

/// One view block. Both kinds share the status grouping and the column order.
fn view(kind: &str, name: &str) -> String {
    let mut out = format!(
        "  - type: {kind}
    name: {name}
    groupBy:
      property: note.status
      direction: ASC
    groupOrder:
"
    );
    for status in crate::model::STATUSES {
        out.push_str(&format!("      - {status}\n"));
    }
    out.push_str(
        "    order:
      - note.title
      - note.status
      - note.repo
      - note.number
      - note.type
      - note.state
      - note.updated
",
    );
    out
}
