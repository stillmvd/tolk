use crossbeam_channel::{bounded, unbounded, Sender};
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::OnceLock;
use std::time::Duration;
use windows::core::Interface;
use windows::Win32::Foundation::RECT;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, SAFEARRAY};
use windows::Win32::System::Ole::{SafeArrayDestroy, SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, GUITHREADINFO};

thread_local! {
    static UIA: Option<IUIAutomation> = unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let uia: Option<IUIAutomation> = CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER).ok();
        if let Some(a) = uia.as_ref().and_then(|u| u.cast::<IUIAutomation2>().ok()) {
            let _ = a.SetTransactionTimeout(600);
            let _ = a.SetConnectionTimeout(600);
        }
        uia
    };
}

pub enum Selection {
    Text { text: String, editable: bool, rect: Option<RECT> },
    Empty { editable: bool },
    Unknown,
}

enum Req {
    Selection,
    SelectWord,
    Caret,
}

enum Resp {
    Selection(Selection),
    Word(Option<String>),
    Caret(Option<RECT>),
}

static WORKER: OnceLock<Sender<(Req, Sender<Resp>)>> = OnceLock::new();
static BUSY: AtomicBool = AtomicBool::new(false);

fn ask(req: Req, limit: Duration) -> Option<Resp> {
    let tx = WORKER.get_or_init(|| {
        let (tx, rx) = unbounded::<(Req, Sender<Resp>)>();
        std::thread::Builder::new()
            .name("tolk-uia".into())
            .spawn(move || {
                for (req, reply) in rx {
                    let resp = std::panic::catch_unwind(|| match req {
                        Req::Selection => Resp::Selection(selection_now()),
                        Req::SelectWord => Resp::Word(select_word_now()),
                        Req::Caret => Resp::Caret(caret_now()),
                    });
                    BUSY.store(false, SeqCst);
                    if let Ok(r) = resp {
                        let _ = reply.send(r);
                    }
                }
            })
            .expect("uia thread");
        tx
    });
    if BUSY.swap(true, SeqCst) {
        return None;
    }
    let (reply, rx) = bounded(1);
    if tx.send((req, reply)).is_err() {
        BUSY.store(false, SeqCst);
        return None;
    }
    rx.recv_timeout(limit).ok()
}

pub fn selection() -> Selection {
    match ask(Req::Selection, Duration::from_millis(900)) {
        Some(Resp::Selection(s)) => s,
        _ => Selection::Unknown,
    }
}

pub fn select_word_before_caret() -> Option<String> {
    match ask(Req::SelectWord, Duration::from_millis(900)) {
        Some(Resp::Word(w)) => w,
        _ => None,
    }
}

pub fn caret_rect() -> Option<RECT> {
    match ask(Req::Caret, Duration::from_millis(300)) {
        Some(Resp::Caret(r)) => r,
        _ => None,
    }
}

fn with<T>(f: impl FnOnce(&IUIAutomation) -> Option<T>) -> Option<T> {
    UIA.with(|u| u.as_ref().and_then(f))
}

fn focused(uia: &IUIAutomation) -> Option<IUIAutomationElement> {
    unsafe { uia.GetFocusedElement() }.ok()
}

pub fn has_caret() -> bool {
    unsafe {
        let thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        let mut info = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
        GetGUIThreadInfo(thread, &mut info).is_ok() && !info.hwndCaret.is_invalid()
    }
}

fn editable(el: &IUIAutomationElement) -> bool {
    unsafe {
        if let Ok(v) = el.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) {
            if let Ok(ro) = v.CurrentIsReadOnly() {
                return !ro.as_bool();
            }
        }
        el.CurrentControlType().is_ok_and(|t| t == UIA_EditControlTypeId) || has_caret()
    }
}

pub fn rects(range: &IUIAutomationTextRange) -> Vec<RECT> {
    unsafe {
        let Ok(arr) = range.GetBoundingRectangles() else { return Vec::new() };
        let out = doubles(arr);
        let _ = SafeArrayDestroy(arr);
        out.chunks_exact(4)
            .filter(|r| r[2] > 0.0 && r[3] > 0.0)
            .map(|r| RECT { left: r[0] as i32, top: r[1] as i32, right: (r[0] + r[2]) as i32, bottom: (r[1] + r[3]) as i32 })
            .collect()
    }
}

unsafe fn doubles(arr: *mut SAFEARRAY) -> Vec<f64> {
    let (Ok(lo), Ok(hi)) = (SafeArrayGetLBound(arr, 1), SafeArrayGetUBound(arr, 1)) else { return Vec::new() };
    (lo..=hi)
        .filter_map(|i| {
            let mut v = 0f64;
            SafeArrayGetElement(arr, &i, &mut v as *mut f64 as *mut _).ok().map(|_| v)
        })
        .collect()
}

fn union(rs: &[RECT]) -> Option<RECT> {
    rs.iter().copied().reduce(|a, b| RECT { left: a.left.min(b.left), top: a.top.min(b.top), right: a.right.max(b.right), bottom: a.bottom.max(b.bottom) })
}

fn selection_now() -> Selection {
    with(|uia| {
        let el = focused(uia)?;
        let edit = editable(&el);
        let tp = unsafe { el.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) }.ok()?;
        let ranges = unsafe { tp.GetSelection() }.ok()?;
        let n = unsafe { ranges.Length() }.unwrap_or(0);
        let mut text = String::new();
        let mut rs = Vec::new();
        for i in 0..n {
            let Ok(r) = (unsafe { ranges.GetElement(i) }) else { continue };
            if let Ok(t) = unsafe { r.GetText(20_001) } {
                text.push_str(&t.to_string());
            }
            rs.extend(rects(&r));
        }
        Some(if text.trim().is_empty() { Selection::Empty { editable: edit } } else { Selection::Text { text, editable: edit, rect: union(&rs) } })
    })
    .unwrap_or(Selection::Unknown)
}

fn select_word_now() -> Option<String> {
    with(|uia| {
        let el = focused(uia)?;
        let tp = unsafe { el.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) }.ok()?;
        let ranges = unsafe { tp.GetSelection() }.ok()?;
        let r = unsafe { ranges.GetElement(0) }.ok()?;
        let r = unsafe { r.Clone() }.ok()?;
        unsafe { r.MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Word, -1) }.ok()?;
        let text = unsafe { r.GetText(200) }.ok()?.to_string();
        let word = text.trim_end();
        if word.is_empty() || !word.chars().all(crate::text::is_word_char) {
            return None;
        }
        if word.len() != text.len() {
            unsafe { r.MoveEndpointByUnit(TextPatternRangeEndpoint_End, TextUnit_Character, -((text.chars().count() - word.chars().count()) as i32)) }.ok()?;
        }
        unsafe { r.Select() }.ok()?;
        Some(word.to_string())
    })
}

fn caret_now() -> Option<RECT> {
    with(|uia| {
        let el = focused(uia)?;
        let tp = unsafe { el.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) }.ok()?;
        let r = unsafe { tp.GetSelection() }.ok().and_then(|s| unsafe { s.GetElement(0) }.ok())?;
        let mut rs = rects(&r);
        if rs.is_empty() {
            let c = unsafe { r.Clone() }.ok()?;
            unsafe { c.MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Character, -1) }.ok()?;
            rs = rects(&c);
        }
        union(&rs)
    })
}
