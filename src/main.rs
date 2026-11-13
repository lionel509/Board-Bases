use board_bases::config::{self, default_out, expand_home};
use board_bases::gh::RealGh;
use board_bases::sync;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: board-bases sync [--dry-run] [--out DIR] [--config FILE]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("sync") {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let mut dry_run = false;
    let mut out_flag: Option<PathBuf> = None;
    let mut config_flag: Option<PathBuf> = None;
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dry-run" => dry_run = true,
            "--out" => match it.next() {
                Some(v) => out_flag = Some(expand_home(v)),
                None => return usage_error(),
            },
            "--config" => match it.next() {
                Some(v) => config_flag = Some(expand_home(v)),
                None => return usage_error(),
            },
            _ => return usage_error(),
        }
    }

    let config_path = config_flag
        .or_else(|| std::env::var("BOARD_BASES_CONFIG").ok().map(|v| expand_home(&v)))
        .unwrap_or_else(|| expand_home("~/.config/board-bases/config.json"));
    let cfg = match config::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("board-bases: {e}");
            return ExitCode::from(1);
        }
    };
    let out = out_flag
        .or_else(|| std::env::var("BOARD_BASES_OUT").ok().map(|v| expand_home(&v)))
        .or_else(|| cfg.output.clone())
        .unwrap_or_else(default_out);

    let mut stdout = std::io::stdout();
    match sync::run(&RealGh, &cfg, &out, dry_run, &mut stdout) {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("board-bases: {e}");
            ExitCode::from(1)
        }
    }
}

fn usage_error() -> ExitCode {
    eprintln!("{USAGE}");
    ExitCode::from(2)
}
