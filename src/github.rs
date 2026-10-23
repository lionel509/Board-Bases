use crate::config::BoardConfig;
use crate::gh::Gh;
use crate::model::{BoardData, BoardKey, Content, Item, Kind, NoteKey, State};
use serde_json::Value;
use std::collections::BTreeMap;

/// Below this many GraphQL points left, no sync writes anything.
const MIN_BUDGET: u64 = 200;

fn query(owner: &str, number: u32, after: Option<&str>) -> String {
    let after_arg = after.map(|c| format!(", after: \"{c}\"")).unwrap_or_default();
    format!(
        r#"query {{ rateLimit {{ remaining }} repositoryOwner(login: "{owner}") {{ ... on ProjectV2Owner {{ projectV2(number: {number}) {{ id title field(name: "Status") {{ ... on ProjectV2SingleSelectField {{ id options {{ id name }} }} }} items(first: 100{after_arg}) {{ pageInfo {{ hasNextPage endCursor }} nodes {{ id updatedAt isArchived fieldValueByName(name: "Status") {{ ... on ProjectV2ItemFieldSingleSelectValue {{ name }} }} content {{ __typename ... on Issue {{ id number title url state updatedAt repository {{ nameWithOwner }} }} ... on PullRequest {{ id number title url state merged updatedAt repository {{ nameWithOwner }} }} ... on DraftIssue {{ id title updatedAt }} }} }} }} }} }} }} }}"#
    )
}

fn parse_item(n: &Value) -> Option<Item> {
    if n["isArchived"].as_bool().unwrap_or(false) {
        return None;
    }
    let c = &n["content"];
    let item_id = n["id"].as_str()?.to_string();
    let status = n["fieldValueByName"]["name"].as_str().map(String::from);
    let updated = c["updatedAt"].as_str().unwrap_or_default().to_string();
    let open_or_closed = |s: &Value| match s.as_str() {
        Some("CLOSED") => State::Closed,
        _ => State::Open,
    };
    let content = match c["__typename"].as_str()? {
        "DraftIssue" => {
            let id = c["id"].as_str()?.to_string();
            Content {
                key: NoteKey::Draft { id },
                title: c["title"].as_str().unwrap_or_default().to_string(),
                kind: Kind::Draft,
                state: State::Open,
                repo: String::new(),
                number: 0,
                url: String::new(),
                updated,
            }
        }
        typ @ ("Issue" | "PullRequest") => {
            let repo = c["repository"]["nameWithOwner"].as_str()?.to_string();
            let number = c["number"].as_u64()? as u32;
            let state = if typ == "PullRequest" && c["merged"].as_bool() == Some(true) {
                State::Merged
            } else {
                open_or_closed(&c["state"])
            };
            Content {
                key: NoteKey::Repo { repo: repo.clone(), number },
                title: c["title"].as_str().unwrap_or_default().to_string(),
                kind: if typ == "Issue" { Kind::Issue } else { Kind::Pr },
                state,
                repo,
                number,
                url: c["url"].as_str().unwrap_or_default().to_string(),
                updated,
            }
        }
        _ => return None,
    };
    Some(Item { item_id, status, content })
}

/// Fetches one board, all pages. Returns the board and the GraphQL budget left.
fn fetch_inner(gh: &dyn Gh, b: &BoardConfig) -> Result<(BoardData, Option<u64>), String> {
    let mut items = Vec::new();
    let mut after: Option<String> = None;
    let mut header: Option<BoardData> = None;
    let mut budget = None;
    loop {
        let resp = gh.graphql(&query(&b.owner, b.number, after.as_deref()))?;
        if let Some(e) = resp.get("errors") {
            if resp["data"].is_null() {
                return Err(format!("{}/{}: {}", b.owner, b.number, e));
            }
        }
        let data = &resp["data"];
        if budget.is_none() {
            budget = data["rateLimit"]["remaining"].as_u64();
        }
        let proj = &data["repositoryOwner"]["projectV2"];
        if proj.is_null() {
            return Err(format!("board {}/{} not found", b.owner, b.number));
        }
        if header.is_none() {
            let options = proj["field"]["options"].as_array().cloned().unwrap_or_default();
            let option_ids: BTreeMap<String, String> = options
                .iter()
                .filter_map(|o| Some((o["name"].as_str()?.to_string(), o["id"].as_str()?.to_string())))
                .collect();
            header = Some(BoardData {
                key: BoardKey { owner: b.owner.clone(), number: b.number },
                name: b.name.clone(),
                project_id: proj["id"].as_str().unwrap_or_default().to_string(),
                status_field_id: proj["field"]["id"].as_str().unwrap_or_default().to_string(),
                option_ids,
                items: Vec::new(),
            });
        }
        let page = &proj["items"];
        for n in page["nodes"].as_array().cloned().unwrap_or_default() {
            if let Some(it) = parse_item(&n) {
                items.push(it);
            }
        }
        if page["pageInfo"]["hasNextPage"].as_bool() == Some(true) {
            after = page["pageInfo"]["endCursor"].as_str().map(String::from);
            if after.is_none() {
                return Err(format!("{}/{}: paging stopped without a cursor", b.owner, b.number));
            }
        } else {
            break;
        }
    }
    let mut board = header.expect("first page sets the header");
    board.items = items;
    Ok((board, budget))
}

pub fn fetch_board(gh: &dyn Gh, b: &BoardConfig) -> Result<BoardData, String> {
    fetch_inner(gh, b).map(|(d, _)| d)
}

/// Fetches every configured board, in config order. Any failure aborts the whole fetch.
pub fn fetch_all(gh: &dyn Gh, boards: &[BoardConfig]) -> Result<Vec<BoardData>, String> {
    let mut out = Vec::new();
    for (i, b) in boards.iter().enumerate() {
        let (data, budget) = fetch_inner(gh, b)?;
        if i == 0 {
            if let Some(left) = budget {
                if left < MIN_BUDGET {
                    return Err(format!("GraphQL budget at {left}, retry after the hourly reset"));
                }
            }
        }
        out.push(data);
    }
    Ok(out)
}
