use crate::base;
use crate::config::Config;
use crate::gh::Gh;
use crate::github;
use crate::merge::{plan, Action};
use crate::model::{BoardData, Item, NoteKey};
use crate::note::{parse, Note};
use crate::state::{self, StateFile};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct Report {
    pub written: Vec<PathBuf>,
    pub pushed: usize,
    pub trashed: Vec<PathBuf>,
    pub bases: Vec<PathBuf>,
    pub conflicts: usize,
    pub warnings: usize,
}

fn md_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                found.extend(md_files(&p));
            } else if p.extension().map(|x| x == "md").unwrap_or(false) {
                found.push(p);
            }
        }
    }
    found.sort();
    found
}

/// First free name in `.trash`: `name.md`, then `name (2).md`, `name (3).md`, ...
fn unique_dest(dest: PathBuf) -> PathBuf {
    if !dest.exists() {
        return dest;
    }
    let stem = dest.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let ext = dest.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
    for i in 2.. {
        let name = if ext.is_empty() { format!("{stem} ({i})") } else { format!("{stem} ({i}).{ext}") };
        let candidate = dest.with_file_name(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn note_key(n: &Note) -> Option<NoteKey> {
    if let Some(id) = n.get_scalar("draft_id") {
        return Some(NoteKey::Draft { id });
    }
    let repo = n.get_scalar("repo").filter(|r| !r.is_empty())?;
    let number = n.get_scalar("number")?.parse().ok()?;
    Some(NoteKey::Repo { repo, number })
}

/// One sync pass: fetch everything, plan per note, then apply (unless `dry_run`).
pub fn run(gh: &dyn Gh, cfg: &Config, out: &Path, dry_run: bool, log: &mut dyn Write) -> Result<Report, String> {
    let boards = github::fetch_all(gh, &cfg.boards)?;
    let st = state::load(out)?;
    let mut report = Report::default();

    let mut index: BTreeMap<NoteKey, Vec<(usize, usize)>> = BTreeMap::new();
    for (bi, bd) in boards.iter().enumerate() {
        for (ii, it) in bd.items.iter().enumerate() {
            index.entry(it.content.key.clone()).or_default().push((bi, ii));
        }
    }

    let mut disk: BTreeMap<NoteKey, (PathBuf, Note)> = BTreeMap::new();
    for path in md_files(&out.join("notes")) {
        let text = fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let note = parse(&text);
        let rel = path.strip_prefix(out).unwrap_or(&path).display().to_string();
        match note_key(&note) {
            Some(k) if !disk.contains_key(&k) => {
                disk.insert(k, (path, note));
            }
            Some(_) => {
                let _ = writeln!(log, "WARN duplicate note {rel}, ignored");
                report.warnings += 1;
            }
            None => {
                let _ = writeln!(log, "WARN unknown note {rel}");
                report.warnings += 1;
            }
        }
    }

    let mut keys: BTreeMap<String, NoteKey> = BTreeMap::new();
    for k in index.keys().chain(disk.keys()) {
        keys.entry(k.to_string_key()).or_insert_with(|| k.clone());
    }
    for s in st.keys() {
        if let Some(k) = NoteKey::parse(s) {
            keys.entry(s.clone()).or_insert(k);
        }
    }

    let mut new_state = StateFile::new();
    for (ks, key) in &keys {
        let present: Vec<(&BoardData, &Item)> = index
            .get(key)
            .map(|v| v.iter().map(|&(bi, ii)| (&boards[bi], &boards[bi].items[ii])).collect())
            .unwrap_or_default();
        let entry = disk.get(key);
        let vault_status = entry.and_then(|(_, n)| n.get_scalar("status"));
        let (acts, snap) = plan(key, st.get(ks), vault_status.as_deref(), &present, entry.is_some());
        if let Some(s) = snap {
            new_state.insert(ks.clone(), s);
        }
        for act in acts {
            match act {
                Action::Write { key, managed } => {
                    let existing = disk.get(&key);
                    let rel = key.rel_path();
                    let target = existing.map(|(p, _)| p.clone()).unwrap_or_else(|| out.join(&rel));
                    if existing.is_none() && target.exists() {
                        // An unidentifiable file already sits at the target path (repo or number
                        // line removed, CRLF, unparsable). Never overwrite the user's text.
                        let _ = writeln!(
                            log,
                            "WARN unknown note {} blocks {}; not overwritten",
                            rel.display(),
                            key.to_string_key()
                        );
                        report.warnings += 1;
                        continue;
                    }
                    let mut note = existing.map(|(_, n)| n.clone()).unwrap_or_default();
                    let needs = existing.is_none() || note.needs_write(&managed);
                    if !needs {
                        continue;
                    }
                    let old = existing.and_then(|(_, n)| n.get_scalar("status"));
                    let shown = rel.display();
                    let _ = match old {
                        Some(o) if o != managed.status => writeln!(log, "WRITE {shown} ({o} -> {})", managed.status),
                        _ => writeln!(log, "WRITE {shown}"),
                    };
                    note.apply(&managed);
                    if !dry_run {
                        if let Some(parent) = target.parent() {
                            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                        }
                        fs::write(&target, note.render()).map_err(|e| format!("cannot write {}: {e}", target.display()))?;
                    }
                    report.written.push(target);
                }
                Action::Push { board, item_id, status } => {
                    let _ = writeln!(log, "PUSH {board} {ks} -> {status}");
                    report.pushed += 1;
                    if !dry_run {
                        let bd = boards.iter().find(|b| b.key == board).ok_or("push to an unknown board")?;
                        let opt = bd.option_ids.get(&status).ok_or_else(|| format!("no Status option '{status}'"))?;
                        gh.set_status(&bd.project_id, &item_id, &bd.status_field_id, opt)?;
                    }
                }
                Action::Trash { key } => {
                    let Some((path, _)) = disk.get(&key) else { continue };
                    let rel = path.strip_prefix(out).unwrap_or(path).to_path_buf();
                    let _ = writeln!(log, "TRASH {}", rel.display());
                    let dest = unique_dest(out.join(".trash").join(&rel));
                    if !dry_run {
                        if let Some(parent) = dest.parent() {
                            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                        }
                        fs::rename(path, &dest).map_err(|e| format!("cannot move {}: {e}", path.display()))?;
                    }
                    report.trashed.push(dest);
                }
                Action::Conflict { key, vault, github, board } => {
                    let _ = writeln!(
                        log,
                        "CONFLICT {}: vault {vault}, github {github} ({board}) -> github kept",
                        key.to_string_key()
                    );
                    report.conflicts += 1;
                }
                Action::Warn(msg) => {
                    let _ = writeln!(log, "WARN {msg}");
                    report.warnings += 1;
                }
            }
        }
    }

    for b in &cfg.boards {
        let text = base::render(b);
        let path = out.join(format!("{}.base", b.name));
        if fs::read_to_string(&path).ok().as_deref() == Some(text.as_str()) {
            continue;
        }
        let _ = writeln!(log, "BASE {}.base", b.name);
        if !dry_run {
            fs::create_dir_all(out).map_err(|e| e.to_string())?;
            fs::write(&path, &text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        }
        report.bases.push(path);
    }

    if !dry_run {
        state::save(out, &new_state)?;
    }

    let (w, p, t, b, c, wn) = (
        report.written.len(),
        report.pushed,
        report.trashed.len(),
        report.bases.len(),
        report.conflicts,
        report.warnings,
    );
    let _ = if dry_run {
        writeln!(log, "dry run: would write {w} notes, push {p}, trash {t}, {b} bases, {c} conflicts, {wn} warnings")
    } else {
        writeln!(log, "{w} notes written, {p} pushed, {t} trashed, {b} bases, {c} conflicts, {wn} warnings")
    };
    Ok(report)
}
