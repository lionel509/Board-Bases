#![allow(dead_code)]
// Shared fixtures: fake GraphQL pages shaped like `gh api graphql` output.

use board_bases::config::{BoardConfig, Config};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub const OWNER: &str = "acme-org";
pub const REPO: &str = "acme/widgets";

pub fn boards(n: u32) -> Vec<BoardConfig> {
    let names = ["Alpha", "Beta", "Gamma"];
    (1..=n)
        .map(|i| BoardConfig {
            owner: OWNER.into(),
            number: i,
            name: names[(i - 1) as usize].into(),
        })
        .collect()
}

pub fn config(n: u32) -> Config {
    Config { output: None, boards: boards(n) }
}

pub fn project_id(b: &BoardConfig) -> String {
    format!("PVT_{}{}", b.owner, b.number)
}

pub fn item_id(b: &BoardConfig, n: u32) -> String {
    format!("PVTI_{}{}_{}", b.owner, b.number, n)
}

pub fn option_id(status: &str) -> String {
    format!("OPT_{}", status.replace(' ', "_"))
}

/// One board's page. `items` is (issue number, Status name).
pub fn page(b: &BoardConfig, items: &[(u32, &str)]) -> Value {
    let nodes: Vec<Value> = items
        .iter()
        .map(|(n, s)| {
            json!({
                "id": item_id(b, *n),
                "updatedAt": "2026-10-01T00:00:00Z",
                "isArchived": false,
                "fieldValueByName": {"name": s},
                "content": {
                    "__typename": "Issue",
                    "number": n,
                    "title": format!("Item {n}"),
                    "url": format!("https://github.com/{REPO}/issues/{n}"),
                    "state": "OPEN",
                    "updatedAt": "2026-10-01T00:00:00Z",
                    "repository": {"nameWithOwner": REPO}
                }
            })
        })
        .collect();
    project_page(b, nodes)
}

pub fn project_page(b: &BoardConfig, nodes: Vec<Value>) -> Value {
    json!({"data": {
        "rateLimit": {"remaining": 4000},
        "repositoryOwner": {"projectV2": {
            "id": project_id(b),
            "title": b.name,
            "field": {"id": "FLD_STATUS", "options": [
                {"id": option_id("Planned"), "name": "Planned"},
                {"id": option_id("In PR"), "name": "In PR"},
                {"id": option_id("Review"), "name": "Review"},
                {"id": option_id("Done"), "name": "Done"}
            ]},
            "items": {"pageInfo": {"hasNextPage": false, "endCursor": null}, "nodes": nodes}
        }}
    }})
}

pub fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("board-bases-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

pub fn note_path(out: &Path, n: u32) -> PathBuf {
    out.join(format!("notes/acme/widgets {n}.md"))
}

/// Every file under `dir`, recursively.
pub fn all_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                found.extend(all_files(&p));
            } else {
                found.push(p);
            }
        }
    }
    found.sort();
    found
}
