use crate::model::{BoardKey, Managed};

/// A note split into its frontmatter entries and its verbatim body.
/// Each entry is one top-level key with its raw continuation lines, so keys the
/// user adds (and their order) survive a rewrite.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Note {
    pub entries: Vec<(String, Vec<String>)>,
    pub body: String,
}

fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        return s[1..s.len() - 1].replace("\\\"", "\"").replace("\\\\", "\\");
    }
    if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
        return s[1..s.len() - 1].replace("''", "'");
    }
    s.to_string()
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Splits a note into frontmatter and body. A file without a closing `---` is all body.
pub fn parse(text: &str) -> Note {
    let mut lines = text.split_inclusive('\n');
    if lines.next() != Some("---\n") {
        return Note { entries: Vec::new(), body: text.to_string() };
    }
    let mut fm: Vec<&str> = Vec::new();
    let mut consumed = "---\n".len();
    let mut closed = false;
    for l in lines {
        consumed += l.len();
        if l == "---\n" || l == "---" {
            closed = true;
            break;
        }
        fm.push(l.trim_end_matches('\n'));
    }
    if !closed {
        return Note { entries: Vec::new(), body: text.to_string() };
    }
    let mut entries: Vec<(String, Vec<String>)> = Vec::new();
    for l in fm {
        let is_continuation = l.starts_with(' ') || l.starts_with('\t') || l.starts_with('-') || !l.contains(':');
        if is_continuation {
            match entries.last_mut() {
                Some((_, v)) => v.push(l.to_string()),
                None => entries.push((String::new(), vec![l.to_string()])),
            }
        } else {
            let key = l.split(':').next().unwrap_or_default().trim().to_string();
            entries.push((key, vec![l.to_string()]));
        }
    }
    Note { entries, body: text[consumed..].to_string() }
}

impl Note {
    fn entry(&self, key: &str) -> Option<&Vec<String>> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn get_scalar(&self, key: &str) -> Option<String> {
        let lines = self.entry(key)?;
        let first = lines.first()?;
        let value = first.splitn(2, ':').nth(1).unwrap_or("");
        Some(strip_quotes(value))
    }

    pub fn get_list(&self, key: &str) -> Vec<String> {
        let Some(lines) = self.entry(key) else { return Vec::new() };
        let first = lines[0].splitn(2, ':').nth(1).unwrap_or("").trim();
        if let Some(inner) = first.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            return inner.split(',').map(strip_quotes).filter(|s| !s.is_empty()).collect();
        }
        lines[1..]
            .iter()
            .filter_map(|l| l.trim_start().strip_prefix("- ").map(|v| strip_quotes(v)))
            .collect()
    }

    /// Rendered lines for each managed key, in the order they are appended when missing.
    fn managed_lines(m: &Managed) -> Vec<(&'static str, Vec<String>)> {
        let mut boards = m.boards.clone();
        boards.sort();
        let mut board_lines = vec!["boards:".to_string()];
        board_lines.extend(boards.iter().map(|b: &BoardKey| format!("  - {b}")));
        let mut v = vec![
            ("title", vec![format!("title: {}", quote(&m.title))]),
            ("status", vec![if m.status.is_empty() { "status: \"\"".into() } else { format!("status: {}", m.status) }]),
            ("repo", vec![if m.repo.is_empty() { "repo: \"\"".into() } else { format!("repo: {}", m.repo) }]),
            ("number", vec![format!("number: {}", m.number)]),
            ("type", vec![format!("type: {}", m.kind.as_str())]),
            ("state", vec![format!("state: {}", m.state.as_str())]),
            ("url", vec![if m.url.is_empty() { "url: \"\"".into() } else { format!("url: {}", m.url) }]),
            ("boards", board_lines),
            ("updated", vec![format!("updated: {}", m.updated)]),
        ];
        if let Some(id) = &m.draft_id {
            v.push(("draft_id", vec![format!("draft_id: {id}")]));
        }
        v
    }

    /// Sets every managed key to its value. Existing keys keep their position; unknown keys are untouched.
    pub fn apply(&mut self, m: &Managed) {
        for (key, lines) in Self::managed_lines(m) {
            match self.entries.iter().position(|(k, _)| k == key) {
                Some(i) => self.entries[i] = (key.to_string(), lines),
                None => self.entries.push((key.to_string(), lines)),
            }
        }
    }

    /// True when a managed value differs from `m`. Quoting alone does not count.
    pub fn needs_write(&self, m: &Managed) -> bool {
        let scalar = |k: &str, want: &str| self.get_scalar(k).as_deref() != Some(want);
        if scalar("title", &m.title)
            || scalar("status", &m.status)
            || scalar("repo", &m.repo)
            || scalar("type", m.kind.as_str())
            || scalar("state", m.state.as_str())
            || scalar("url", &m.url)
            || scalar("updated", &m.updated)
        {
            return true;
        }
        if self.get_scalar("number").and_then(|s| s.parse::<u32>().ok()) != Some(m.number) {
            return true;
        }
        if let Some(id) = &m.draft_id {
            if scalar("draft_id", id) {
                return true;
            }
        }
        let mut have = self.get_list("boards");
        have.sort();
        let mut want: Vec<String> = m.boards.iter().map(|b| b.to_string()).collect();
        want.sort();
        self.entry("boards").is_none() || have != want
    }

    pub fn render(&self) -> String {
        let mut s = String::new();
        if !self.entries.is_empty() {
            s.push_str("---\n");
            for (_, lines) in &self.entries {
                for l in lines {
                    s.push_str(l);
                    s.push('\n');
                }
            }
            s.push_str("---\n");
        }
        s.push_str(&self.body);
        s
    }
}
