//! Makes an Overwatch Workshop script that replays a recorded take on a dummy Doomfist.
//!
//!   cargo run --bin playback -- <log.txt> [--take N] > script.txt
//!
//! Works for bean recordings and for manual Overwatch takes. Default take: the last one.

use doomfist::log::{parse_log, read_log_file, split_takes};
use doomfist::playback::{ticks_from_take, workshop_script, MAX_TICKS};
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut path, mut take) = (None, None);
    while let Some(a) = args.next() {
        if a == "--take" {
            take = args.next().and_then(|v| v.parse::<i64>().ok());
        } else {
            path = Some(PathBuf::from(a));
        }
    }
    let Some(path) = path else {
        eprintln!("usage: playback <log.txt> [--take N] > script.txt");
        std::process::exit(2);
    };
    let text = read_log_file(&path).unwrap_or_else(|e| {
        eprintln!("can't read {}: {e}", path.display());
        std::process::exit(1);
    });
    let takes = split_takes(parse_log(&text));
    let chosen = match take {
        Some(n) => takes.iter().find(|t| t.id == n),
        None => takes.last(),
    };
    let Some(t) = chosen else {
        eprintln!("take not found");
        std::process::exit(1);
    };
    let ticks = ticks_from_take(t);
    if ticks.len() > MAX_TICKS {
        eprintln!("take is {} ticks; only the first {MAX_TICKS} ({} s) fit in a Workshop script", ticks.len(), MAX_TICKS as f32 / 62.5);
    }
    let label = format!("{} take {}", path.file_stem().unwrap_or_default().to_string_lossy(), t.id);
    print!("{}", workshop_script(&ticks, &label));
    eprintln!("{} ticks ({:.1} s)", ticks.len().min(MAX_TICKS), ticks.len().min(MAX_TICKS) as f32 / 62.5);
}
