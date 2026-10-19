use crate::config::Config;
use crate::text::direction;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub const LIMIT: usize = 500;

static LOCK: Mutex<()> = Mutex::new(());
static ON: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub t: u64,
    pub from: String,
    pub to: String,
    pub src: String,
    pub dst: String,
}

pub fn path() -> PathBuf {
    Config::dir().join("history.jsonl")
}

pub fn set_enabled(on: bool) -> std::io::Result<()> {
    let _guard = LOCK.lock().unwrap();
    ON.store(on, SeqCst);
    if on {
        Ok(())
    } else {
        wipe()
    }
}

pub fn add(src: &str, dst: &str, dir: Option<(&str, &str)>) {
    let (src, dst) = (src.trim(), dst.trim());
    let Some((from, to)) = dir.or_else(|| direction(src).map(|d| (d.from().code(), d.to().code()))) else { return };
    if src.is_empty() || dst.is_empty() {
        return;
    }
    let t = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let entry = Entry { t, from: from.into(), to: to.into(), src: src.into(), dst: dst.into() };
    let _guard = LOCK.lock().unwrap();
    if !ON.load(SeqCst) {
        return;
    }
    let mut all = read();
    if all.last().is_some_and(|e| e.src == entry.src && e.dst == entry.dst) {
        return;
    }
    all.push(entry);
    let keep = all.len().saturating_sub(LIMIT);
    write(&all[keep..]);
}

pub fn list() -> Vec<Entry> {
    let _guard = LOCK.lock().unwrap();
    let mut all = read();
    all.reverse();
    all
}

pub fn clear() -> std::io::Result<()> {
    let _guard = LOCK.lock().unwrap();
    wipe()
}

fn wipe() -> std::io::Result<()> {
    for p in [path(), path().with_extension("tmp")] {
        match std::fs::remove_file(p) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

fn read() -> Vec<Entry> {
    std::fs::read_to_string(path()).map(|s| parse(&s)).unwrap_or_default()
}

fn parse(s: &str) -> Vec<Entry> {
    s.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

fn write(entries: &[Entry]) {
    let body: String = entries.iter().filter_map(|e| serde_json::to_string(e).ok()).map(|l| l + "\n").collect();
    let p = path();
    let _ = std::fs::create_dir_all(Config::dir());
    let tmp = p.with_extension("tmp");
    if std::fs::write(&tmp, body).is_ok() {
        let _ = std::fs::rename(&tmp, &p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lines_and_skips_garbage() {
        let s = "{\"t\":1,\"from\":\"ru\",\"to\":\"en\",\"src\":\"замок\",\"dst\":\"lock\"}\nnot json\n";
        assert_eq!(parse(s), vec![Entry { t: 1, from: "ru".into(), to: "en".into(), src: "замок".into(), dst: "lock".into() }]);
    }
}
