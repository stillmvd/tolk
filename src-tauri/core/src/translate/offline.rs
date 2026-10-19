use super::{models, Failure};
use crate::text::Dir;
use crossbeam_channel::{unbounded, Receiver};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::SeqCst};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::JobObjects::*;

const NO_WINDOW: u32 = 0x0800_0000;
const READY_IN: Duration = Duration::from_secs(30);
const ANSWER_IN: Duration = Duration::from_secs(60);
const IDLE: Duration = Duration::from_secs(600);
const REAP_EVERY: Duration = Duration::from_secs(30);

struct Proc {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<String>,
}

impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Default)]
pub struct Sidecar {
    proc: Arc<Mutex<Option<Proc>>>,
    last: Arc<Mutex<Option<Instant>>>,
    reaping: Arc<AtomicBool>,
    blocked: AtomicBool,
    next: AtomicU64,
}

impl Sidecar {
    pub fn translate(&self, text: &str, dir: Dir, n: usize) -> Result<(String, Vec<String>), Failure> {
        *self.last.lock().unwrap() = Some(Instant::now());
        let mut guard = self.proc.lock().unwrap();
        if self.blocked.load(SeqCst) || !models::ready() {
            return Err(Failure::NoModel);
        }
        let id = self.next.fetch_add(1, SeqCst);
        let req = json!({ "id": id, "dir": format!("{}-{}", dir.from().code(), dir.to().code()), "text": text, "n": n });
        let mut sent = false;
        for _ in 0..2 {
            if guard.is_none() {
                *guard = Some(spawn().ok_or(Failure::Service)?);
                self.reap();
            }
            let p = guard.as_mut().ok_or(Failure::Service)?;
            if writeln!(p.stdin, "{req}").and_then(|_| p.stdin.flush()).is_ok() {
                sent = true;
                break;
            }
            *guard = None;
        }
        let p = guard.as_mut().filter(|_| sent).ok_or(Failure::Service)?;
        loop {
            let Ok(line) = p.rx.recv_timeout(ANSWER_IN) else {
                *guard = None;
                return Err(Failure::Service);
            };
            let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
            if v["id"].as_u64() != Some(id) {
                continue;
            }
            if v.get("error").is_some() {
                return Err(Failure::Service);
            }
            let text = v["text"].as_str().unwrap_or_default().to_string();
            let variants = v["variants"].as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default();
            return Ok((text, variants));
        }
    }

    pub fn without<R>(&self, f: impl FnOnce() -> R) -> R {
        self.blocked.store(true, SeqCst);
        *self.proc.lock().unwrap() = None;
        let r = f();
        self.blocked.store(false, SeqCst);
        r
    }

    fn reap(&self) {
        if self.reaping.swap(true, SeqCst) {
            return;
        }
        let (proc, last, reaping) = (self.proc.clone(), self.last.clone(), self.reaping.clone());
        let spawned = std::thread::Builder::new().name("tolk-mt-idle".into()).spawn(move || loop {
            std::thread::sleep(REAP_EVERY);
            let mut p = proc.lock().unwrap();
            if last.lock().unwrap().is_none_or(|t| t.elapsed() > IDLE) {
                *p = None;
            }
            if p.is_none() {
                reaping.store(false, SeqCst);
                break;
            }
        });
        if spawned.is_err() {
            self.reaping.store(false, SeqCst);
        }
    }
}

fn spawn() -> Option<Proc> {
    let exe = std::env::current_exe().ok()?.with_file_name("tolk-mt.exe");
    let mut child = Command::new(exe).arg(models::dir()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).creation_flags(NO_WINDOW).spawn().ok()?;
    tie(&child);
    let (stdin, stdout) = (child.stdin.take()?, child.stdout.take()?);
    let (tx, rx) = unbounded();
    std::thread::Builder::new()
        .name("tolk-mt-out".into())
        .spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        })
        .ok()?;
    let proc = Proc { child, stdin, rx };
    match proc.rx.recv_timeout(READY_IN) {
        Ok(line) if line.contains("\"ready\"") => Some(proc),
        _ => None,
    }
}

fn tie(child: &Child) {
    static JOB: OnceLock<usize> = OnceLock::new();
    let job = *JOB.get_or_init(|| unsafe {
        let Ok(job) = CreateJobObjectW(None, None) else { return 0 };
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let _ = SetInformationJobObject(job, JobObjectExtendedLimitInformation, (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(), std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32);
        job.0 as usize
    });
    if job != 0 {
        unsafe {
            let _ = AssignProcessToJobObject(HANDLE(job as *mut _), HANDLE(child.as_raw_handle()));
        }
    }
}
