use super::keys;
use clipboard_win::{raw, Clipboard};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Gdi::{DeleteEnhMetaFile, GetEnhMetaFileBits, SetEnhMetaFileBits, HENHMETAFILE};
use windows::Win32::System::DataExchange::{GetClipboardSequenceNumber, SetClipboardData};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_C, VK_CONTROL, VK_INSERT, VK_SHIFT, VK_V};

const CF_ENHMETAFILE: u32 = 14;
const HANDLE_FORMATS: [u32; 6] = [2, 3, 9, 0x80, 0x82, 0x8E];
const PRIVATE: [&str; 4] = ["ExcludeClipboardContentFromMonitorProcessing", "CanIncludeInClipboardHistory", "CanUploadToCloudClipboard", "Clipboard Viewer Ignore"];

enum Data {
    Bytes(Vec<u8>),
    Emf(Vec<u8>),
}

pub struct Snapshot(Vec<(u32, Data)>);

fn open() -> Option<Clipboard> {
    Clipboard::new_attempts(20).ok()
}

fn emf_bytes() -> Option<Vec<u8>> {
    let handle = raw::get_clipboard_data(CF_ENHMETAFILE).ok()?;
    let emf = HENHMETAFILE(handle.as_ptr());
    let size = unsafe { GetEnhMetaFileBits(emf, None) };
    let mut buf = vec![0u8; size as usize];
    (size > 0 && unsafe { GetEnhMetaFileBits(emf, Some(&mut buf)) } == size).then_some(buf)
}

fn set_emf(bytes: &[u8]) -> bool {
    let emf = unsafe { SetEnhMetaFileBits(bytes) };
    if emf.is_invalid() {
        return false;
    }
    let ok = unsafe { SetClipboardData(CF_ENHMETAFILE, Some(HANDLE(emf.0))) }.is_ok();
    if !ok {
        unsafe {
            let _ = DeleteEnhMetaFile(Some(emf));
        }
    }
    ok
}

pub fn snapshot() -> Option<Snapshot> {
    let _open = open()?;
    let mut items = Vec::new();
    for f in raw::EnumFormats::new() {
        if HANDLE_FORMATS.contains(&f) || (0x0300..=0x03FF).contains(&f) {
            continue;
        }
        if f == CF_ENHMETAFILE {
            if let Some(b) = emf_bytes() {
                items.push((f, Data::Emf(b)));
            }
            continue;
        }
        let mut buf = Vec::new();
        if raw::get_vec(f, &mut buf).is_ok() {
            items.push((f, Data::Bytes(buf)));
        }
    }
    Some(Snapshot(items))
}

pub fn restore(s: &Snapshot) -> bool {
    let Some(_open) = open() else { return false };
    if raw::empty().is_err() {
        return false;
    }
    let mut ok = true;
    for (f, data) in &s.0 {
        ok &= match data {
            Data::Bytes(b) => raw::set_without_clear(*f, b).is_ok(),
            Data::Emf(b) => set_emf(b),
        };
    }
    ok
}

fn seq() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

fn wait_change(from: u32, limit: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if seq() != from {
            std::thread::sleep(Duration::from_millis(15));
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

fn text() -> Option<String> {
    clipboard_win::get_clipboard_string().ok()
}

fn put(text: &str) -> bool {
    let Some(_open) = open() else { return false };
    if raw::set_string(text).is_err() {
        return false;
    }
    for name in PRIVATE {
        if let Some(f) = raw::register_format(name) {
            let _ = raw::set_without_clear(f.get(), &0u32.to_le_bytes());
        }
    }
    true
}

pub fn copy_selection(console: bool) -> Option<String> {
    let saved = snapshot()?;
    let before = seq();
    if console {
        keys::chord(&[VK_CONTROL], VK_INSERT);
    } else {
        keys::chord(&[VK_CONTROL], VK_C);
    }
    let changed = wait_change(before, Duration::from_millis(500));
    let got = if changed { text().filter(|t| !t.trim().is_empty()) } else { None };
    if changed && !restore(&saved) {
        std::thread::sleep(Duration::from_millis(30));
        restore(&saved);
    }
    got
}

pub fn paste(text: &str, console: bool) -> bool {
    let Some(saved) = snapshot() else { return false };
    if !put(text) {
        restore(&saved);
        return false;
    }
    let mine = seq();
    let ok = if console { keys::chord(&[VK_SHIFT], VK_INSERT) } else { keys::chord(&[VK_CONTROL], VK_V) };
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1000));
        if seq() == mine {
            restore(&saved);
        }
    });
    ok
}

pub fn copy(text: &str) -> bool {
    let Some(_open) = open() else { return false };
    raw::set_string(text).is_ok()
}
