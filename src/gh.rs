use serde_json::Value;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::process::{Command, Stdio};

/// The two calls the sync makes to GitHub. Tests use `FakeGh`.
pub trait Gh {
    fn graphql(&self, query: &str) -> Result<Value, String>;
    fn set_status(&self, project_id: &str, item_id: &str, field_id: &str, option_id: &str) -> Result<(), String>;
}

/// Shells out to the authenticated `gh` CLI.
pub struct RealGh;

impl Gh for RealGh {
    fn graphql(&self, query: &str) -> Result<Value, String> {
        let out = Command::new("gh")
            .args(["api", "graphql", "-f", &format!("query={query}")])
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("cannot run gh: {e}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        serde_json::from_slice(&out.stdout).map_err(|e| format!("bad gh output: {e}"))
    }

    fn set_status(&self, project_id: &str, item_id: &str, field_id: &str, option_id: &str) -> Result<(), String> {
        let out = Command::new("gh")
            .args([
                "project", "item-edit", "--id", item_id, "--project-id", project_id,
                "--field-id", field_id, "--single-select-option-id", option_id,
            ])
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("cannot run gh: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    }
}

/// Canned responses in order, and a record of every status write.
#[derive(Default)]
pub struct FakeGh {
    pub pages: RefCell<VecDeque<Value>>,
    /// (project id, item id, field id, option id)
    pub pushes: RefCell<Vec<(String, String, String, String)>>,
}

impl FakeGh {
    pub fn with_pages(pages: Vec<Value>) -> FakeGh {
        FakeGh { pages: RefCell::new(pages.into()), pushes: RefCell::new(Vec::new()) }
    }
}

impl Gh for FakeGh {
    fn graphql(&self, _query: &str) -> Result<Value, String> {
        self.pages.borrow_mut().pop_front().ok_or_else(|| "no fake page left".to_string())
    }

    fn set_status(&self, project_id: &str, item_id: &str, field_id: &str, option_id: &str) -> Result<(), String> {
        self.pushes.borrow_mut().push((
            project_id.to_string(),
            item_id.to_string(),
            field_id.to_string(),
            option_id.to_string(),
        ));
        Ok(())
    }
}
