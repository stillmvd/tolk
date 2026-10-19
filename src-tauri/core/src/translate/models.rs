use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::time::Duration;

pub struct File {
    pub dir: &'static str,
    pub name: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

const EN_RU: (&str, &str) = ("manancode/opus-mt-en-ru-ctranslate2-android", "8693481face72247976d2a7da8610ed4c75d9e83");
const RU_EN: (&str, &str) = ("manancode/opus-mt-ru-en-ctranslate2-android", "9f66440658d009510d9e670ae813c7452fd38234");
const CONFIG: &str = "8f6496adfc930cbfecbe8281112197705c488fab47d34b4829b06d7f478909af";
const VOCAB: &str = "bf1812383ed402163a2ed1b553e175e7db96a54ed68b6c15566018fdf945ed99";
const RU_SPM: &str = "745998e51ba5b058e38b7ac7765c25c43ed5c1c39cc92b27163b9b2e323c9d7c";
const EN_SPM: &str = "16bebef1389a0b8ab452772c4e35b9e605e5713f8ac7baa71ca701394eaa086d";

pub const FILES: [File; 10] = [
    File { dir: "en-ru", name: "model.bin", size: 78_276_475, sha256: "f5ae1da30bcb61e877c549be8fb5a434193893eb97509ccd0b954465f9a8f80c" },
    File { dir: "en-ru", name: "config.json", size: 223, sha256: CONFIG },
    File { dir: "en-ru", name: "shared_vocabulary.json", size: 2_300_269, sha256: VOCAB },
    File { dir: "en-ru", name: "source.spm", size: 802_781, sha256: EN_SPM },
    File { dir: "en-ru", name: "target.spm", size: 1_080_169, sha256: RU_SPM },
    File { dir: "ru-en", name: "model.bin", size: 78_276_475, sha256: "0efde44155f1d73b07d81bae640d9cca79657c6ade8e338d0862e34c30ffeee9" },
    File { dir: "ru-en", name: "config.json", size: 223, sha256: CONFIG },
    File { dir: "ru-en", name: "shared_vocabulary.json", size: 2_300_269, sha256: VOCAB },
    File { dir: "ru-en", name: "source.spm", size: 1_080_169, sha256: RU_SPM },
    File { dir: "ru-en", name: "target.spm", size: 802_781, sha256: EN_SPM },
];

pub fn dir() -> PathBuf {
    PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default()).join("Tolk").join("models")
}

fn path(f: &File) -> PathBuf {
    dir().join(f.dir).join(f.name)
}

fn url(f: &File) -> String {
    let (repo, rev) = if f.dir == "en-ru" { EN_RU } else { RU_EN };
    format!("https://huggingface.co/{repo}/resolve/{rev}/{}", f.name)
}

pub fn total() -> u64 {
    FILES.iter().map(|f| f.size).sum()
}

pub fn ready() -> bool {
    FILES.iter().all(|f| path(f).metadata().is_ok_and(|m| m.len() == f.size))
}

pub fn remove() -> std::io::Result<()> {
    match std::fs::remove_dir_all(dir()) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[derive(Debug, PartialEq)]
pub enum Fail {
    Cancelled,
    Network,
    Corrupt,
    Disk,
}

impl Fail {
    pub fn message(&self) -> &'static str {
        match self {
            Fail::Cancelled => "Скачивание отменено",
            Fail::Network => "Не получилось скачать — проверь сеть и попробуй ещё раз",
            Fail::Corrupt => "Файл модели скачался с ошибкой — попробуй ещё раз",
            Fail::Disk => "Не получилось записать модель на диск",
        }
    }
}

pub fn download(cancel: &AtomicBool, mut progress: impl FnMut(u64, u64)) -> Result<(), Fail> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_connect(Some(Duration::from_secs(15))).timeout_recv_body(Some(Duration::from_secs(30))).build().into();
    let total = total();
    let mut done = 0u64;
    for f in &FILES {
        let target = path(f);
        if target.metadata().is_ok_and(|m| m.len() == f.size) {
            done += f.size;
            progress(done, total);
            continue;
        }
        std::fs::create_dir_all(target.parent().unwrap_or(&dir())).map_err(|_| Fail::Disk)?;
        let part = target.with_extension("part");
        let got = fetch(&agent, f, &part, cancel, |n| progress(done + n, total));
        if got.is_err() {
            let _ = std::fs::remove_file(&part);
        }
        got?;
        std::fs::rename(&part, &target).map_err(|_| Fail::Disk)?;
        done += f.size;
    }
    Ok(())
}

fn fetch(agent: &ureq::Agent, f: &File, part: &PathBuf, cancel: &AtomicBool, mut progress: impl FnMut(u64)) -> Result<(), Fail> {
    let resp = agent.get(&url(f)).call().map_err(|_| Fail::Network)?;
    let mut body = resp.into_body().into_reader();
    let mut out = std::fs::File::create(part).map_err(|_| Fail::Disk)?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut got = 0u64;
    loop {
        if cancel.load(SeqCst) {
            return Err(Fail::Cancelled);
        }
        let n = body.read(&mut buf).map_err(|_| Fail::Network)?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|_| Fail::Disk)?;
        hash.update(&buf[..n]);
        got += n as u64;
        progress(got);
    }
    out.flush().map_err(|_| Fail::Disk)?;
    let hex: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if got != f.size || hex != f.sha256 {
        return Err(Fail::Corrupt);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_files_cover_both_directions() {
        for d in ["en-ru", "ru-en"] {
            let names: Vec<_> = FILES.iter().filter(|f| f.dir == d).map(|f| f.name).collect();
            assert_eq!(names, ["model.bin", "config.json", "shared_vocabulary.json", "source.spm", "target.spm"]);
        }
        assert!(FILES.iter().all(|f| f.sha256.len() == 64));
        assert!(url(&FILES[0]).contains("/resolve/8693481f"));
        assert_eq!(total(), 2 * (78_276_475 + 223 + 2_300_269 + 802_781 + 1_080_169));
    }
}
