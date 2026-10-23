mod support;

use board_bases::config::BoardConfig;
use board_bases::gh::FakeGh;
use board_bases::note;
use board_bases::sync;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use support::*;

const ITEMS: usize = 6;
const BOARDS: usize = 3;
const ROUNDS: usize = 200;

fn two_board_pages(a: &[(u32, &str)], b: &[(u32, &str)]) -> Vec<Value> {
    let bs = boards(2);
    vec![page(&bs[0], a), page(&bs[1], b)]
}

fn read_status(p: &Path) -> Option<String> {
    let text = fs::read_to_string(p).ok()?;
    note::parse(&text).get_scalar("status")
}

fn drag(p: &Path, status: &str, quoted: bool) {
    let text = fs::read_to_string(p).unwrap();
    let line = if quoted { format!("status: \"{status}\"") } else { format!("status: {status}") };
    let mut out: String = text
        .lines()
        .map(|l| if l.starts_with("status:") { line.clone() } else { l.to_string() })
        .collect::<Vec<_>>()
        .join("\n");
    out.push('\n');
    fs::write(p, out).unwrap();
}

fn all_text(out: &Path) -> String {
    let mut s = String::new();
    for p in all_files(&out.join("notes")).into_iter().chain(all_files(&out.join(".trash"))) {
        s.push_str(&fs::read_to_string(p).unwrap_or_default());
    }
    s
}

fn md_count(out: &Path) -> usize {
    all_files(&out.join("notes"))
        .into_iter()
        .chain(all_files(&out.join(".trash")))
        .filter(|p| p.extension().map(|e| e == "md").unwrap_or(false))
        .count()
}

#[test]
fn second_sync_writes_nothing() {
    let out = tmp("second");
    let pages = || two_board_pages(&[(1, "Planned"), (2, "Review")], &[(1, "Planned")]);
    let conf = config(2);
    sync::run(&FakeGh::with_pages(pages()), &conf, &out, false, &mut Vec::new()).unwrap();
    let before: Vec<_> = all_files(&out)
        .into_iter()
        .map(|p| {
            let t = fs::metadata(&p).unwrap().modified().unwrap();
            (p, t)
        })
        .collect();
    assert!(!before.is_empty());
    std::thread::sleep(std::time::Duration::from_millis(20));
    let r = sync::run(&FakeGh::with_pages(pages()), &conf, &out, false, &mut Vec::new()).unwrap();
    assert!(r.written.is_empty());
    assert!(r.bases.is_empty());
    assert!(r.trashed.is_empty());
    assert_eq!(r.pushed, 0);
    for (p, t) in before {
        assert_eq!(fs::metadata(&p).unwrap().modified().unwrap(), t, "{}", p.display());
    }
}

#[test]
fn dry_run_writes_nothing_and_pushes_nothing() {
    let out = tmp("dry");
    fs::create_dir_all(&out).unwrap();
    let gh = FakeGh::with_pages(two_board_pages(&[(1, "Planned")], &[(1, "Planned")]));
    let mut log: Vec<u8> = Vec::new();
    let r = sync::run(&gh, &config(2), &out, true, &mut log).unwrap();
    assert!(all_files(&out).is_empty());
    assert!(gh.pushes.borrow().is_empty());
    assert_eq!(r.written.len(), 1);
    assert!(String::from_utf8(log).unwrap().contains("dry run:"));
}

#[test]
fn drag_in_vault_pushes_to_github() {
    let out = tmp("drag");
    let pages = || two_board_pages(&[(1, "Planned")], &[(1, "Planned")]);
    sync::run(&FakeGh::with_pages(pages()), &config(2), &out, false, &mut Vec::new()).unwrap();
    let p = note_path(&out, 1);
    let text = fs::read_to_string(&p).unwrap();
    assert!(text.contains("status: Planned\n"));
    // Obsidian re-quotes the value when a card is dragged.
    let dragged = text.replace("status: Planned\n", "status: \"Review\"\n");
    fs::write(&p, &dragged).unwrap();
    let gh = FakeGh::with_pages(pages());
    let r = sync::run(&gh, &config(2), &out, false, &mut Vec::new()).unwrap();
    let pushes = gh.pushes.borrow();
    assert_eq!(pushes.len(), 2, "one push per board holding the item");
    for (_, _, _, opt) in pushes.iter() {
        assert_eq!(opt, &option_id("Review"));
    }
    assert_eq!(r.pushed, 2);
    assert_eq!(fs::read_to_string(&p).unwrap(), dragged, "the dragged note is not rewritten");
}

#[test]
fn trashed_note_is_byte_identical() {
    let out = tmp("trash");
    let both = || two_board_pages(&[(1, "Planned"), (2, "Done")], &[(1, "Planned"), (2, "Done")]);
    sync::run(&FakeGh::with_pages(both()), &config(2), &out, false, &mut Vec::new()).unwrap();
    let original = fs::read(note_path(&out, 2)).unwrap();
    let r = sync::run(
        &FakeGh::with_pages(two_board_pages(&[(1, "Planned")], &[(1, "Planned")])),
        &config(2),
        &out,
        false,
        &mut Vec::new(),
    )
    .unwrap();
    assert!(!note_path(&out, 2).exists());
    assert_eq!(r.trashed.len(), 1);
    let trash = out.join(".trash/notes/acme/widgets 2.md");
    assert_eq!(fs::read(trash).unwrap(), original);
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn status(&mut self) -> &'static str {
        ["Planned", "In PR", "Review", "Done"][self.below(4)]
    }
}

fn board_pages(bs: &[BoardConfig], model: &[Vec<Option<String>>]) -> Vec<Value> {
    bs.iter()
        .enumerate()
        .map(|(bi, b)| {
            let items: Vec<(u32, &str)> = (0..ITEMS)
                .filter_map(|i| model[bi][i].as_deref().map(|s| (i as u32 + 1, s)))
                .collect();
            page(b, &items)
        })
        .collect()
}

#[test]
fn random_edits_keep_invariants() {
    let out = tmp("random");
    let conf = config(BOARDS as u32);
    let bs = boards(BOARDS as u32);
    let mut rng = Lcg(42);
    let mut model: Vec<Vec<Option<String>>> = vec![vec![None; ITEMS]; BOARDS];
    for i in 0..ITEMS {
        model[0][i] = Some("Planned".into());
    }
    let mut fragments: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    let mut last_synced: BTreeMap<u32, Option<String>> = BTreeMap::new();
    let mut prev_md = 0usize;

    for round in 0..ROUNDS {
        for _ in 0..1 + rng.below(3) {
            let b = rng.below(BOARDS);
            let i = rng.below(ITEMS);
            let n = i as u32 + 1;
            match rng.below(4) {
                0 => {
                    // GitHub-side status change
                    if model[b][i].is_some() {
                        model[b][i] = Some(rng.status().into());
                    }
                }
                1 => {
                    // item added to or removed from a board
                    model[b][i] = if model[b][i].is_some() { None } else { Some(rng.status().into()) };
                }
                2 => {
                    // card dragged in Obsidian
                    let p = note_path(&out, n);
                    if p.exists() {
                        let quoted = rng.below(2) == 0;
                        drag(&p, rng.status(), quoted);
                    }
                }
                _ => {
                    // extra frontmatter key and body line, which must survive every sync
                    let p = note_path(&out, n);
                    if p.exists() {
                        let key = format!("extra{round}x{n}: yes");
                        let body = format!("body {round} {n}");
                        let text = fs::read_to_string(&p).unwrap();
                        let new = text.replacen("---\n", &format!("---\n{key}\n"), 1)
                            + &format!("{body}\n");
                        fs::write(&p, new).unwrap();
                        fragments.entry(n).or_default().push(key);
                        fragments.entry(n).or_default().push(body);
                    }
                }
            }
        }

        let vault_before: Vec<Option<String>> =
            (1..=ITEMS as u32).map(|n| read_status(&note_path(&out, n))).collect();
        let gh = FakeGh::with_pages(board_pages(&bs, &model));
        sync::run(&gh, &conf, &out, false, &mut Vec::new()).unwrap();
        let pushes = gh.pushes.borrow().clone();

        for (p_id, i_id, _, o_id) in &pushes {
            let bi = bs.iter().position(|b| &project_id(b) == p_id).unwrap();
            let n: u32 = i_id.rsplit('_').next().unwrap().parse().unwrap();
            let status = o_id.trim_start_matches("OPT_").replace('_', " ");
            // I1: a push only follows a real vault-side change, and carries that value
            assert_eq!(vault_before[(n - 1) as usize].as_deref(), Some(status.as_str()), "round {round}: push without a vault edit");
            let prev = last_synced.get(&n).cloned().flatten();
            assert_ne!(prev.as_deref(), Some(status.as_str()), "round {round}: push without a vault edit");
            assert!(model[bi][(n - 1) as usize].is_some(), "round {round}: push to a board not holding the item");
            model[bi][(n - 1) as usize] = Some(status);
        }

        // I5: the note shows the first board's status, after pushes
        for n in 1..=ITEMS as u32 {
            let i = (n - 1) as usize;
            match (0..BOARDS).find(|&bi| model[bi][i].is_some()) {
                Some(bi) => assert_eq!(
                    read_status(&note_path(&out, n)).as_deref(),
                    model[bi][i].as_deref(),
                    "round {round}: note {n} status"
                ),
                None => assert!(!note_path(&out, n).exists(), "round {round}: note {n} should be trashed"),
            }
            last_synced.insert(n, read_status(&note_path(&out, n)));
        }

        // I2: every extra key and body line ever added is still on disk, in a note or its trash copy
        let text = all_text(&out);
        for frags in fragments.values() {
            for f in frags {
                assert!(text.contains(f), "round {round}: lost {f}");
            }
        }

        // I3: notes and trash copies never disappear
        let md = md_count(&out);
        assert!(md >= prev_md, "round {round}: note count fell");
        prev_md = md;

        // I4: an immediate second sync has nothing to do
        let r2 = sync::run(&FakeGh::with_pages(board_pages(&bs, &model)), &conf, &out, false, &mut Vec::new()).unwrap();
        assert!(r2.written.is_empty() && r2.bases.is_empty() && r2.trashed.is_empty() && r2.pushed == 0, "round {round}: second sync wrote");
    }
}

#[test]
fn null_github_status_follows_github() {
    let out = tmp("null-status");
    let b = boards(1)[0].clone();
    let conf = config(1);
    let null_page = || {
        let mut p = page(&b, &[(1, "Planned")]);
        p["data"]["repositoryOwner"]["projectV2"]["items"]["nodes"][0]["fieldValueByName"] = serde_json::json!({"name": null});
        p
    };
    sync::run(&FakeGh::with_pages(vec![null_page()]), &conf, &out, false, &mut Vec::new()).unwrap();
    let mut log = Vec::new();
    sync::run(&FakeGh::with_pages(vec![null_page()]), &conf, &out, false, &mut log).unwrap();
    let log = String::from_utf8(log).unwrap();
    assert!(!log.contains("WARN"), "second sync warned on a null status; log:\n{log}");

    let gh = FakeGh::with_pages(vec![page(&b, &[(1, "Planned")])]);
    let mut log = Vec::new();
    sync::run(&gh, &conf, &out, false, &mut log).unwrap();
    assert!(gh.pushes.borrow().is_empty(), "GitHub-only change must not push");
    assert_eq!(read_status(&note_path(&out, 1)).as_deref(), Some("Planned"));
}
