use super::hook::HookEvent;
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::{Mutex, OnceLock};
use windows::core::BOOL;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, SetWinEventHook, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Context {
    pub hwnd: isize,
    pub exe: String,
    pub elevated: bool,
    pub password: bool,
    pub own: bool,
    pub console: bool,
}

static SENDER: OnceLock<Sender<HookEvent>> = OnceLock::new();
static CURRENT: Mutex<Option<Context>> = Mutex::new(None);
static LAST_FOREIGN: Mutex<isize> = Mutex::new(0);
static FOCUS_QUEUED: AtomicBool = AtomicBool::new(false);

thread_local! {
    pub static UIA: Option<IUIAutomation> = unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()
    };
}

pub fn current() -> Context {
    CURRENT.lock().unwrap().clone().unwrap_or_default()
}

pub fn last_foreign() -> HWND {
    HWND(*LAST_FOREIGN.lock().unwrap() as *mut _)
}

fn process_name(pid: u32) -> String {
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return String::new() };
        let mut buf = [0u16; 520];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
        let _ = CloseHandle(h);
        if !ok {
            return String::new();
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit('\\').next().unwrap_or_default().to_lowercase()
    }
}

fn token_elevated(process: HANDLE) -> Option<bool> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
        let mut e = TOKEN_ELEVATION::default();
        let mut ret = 0;
        let ok = GetTokenInformation(token, TokenElevation, Some(&mut e as *mut _ as *mut _), std::mem::size_of::<TOKEN_ELEVATION>() as u32, &mut ret).is_ok();
        let _ = CloseHandle(token);
        ok.then_some(e.TokenIsElevated != 0)
    }
}

fn elevated(pid: u32) -> bool {
    if token_elevated(unsafe { GetCurrentProcess() }) == Some(true) {
        return false;
    }
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return true };
        let r = token_elevated(h).unwrap_or(true);
        let _ = CloseHandle(h);
        r
    }
}

pub fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 128];
    let n = unsafe { GetClassNameW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn password(focus: HWND) -> bool {
    if !focus.is_invalid() && class_name(focus).eq_ignore_ascii_case("edit") {
        let style = unsafe { GetWindowLongW(focus, GWL_STYLE) } as u32;
        if style & ES_PASSWORD as u32 != 0 {
            return true;
        }
    }
    UIA.with(|uia| {
        uia.as_ref()
            .and_then(|a| unsafe { a.GetFocusedElement() }.ok())
            .and_then(|e| unsafe { e.CurrentIsPassword() }.ok())
            .is_some_and(|b| b.as_bool())
    })
}

unsafe extern "system" fn find_child(child: HWND, lparam: LPARAM) -> BOOL {
    let out = &mut *(lparam.0 as *mut (u32, u32));
    let mut pid = 0;
    GetWindowThreadProcessId(child, Some(&mut pid));
    if pid != out.0 {
        out.1 = pid;
        return false.into();
    }
    true.into()
}

fn compute() -> Context {
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut pid = 0;
        let thread = GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let mut exe = process_name(pid);
        if exe == "applicationframehost.exe" {
            let mut pair = (pid, 0u32);
            let _ = EnumChildWindows(Some(hwnd), Some(find_child), LPARAM(&mut pair as *mut _ as isize));
            if pair.1 != 0 {
                pid = pair.1;
                exe = process_name(pid);
            }
        }
        let mut info = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
        let focus = if GetGUIThreadInfo(thread, &mut info).is_ok() { info.hwndFocus } else { HWND::default() };
        let class = class_name(hwnd);
        Context {
            hwnd: hwnd.0 as isize,
            console: class == "ConsoleWindowClass" || class == "CASCADIA_HOSTING_WINDOW_CLASS",
            elevated: elevated(pid),
            password: password(focus),
            own: pid == GetCurrentProcessId(),
            exe,
        }
    }
}

pub fn refresh() -> Context {
    let ctx = compute();
    if !ctx.own && ctx.hwnd != 0 && class_name(HWND(ctx.hwnd as *mut _)) != "Shell_TrayWnd" {
        *LAST_FOREIGN.lock().unwrap() = ctx.hwnd;
    }
    *CURRENT.lock().unwrap() = Some(ctx.clone());
    ctx
}

unsafe extern "system" fn on_event(_: HWINEVENTHOOK, _: u32, _: HWND, _: i32, _: i32, _: u32, _: u32) {
    let _ = std::panic::catch_unwind(|| {
        refresh();
        if !FOCUS_QUEUED.swap(true, SeqCst) {
            if let Some(tx) = SENDER.get() {
                if tx.try_send(HookEvent::Focus).is_err() {
                    FOCUS_QUEUED.store(false, SeqCst);
                }
            }
        }
    });
}

pub fn focus_taken() {
    FOCUS_QUEUED.store(false, SeqCst);
}

pub fn run(tx: Sender<HookEvent>) -> std::thread::JoinHandle<()> {
    let _ = SENDER.set(tx);
    std::thread::Builder::new()
        .name("tolk-focus".into())
        .spawn(|| unsafe {
            let flags = WINEVENT_OUTOFCONTEXT;
            let _a = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, None, Some(on_event), 0, 0, flags);
            let _b = SetWinEventHook(EVENT_OBJECT_FOCUS, EVENT_OBJECT_FOCUS, None, Some(on_event), 0, 0, flags);
            on_event(HWINEVENTHOOK::default(), 0, HWND::default(), 0, 0, 0, 0);
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        })
        .expect("focus thread")
}
